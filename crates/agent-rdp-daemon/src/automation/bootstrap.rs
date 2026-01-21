//! Bootstrap automation agent on remote Windows machine.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use agent_rdp_protocol::DriveMapping;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{debug, info, warn};

use super::{AutomationState, FileIpc};
use crate::rdp_session::RdpSession;

/// Embedded PowerShell agent script.
const AGENT_SCRIPT: &str = include_str!("scripts/agent.ps1");

/// Automation bootstrap handler.
pub struct AutomationBootstrap {
    /// Session directory path (kept for potential future use).
    _session_dir: PathBuf,
}

impl AutomationBootstrap {
    /// Create a new automation bootstrap handler.
    pub fn new(session_dir: PathBuf) -> Self {
        Self { _session_dir: session_dir }
    }

    /// Initialize automation directory and write the agent script.
    pub async fn initialize(&self, state: &mut AutomationState) -> anyhow::Result<()> {
        info!("Initializing automation for session");

        // Create automation directory structure
        let automation_dir = &state.automation_dir;
        tokio::fs::create_dir_all(automation_dir).await?;
        tokio::fs::create_dir_all(automation_dir.join("scripts")).await?;
        tokio::fs::create_dir_all(automation_dir.join("requests")).await?;
        tokio::fs::create_dir_all(automation_dir.join("responses")).await?;

        // Write the PowerShell agent script
        let script_path = state.script_path();
        tokio::fs::write(&script_path, AGENT_SCRIPT).await?;
        debug!("Wrote automation agent script to {:?}", script_path);

        // Initialize file IPC
        let ipc = FileIpc::new(automation_dir.clone());
        ipc.initialize().await?;
        state.ipc = Some(ipc);

        state.enabled = true;
        info!(
            "Automation initialized with ID {} at {:?}",
            state.automation_id, automation_dir
        );

        Ok(())
    }

    /// Get the drive mapping for the automation directory.
    pub fn get_drive_mapping(&self, state: &AutomationState) -> DriveMapping {
        DriveMapping {
            path: state.automation_dir.to_string_lossy().to_string(),
            name: state.drive_name.clone(),
        }
    }

    /// Launch the automation agent on the remote Windows machine via Win+R.
    pub async fn launch_agent(
        &self,
        rdp: &RdpSession,
        state: &AutomationState,
    ) -> anyhow::Result<()> {
        info!("Launching automation agent on remote Windows machine");

        // Wait for desktop to stabilize after RDP connection
        debug!("Waiting for remote desktop to stabilize...");
        sleep(Duration::from_secs(2)).await;

        // The command to run via Win+R
        // Uses the mapped drive path: \\TSCLIENT\<drive_name>\scripts\agent.ps1
        let ps_command = format!(
            "powershell -ExecutionPolicy Bypass -WindowStyle Hidden -File \"\\\\TSCLIENT\\{}\\scripts\\agent.ps1\" -BasePath \"\\\\TSCLIENT\\{}\"",
            state.drive_name,
            state.drive_name
        );

        debug!("PowerShell command: {}", ps_command);

        // Press Win+R to open Run dialog
        rdp.send_key_press("super+r").await?;
        sleep(Duration::from_millis(500)).await;

        // Type the PowerShell command
        rdp.send_text(&ps_command).await?;
        sleep(Duration::from_millis(200)).await;

        // Press Enter to execute
        rdp.send_key_press("return").await?;

        info!("Automation agent launch command sent");
        Ok(())
    }

    /// Wait for the agent to complete handshake.
    pub async fn wait_for_agent(
        &self,
        state: &mut AutomationState,
        max_attempts: u32,
    ) -> anyhow::Result<()> {
        let ipc = state
            .ipc
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("IPC not initialized"))?;

        let handshake = ipc.wait_for_handshake(max_attempts).await?;

        state.agent_ready = true;
        state.agent_pid = Some(handshake.agent_pid);

        info!(
            "Automation agent ready: PID={}, version={}, capabilities={:?}",
            handshake.agent_pid, handshake.version, handshake.capabilities
        );

        Ok(())
    }

    /// Full bootstrap sequence: initialize, launch, and verify handshake.
    pub async fn bootstrap(
        &self,
        rdp: &RdpSession,
        state: Arc<Mutex<AutomationState>>,
    ) -> anyhow::Result<()> {
        // Initialize
        {
            let mut state = state.lock().await;
            self.initialize(&mut state).await?;
        }

        // Launch agent
        {
            let state = state.lock().await;
            self.launch_agent(rdp, &state).await?;
        }

        // Wait for handshake
        {
            let mut state = state.lock().await;
            self.wait_for_agent(&mut state, 10).await?;
        }

        Ok(())
    }

    /// Clean up automation resources.
    pub async fn cleanup(&self, state: &mut AutomationState) -> anyhow::Result<()> {
        if !state.enabled {
            return Ok(());
        }

        info!("Cleaning up automation resources");

        // Clean up IPC files
        if let Some(ref ipc) = state.ipc {
            if let Err(e) = ipc.cleanup().await {
                warn!("Failed to cleanup IPC files: {}", e);
            }
        }

        // Remove automation directory
        let automation_dir = &state.automation_dir;
        if automation_dir.exists() {
            if let Err(e) = tokio::fs::remove_dir_all(automation_dir).await {
                warn!("Failed to remove automation directory: {}", e);
            } else {
                debug!("Removed automation directory: {:?}", automation_dir);
            }
        }

        state.enabled = false;
        state.agent_ready = false;
        state.agent_pid = None;
        state.ipc = None;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_initialize_creates_structure() {
        let temp_dir = TempDir::new().unwrap();
        let bootstrap = AutomationBootstrap::new(temp_dir.path().to_path_buf());
        let mut state = AutomationState::new(temp_dir.path().to_path_buf());

        bootstrap.initialize(&mut state).await.unwrap();

        assert!(state.enabled);
        assert!(state.automation_dir.exists());
        assert!(state.script_path().exists());
        assert!(state.requests_dir().exists());
        assert!(state.responses_dir().exists());
    }

    #[test]
    fn test_get_drive_mapping() {
        let temp_dir = TempDir::new().unwrap();
        let bootstrap = AutomationBootstrap::new(temp_dir.path().to_path_buf());
        let state = AutomationState::new(temp_dir.path().to_path_buf());

        let mapping = bootstrap.get_drive_mapping(&state);

        assert_eq!(mapping.name, "agent-automation");
        assert!(mapping.path.contains("automation-"));
    }
}
