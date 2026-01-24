//! Request types for CLI to daemon communication.

use crate::automation::AutomateRequest;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A request from the CLI to the daemon.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    /// Connect to an RDP server.
    Connect(ConnectRequest),

    /// Disconnect from the RDP server.
    Disconnect,

    /// Take a screenshot.
    Screenshot(ScreenshotRequest),

    /// Mouse operation.
    Mouse(MouseRequest),

    /// Keyboard operation.
    Keyboard(KeyboardRequest),

    /// Scroll operation.
    Scroll(ScrollRequest),

    /// Clipboard operation.
    Clipboard(ClipboardRequest),

    /// Drive mapping operation.
    Drive(DriveRequest),

    /// UI Automation operation.
    Automate(AutomateRequest),

    /// OCR-based text location.
    Locate(LocateRequest),

    /// Get session info.
    SessionInfo,

    /// Ping the daemon (for health checks).
    Ping,

    /// Shutdown the daemon gracefully.
    Shutdown,
}

/// A drive to map at connect time.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
pub struct DriveMapping {
    /// Local path to map.
    pub path: String,
    /// Name for the mapped drive (shown in Windows).
    pub name: String,
}

/// RDP connection parameters.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
pub struct ConnectRequest {
    /// Server hostname or IP address.
    pub host: String,

    /// Server port (default: 3389).
    pub port: u16,

    /// Username for authentication.
    pub username: String,

    /// Password for authentication.
    pub password: String,

    /// Optional domain.
    #[serde(default)]
    #[ts(optional)]
    pub domain: Option<String>,

    /// Desktop width in pixels.
    pub width: u16,

    /// Desktop height in pixels.
    pub height: u16,

    /// Drives to map at connect time.
    #[serde(default)]
    pub drives: Vec<DriveMapping>,

    /// Enable Windows UI Automation.
    #[serde(default)]
    pub enable_win_automation: bool,

    /// WebSocket streaming port (0 = disabled).
    #[serde(default)]
    pub stream_port: u16,

    /// Streaming frame rate (default: 10).
    #[serde(default = "default_stream_fps")]
    pub stream_fps: u32,

    /// Streaming JPEG quality 0-100 (default: 80).
    #[serde(default = "default_stream_quality")]
    pub stream_quality: u8,

    /// Serve the embedded HTML viewer on the streaming port (default: false).
    /// When false, only WebSocket connections are accepted.
    #[serde(default)]
    pub serve_viewer: bool,
}

fn default_stream_fps() -> u32 {
    10
}

fn default_stream_quality() -> u8 {
    80
}

impl Default for ConnectRequest {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 3389,
            username: String::new(),
            password: String::new(),
            domain: None,
            width: 1280,
            height: 800,
            drives: Vec::new(),
            enable_win_automation: false,
            stream_port: 0,
            stream_fps: default_stream_fps(),
            stream_quality: default_stream_quality(),
            serve_viewer: false,
        }
    }
}

/// Screenshot request parameters.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
pub struct ScreenshotRequest {
    /// Image format.
    #[serde(default)]
    pub format: ImageFormat,
}

/// Supported image formats.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    #[default]
    Png,
    Jpeg,
}

/// Mouse operation request.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum MouseRequest {
    /// Move the mouse cursor.
    Move { x: u16, y: u16 },

    /// Left click.
    Click { x: u16, y: u16 },

    /// Right click.
    RightClick { x: u16, y: u16 },

    /// Double click.
    DoubleClick { x: u16, y: u16 },

    /// Middle click.
    MiddleClick { x: u16, y: u16 },

    /// Drag from one position to another.
    Drag {
        from_x: u16,
        from_y: u16,
        to_x: u16,
        to_y: u16,
    },

    /// Press and hold a mouse button.
    ButtonDown { button: MouseButton },

    /// Release a mouse button.
    ButtonUp { button: MouseButton },
}

/// Mouse button identifiers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Keyboard operation request.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum KeyboardRequest {
    /// Type a text string (Unicode).
    Type { text: String },

    /// Press a key combination (e.g., "ctrl+c", "alt+tab", or single key like "enter").
    Press { keys: String },

    /// Press and hold a key.
    KeyDown { key: String },

    /// Release a held key.
    KeyUp { key: String },
}

/// Scroll operation request.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
pub struct ScrollRequest {
    /// Scroll direction.
    pub direction: ScrollDirection,

    /// Amount to scroll (in notches, default: 3).
    #[serde(default = "default_scroll_amount")]
    pub amount: u32,

    /// Optional position to scroll at.
    #[serde(default)]
    #[ts(optional)]
    pub x: Option<u16>,

    #[serde(default)]
    #[ts(optional)]
    pub y: Option<u16>,
}

fn default_scroll_amount() -> u32 {
    3
}

/// Scroll direction.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
#[serde(rename_all = "lowercase")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Clipboard operation request.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ClipboardRequest {
    /// Get clipboard text content.
    Get,

    /// Set clipboard text content.
    Set { text: String },
}

/// Drive mapping operation request.
/// Note: Drives are configured at connect time with --drive flag.
/// Dynamic mapping/unmapping is not supported by the RDP protocol.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum DriveRequest {
    /// List mapped drives.
    List,
}

/// OCR-based text location request.
/// Uses screenshot + OCR to find text on screen and return coordinates.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/agent-rdp/src/generated/")]
pub struct LocateRequest {
    /// Text to search for (ignored if `all` is true).
    #[serde(default)]
    pub text: String,

    /// Use pattern matching (glob-style: * and ?).
    #[serde(default)]
    pub pattern: bool,

    /// Case-insensitive matching (default: true).
    #[serde(default = "default_true")]
    pub ignore_case: bool,

    /// Return all text on screen (ignores text/pattern/ignore_case).
    #[serde(default)]
    pub all: bool,
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let req = Request::Connect(ConnectRequest {
            host: "192.168.1.100".to_string(),
            port: 3389,
            username: "admin".to_string(),
            password: "secret".to_string(),
            domain: Some("WORKGROUP".to_string()),
            width: 1920,
            height: 1080,
            drives: vec![],
            enable_win_automation: false,
            ..Default::default()
        });

        let json = serde_json::to_string(&req).unwrap();
        let parsed: Request = serde_json::from_str(&json).unwrap();

        match parsed {
            Request::Connect(c) => {
                assert_eq!(c.host, "192.168.1.100");
                assert_eq!(c.port, 3389);
            }
            _ => panic!("unexpected request type"),
        }
    }

    #[test]
    fn test_connect_with_drives() {
        let req = Request::Connect(ConnectRequest {
            host: "192.168.1.100".to_string(),
            port: 3389,
            username: "admin".to_string(),
            password: "secret".to_string(),
            domain: None,
            width: 1920,
            height: 1080,
            drives: vec![
                DriveMapping {
                    path: "/home/user/docs".to_string(),
                    name: "Documents".to_string(),
                },
                DriveMapping {
                    path: "/tmp/shared".to_string(),
                    name: "Shared".to_string(),
                },
            ],
            enable_win_automation: false,
            ..Default::default()
        });

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"drives\""));
        assert!(json.contains("Documents"));
        assert!(json.contains("Shared"));

        let parsed: Request = serde_json::from_str(&json).unwrap();
        match parsed {
            Request::Connect(c) => {
                assert_eq!(c.drives.len(), 2);
                assert_eq!(c.drives[0].name, "Documents");
                assert_eq!(c.drives[1].path, "/tmp/shared");
            }
            _ => panic!("unexpected request type"),
        }
    }

    #[test]
    fn test_mouse_request_serialization() {
        let req = Request::Mouse(MouseRequest::Click { x: 100, y: 200 });
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"action\":\"click\""));
        assert!(json.contains("\"x\":100"));
    }

    #[test]
    fn test_keyboard_request_serialization() {
        let req = Request::Keyboard(KeyboardRequest::Press {
            keys: "ctrl+c".to_string(),
        });
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"action\":\"press\""));
        assert!(json.contains("ctrl+c"));
    }
}
