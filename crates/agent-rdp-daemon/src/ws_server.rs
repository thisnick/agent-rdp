//! WebSocket server for streaming RDP desktop and handling input.
//!
//! Provides a WebSocket interface matching the agent-browser protocol for
//! debugging and interactive viewing of the remote desktop.

use std::collections::HashSet;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde::Serialize;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info};

use crate::rdp_session::RdpSession;
use crate::ws_input::{keyboard_to_fastpath, mouse_to_fastpath, WsInputMessage};

/// Frame message sent to clients.
#[derive(Debug, Serialize)]
struct FrameMessage {
    #[serde(rename = "type")]
    msg_type: &'static str,
    data: String,
    metadata: FrameMetadata,
}

/// Metadata included with frame messages.
#[derive(Debug, Serialize)]
struct FrameMetadata {
    #[serde(rename = "deviceWidth")]
    device_width: u16,
    #[serde(rename = "deviceHeight")]
    device_height: u16,
}

/// Status message sent to clients.
#[derive(Debug, Serialize)]
struct StatusMessage {
    #[serde(rename = "type")]
    msg_type: &'static str,
    connected: bool,
    streaming: bool,
    #[serde(rename = "viewportWidth")]
    viewport_width: u16,
    #[serde(rename = "viewportHeight")]
    viewport_height: u16,
}

/// Client ID type.
type ClientId = u64;

/// WebSocket server for desktop streaming.
pub struct WsServer {
    port: u16,
    jpeg_quality: u8,
    /// Active clients (by ID).
    clients: Arc<Mutex<HashSet<ClientId>>>,
    /// Next client ID.
    next_client_id: Arc<Mutex<ClientId>>,
}

/// Configuration for the WebSocket server.
pub struct WsServerConfig {
    pub port: u16,
    pub fps: u32,
    pub jpeg_quality: u8,
}

impl Default for WsServerConfig {
    fn default() -> Self {
        Self {
            port: 9224,
            fps: 10,
            jpeg_quality: 80,
        }
    }
}

impl WsServer {
    /// Create a new WebSocket server.
    pub fn new(config: WsServerConfig) -> Self {
        Self {
            port: config.port,
            jpeg_quality: config.jpeg_quality,
            clients: Arc::new(Mutex::new(HashSet::new())),
            next_client_id: Arc::new(Mutex::new(0)),
        }
    }

    /// Start the WebSocket server.
    ///
    /// Returns a handle that can be used to broadcast frames to clients.
    pub async fn start(
        &self,
        rdp_session: Arc<tokio::sync::Mutex<Option<RdpSession>>>,
    ) -> anyhow::Result<WsServerHandle> {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        info!("WebSocket server listening on ws://{}", addr);

        // Create broadcast channel
        let (broadcast_tx, _) = tokio::sync::broadcast::channel::<String>(16);
        let broadcast_tx_clone = broadcast_tx.clone();

        // Spawn accept loop
        let clients = Arc::clone(&self.clients);
        let next_client_id = Arc::clone(&self.next_client_id);
        let jpeg_quality = self.jpeg_quality;

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        debug!("WebSocket connection from {}", addr);

                        let client_id = {
                            let mut id = next_client_id.lock();
                            *id += 1;
                            *id
                        };

                        let clients = Arc::clone(&clients);
                        let rdp_session = Arc::clone(&rdp_session);
                        let broadcast_rx = broadcast_tx.subscribe();
                        let jpeg_quality = jpeg_quality;

                        tokio::spawn(async move {
                            if let Err(e) = handle_client(
                                stream,
                                client_id,
                                clients,
                                rdp_session,
                                broadcast_rx,
                                jpeg_quality,
                            )
                            .await
                            {
                                debug!("Client {} disconnected: {}", client_id, e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept WebSocket connection: {}", e);
                    }
                }
            }
        });

        Ok(WsServerHandle {
            broadcast_tx: broadcast_tx_clone,
            clients: Arc::clone(&self.clients),
            jpeg_quality: self.jpeg_quality,
        })
    }
}

/// Handle for broadcasting frames to WebSocket clients.
pub struct WsServerHandle {
    broadcast_tx: tokio::sync::broadcast::Sender<String>,
    clients: Arc<Mutex<HashSet<ClientId>>>,
    jpeg_quality: u8,
}

impl WsServerHandle {
    /// Check if there are any connected clients.
    pub fn has_clients(&self) -> bool {
        !self.clients.lock().is_empty()
    }

    /// Broadcast a frame to all connected clients.
    ///
    /// Takes the raw RGBA image data and converts it to JPEG.
    pub fn broadcast_frame(&self, width: u16, height: u16, rgba_data: &[u8]) {
        if !self.has_clients() {
            return;
        }

        // Convert RGBA to JPEG
        let jpeg_data = match encode_jpeg(width, height, rgba_data, self.jpeg_quality) {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to encode JPEG: {}", e);
                return;
            }
        };

        // Base64 encode
        let base64_data = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &jpeg_data,
        );

        // Create frame message
        let msg = FrameMessage {
            msg_type: "frame",
            data: base64_data,
            metadata: FrameMetadata {
                device_width: width,
                device_height: height,
            },
        };

        if let Ok(json) = serde_json::to_string(&msg) {
            let _ = self.broadcast_tx.send(json);
        }
    }
}

/// Handle a single WebSocket client connection.
async fn handle_client(
    stream: TcpStream,
    client_id: ClientId,
    clients: Arc<Mutex<HashSet<ClientId>>>,
    rdp_session: Arc<tokio::sync::Mutex<Option<RdpSession>>>,
    mut broadcast_rx: tokio::sync::broadcast::Receiver<String>,
    jpeg_quality: u8,
) -> anyhow::Result<()> {
    // Upgrade to WebSocket
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let (mut ws_sink, mut ws_stream) = ws_stream.split();

    // Register client
    {
        clients.lock().insert(client_id);
    }
    info!("Client {} connected (total: {})", client_id, clients.lock().len());

    // Send initial status
    {
        let session = rdp_session.lock().await;
        let (connected, width, height) = if let Some(ref rdp) = *session {
            (true, rdp.width(), rdp.height())
        } else {
            (false, 0, 0)
        };

        let status = StatusMessage {
            msg_type: "status",
            connected,
            streaming: true,
            viewport_width: width,
            viewport_height: height,
        };

        if let Ok(json) = serde_json::to_string(&status) {
            let _ = ws_sink.send(Message::Text(json.into())).await;
        }
    }

    // Send initial frame
    {
        let session = rdp_session.lock().await;
        if let Some(ref rdp) = *session {
            let (width, height, data) = rdp.get_image_data();
            if let Ok(jpeg_data) = encode_jpeg(width, height, &data, jpeg_quality) {
                let base64_data = base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    &jpeg_data,
                );
                let msg = FrameMessage {
                    msg_type: "frame",
                    data: base64_data,
                    metadata: FrameMetadata {
                        device_width: width,
                        device_height: height,
                    },
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    let _ = ws_sink.send(Message::Text(json.into())).await;
                }
            }
        }
    }

    loop {
        tokio::select! {
            // Receive broadcast frames
            result = broadcast_rx.recv() => {
                match result {
                    Ok(json) => {
                        if let Err(e) = ws_sink.send(Message::Text(json.into())).await {
                            debug!("Failed to send frame to client {}: {}", client_id, e);
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        debug!("Client {} lagged {} frames", client_id, n);
                    }
                    Err(_) => break,
                }
            }

            // Receive client messages
            result = ws_stream.next() => {
                match result {
                    Some(Ok(msg)) => {
                        if let Message::Text(text) = msg {
                            handle_client_message(&text, &rdp_session).await;
                        } else if let Message::Close(_) = msg {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        debug!("WebSocket error for client {}: {}", client_id, e);
                        break;
                    }
                    None => break,
                }
            }
        }
    }

    // Unregister client
    {
        clients.lock().remove(&client_id);
    }
    info!("Client {} disconnected (total: {})", client_id, clients.lock().len());

    Ok(())
}

/// Handle an incoming message from a WebSocket client.
async fn handle_client_message(
    text: &str,
    rdp_session: &Arc<tokio::sync::Mutex<Option<RdpSession>>>,
) {
    // Parse the input message
    let input: WsInputMessage = match serde_json::from_str(text) {
        Ok(msg) => msg,
        Err(e) => {
            debug!("Failed to parse WebSocket message: {} - {}", e, text);
            return;
        }
    };

    // Convert to FastPath events
    let events = match input {
        WsInputMessage::Mouse(payload) => mouse_to_fastpath(&payload),
        WsInputMessage::Keyboard(payload) => keyboard_to_fastpath(&payload),
    };

    if events.is_empty() {
        return;
    }

    // Send to RDP session
    let session = rdp_session.lock().await;
    if let Some(ref rdp) = *session {
        if let Err(e) = rdp.send_input(events).await {
            error!("Failed to send input to RDP session: {}", e);
        }
    }
}

/// Encode RGBA image data to JPEG.
fn encode_jpeg(width: u16, height: u16, rgba_data: &[u8], quality: u8) -> anyhow::Result<Vec<u8>> {
    use image::{ImageBuffer, Rgba};

    // Create image buffer from RGBA data
    let img: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_raw(
        width as u32,
        height as u32,
        rgba_data.to_vec(),
    )
    .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;

    // Convert to RGB (JPEG doesn't support alpha)
    let rgb_img = image::DynamicImage::ImageRgba8(img).into_rgb8();

    // Encode to JPEG
    let mut jpeg_data = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_data, quality);
    rgb_img.write_with_encoder(encoder)?;

    Ok(jpeg_data)
}

/// Get the stream port from environment or default.
pub fn get_stream_port() -> u16 {
    std::env::var("AGENT_RDP_STREAM_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// Get the stream FPS from environment or default.
pub fn get_stream_fps() -> u32 {
    std::env::var("AGENT_RDP_STREAM_FPS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10)
}

/// Get the stream JPEG quality from environment or default.
pub fn get_stream_quality() -> u8 {
    std::env::var("AGENT_RDP_STREAM_QUALITY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(80)
}
