//! Windows UI Automation module.
//!
//! This module provides DVC-based IPC communication with a PowerShell agent
//! running on the remote Windows machine for UI automation via the Windows
//! UI Automation API.

mod bootstrap;
pub mod dvc_channel;
mod dvc_ipc;

pub use bootstrap::AutomationBootstrap;
pub use dvc_channel::{
    new_shared_dvc_state, AutomationDvc, DvcCommandReceiver, DvcCommandSender, DvcHandshake,
    DvcSendCommand, SharedDvcState, CHANNEL_NAME,
};
pub use dvc_ipc::DvcIpc;

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Automation state that persists across requests.
#[derive(Debug)]
pub struct AutomationState {
    /// Whether automation is enabled for this session.
    pub enabled: bool,
    /// Unique ID for this automation session (different from RDP session ID).
    pub automation_id: String,
    /// Path to the automation directory on the host side (for RDPDR bootstrap).
    pub automation_dir: PathBuf,
    /// Drive name mapped via RDPDR (still needed for bootstrap).
    pub drive_name: String,
    /// DVC-based IPC client.
    pub dvc_ipc: Option<DvcIpc>,
    /// Shared DVC state (for processor access).
    pub dvc_state: Option<SharedDvcState>,
    /// Whether the agent has completed handshake.
    pub agent_ready: bool,
    /// Agent process ID (if known).
    pub agent_pid: Option<u32>,
}

impl AutomationState {
    /// Create a new automation state.
    pub fn new(session_dir: PathBuf) -> Self {
        let automation_id = Uuid::new_v4().to_string()[..8].to_string();
        let automation_dir = session_dir.join(format!("automation-{}", automation_id));

        Self {
            enabled: false,
            automation_id,
            automation_dir,
            drive_name: "agent-automation".to_string(),
            dvc_ipc: None,
            dvc_state: None,
            agent_ready: false,
            agent_pid: None,
        }
    }

    /// Get the path where the PowerShell script should be written.
    pub fn script_path(&self) -> PathBuf {
        self.automation_dir.join("scripts").join("agent.ps1")
    }

    /// Check if DVC IPC is ready.
    pub fn is_dvc_ready(&self) -> bool {
        self.dvc_ipc.as_ref().map(|ipc| ipc.is_ready()).unwrap_or(false)
    }
}

/// Thread-safe automation state handle.
pub type SharedAutomationState = Arc<Mutex<AutomationState>>;

/// Create a new shared automation state.
pub fn new_shared_state(session_dir: PathBuf) -> SharedAutomationState {
    Arc::new(Mutex::new(AutomationState::new(session_dir)))
}
