//! Response types for daemon to CLI communication.

use crate::automation::{
    AccessibilitySnapshot, AutomationStatus, ClickResult, ElementValue, RunResult, WindowInfo,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A response from the daemon to the CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Whether the operation succeeded.
    pub success: bool,

    /// Response data on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ResponseData>,

    /// Error details on failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
}

impl Response {
    /// Create a successful response with data.
    pub fn success(data: ResponseData) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    /// Create a simple success response with no data.
    pub fn ok() -> Self {
        Self {
            success: true,
            data: Some(ResponseData::Ok),
            error: None,
        }
    }

    /// Create an error response.
    pub fn error(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(ErrorInfo {
                code,
                message: message.into(),
            }),
        }
    }
}

/// Response data variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseData {
    /// Simple acknowledgment.
    Ok,

    /// Connection established.
    Connected {
        /// Server hostname.
        host: String,
        /// Desktop width.
        width: u16,
        /// Desktop height.
        height: u16,
    },

    /// Screenshot data.
    Screenshot {
        /// Image width.
        width: u32,
        /// Image height.
        height: u32,
        /// Image format.
        format: String,
        /// Base64-encoded image data.
        base64: String,
    },

    /// Clipboard text content.
    Clipboard {
        /// Text content.
        text: String,
    },

    /// Session information.
    SessionInfo(SessionInfo),

    /// List of mapped drives.
    DriveList {
        /// Mapped drives.
        drives: Vec<MappedDrive>,
    },

    /// List of active sessions.
    SessionList {
        /// Active sessions.
        sessions: Vec<SessionSummary>,
    },

    /// Pong response for ping.
    Pong,

    /// Accessibility tree snapshot.
    Snapshot(AccessibilitySnapshot),

    /// Element value/properties.
    Element(ElementValue),

    /// Window list.
    WindowList {
        /// List of windows.
        windows: Vec<WindowInfo>,
    },

    /// Automation agent status.
    AutomationStatus(AutomationStatus),

    /// Command run result.
    RunResult(RunResult),

    /// Click action result.
    ClickResult(ClickResult),

    /// OCR locate result.
    LocateResult(LocateResult),
}

/// Session information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Session name.
    pub name: String,

    /// Connection state.
    pub state: ConnectionState,

    /// Connected server host (if connected).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,

    /// Desktop width (if connected).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u16>,

    /// Desktop height (if connected).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u16>,

    /// Daemon process ID.
    pub pid: u32,

    /// Time since daemon started (seconds).
    pub uptime_secs: u64,
}

/// Connection state.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    /// Not connected to any RDP server.
    Disconnected,
    /// Currently connecting.
    Connecting,
    /// Connected and active.
    Connected,
    /// Connection failed.
    Failed,
}

/// Summary of a session for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    /// Session name.
    pub name: String,
    /// Connection state.
    pub state: ConnectionState,
    /// Connected host (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

/// Mapped drive information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappedDrive {
    /// Drive name.
    pub name: String,
    /// Local path.
    pub path: String,
}

/// OCR locate result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocateResult {
    /// Matching text regions found.
    pub matches: Vec<OcrMatch>,
    /// Total words detected on screen.
    pub total_words: u32,
}

/// A text region found by OCR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrMatch {
    /// Recognized text.
    pub text: String,
    /// Left edge X coordinate.
    pub x: i32,
    /// Top edge Y coordinate.
    pub y: i32,
    /// Width of bounding box.
    pub width: i32,
    /// Height of bounding box.
    pub height: i32,
    /// Center X coordinate (for clicking).
    pub center_x: i32,
    /// Center Y coordinate (for clicking).
    pub center_y: i32,
}

/// Error information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    /// Error code.
    pub code: ErrorCode,
    /// Human-readable error message.
    pub message: String,
}

/// Error codes for structured error handling.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Error)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Not connected to an RDP server.
    #[error("not connected")]
    NotConnected,

    /// Already connected to an RDP server.
    #[error("already connected")]
    AlreadyConnected,

    /// Failed to establish RDP connection.
    #[error("connection failed")]
    ConnectionFailed,

    /// Authentication failed.
    #[error("authentication failed")]
    AuthenticationFailed,

    /// Connection timed out.
    #[error("timeout")]
    Timeout,

    /// Invalid request parameters.
    #[error("invalid request")]
    InvalidRequest,

    /// Requested feature is not supported.
    #[error("not supported")]
    NotSupported,

    /// Internal daemon error.
    #[error("internal error")]
    InternalError,

    /// Session not found.
    #[error("session not found")]
    SessionNotFound,

    /// IPC communication error.
    #[error("ipc error")]
    IpcError,

    /// Daemon not running.
    #[error("daemon not running")]
    DaemonNotRunning,

    /// Clipboard operation failed.
    #[error("clipboard error")]
    ClipboardError,

    /// Drive mapping error.
    #[error("drive error")]
    DriveError,

    /// Automation agent not running.
    #[error("automation not enabled")]
    AutomationNotEnabled,

    /// Automation agent error.
    #[error("automation error")]
    AutomationError,

    /// Element not found.
    #[error("element not found")]
    ElementNotFound,

    /// Stale element reference.
    #[error("stale reference")]
    StaleRef,

    /// Automation command failed.
    #[error("command failed")]
    CommandFailed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_response() {
        let resp = Response::success(ResponseData::Connected {
            host: "192.168.1.100".to_string(),
            width: 1920,
            height: 1080,
        });

        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"type\":\"connected\""));
    }

    #[test]
    fn test_error_response() {
        let resp = Response::error(ErrorCode::ConnectionFailed, "Connection refused");

        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"code\":\"connection_failed\""));
    }

    #[test]
    fn test_screenshot_response() {
        let resp = Response::success(ResponseData::Screenshot {
            width: 1920,
            height: 1080,
            format: "png".to_string(),
            base64: "iVBORw0KGgo...".to_string(),
        });

        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"type\":\"screenshot\""));
    }
}
