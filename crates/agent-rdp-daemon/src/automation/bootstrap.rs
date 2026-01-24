//! Bootstrap automation agent on remote Windows machine.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use agent_rdp_protocol::DriveMapping;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{debug, info, warn};

use super::{new_shared_dvc_state, AutomationState, DvcIpc};
use crate::rdp_session::RdpSession;

/// Embedded PowerShell agent script (main entry point).
const AGENT_SCRIPT: &str = include_str!("scripts/agent.ps1");

/// Embedded PowerShell library files.
const LIB_TYPES: &str = include_str!("scripts/lib/types.ps1");
const LIB_SNAPSHOT: &str = include_str!("scripts/lib/snapshot.ps1");
const LIB_SELECTORS: &str = include_str!("scripts/lib/selectors.ps1");
const LIB_ACTIONS: &str = include_str!("scripts/lib/actions.ps1");
const LIB_DVC: &str = include_str!("scripts/lib/dvc.ps1");

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
    ///
    /// This creates the directory structure and scripts needed for RDPDR-based
    /// bootstrap. The actual IPC will be over DVC once the agent starts.
    pub async fn initialize(&self, state: &mut AutomationState) -> anyhow::Result<()> {
        info!("Initializing automation for session");

        // Create automation directory structure
        let automation_dir = &state.automation_dir;
        tokio::fs::create_dir_all(automation_dir).await?;
        tokio::fs::create_dir_all(automation_dir.join("scripts")).await?;
        tokio::fs::create_dir_all(automation_dir.join("scripts/lib")).await?;

        // Write the PowerShell agent script (main entry point)
        let script_path = state.script_path();
        tokio::fs::write(&script_path, AGENT_SCRIPT).await?;
        debug!("Wrote automation agent script to {:?}", script_path);

        // Write the PowerShell library files
        let lib_dir = automation_dir.join("scripts/lib");
        tokio::fs::write(lib_dir.join("types.ps1"), LIB_TYPES).await?;
        tokio::fs::write(lib_dir.join("snapshot.ps1"), LIB_SNAPSHOT).await?;
        tokio::fs::write(lib_dir.join("selectors.ps1"), LIB_SELECTORS).await?;
        tokio::fs::write(lib_dir.join("actions.ps1"), LIB_ACTIONS).await?;
        tokio::fs::write(lib_dir.join("dvc.ps1"), LIB_DVC).await?;
        debug!("Wrote automation library files to {:?}", lib_dir);

        // Initialize DVC state and IPC
        let dvc_state = new_shared_dvc_state();
        let dvc_ipc = DvcIpc::new(dvc_state.clone());
        state.dvc_state = Some(dvc_state);
        state.dvc_ipc = Some(dvc_ipc);

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

    /// Wait for the agent to complete DVC handshake.
    pub async fn wait_for_agent(
        &self,
        state: &mut AutomationState,
        max_attempts: u32,
    ) -> anyhow::Result<()> {
        let dvc_ipc = state
            .dvc_ipc
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("DVC IPC not initialized"))?;

        let mut delay = Duration::from_millis(500);
        let max_delay = Duration::from_secs(5);

        for attempt in 1..=max_attempts {
            debug!(
                "Checking for automation agent DVC handshake (attempt {}/{})",
                attempt, max_attempts
            );

            if dvc_ipc.is_ready() {
                let version = dvc_ipc.agent_version().unwrap_or_default();
                let pid = dvc_ipc.agent_pid().unwrap_or(0);
                let caps = dvc_ipc.capabilities();

                state.agent_ready = true;
                state.agent_pid = Some(pid);

                info!(
                    "Automation agent ready via DVC: PID={}, version={}, capabilities={:?}",
                    pid, version, caps
                );

                return Ok(());
            }

            if attempt < max_attempts {
                debug!("Waiting {:?} before next handshake check", delay);
                sleep(delay).await;
                delay = (delay * 3 / 2).min(max_delay); // Exponential backoff (1.5x)
            }
        }

        anyhow::bail!(
            "Automation agent DVC handshake timed out after {} attempts",
            max_attempts
        )
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
        state.dvc_ipc = None;
        state.dvc_state = None;

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

        // Verify library files are created
        let lib_dir = state.automation_dir.join("scripts/lib");
        assert!(lib_dir.join("types.ps1").exists());
        assert!(lib_dir.join("snapshot.ps1").exists());
        assert!(lib_dir.join("selectors.ps1").exists());
        assert!(lib_dir.join("actions.ps1").exists());
        assert!(lib_dir.join("dvc.ps1").exists());

        // Verify DVC IPC is initialized
        assert!(state.dvc_ipc.is_some());
        assert!(state.dvc_state.is_some());
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
