//! Main daemon event loop.

use std::sync::Arc;
use std::time::{Duration, Instant};

use agent_rdp_protocol::{Request, Response, ResponseData, SessionInfo, ConnectionState, ErrorCode};
use tokio::sync::{broadcast, Mutex};
use tracing::{error, info, warn};

use crate::handlers;
use crate::ipc_server::IpcServer;
use crate::rdp_session::RdpSession;
use crate::ws_server::{WsServer, WsServerConfig, WsServerHandle};

/// The main daemon that manages an RDP session.
pub struct Daemon {
    /// Session name.
    session_name: String,

    /// The RDP session (if connected).
    rdp_session: Arc<Mutex<Option<RdpSession>>>,

    /// IPC server for CLI communication.
    ipc_server: IpcServer,

    /// Time when daemon started.
    start_time: Instant,

    /// Shutdown signal sender.
    shutdown_tx: broadcast::Sender<()>,

    /// Channel to receive connection drop notifications from RDP session.
    disconnect_rx: tokio::sync::mpsc::Receiver<()>,

    /// Sender for connection drop notifications (passed to RDP sessions).
    disconnect_tx: tokio::sync::mpsc::Sender<()>,

    /// WebSocket server handle for streaming (if enabled).
    ws_handle: Option<WsServerHandle>,

    /// WebSocket streaming frame interval.
    stream_fps: u32,
}

impl Daemon {
    /// Create a new daemon for the given session.
    pub async fn new(session_name: String) -> anyhow::Result<Self> {
        let socket_path = crate::get_socket_path(&session_name);

        // Clean up stale socket if it exists
        if socket_path.exists() {
            std::fs::remove_file(&socket_path)?;
        }

        let ipc_server = IpcServer::bind(&socket_path).await?;
        let (shutdown_tx, _) = broadcast::channel(1);
        let (disconnect_tx, disconnect_rx) = tokio::sync::mpsc::channel(1);

        // Check for WebSocket streaming configuration
        let stream_port = crate::ws_server::get_stream_port();
        let stream_fps = crate::ws_server::get_stream_fps();
        let stream_quality = crate::ws_server::get_stream_quality();

        let rdp_session = Arc::new(Mutex::new(None));

        // Start WebSocket server if configured
        let ws_handle = if stream_port > 0 {
            let config = WsServerConfig {
                port: stream_port,
                fps: stream_fps,
                jpeg_quality: stream_quality,
            };
            let ws_server = WsServer::new(config);
            match ws_server.start(Arc::clone(&rdp_session)).await {
                Ok(handle) => {
                    info!("WebSocket streaming enabled on port {}", stream_port);
                    Some(handle)
                }
                Err(e) => {
                    warn!("Failed to start WebSocket server: {}", e);
                    None
                }
            }
        } else {
            None
        };

        info!("Daemon started for session '{}' at {:?}", session_name, socket_path);

        Ok(Self {
            session_name,
            rdp_session,
            ipc_server,
            start_time: Instant::now(),
            shutdown_tx,
            disconnect_rx,
            disconnect_tx,
            ws_handle,
            stream_fps,
        })
    }

    /// Run the daemon event loop.
    pub async fn run(&mut self) -> anyhow::Result<()> {
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        // Frame broadcast interval for WebSocket streaming
        let frame_interval = Duration::from_millis(1000 / self.stream_fps.max(1) as u64);
        let mut frame_timer = tokio::time::interval(frame_interval);
        frame_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // Accept new CLI connections
                result = self.ipc_server.accept() => {
                    match result {
                        Ok(stream) => {
                            let session = Arc::clone(&self.rdp_session);
                            let session_name = self.session_name.clone();
                            let start_time = self.start_time;
                            let shutdown_tx = self.shutdown_tx.clone();
                            let disconnect_tx = self.disconnect_tx.clone();

                            tokio::spawn(async move {
                                if let Err(e) = handle_client(stream, session, session_name, start_time, shutdown_tx, disconnect_tx).await {
                                    error!("Client handler error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("Failed to accept connection: {}", e);
                        }
                    }
                }

                // Handle connection drop from RDP session
                _ = self.disconnect_rx.recv() => {
                    info!("RDP connection dropped, shutting down daemon");
                    break;
                }

                // Handle shutdown signal from client
                _ = shutdown_rx.recv() => {
                    info!("Received shutdown request from client");
                    break;
                }

                // Handle Ctrl+C
                _ = tokio::signal::ctrl_c() => {
                    info!("Received Ctrl+C, cleaning up...");
                    break;
                }

                // Broadcast frames to WebSocket clients
                _ = frame_timer.tick(), if self.ws_handle.is_some() => {
                    if let Some(ref handle) = self.ws_handle {
                        if handle.has_clients() {
                            let session = self.rdp_session.lock().await;
                            if let Some(ref rdp) = *session {
                                let (width, height, data) = rdp.get_image_data();
                                drop(session); // Release lock before broadcasting
                                handle.broadcast_frame(width, height, &data);
                            }
                        }
                    }
                }
            }
        }

        // Graceful shutdown
        self.shutdown().await
    }

    /// Gracefully shut down the daemon.
    async fn shutdown(&mut self) -> anyhow::Result<()> {
        info!("Shutting down daemon...");

        // Disconnect RDP session if connected
        let mut session = self.rdp_session.lock().await;
        if let Some(rdp) = session.take() {
            if let Err(e) = rdp.disconnect().await {
                warn!("Error during RDP disconnect: {}", e);
            }
        }

        // Clean up socket and PID files
        let socket_path = crate::get_socket_path(&self.session_name);
        let pid_path = crate::get_pid_path(&self.session_name);

        let _ = std::fs::remove_file(&socket_path);
        let _ = std::fs::remove_file(&pid_path);

        info!("Daemon shutdown complete");
        Ok(())
    }
}

/// Handle a single client connection.
async fn handle_client(
    stream: crate::ipc_server::IpcStream,
    rdp_session: Arc<Mutex<Option<RdpSession>>>,
    session_name: String,
    start_time: Instant,
    shutdown_tx: broadcast::Sender<()>,
    disconnect_tx: tokio::sync::mpsc::Sender<()>,
) -> anyhow::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;

        if n == 0 {
            // Client disconnected
            break;
        }

        let request: Request = match serde_json::from_str(line.trim()) {
            Ok(req) => req,
            Err(e) => {
                let resp = Response::error(ErrorCode::InvalidRequest, format!("Invalid request: {}", e));
                let json = serde_json::to_string(&resp)? + "\n";
                writer.write_all(json.as_bytes()).await?;
                writer.flush().await?;
                continue;
            }
        };

        let is_shutdown = matches!(request, Request::Shutdown);

        let response = process_request(
            request,
            &rdp_session,
            &session_name,
            start_time,
            &disconnect_tx,
        ).await;

        let json = serde_json::to_string(&response)? + "\n";
        writer.write_all(json.as_bytes()).await?;
        writer.flush().await?;

        // Trigger daemon shutdown if this was a shutdown request
        if is_shutdown {
            info!("Shutdown request received, signaling daemon to exit");
            let _ = shutdown_tx.send(());
            break;
        }
    }

    Ok(())
}

/// Process a single request and return a response.
async fn process_request(
    request: Request,
    rdp_session: &Arc<Mutex<Option<RdpSession>>>,
    session_name: &str,
    start_time: Instant,
    disconnect_tx: &tokio::sync::mpsc::Sender<()>,
) -> Response {
    match request {
        Request::Ping => Response::success(ResponseData::Pong),

        Request::SessionInfo => {
            let session = rdp_session.lock().await;
            let (state, host, width, height) = if let Some(ref rdp) = *session {
                (
                    ConnectionState::Connected,
                    Some(rdp.host().to_string()),
                    Some(rdp.width()),
                    Some(rdp.height()),
                )
            } else {
                (ConnectionState::Disconnected, None, None, None)
            };

            Response::success(ResponseData::SessionInfo(SessionInfo {
                name: session_name.to_string(),
                state,
                host,
                width,
                height,
                pid: std::process::id(),
                uptime_secs: start_time.elapsed().as_secs(),
            }))
        }

        Request::Shutdown => {
            // Will trigger shutdown after response is sent
            Response::ok()
        }

        Request::Connect(params) => {
            handlers::connect::handle(rdp_session, params, disconnect_tx.clone()).await
        }

        Request::Disconnect => {
            handlers::connect::handle_disconnect(rdp_session).await
        }

        Request::Screenshot(params) => {
            handlers::screenshot::handle(rdp_session, params).await
        }

        Request::Mouse(action) => {
            handlers::mouse::handle(rdp_session, action).await
        }

        Request::Keyboard(action) => {
            handlers::keyboard::handle(rdp_session, action).await
        }

        Request::Scroll(params) => {
            handlers::scroll::handle(rdp_session, params).await
        }

        Request::Clipboard(action) => {
            handlers::clipboard::handle(rdp_session, action).await
        }

        Request::Drive(action) => {
            handlers::drive::handle(rdp_session, action).await
        }
    }
}
