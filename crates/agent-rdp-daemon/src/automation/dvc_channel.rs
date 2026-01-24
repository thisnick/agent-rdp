//! DVC processor for automation communication.
//!
//! Implements the DvcProcessor trait for bidirectional communication with the
//! PowerShell automation agent via Dynamic Virtual Channel.

use std::collections::HashMap;
use std::sync::Arc;

use ironrdp_dvc::ironrdp_pdu::PduResult;
use ironrdp_dvc::{DvcMessage, DvcProcessor};
use ironrdp_svc::impl_as_any;
use parking_lot::Mutex;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, trace, warn};

/// DVC channel name for automation.
pub const CHANNEL_NAME: &str = "AgentRdp::Automation";

/// Message types for DVC protocol.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DvcProtocolMessage {
    /// Handshake sent by PowerShell agent when channel opens.
    Handshake {
        version: String,
        agent_pid: u32,
        capabilities: Vec<String>,
    },
    /// Request sent from Rust to PowerShell.
    Request {
        id: String,
        command: String,
        params: serde_json::Value,
    },
    /// Response sent from PowerShell to Rust.
    Response {
        id: String,
        success: bool,
        data: Option<serde_json::Value>,
        error: Option<DvcError>,
    },
    /// Poll message from PowerShell to trigger sending queued requests.
    Poll,
}

/// Error in DVC response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DvcError {
    pub code: String,
    pub message: String,
}

/// Handshake data from the PowerShell agent.
#[derive(Debug, Clone)]
pub struct DvcHandshake {
    pub version: String,
    pub agent_pid: u32,
    pub capabilities: Vec<String>,
}

/// Response data for pending requests.
#[derive(Debug)]
pub struct DvcResponse {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<DvcError>,
}

/// Command to send DVC data through the RDP session.
#[derive(Debug)]
pub struct DvcSendCommand {
    pub channel_id: u32,
    pub data: Vec<u8>,
}

/// Sender for DVC commands to the RDP session.
pub type DvcCommandSender = mpsc::UnboundedSender<DvcSendCommand>;
/// Receiver for DVC commands in the RDP session.
pub type DvcCommandReceiver = mpsc::UnboundedReceiver<DvcSendCommand>;

/// Shared state for DVC communication, accessible from both the processor and IPC.
#[derive(Debug)]
pub struct DvcSharedState {
    /// Pending requests awaiting response (id -> sender).
    pub pending: HashMap<String, oneshot::Sender<DvcResponse>>,
    /// Handshake received from PowerShell.
    pub handshake: Option<DvcHandshake>,
    /// Channel ID (set when opened).
    pub channel_id: Option<u32>,
    /// Sender to send DVC data through the RDP session.
    pub command_tx: Option<DvcCommandSender>,
}

impl Default for DvcSharedState {
    fn default() -> Self {
        Self {
            pending: HashMap::new(),
            handshake: None,
            channel_id: None,
            command_tx: None,
        }
    }
}

/// Shared state handle.
pub type SharedDvcState = Arc<Mutex<DvcSharedState>>;

/// Create a new shared DVC state.
pub fn new_shared_dvc_state() -> SharedDvcState {
    Arc::new(Mutex::new(DvcSharedState::default()))
}

/// DVC processor for automation channel.
pub struct AutomationDvc {
    /// Shared state for communication with IPC layer.
    state: SharedDvcState,
    /// Notify channel for handshake completion.
    handshake_tx: Option<mpsc::UnboundedSender<DvcHandshake>>,
}

impl AutomationDvc {
    /// Create a new automation DVC processor.
    pub fn new(state: SharedDvcState) -> Self {
        Self {
            state,
            handshake_tx: None,
        }
    }

    /// Create with a handshake notification channel.
    pub fn with_handshake_notify(
        state: SharedDvcState,
        handshake_tx: mpsc::UnboundedSender<DvcHandshake>,
    ) -> Self {
        Self {
            state,
            handshake_tx: Some(handshake_tx),
        }
    }

    /// Decode a JSON message from the buffer (DVC handles message framing).
    fn decode_message(payload: &[u8]) -> Result<DvcProtocolMessage, String> {
        if payload.is_empty() {
            return Err("Empty payload".to_string());
        }

        // Handle potential UTF-8 BOM
        let json_str = std::str::from_utf8(payload)
            .map_err(|e| format!("Invalid UTF-8: {}", e))?;
        let json_str = json_str.strip_prefix('\u{feff}').unwrap_or(json_str);

        serde_json::from_str(json_str)
            .map_err(|e| format!("JSON parse error: {}", e))
    }

    /// Encode a message as JSON (used by tests).
    #[cfg(test)]
    fn encode_message(msg: &DvcProtocolMessage) -> Result<Vec<u8>, String> {
        serde_json::to_string(msg)
            .map(|s| s.into_bytes())
            .map_err(|e| format!("JSON encode error: {}", e))
    }
}

impl_as_any!(AutomationDvc);

impl DvcProcessor for AutomationDvc {
    fn channel_name(&self) -> &str {
        CHANNEL_NAME
    }

    fn start(&mut self, channel_id: u32) -> PduResult<Vec<DvcMessage>> {
        debug!("AutomationDvc channel started with ID {}", channel_id);

        {
            let mut state = self.state.lock();
            state.channel_id = Some(channel_id);
        }

        // No initial messages from client side - we wait for PowerShell to send handshake
        Ok(Vec::new())
    }

    fn process(&mut self, channel_id: u32, payload: &[u8]) -> PduResult<Vec<DvcMessage>> {
        trace!(
            "AutomationDvc received {} bytes on channel {}",
            payload.len(),
            channel_id
        );

        // Decode the incoming message
        let msg = match Self::decode_message(payload) {
            Ok(msg) => msg,
            Err(e) => {
                error!("Failed to decode DVC message: {}", e);
                return Ok(Vec::new());
            }
        };

        match msg {
            DvcProtocolMessage::Handshake {
                version,
                agent_pid,
                capabilities,
            } => {
                debug!(
                    "Received DVC handshake: version={}, pid={}, caps={:?}",
                    version, agent_pid, capabilities
                );

                let handshake = DvcHandshake {
                    version,
                    agent_pid,
                    capabilities,
                };

                // Store handshake and notify
                {
                    let mut state = self.state.lock();
                    state.handshake = Some(handshake.clone());
                }

                if let Some(ref tx) = self.handshake_tx {
                    let _ = tx.send(handshake);
                }
            }

            DvcProtocolMessage::Response {
                id,
                success,
                data,
                error,
            } => {
                debug!("Received DVC response for request {}: success={}", id, success);

                // Route to pending request
                let sender = {
                    let mut state = self.state.lock();
                    state.pending.remove(&id)
                };

                if let Some(sender) = sender {
                    let response = DvcResponse {
                        success,
                        data,
                        error,
                    };
                    let _ = sender.send(response);
                } else {
                    warn!("Received response for unknown request ID: {}", id);
                }
            }

            DvcProtocolMessage::Request { .. } => {
                // Unexpected - requests should only go from Rust to PowerShell
                warn!("Received unexpected request message from PowerShell");
            }

            DvcProtocolMessage::Poll => {
                // Poll message - no longer needed since we send proactively
                // Just acknowledge receipt
                trace!("Received poll from PowerShell (ignored - using proactive send)");
            }
        }

        // We now send data proactively through the command channel, so no queued messages
        Ok(Vec::new())
    }

    fn close(&mut self, channel_id: u32) {
        debug!("AutomationDvc channel {} closed", channel_id);

        let mut state = self.state.lock();
        state.channel_id = None;
        state.handshake = None;

        // Notify all pending requests that the channel closed
        for (id, sender) in state.pending.drain() {
            warn!("Channel closed, failing pending request {}", id);
            let _ = sender.send(DvcResponse {
                success: false,
                data: None,
                error: Some(DvcError {
                    code: "channel_closed".to_string(),
                    message: "DVC channel was closed".to_string(),
                }),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_handshake() {
        let msg = DvcProtocolMessage::Handshake {
            version: "1.0.0".to_string(),
            agent_pid: 1234,
            capabilities: vec!["snapshot".to_string(), "click".to_string()],
        };

        let encoded = AutomationDvc::encode_message(&msg).unwrap();
        let decoded = AutomationDvc::decode_message(&encoded).unwrap();

        match decoded {
            DvcProtocolMessage::Handshake {
                version,
                agent_pid,
                capabilities,
            } => {
                assert_eq!(version, "1.0.0");
                assert_eq!(agent_pid, 1234);
                assert_eq!(capabilities.len(), 2);
            }
            _ => panic!("Expected handshake"),
        }
    }

    #[test]
    fn test_encode_decode_request() {
        let msg = DvcProtocolMessage::Request {
            id: "abc123".to_string(),
            command: "snapshot".to_string(),
            params: serde_json::json!({"interactive_only": true}),
        };

        let encoded = AutomationDvc::encode_message(&msg).unwrap();
        let decoded = AutomationDvc::decode_message(&encoded).unwrap();

        match decoded {
            DvcProtocolMessage::Request { id, command, params } => {
                assert_eq!(id, "abc123");
                assert_eq!(command, "snapshot");
                assert_eq!(params["interactive_only"], true);
            }
            _ => panic!("Expected request"),
        }
    }

    #[test]
    fn test_encode_decode_response() {
        let msg = DvcProtocolMessage::Response {
            id: "abc123".to_string(),
            success: true,
            data: Some(serde_json::json!({"result": "ok"})),
            error: None,
        };

        let encoded = AutomationDvc::encode_message(&msg).unwrap();
        let decoded = AutomationDvc::decode_message(&encoded).unwrap();

        match decoded {
            DvcProtocolMessage::Response {
                id,
                success,
                data,
                error,
            } => {
                assert_eq!(id, "abc123");
                assert!(success);
                assert!(data.is_some());
                assert!(error.is_none());
            }
            _ => panic!("Expected response"),
        }
    }
}
