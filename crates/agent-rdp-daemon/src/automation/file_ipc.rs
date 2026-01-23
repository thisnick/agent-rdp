//! File-based IPC for communication with PowerShell automation agent.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use agent_rdp_protocol::{
    AutomateRequest, AutomationHandshake, FileIpcError, FileIpcRequest, FileIpcResponse,
};
use tokio::fs;
use tokio::time::sleep;
use tracing::{debug, error, trace};
use uuid::Uuid;

/// Number of consecutive failures before suggesting reconnection.
const CONSECUTIVE_FAILURE_THRESHOLD: u32 = 3;

/// File-based IPC client for communicating with the PowerShell agent.
#[derive(Debug)]
pub struct FileIpc {
    /// Base path for IPC files (the automation directory).
    base_path: PathBuf,
    /// Request directory.
    requests_dir: PathBuf,
    /// Response directory.
    responses_dir: PathBuf,
    /// Timeout for waiting on responses.
    timeout: Duration,
    /// Count of consecutive failures (for detecting dead channel).
    consecutive_failures: AtomicU32,
}

impl FileIpc {
    /// Create a new file IPC client.
    pub fn new(base_path: PathBuf) -> Self {
        let requests_dir = base_path.join("requests");
        let responses_dir = base_path.join("responses");

        Self {
            base_path,
            requests_dir,
            responses_dir,
            timeout: Duration::from_secs(30),
            consecutive_failures: AtomicU32::new(0),
        }
    }

    /// Get the number of consecutive failures.
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures.load(Ordering::Relaxed)
    }

    /// Reset the failure counter (call after successful request).
    fn reset_failures(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
    }

    /// Increment the failure counter and return the new count.
    fn increment_failures(&self) -> u32 {
        self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Initialize the IPC directories.
    pub async fn initialize(&self) -> anyhow::Result<()> {
        fs::create_dir_all(&self.requests_dir).await?;
        fs::create_dir_all(&self.responses_dir).await?;
        Ok(())
    }

    /// Set the timeout for responses.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Check if the agent handshake file exists and is valid.
    pub async fn check_handshake(&self) -> anyhow::Result<Option<AutomationHandshake>> {
        let handshake_path = self.base_path.join("handshake.json");

        if !handshake_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&handshake_path).await?;
        // Strip UTF-8 BOM if present (PowerShell on Windows adds this)
        let content = content.strip_prefix('\u{feff}').unwrap_or(&content);
        let handshake: AutomationHandshake = serde_json::from_str(content)?;

        if handshake.ready {
            Ok(Some(handshake))
        } else {
            Ok(None)
        }
    }

    /// Wait for the agent handshake with exponential backoff.
    pub async fn wait_for_handshake(&self, max_attempts: u32) -> anyhow::Result<AutomationHandshake> {
        let mut delay = Duration::from_millis(500);
        let max_delay = Duration::from_secs(5);

        for attempt in 1..=max_attempts {
            debug!("Checking for automation agent handshake (attempt {}/{})", attempt, max_attempts);

            if let Some(handshake) = self.check_handshake().await? {
                debug!("Automation agent handshake successful: v{}", handshake.version);
                return Ok(handshake);
            }

            if attempt < max_attempts {
                trace!("Waiting {:?} before next handshake check", delay);
                sleep(delay).await;
                delay = (delay * 3 / 2).min(max_delay); // Exponential backoff (1.5x)
            }
        }

        anyhow::bail!("Automation agent handshake timed out after {} attempts", max_attempts)
    }

    /// Send a request to the PowerShell agent and wait for response.
    pub async fn send_request(&self, request: &AutomateRequest) -> anyhow::Result<serde_json::Value> {
        let request_id = Uuid::new_v4().to_string()[..8].to_string();

        // Convert AutomateRequest to command name and params
        let (command, params) = self.serialize_request(request)?;

        let ipc_request = FileIpcRequest {
            id: request_id.clone(),
            command,
            params,
        };

        // Write request atomically: write to .tmp first, then rename
        // This prevents PowerShell from reading a partially-written file
        let request_path = self.requests_dir.join(format!("req_{}.json", request_id));
        let tmp_path = self.requests_dir.join(format!("req_{}.tmp", request_id));

        let json = serde_json::to_string_pretty(&ipc_request)?;
        debug!("Writing request to: {:?} (via temp file)", request_path);
        fs::write(&tmp_path, &json).await?;
        fs::rename(&tmp_path, &request_path).await?;

        debug!("Sent automation request {} ({}) to {:?}", request_id, ipc_request.command, request_path);

        // Wait for response
        // NOTE: PowerShell handles all file cleanup to avoid RDPDR race conditions
        let response = match self.wait_for_response(&request_id).await {
            Ok(resp) => {
                // Success - reset failure counter
                self.reset_failures();
                resp
            }
            Err(e) => {
                // Failure - increment counter and add context
                let failures = self.increment_failures();
                if failures >= CONSECUTIVE_FAILURE_THRESHOLD {
                    error!(
                        "Automation request failed {} consecutive times. \
                        The automation channel may be dead. Consider reconnecting with --enable-win-automation.",
                        failures
                    );
                    anyhow::bail!(
                        "Automation failed ({} consecutive failures). \
                        The RDP automation channel appears to be dead. \
                        Please disconnect and reconnect with --enable-win-automation to restore automation.",
                        failures
                    );
                }
                return Err(e);
            }
        };

        if response.success {
            Ok(response.data.unwrap_or(serde_json::Value::Null))
        } else {
            let error = response.error.unwrap_or(FileIpcError {
                code: "unknown".to_string(),
                message: "Unknown error".to_string(),
            });
            anyhow::bail!("{}: {}", error.code, error.message)
        }
    }

    /// Serialize an AutomateRequest to command name and parameters.
    fn serialize_request(&self, request: &AutomateRequest) -> anyhow::Result<(String, serde_json::Value)> {
        // Serialize the request to get the command tag and data
        let json = serde_json::to_value(request)?;

        // The command is in the "op" field (serde tag), rest are params
        if let serde_json::Value::Object(mut obj) = json {
            let command = obj.remove("op")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or_else(|| anyhow::anyhow!("Request missing op field"))?;

            Ok((command, serde_json::Value::Object(obj)))
        } else {
            anyhow::bail!("Failed to serialize request")
        }
    }

    /// Wait for a response file to appear.
    async fn wait_for_response(&self, request_id: &str) -> anyhow::Result<FileIpcResponse> {
        let response_path = self.responses_dir.join(format!("res_{}.json", request_id));
        let poll_interval = Duration::from_millis(50);
        let start = std::time::Instant::now();

        loop {
            if response_path.exists() {
                // Read and parse response - retry on parse errors (file may still be writing)
                let content = fs::read_to_string(&response_path).await?;
                // Strip UTF-8 BOM if present (PowerShell on Windows adds this)
                let content = content.strip_prefix('\u{feff}').unwrap_or(&content);

                match serde_json::from_str::<FileIpcResponse>(content) {
                    Ok(response) => {
                        // Don't delete files from Rust side - let PowerShell handle cleanup
                        // to avoid race conditions with RDPDR
                        trace!("Received automation response {}", request_id);
                        return Ok(response);
                    }
                    Err(e) => {
                        // File may still be writing - wait and retry
                        trace!("JSON parse error, retrying: {}", e);
                        sleep(Duration::from_millis(100)).await;

                        // Check timeout before retrying
                        if start.elapsed() > self.timeout {
                            error!("Timeout waiting for automation response {} (last error: {})", request_id, e);
                            anyhow::bail!("Timeout waiting for automation agent response");
                        }
                        continue;
                    }
                }
            }

            if start.elapsed() > self.timeout {
                error!("Timeout waiting for automation response {}", request_id);
                anyhow::bail!("Timeout waiting for automation agent response");
            }

            sleep(poll_interval).await;
        }
    }

    /// Clean up IPC state.
    ///
    /// Note: We don't delete files from the Rust side to avoid RDPDR race conditions.
    /// The PowerShell agent handles file cleanup, and the session temp directory
    /// is removed when the session ends.
    pub async fn cleanup(&self) -> anyhow::Result<()> {
        // Reset failure counter
        self.reset_failures();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_serialize_snapshot_request() {
        let ipc = FileIpc::new(PathBuf::from("/tmp/test"));

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
        let ipc = FileIpc::new(PathBuf::from("/tmp/test"));

        let request = AutomateRequest::Click {
            selector: "@5".to_string(),
            double_click: false,
        };

        let (command, params) = ipc.serialize_request(&request).unwrap();
        assert_eq!(command, "click");
        assert_eq!(params["selector"], "@5");
    }

    #[test]
    fn test_serialize_toggle_request() {
        let ipc = FileIpc::new(PathBuf::from("/tmp/test"));

        let request = AutomateRequest::Toggle {
            selector: "@5".to_string(),
            state: Some(true),
        };

        let (command, params) = ipc.serialize_request(&request).unwrap();
        assert_eq!(command, "toggle");
        assert_eq!(params["selector"], "@5");
        assert_eq!(params["state"], true);
    }

    #[tokio::test]
    async fn test_initialize_creates_directories() {
        let temp_dir = TempDir::new().unwrap();
        let ipc = FileIpc::new(temp_dir.path().to_path_buf());

        ipc.initialize().await.unwrap();

        assert!(temp_dir.path().join("requests").exists());
        assert!(temp_dir.path().join("responses").exists());
    }
}
