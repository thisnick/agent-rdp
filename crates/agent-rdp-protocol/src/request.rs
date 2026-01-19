//! Request types for CLI to daemon communication.

use serde::{Deserialize, Serialize};

/// A request from the CLI to the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// Get session info.
    SessionInfo,

    /// Ping the daemon (for health checks).
    Ping,

    /// Shutdown the daemon gracefully.
    Shutdown,
}

/// A drive to map at connect time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveMapping {
    /// Local path to map.
    pub path: String,
    /// Name for the mapped drive (shown in Windows).
    pub name: String,
}

/// RDP connection parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub domain: Option<String>,

    /// Desktop width in pixels.
    pub width: u16,

    /// Desktop height in pixels.
    pub height: u16,

    /// Drives to map at connect time.
    #[serde(default)]
    pub drives: Vec<DriveMapping>,
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
        }
    }
}

/// Screenshot request parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotRequest {
    /// Image format.
    #[serde(default)]
    pub format: ImageFormat,
}

/// Supported image formats.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    #[default]
    Png,
    Jpeg,
}

/// Mouse operation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Keyboard operation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum KeyboardRequest {
    /// Type a text string (Unicode).
    Type { text: String },

    /// Press a key combination (e.g., "ctrl+c", "alt+tab").
    Press { keys: String },

    /// Press and release a single key.
    Key { key: String },

    /// Press and hold a key.
    KeyDown { key: String },

    /// Release a held key.
    KeyUp { key: String },
}

/// Scroll operation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollRequest {
    /// Scroll direction.
    pub direction: ScrollDirection,

    /// Amount to scroll (in notches, default: 3).
    #[serde(default = "default_scroll_amount")]
    pub amount: u32,

    /// Optional position to scroll at.
    #[serde(default)]
    pub x: Option<u16>,

    #[serde(default)]
    pub y: Option<u16>,
}

fn default_scroll_amount() -> u32 {
    3
}

/// Scroll direction.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Clipboard operation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ClipboardRequest {
    /// Get clipboard text content.
    GetText,

    /// Set clipboard text content.
    SetText { text: String },

    /// Get file from clipboard.
    GetFile,

    /// Set file to clipboard.
    SetFile(SetFileSource),
}

/// Source for setting a file to clipboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum SetFileSource {
    /// File path - daemon reads on-demand when server requests.
    Path { path: String },

    /// In-memory data (from stdin) - base64 encoded.
    Data {
        /// File name to use on clipboard.
        name: String,
        /// Base64-encoded file data.
        data: String,
    },
}

/// Drive mapping operation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum DriveRequest {
    /// Map a local directory to a drive.
    Map {
        /// Local path to map.
        path: String,
        /// Name for the mapped drive.
        name: String,
    },

    /// Unmap a drive.
    Unmap { name: String },

    /// List mapped drives.
    List,
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
