//! DVC-based IPC for automation communication.
//!
//! Provides an async interface for sending requests to the PowerShell agent
//! over the DVC channel and receiving responses.

use std::sync::Arc;
use std::time::Duration;

use agent_rdp_protocol::AutomateRequest;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tracing::{debug, error, trace};
use uuid::Uuid;

use super::dvc_channel::{DvcError, DvcProtocolMessage, DvcSendCommand, SharedDvcState};

/// Number of consecutive failures before suggesting reconnection.
const CONSECUTIVE_FAILURE_THRESHOLD: u32 = 3;

/// DVC-based IPC client for communicating with the PowerShell agent.
#[derive(Debug, Clone)]
pub struct DvcIpc {
    /// Shared state with the DVC processor.
    state: SharedDvcState,
    /// Timeout for waiting on responses.
    timeout: Duration,
    /// Count of consecutive failures (for detecting dead channel).
    consecutive_failures: Arc<std::sync::atomic::AtomicU32>,
}

impl DvcIpc {
    /// Create a new DVC IPC client.
    pub fn new(state: SharedDvcState) -> Self {
        Self {
            state,
            timeout: Duration::from_secs(10),
            consecutive_failures: Arc::new(std::sync::atomic::AtomicU32::new(0)),
        }
    }

    /// Set the timeout for responses.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Get the number of consecutive failures.
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Reset the failure counter.
    fn reset_failures(&self) {
        self.consecutive_failures
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }

    /// Increment the failure counter and return the new count.
    fn increment_failures(&self) -> u32 {
        self.consecutive_failures
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1
    }

    /// Check if the DVC channel is ready (handshake received).
    pub fn is_ready(&self) -> bool {
        let state = self.state.lock();
        state.handshake.is_some() && state.channel_id.is_some()
    }

    /// Get the agent version from the handshake.
    pub fn agent_version(&self) -> Option<String> {
        let state = self.state.lock();
        state.handshake.as_ref().map(|h| h.version.clone())
    }

    /// Get the agent PID from the handshake.
    pub fn agent_pid(&self) -> Option<u32> {
        let state = self.state.lock();
        state.handshake.as_ref().map(|h| h.agent_pid)
    }

    /// Get the agent capabilities from the handshake.
    pub fn capabilities(&self) -> Vec<String> {
        let state = self.state.lock();
        state
            .handshake
            .as_ref()
            .map(|h| h.capabilities.clone())
            .unwrap_or_default()
    }

    /// Send a request to the PowerShell agent and wait for response.
    pub async fn send_request(&self, request: &AutomateRequest) -> anyhow::Result<serde_json::Value> {
        let request_id = Uuid::new_v4().to_string()[..8].to_string();

        // Convert AutomateRequest to command name and params
        let (command, params) = self.serialize_request(request)?;

        debug!(
            "Sending DVC request {}: command={}",
            request_id, command
        );

        // Create the protocol message
        let msg = DvcProtocolMessage::Request {
            id: request_id.clone(),
            command,
            params,
        };

        // Encode the message
        let encoded = Self::encode_message(&msg)?;

        // Create oneshot channel for response
        let (tx, rx) = oneshot::channel();

        // Register pending request and send via command channel
        let channel_id = {
            let mut state = self.state.lock();

            // Check if channel is open
            let channel_id = state
                .channel_id
                .ok_or_else(|| anyhow::anyhow!("DVC channel not open"))?;

            // Check if command sender is available
            let command_tx = state
                .command_tx
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("DVC command sender not configured"))?;

            // Send the data through the RDP session
            command_tx
                .send(DvcSendCommand {
                    channel_id,
                    data: encoded,
                })
                .map_err(|_| anyhow::anyhow!("Failed to send DVC command"))?;

            state.pending.insert(request_id.clone(), tx);
            channel_id
        };

        debug!("Sent DVC request on channel {}", channel_id);

        // Wait for response with timeout
        let response = match timeout(self.timeout, rx).await {
            Ok(Ok(response)) => {
                self.reset_failures();
                response
            }
            Ok(Err(_)) => {
                // Channel closed
                let failures = self.increment_failures();
                if failures >= CONSECUTIVE_FAILURE_THRESHOLD {
                    error!(
                        "DVC request failed {} consecutive times. Channel may be dead.",
                        failures
                    );
                    anyhow::bail!(
                        "DVC channel appears to be dead ({} consecutive failures). \
                        Please reconnect with --enable-win-automation.",
                        failures
                    );
                }
                anyhow::bail!("Response channel closed unexpectedly");
            }
            Err(_) => {
                // Timeout - remove pending request
                {
                    let mut state = self.state.lock();
                    state.pending.remove(&request_id);
                }
                let failures = self.increment_failures();
                if failures >= CONSECUTIVE_FAILURE_THRESHOLD {
                    error!(
                        "DVC request timed out {} consecutive times. Channel may be dead.",
                        failures
                    );
                    anyhow::bail!(
                        "DVC channel appears to be dead ({} consecutive failures). \
                        Please reconnect with --enable-win-automation.",
                        failures
                    );
                }
                anyhow::bail!("Timeout waiting for DVC response");
            }
        };

        trace!("Received DVC response: success={}", response.success);

        if response.success {
            Ok(response.data.unwrap_or(serde_json::Value::Null))
        } else {
            let error = response.error.unwrap_or(DvcError {
                code: "unknown".to_string(),
                message: "Unknown error".to_string(),
            });
            anyhow::bail!("{}: {}", error.code, error.message)
        }
    }

    /// Serialize an AutomateRequest to command name and parameters.
    fn serialize_request(
        &self,
        request: &AutomateRequest,
    ) -> anyhow::Result<(String, serde_json::Value)> {
        // Serialize the request to get the command tag and data
        let json = serde_json::to_value(request)?;

        // The command is in the "op" field (serde tag), rest are params
        if let serde_json::Value::Object(mut obj) = json {
            let command = obj
                .remove("op")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or_else(|| anyhow::anyhow!("Request missing op field"))?;

            Ok((command, serde_json::Value::Object(obj)))
        } else {
            anyhow::bail!("Failed to serialize request")
        }
    }

    /// Encode a message as JSON (DVC handles message framing).
    fn encode_message(msg: &DvcProtocolMessage) -> anyhow::Result<Vec<u8>> {
        let json = serde_json::to_string(msg)?;
        Ok(json.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::dvc_channel::new_shared_dvc_state;

    #[test]
    fn test_serialize_snapshot_request() {
        let state = new_shared_dvc_state();
        let ipc = DvcIpc::new(state);

        let request = AutomateRequest::Snapshot {
            interactive_only: true,
            compact: false,
            max_depth: 10,
            selector: None,
            focused: false,
        };

        let (command, params) = ipc.serialize_request(&request).unwrap();
        assert_eq!(command, "snapshot");
        assert_eq!(params["interactive_only"], true);
        assert_eq!(params["max_depth"], 10);
    }

    #[test]
    fn test_serialize_click_request() {
        let state = new_shared_dvc_state();
        let ipc = DvcIpc::new(state);

        let request = AutomateRequest::Click {
            selector: "@5".to_string(),
            double_click: false,
        };

        let (command, params) = ipc.serialize_request(&request).unwrap();
        assert_eq!(command, "click");
        assert_eq!(params["selector"], "@5");
    }

    #[test]
    fn test_is_ready_false_initially() {
        let state = new_shared_dvc_state();
        let ipc = DvcIpc::new(state);
        assert!(!ipc.is_ready());
    }
}
