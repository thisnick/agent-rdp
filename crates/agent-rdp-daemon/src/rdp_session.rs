//! RDP session wrapper using IronRDP.

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::RwLock;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use agent_rdp_protocol::DriveMapping;
use ironrdp::connector::{self, ClientConnector, ConnectorResult, Credentials, ServerName};
use ironrdp::pdu::gcc::KeyboardType;
use ironrdp::pdu::input::fast_path::FastPathInputEvent;
use ironrdp::pdu::rdp::capability_sets::MajorPlatformType;
use ironrdp::pdu::rdp::client_info::PerformanceFlags;
use ironrdp::session::image::DecodedImage;
use ironrdp::session::{ActiveStage, ActiveStageOutput};
use ironrdp_rdpdr::Rdpdr;

use crate::rdpdr_backend::PlatformRdpdrBackend;
use ironrdp_rdpsnd::client::{NoopRdpsndBackend, Rdpsnd};
use ironrdp_tokio::{FramedWrite, TokioFramed};
use tokio::net::TcpStream;

pub mod clipboard;

#[derive(Error, Debug)]
pub enum RdpError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("TLS error: {0}")]
    TlsError(String),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Not connected")]
    NotConnected,

    #[error("Session closed")]
    SessionClosed,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Configuration for an RDP connection.
pub struct RdpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub domain: Option<String>,
    pub width: u16,
    pub height: u16,
    /// Drives to map at connect time.
    pub drives: Vec<DriveMapping>,
}

/// Commands sent to the background frame processor.
enum SessionCommand {
    SendInput(Vec<FastPathInputEvent>),
    /// Set clipboard text and announce to remote.
    ClipboardSet {
        text: String,
        response_tx: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// Get clipboard text from remote.
    ClipboardGet {
        response_tx: tokio::sync::oneshot::Sender<Result<Option<String>, String>>,
    },
    Shutdown,
}

/// Shared session state accessible from the main thread.
struct SharedState {
    image: DecodedImage,
    host: String,
    width: u16,
    height: u16,
    /// Drives that were mapped at connect time.
    drives: Vec<DriveMapping>,
    /// Clipboard state for CLIPRDR.
    clipboard: Arc<parking_lot::Mutex<clipboard::ClipboardState>>,
}

/// An active RDP session with background frame processing.
pub struct RdpSession {
    /// Shared state (image, connection info)
    shared: Arc<RwLock<SharedState>>,
    /// Channel to send commands to the background task
    command_tx: mpsc::Sender<SessionCommand>,
    /// Handle to the background task
    _task_handle: tokio::task::JoinHandle<()>,
}

/// Callback type for connection drop notification.
pub type DisconnectNotify = mpsc::Sender<()>;

impl RdpSession {
    /// Establish a new RDP connection.
    ///
    /// If `disconnect_notify` is provided, it will be signaled when the connection drops.
    pub async fn connect(
        config: RdpConfig,
        disconnect_notify: Option<DisconnectNotify>,
    ) -> Result<Self, RdpError> {
        info!("Connecting to {}:{}", config.host, config.port);

        // Build connector config
        let connector_config = connector::Config {
            credentials: Credentials::UsernamePassword {
                username: config.username.clone(),
                password: config.password.clone(),
            },
            domain: config.domain.clone(),
            enable_tls: true,
            enable_credssp: true,
            keyboard_type: KeyboardType::IbmEnhanced,
            keyboard_subtype: 0,
            keyboard_functional_keys_count: 12,
            keyboard_layout: 0x409, // US English
            ime_file_name: String::new(),
            dig_product_id: String::new(),
            desktop_size: connector::DesktopSize {
                width: config.width,
                height: config.height,
            },
            bitmap: None,
            client_build: 0,
            client_name: "agent-rdp".to_string(),
            client_dir: String::new(),
            #[cfg(windows)]
            platform: MajorPlatformType::WINDOWS,
            #[cfg(target_os = "macos")]
            platform: MajorPlatformType::MACINTOSH,
            #[cfg(all(not(windows), not(target_os = "macos")))]
            platform: MajorPlatformType::UNIX,
            pointer_software_rendering: true,
            performance_flags: PerformanceFlags::default(),
            enable_server_pointer: false,
            request_data: None,
            autologon: false,
            enable_audio_playback: false,
            desktop_scale_factor: 0,
            hardware_id: None,
            license_cache: None,
            timezone_info: Default::default(),
        };

        // Establish TCP connection
        let addr = format!("{}:{}", config.host, config.port);
        let tcp_stream = TcpStream::connect(&addr).await?;
        let client_addr: SocketAddr = tcp_stream.local_addr()?;
        debug!("TCP connection established from {:?}", client_addr);

        // Create framed transport for initial connection
        let mut framed: TokioFramed<TcpStream> = TokioFramed::new(tcp_stream);

        // Create connector
        let mut connector = ClientConnector::new(connector_config, client_addr);

        // Create clipboard state (shared between backend and session)
        let clipboard_state = Arc::new(parking_lot::Mutex::new(clipboard::ClipboardState::default()));

        // RDPSND (audio) channel - required for RDPDR on Windows 2012+ and good to have
        let rdpsnd = Rdpsnd::new(Box::new(NoopRdpsndBackend));
        connector.attach_static_channel(rdpsnd);

        // Set up CLIPRDR (clipboard) with our custom backend
        let (cliprdr, clipboard_backend_rx) = clipboard::create_cliprdr(Arc::clone(&clipboard_state));
        connector.attach_static_channel(cliprdr);
        info!("Clipboard redirection enabled");

        // Set up RDPDR (drive redirection) if drives are configured
        if !config.drives.is_empty() {
            // Use the first drive's path as the base directory for the native backend
            // Note: PlatformRdpdrBackend only supports a single base directory
            let first_drive = &config.drives[0];
            let backend = Box::new(PlatformRdpdrBackend::new(first_drive.path.clone()));
            let rdpdr = Rdpdr::new(backend, "agent-rdp".to_string());

            // Configure drives - convert DriveMapping to (device_id, name) pairs
            let drive_list: Vec<(u32, String)> = config
                .drives
                .iter()
                .enumerate()
                .map(|(idx, d)| ((idx + 1) as u32, d.name.clone()))
                .collect();

            let rdpdr = rdpdr.with_drives(Some(drive_list));
            connector.attach_static_channel(rdpdr);

            info!(
                "Drive redirection enabled: {} -> \\\\TSCLIENT\\{}",
                first_drive.path, first_drive.name
            );
        }

        // Begin connection (pre-TLS)
        let should_upgrade = ironrdp_tokio::connect_begin(&mut framed, &mut connector)
            .await
            .map_err(|e| RdpError::ConnectionFailed(e.to_string()))?;

        // Perform TLS upgrade
        let initial_stream: TcpStream = framed.into_inner_no_leftover();
        let (tls_stream, server_cert) = Self::tls_upgrade(initial_stream, &config.host)
            .await
            .map_err(|e| RdpError::TlsError(e.to_string()))?;
        debug!("TLS connection established");

        // Mark upgrade as done
        let upgraded = ironrdp_tokio::mark_as_upgraded(should_upgrade, &mut connector);

        // Create framed transport for upgraded connection
        let mut upgraded_framed: TokioFramed<tokio_rustls::client::TlsStream<TcpStream>> =
            TokioFramed::new(tls_stream);

        // Extract server public key from certificate
        let server_public_key = Self::extract_public_key(&server_cert)?;

        // Create network client for CredSSP
        let mut network_client = NoopNetworkClient;

        // Convert host to ServerName
        let server_name: ServerName = config.host.clone().into();

        // Finalize connection (post-TLS)
        let connection_result = ironrdp_tokio::connect_finalize(
            upgraded,
            connector,
            &mut upgraded_framed,
            &mut network_client,
            server_name,
            server_public_key,
            None, // No Kerberos
        )
        .await
        .map_err(|e| RdpError::ConnectionFailed(e.to_string()))?;

        info!("RDP connection established to {}", config.host);

        // Create decoded image for storing desktop state
        let image = DecodedImage::new(
            ironrdp_graphics::image_processing::PixelFormat::RgbA32,
            connection_result.desktop_size.width,
            connection_result.desktop_size.height,
        );

        // Create active stage for ongoing communication
        let active_stage = ActiveStage::new(connection_result);

        // Create shared state
        let shared = Arc::new(RwLock::new(SharedState {
            image,
            host: config.host.clone(),
            width: config.width,
            height: config.height,
            drives: config.drives.clone(),
            clipboard: clipboard_state,
        }));

        // Create command channel
        let (command_tx, command_rx) = mpsc::channel(32);

        // Spawn background frame processor
        let shared_clone = Arc::clone(&shared);
        let task_handle = tokio::spawn(async move {
            run_frame_processor(
                upgraded_framed,
                active_stage,
                shared_clone,
                command_rx,
                disconnect_notify,
                clipboard_backend_rx,
            )
            .await;
        });

        Ok(Self {
            shared,
            command_tx,
            _task_handle: task_handle,
        })
    }

    /// Perform TLS upgrade on the stream.
    async fn tls_upgrade(
        stream: TcpStream,
        server_name: &str,
    ) -> Result<(tokio_rustls::client::TlsStream<TcpStream>, Vec<u8>), std::io::Error> {
        use tokio_rustls::TlsConnector;

        let tls_config = Self::create_tls_config();
        let connector = TlsConnector::from(Arc::new(tls_config));

        // Try to parse as IP address first, then as DNS name
        let server_name = if let Ok(ip) = server_name.parse::<std::net::IpAddr>() {
            rustls::pki_types::ServerName::IpAddress(ip.into())
        } else {
            rustls::pki_types::ServerName::try_from(server_name.to_string())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?
        };

        let tls_stream = connector.connect(server_name, stream).await?;

        // Get peer certificate
        let (_, server_conn) = tls_stream.get_ref();
        let certs = server_conn
            .peer_certificates()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "No peer certificate"))?;

        let cert_der = certs
            .first()
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::Other, "Empty certificate chain")
            })?
            .to_vec();

        Ok((tls_stream, cert_der))
    }

    /// Create TLS configuration that accepts self-signed certificates.
    fn create_tls_config() -> rustls::ClientConfig {
        // Install ring as the default crypto provider
        let _ = rustls::crypto::ring::default_provider().install_default();

        let mut root_store = rustls::RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        // RDP servers often use self-signed certificates
        rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth()
    }

    /// Extract public key from DER-encoded certificate.
    fn extract_public_key(cert_der: &[u8]) -> Result<Vec<u8>, RdpError> {
        use x509_cert::der::Decode;

        let cert = x509_cert::Certificate::from_der(cert_der)
            .map_err(|e| RdpError::TlsError(format!("Failed to parse certificate: {}", e)))?;

        Ok(cert
            .tbs_certificate
            .subject_public_key_info
            .subject_public_key
            .as_bytes()
            .ok_or_else(|| RdpError::TlsError("No public key in certificate".into()))?
            .to_vec())
    }

    /// Get the connected host.
    pub fn host(&self) -> String {
        self.shared.read().host.clone()
    }

    /// Get the desktop width.
    pub fn width(&self) -> u16 {
        self.shared.read().width
    }

    /// Get the desktop height.
    pub fn height(&self) -> u16 {
        self.shared.read().height
    }

    /// Get the drives that were mapped at connect time.
    pub fn get_drives(&self) -> Vec<DriveMapping> {
        self.shared.read().drives.clone()
    }

    /// Get a copy of the current desktop image data.
    pub fn get_image_data(&self) -> (u16, u16, Vec<u8>) {
        let state = self.shared.read();
        let width = state.image.width();
        let height = state.image.height();
        let data = state.image.data().to_vec();
        (width, height, data)
    }

    /// Send input events to the remote desktop.
    pub async fn send_input(&self, events: Vec<FastPathInputEvent>) -> Result<(), RdpError> {
        debug!("Sending {} input events to frame processor", events.len());
        self.command_tx
            .send(SessionCommand::SendInput(events))
            .await
            .map_err(|_| RdpError::SessionClosed)
    }

    /// Set clipboard text (will be available when remote pastes).
    pub async fn clipboard_set(&self, text: String) -> Result<(), RdpError> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.command_tx
            .send(SessionCommand::ClipboardSet { text, response_tx })
            .await
            .map_err(|_| RdpError::SessionClosed)?;

        response_rx
            .await
            .map_err(|_| RdpError::SessionClosed)?
            .map_err(|e| RdpError::ProtocolError(e))
    }

    /// Get clipboard text from remote.
    pub async fn clipboard_get(&self) -> Result<Option<String>, RdpError> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.command_tx
            .send(SessionCommand::ClipboardGet { response_tx })
            .await
            .map_err(|_| RdpError::SessionClosed)?;

        response_rx
            .await
            .map_err(|_| RdpError::SessionClosed)?
            .map_err(|e| RdpError::ProtocolError(e))
    }

    /// Disconnect from the RDP server.
    pub async fn disconnect(self) -> Result<(), RdpError> {
        info!("Disconnecting from RDP session");
        let _ = self.command_tx.send(SessionCommand::Shutdown).await;
        Ok(())
    }
}

/// Background task that continuously processes RDP frames.
async fn run_frame_processor(
    mut framed: TokioFramed<tokio_rustls::client::TlsStream<TcpStream>>,
    mut active_stage: ActiveStage,
    shared: Arc<RwLock<SharedState>>,
    mut command_rx: mpsc::Receiver<SessionCommand>,
    disconnect_notify: Option<DisconnectNotify>,
    mut clipboard_backend_rx: mpsc::UnboundedReceiver<clipboard::BackendMessage>,
) {
    info!("Frame processor started");
    let mut graceful_shutdown = false;

    loop {
        tokio::select! {
            // Handle incoming commands
            cmd = command_rx.recv() => {
                match cmd {
                    Some(SessionCommand::SendInput(events)) => {
                        debug!("Frame processor received {} input events", events.len());
                        // Process input and collect response frames
                        let frames_to_send: Vec<Vec<u8>> = {
                            let mut state = shared.write();
                            match active_stage.process_fastpath_input(&mut state.image, &events) {
                                Ok(outputs) => {
                                    debug!("Input processing generated {} outputs", outputs.len());
                                    outputs.into_iter()
                                        .filter_map(|o| {
                                            if let ActiveStageOutput::ResponseFrame(frame) = o {
                                                Some(frame)
                                            } else {
                                                None
                                            }
                                        })
                                        .collect()
                                }
                                Err(e) => {
                                    error!("Failed to process input: {}", e);
                                    Vec::new()
                                }
                            }
                        };
                        // Send frames after releasing lock
                        debug!("Sending {} input response frames", frames_to_send.len());
                        for frame in &frames_to_send {
                            debug!("Sending input frame of {} bytes", frame.len());
                            if let Err(e) = framed.write_all(frame).await {
                                error!("Failed to send input frame: {}", e);
                            }
                        }
                    }
                    Some(SessionCommand::ClipboardSet { text, response_tx }) => {
                        debug!("Clipboard set: {} chars", text.len());
                        // Store text in clipboard state
                        {
                            let state = shared.read();
                            let mut clipboard = state.clipboard.lock();
                            clipboard.local_text = Some(text);
                        }
                        // Trigger initiate_copy to announce we have data
                        if let Some(cliprdr) = active_stage.get_svc_processor_mut::<clipboard::CliprdrClient>() {
                            let formats = vec![clipboard::ClipboardFormat::new(clipboard::cf_unicodetext())];
                            match cliprdr.initiate_copy(&formats) {
                                Ok(messages) => {
                                    if let Ok(pdu_bytes) = active_stage.process_svc_processor_messages(messages) {
                                        let _ = framed.write_all(&pdu_bytes).await;
                                    }
                                    let _ = response_tx.send(Ok(()));
                                }
                                Err(e) => {
                                    let _ = response_tx.send(Err(format!("initiate_copy failed: {}", e)));
                                }
                            }
                        } else {
                            let _ = response_tx.send(Err("Clipboard not available".to_string()));
                        }
                    }
                    Some(SessionCommand::ClipboardGet { response_tx }) => {
                        debug!("Clipboard get requested");
                        // Check if we already have remote text cached
                        let cached = {
                            let state = shared.read();
                            let clipboard = state.clipboard.lock();
                            clipboard.remote_text.clone()
                        };
                        if let Some(text) = cached {
                            let _ = response_tx.send(Ok(Some(text)));
                        } else {
                            // Need to request from remote - store the response channel
                            {
                                let state = shared.read();
                                let mut clipboard = state.clipboard.lock();
                                clipboard.pending_get = Some(response_tx);
                            }
                            // Initiate paste to request data
                            if let Some(cliprdr) = active_stage.get_svc_processor_mut::<clipboard::CliprdrClient>() {
                                match cliprdr.initiate_paste(clipboard::cf_unicodetext()) {
                                    Ok(messages) => {
                                        if let Ok(pdu_bytes) = active_stage.process_svc_processor_messages(messages) {
                                            let _ = framed.write_all(&pdu_bytes).await;
                                        }
                                    }
                                    Err(e) => {
                                        error!("initiate_paste failed: {}", e);
                                        // Return pending response with error
                                        let state = shared.read();
                                        let mut clipboard = state.clipboard.lock();
                                        if let Some(tx) = clipboard.pending_get.take() {
                                            let _ = tx.send(Err(format!("initiate_paste failed: {}", e)));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Some(SessionCommand::Shutdown) => {
                        info!("Shutdown command received");
                        graceful_shutdown = true;
                        // Collect shutdown frames
                        let frames_to_send: Vec<Vec<u8>> = {
                            if let Ok(outputs) = active_stage.graceful_shutdown() {
                                outputs.into_iter()
                                    .filter_map(|o| {
                                        if let ActiveStageOutput::ResponseFrame(frame) = o {
                                            Some(frame)
                                        } else {
                                            None
                                        }
                                    })
                                    .collect()
                            } else {
                                Vec::new()
                            }
                        };
                        // Send frames
                        for frame in frames_to_send {
                            let _ = framed.write_all(&frame).await;
                        }
                        break;
                    }
                    None => {
                        // Channel closed, exit
                        break;
                    }
                }
            }

            // Process incoming RDP frames
            result = framed.read_pdu() => {
                match result {
                    Ok((action, payload)) => {
                        // Process frame and collect responses
                        let (frames_to_send, should_terminate) = {
                            let mut state = shared.write();
                            match active_stage.process(&mut state.image, action, &payload) {
                                Ok(outputs) => {
                                    let mut frames = Vec::new();
                                    let mut terminate = false;
                                    for output in outputs {
                                        match output {
                                            ActiveStageOutput::ResponseFrame(frame) => {
                                                frames.push(frame);
                                            }
                                            ActiveStageOutput::Terminate(reason) => {
                                                warn!("Session terminated: {:?}", reason);
                                                terminate = true;
                                            }
                                            _ => {}
                                        }
                                    }
                                    (frames, terminate)
                                }
                                Err(e) => {
                                    error!("Failed to process frame: {}", e);
                                    (Vec::new(), false)
                                }
                            }
                        };
                        // Send frames after releasing lock
                        for frame in frames_to_send {
                            if let Err(e) = framed.write_all(&frame).await {
                                error!("Failed to send response frame: {}", e);
                            }
                        }
                        if should_terminate {
                            // Server-initiated termination - notify daemon
                            if let Some(notify) = disconnect_notify {
                                let _ = notify.send(()).await;
                            }
                            return;
                        }
                    }
                    Err(e) => {
                        error!("Failed to read PDU: {}", e);
                        break;
                    }
                }
            }

            // Handle clipboard backend messages
            msg = clipboard_backend_rx.recv() => {
                if let Some(msg) = msg {
                    match msg {
                        clipboard::BackendMessage::InitiateCopy(formats) => {
                            debug!("Backend: InitiateCopy with {} formats", formats.len());
                            if let Some(cliprdr) = active_stage.get_svc_processor_mut::<clipboard::CliprdrClient>() {
                                if let Ok(messages) = cliprdr.initiate_copy(&formats) {
                                    if let Ok(pdu_bytes) = active_stage.process_svc_processor_messages(messages) {
                                        let _ = framed.write_all(&pdu_bytes).await;
                                    }
                                }
                            }
                        }
                        clipboard::BackendMessage::FormatData(response) => {
                            debug!("Backend: FormatData");
                            if let Some(cliprdr) = active_stage.get_svc_processor_mut::<clipboard::CliprdrClient>() {
                                if let Ok(messages) = cliprdr.submit_format_data(response) {
                                    if let Ok(pdu_bytes) = active_stage.process_svc_processor_messages(messages) {
                                        let _ = framed.write_all(&pdu_bytes).await;
                                    }
                                }
                            }
                        }
                        clipboard::BackendMessage::InitiatePaste(format_id) => {
                            debug!("Backend: InitiatePaste for {:?}", format_id);
                            if let Some(cliprdr) = active_stage.get_svc_processor_mut::<clipboard::CliprdrClient>() {
                                if let Ok(messages) = cliprdr.initiate_paste(format_id) {
                                    if let Ok(pdu_bytes) = active_stage.process_svc_processor_messages(messages) {
                                        let _ = framed.write_all(&pdu_bytes).await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    info!("Frame processor stopped (graceful={})", graceful_shutdown);

    // Notify daemon of connection drop (unless this was a graceful shutdown)
    if !graceful_shutdown {
        if let Some(notify) = disconnect_notify {
            info!("Notifying daemon of connection drop");
            let _ = notify.send(()).await;
        }
    }
}

/// Custom certificate verifier that accepts all certificates.
/// This is necessary because RDP servers typically use self-signed certificates.
#[derive(Debug)]
struct NoVerifier;

impl rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::ED25519,
        ]
    }
}

/// No-op network client for CredSSP.
/// This works for basic NTLM authentication but doesn't support Kerberos.
struct NoopNetworkClient;

impl ironrdp_tokio::NetworkClient for NoopNetworkClient {
    fn send(
        &mut self,
        _network_request: &ironrdp::connector::sspi::generator::NetworkRequest,
    ) -> impl Future<Output = ConnectorResult<Vec<u8>>> {
        async move {
            // Return empty response - NTLM auth doesn't need network calls
            Ok(Vec::new())
        }
    }
}
