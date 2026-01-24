//! Automation types for Windows UI Automation via file-based IPC.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Automation request sent from CLI to daemon.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum AutomateRequest {
    /// Take a snapshot of the accessibility tree.
    Snapshot {
        /// Filter to interactive elements only (buttons, inputs, links).
        #[serde(default)]
        interactive_only: bool,
        /// Compact mode - remove empty structural elements.
        #[serde(default)]
        compact: bool,
        /// Maximum tree depth to traverse.
        #[serde(default = "default_max_depth")]
        max_depth: u32,
        /// Scope to a specific element (window, panel, etc.) via selector.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        selector: Option<String>,
        /// Start from the currently focused element.
        #[serde(default)]
        focused: bool,
    },

    /// Get element properties.
    Get {
        /// Element selector.
        selector: String,
        /// Property to retrieve (name, value, states, bounds, or all).
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        property: Option<String>,
    },

    /// Set focus to an element.
    Focus {
        /// Element selector.
        selector: String,
    },

    /// Click an element - for buttons, links, menu items.
    Click {
        /// Element selector.
        selector: String,
        /// Use double-click instead of single click.
        #[serde(default)]
        double_click: bool,
    },

    /// Select an element (SelectionItemPattern) - for list items, radio buttons.
    /// Can also select by item name within a container.
    Select {
        /// Element selector (container or item directly).
        selector: String,
        /// Item name to select within container (optional).
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        item: Option<String>,
    },

    /// Toggle an element (TogglePattern) - for checkboxes.
    Toggle {
        /// Element selector.
        selector: String,
        /// Target state: true=on, false=off, None=toggle.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        state: Option<bool>,
    },

    /// Expand an element (ExpandCollapsePattern) - for menus, tree items, combo boxes.
    Expand {
        /// Element selector.
        selector: String,
    },

    /// Collapse an element (ExpandCollapsePattern).
    Collapse {
        /// Element selector.
        selector: String,
    },

    /// Open context menu for an element (Focus + Shift+F10).
    ContextMenu {
        /// Element selector.
        selector: String,
    },

    /// Clear and fill text in an element.
    Fill {
        /// Element selector.
        selector: String,
        /// Text to fill.
        text: String,
    },

    /// Clear text from an element.
    Clear {
        /// Element selector.
        selector: String,
    },

    /// Scroll an element.
    Scroll {
        /// Element selector.
        selector: String,
        /// Scroll direction.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        direction: Option<AutomationScrollDirection>,
        /// Scroll amount.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        amount: Option<i32>,
        /// Child element to scroll into view.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        to_child: Option<String>,
    },

    /// Window operations.
    Window {
        /// Window action to perform.
        action: WindowAction,
        /// Window selector (optional, uses foreground window if not specified).
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        selector: Option<String>,
    },

    /// Run a PowerShell command.
    Run {
        /// Command to run.
        command: String,
        /// Command arguments.
        #[serde(default)]
        args: Vec<String>,
        /// Wait for command to complete.
        #[serde(default)]
        wait: bool,
        /// Run with hidden window.
        #[serde(default)]
        hidden: bool,
        /// Timeout in milliseconds when waiting.
        #[serde(default = "default_run_timeout")]
        #[ts(type = "number")]
        timeout_ms: u64,
    },

    /// Wait for an element to reach a state.
    WaitFor {
        /// Element selector.
        selector: String,
        /// Timeout in milliseconds.
        #[serde(default = "default_wait_timeout")]
        #[ts(type = "number")]
        timeout_ms: u64,
        /// State to wait for.
        #[serde(default)]
        state: WaitState,
    },

    /// Get automation agent status.
    Status,
}

fn default_max_depth() -> u32 {
    10
}

fn default_wait_timeout() -> u64 {
    30000
}

fn default_run_timeout() -> u64 {
    10000
}

/// Scroll direction for automation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
#[serde(rename_all = "snake_case")]
pub enum AutomationScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Window action for automation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
#[serde(rename_all = "snake_case")]
pub enum WindowAction {
    /// List all windows.
    List,
    /// Focus a window.
    Focus,
    /// Maximize a window.
    Maximize,
    /// Minimize a window.
    Minimize,
    /// Restore a window.
    Restore,
    /// Close a window.
    Close,
}

/// State to wait for in WaitFor command.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
#[serde(rename_all = "snake_case")]
pub enum WaitState {
    /// Element is visible.
    #[default]
    Visible,
    /// Element is enabled.
    Enabled,
    /// Element is gone (no longer exists).
    Gone,
}

/// Accessibility tree snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
pub struct AccessibilitySnapshot {
    /// Unique snapshot ID.
    pub snapshot_id: String,
    /// Total number of elements with refs.
    pub ref_count: u32,
    /// Whether the tree was truncated due to depth limit.
    #[serde(default)]
    pub truncated: bool,
    /// Maximum depth used for this snapshot.
    #[serde(default)]
    pub max_depth: u32,
    /// Root element of the tree.
    pub root: AccessibilityElement,
}

/// An element in the accessibility tree.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
pub struct AccessibilityElement {
    /// Reference number (for @ref selectors).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub r#ref: Option<u32>,
    /// Element role (control type).
    pub role: String,
    /// Element name.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    /// Automation ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub automation_id: Option<String>,
    /// Win32 class name.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub class_name: Option<String>,
    /// Bounding rectangle.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bounds: Option<ElementBounds>,
    /// Element states.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<String>,
    /// Current value (for editable elements).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value: Option<String>,
    /// Supported UI Automation patterns.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patterns: Vec<String>,
    /// Child elements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<AccessibilityElement>,
}

/// Bounding rectangle for an element.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
pub struct ElementBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Element value response.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
pub struct ElementValue {
    /// Element name.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    /// Element value.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value: Option<String>,
    /// Element states.
    #[serde(default)]
    pub states: Vec<String>,
    /// Element bounds.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bounds: Option<ElementBounds>,
}

/// Window information.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
pub struct WindowInfo {
    /// Window title.
    pub title: String,
    /// Process name.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub process_name: Option<String>,
    /// Process ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub process_id: Option<u32>,
    /// Window bounds.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bounds: Option<ElementBounds>,
    /// Whether the window is minimized.
    #[serde(default)]
    pub minimized: bool,
    /// Whether the window is maximized.
    #[serde(default)]
    pub maximized: bool,
}

/// Automation agent status.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
pub struct AutomationStatus {
    /// Whether the automation agent is running.
    pub agent_running: bool,
    /// Agent process ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub agent_pid: Option<u32>,
    /// Supported capabilities.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Agent version.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub version: Option<String>,
}

/// Command run result.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
pub struct RunResult {
    /// Exit code (if waited).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub exit_code: Option<i32>,
    /// Standard output (if waited).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stdout: Option<String>,
    /// Standard error (if waited).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stderr: Option<String>,
    /// Process ID (if not waited).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pid: Option<u32>,
}

/// Click action result.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
pub struct ClickResult {
    /// Whether the click was performed.
    pub clicked: bool,
    /// Method used (click or double_click).
    pub method: String,
    /// X coordinate of click.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub x: Option<i32>,
    /// Y coordinate of click.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub y: Option<i32>,
}

/// Handshake data from PowerShell agent.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
pub struct AutomationHandshake {
    /// Agent version.
    pub version: String,
    /// Agent process ID.
    pub agent_pid: u32,
    /// Start timestamp (optional for backwards compatibility).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub started_at: Option<String>,
    /// Supported capabilities.
    pub capabilities: Vec<String>,
    /// Whether the agent is ready.
    pub ready: bool,
}

/// Request sent to PowerShell agent via file IPC.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
pub struct FileIpcRequest {
    /// Unique request ID.
    pub id: String,
    /// Command to execute.
    pub command: String,
    /// Command parameters.
    #[ts(type = "unknown")]
    pub params: serde_json::Value,
}

/// Response from PowerShell agent via file IPC.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
pub struct FileIpcResponse {
    /// Request ID this responds to.
    pub id: String,
    /// Response timestamp.
    pub timestamp: String,
    /// Whether the command succeeded.
    pub success: bool,
    /// Response data on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "unknown")]
    pub data: Option<serde_json::Value>,
    /// Error details on failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<FileIpcError>,
}

/// Error from PowerShell agent.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
pub struct FileIpcError {
    /// Error code.
    pub code: String,
    /// Error message.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_automate_request_serialization() {
        let req = AutomateRequest::Snapshot {
            interactive_only: true,
            compact: false,
            max_depth: 10,
            selector: None,
            focused: false,
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"op\":\"snapshot\""));
        assert!(json.contains("\"interactive_only\":true"));

        let parsed: AutomateRequest = serde_json::from_str(&json).unwrap();
        match parsed {
            AutomateRequest::Snapshot { interactive_only, .. } => {
                assert!(interactive_only);
            }
            _ => panic!("unexpected request type"),
        }
    }

    #[test]
    fn test_click_request_serialization() {
        let req = AutomateRequest::Click {
            selector: "@5".to_string(),
            double_click: false,
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"op\":\"click\""));
        assert!(json.contains("\"selector\":\"@5\""));
    }

    #[test]
    fn test_toggle_request_serialization() {
        let req = AutomateRequest::Toggle {
            selector: "@5".to_string(),
            state: Some(true),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"op\":\"toggle\""));
        assert!(json.contains("\"state\":true"));
    }

    #[test]
    fn test_window_action_serialization() {
        let req = AutomateRequest::Window {
            action: WindowAction::Maximize,
            selector: Some("#Notepad".to_string()),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"op\":\"window\""));
        assert!(json.contains("\"maximize\""));
    }

    #[test]
    fn test_accessibility_element_serialization() {
        let elem = AccessibilityElement {
            r#ref: Some(1),
            role: "button".to_string(),
            name: Some("OK".to_string()),
            automation_id: Some("btnOK".to_string()),
            class_name: Some("Button".to_string()),
            bounds: Some(ElementBounds {
                x: 100,
                y: 200,
                width: 80,
                height: 30,
            }),
            states: vec!["focusable".to_string(), "enabled".to_string()],
            value: None,
            patterns: vec!["invoke".to_string()],
            children: vec![],
        };

        let json = serde_json::to_string(&elem).unwrap();
        assert!(json.contains("\"role\":\"button\""));
        assert!(json.contains("\"ref\":1"));
    }
}
