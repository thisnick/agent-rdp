//! CLI command definitions using clap.

use clap::{Parser, Subcommand};

pub mod commands;

/// CLI tool for AI agents to control Windows Remote Desktop sessions.
#[derive(Parser)]
#[command(name = "agent-rdp")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Session name
    #[arg(long, default_value = "default", env = "AGENT_RDP_SESSION")]
    pub session: String,

    /// Output in JSON format for AI consumption
    #[arg(long, global = true)]
    pub json: bool,

    /// Command timeout in milliseconds
    #[arg(long, default_value = "30000", global = true)]
    pub timeout: u64,

    /// WebSocket streaming port (0 = disabled, enables browser viewer for debugging)
    #[arg(long, default_value = "0", env = "AGENT_RDP_STREAM_PORT", global = true)]
    pub stream_port: u16,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Connect to an RDP server
    Connect(ConnectArgs),

    /// Disconnect from RDP and close the session
    Disconnect,

    /// Take a screenshot
    Screenshot(ScreenshotArgs),

    /// Mouse operations
    Mouse(MouseArgs),

    /// Keyboard operations
    Keyboard(KeyboardArgs),

    /// Scroll operations
    Scroll(ScrollArgs),

    /// Clipboard operations
    Clipboard(ClipboardArgs),

    /// Drive mapping operations
    Drive(DriveArgs),

    /// Windows UI Automation operations
    Automate(AutomateArgs),

    /// OCR-based text location (find text on screen)
    Locate(LocateArgs),

    /// Session management
    Session(SessionArgs),

    /// Wait for specified milliseconds
    Wait {
        /// Milliseconds to wait
        ms: u64,
    },

    /// Open the web viewer in a browser
    View(ViewArgs),
}

/// View command arguments.
#[derive(Parser)]
pub struct ViewArgs {
    /// WebSocket streaming port to connect to
    #[arg(long, default_value = "9224")]
    pub port: u16,
}

/// Connect command arguments.
#[derive(Parser)]
pub struct ConnectArgs {
    /// Server hostname or IP
    #[arg(long, required = true)]
    pub host: String,

    /// Server port
    #[arg(long, default_value = "3389")]
    pub port: u16,

    /// Username (or set AGENT_RDP_USERNAME)
    #[arg(long, short = 'u', env = "AGENT_RDP_USERNAME", required = true)]
    pub username: String,

    /// Password (or set AGENT_RDP_PASSWORD, or use --password-stdin)
    #[arg(long, short = 'p', env = "AGENT_RDP_PASSWORD")]
    pub password: Option<String>,

    /// Read password from stdin (more secure than command line)
    #[arg(long)]
    pub password_stdin: bool,

    /// Domain
    #[arg(long, short = 'd')]
    pub domain: Option<String>,

    /// Desktop width
    #[arg(long, default_value = "1280")]
    pub width: u16,

    /// Desktop height
    #[arg(long, default_value = "800")]
    pub height: u16,

    /// Map local directories as drives (format: /path:DriveName, can be specified multiple times)
    #[arg(long = "drive", value_name = "PATH:NAME")]
    pub drives: Vec<String>,

    /// Enable Windows UI Automation (requires automation agent on remote host)
    #[arg(long)]
    pub enable_win_automation: bool,
}

/// Screenshot command arguments.
#[derive(Parser)]
pub struct ScreenshotArgs {
    /// Save to file path
    #[arg(long, short = 'o', default_value = "./screenshot.png")]
    pub output: String,

    /// Image format
    #[arg(long, default_value = "png")]
    pub format: String,
}

/// Mouse command arguments.
#[derive(Parser)]
pub struct MouseArgs {
    #[command(subcommand)]
    pub action: MouseAction,
}

#[derive(Subcommand)]
pub enum MouseAction {
    /// Left click at position
    Click {
        /// X coordinate
        x: u16,
        /// Y coordinate
        y: u16,
    },

    /// Right click at position
    RightClick {
        /// X coordinate
        x: u16,
        /// Y coordinate
        y: u16,
    },

    /// Double click at position
    DoubleClick {
        /// X coordinate
        x: u16,
        /// Y coordinate
        y: u16,
    },

    /// Move cursor to position
    Move {
        /// X coordinate
        x: u16,
        /// Y coordinate
        y: u16,
    },

    /// Drag from one position to another
    Drag {
        /// Start X coordinate
        x1: u16,
        /// Start Y coordinate
        y1: u16,
        /// End X coordinate
        x2: u16,
        /// End Y coordinate
        y2: u16,
    },
}

/// Keyboard command arguments.
#[derive(Parser)]
pub struct KeyboardArgs {
    #[command(subcommand)]
    pub action: KeyboardAction,
}

#[derive(Subcommand)]
pub enum KeyboardAction {
    /// Type a text string
    Type {
        /// Text to type
        text: String,
    },

    /// Press a key combination (e.g., "ctrl+c", "alt+tab") or single key (e.g., "enter")
    Press {
        /// Key combination or single key
        keys: String,
    },
}

/// Scroll command arguments.
#[derive(Parser)]
pub struct ScrollArgs {
    #[command(subcommand)]
    pub direction: ScrollDirection,
}

#[derive(Subcommand)]
pub enum ScrollDirection {
    /// Scroll up
    Up {
        /// Amount to scroll
        #[arg(default_value = "3")]
        amount: u32,
        /// Position to scroll at (x y)
        #[arg(long = "at", num_args = 2, value_names = ["X", "Y"])]
        at: Option<Vec<u16>>,
    },

    /// Scroll down
    Down {
        /// Amount to scroll
        #[arg(default_value = "3")]
        amount: u32,
        /// Position to scroll at (x y)
        #[arg(long = "at", num_args = 2, value_names = ["X", "Y"])]
        at: Option<Vec<u16>>,
    },

    /// Scroll left
    Left {
        /// Amount to scroll
        #[arg(default_value = "3")]
        amount: u32,
        /// Position to scroll at (x y)
        #[arg(long = "at", num_args = 2, value_names = ["X", "Y"])]
        at: Option<Vec<u16>>,
    },

    /// Scroll right
    Right {
        /// Amount to scroll
        #[arg(default_value = "3")]
        amount: u32,
        /// Position to scroll at (x y)
        #[arg(long = "at", num_args = 2, value_names = ["X", "Y"])]
        at: Option<Vec<u16>>,
    },
}

/// Clipboard command arguments.
#[derive(Parser)]
pub struct ClipboardArgs {
    #[command(subcommand)]
    pub action: ClipboardAction,
}

#[derive(Subcommand)]
pub enum ClipboardAction {
    /// Get clipboard text
    Get,

    /// Set clipboard text
    Set {
        /// Text to set
        text: String,
    },
}

/// Drive command arguments.
#[derive(Parser)]
pub struct DriveArgs {
    #[command(subcommand)]
    pub action: DriveAction,
}

#[derive(Subcommand)]
pub enum DriveAction {
    /// List mapped drives (drives are configured at connect time with --drive)
    List,
}

/// Session command arguments.
#[derive(Parser)]
pub struct SessionArgs {
    #[command(subcommand)]
    pub action: SessionAction,
}

#[derive(Subcommand)]
pub enum SessionAction {
    /// List active sessions
    List,

    /// Get current session info
    Info,

    /// Run as background daemon for this session (starts automatically on connect)
    Daemon,
}

/// Automate command arguments.
#[derive(Parser)]
pub struct AutomateArgs {
    #[command(subcommand)]
    pub action: AutomateAction,
}

#[derive(Subcommand)]
pub enum AutomateAction {
    /// Take a snapshot of the accessibility tree
    Snapshot {
        /// Filter to interactive elements only (buttons, inputs, focusable)
        #[arg(short = 'i', long)]
        interactive: bool,

        /// Compact mode - remove empty structural elements
        #[arg(short = 'c', long)]
        compact: bool,

        /// Maximum tree depth (default: 10)
        #[arg(short = 'd', long)]
        depth: Option<u32>,

        /// Scope to a specific element (window, panel, etc.) via selector
        #[arg(short = 's', long)]
        selector: Option<String>,

        /// Start from the currently focused element
        #[arg(short = 'f', long)]
        focused: bool,
    },

    /// Get element properties
    Get {
        /// Element selector
        selector: String,

        /// Property to retrieve (name, value, states, bounds, or all)
        #[arg(long)]
        property: Option<String>,
    },

    /// Set focus to an element
    Focus {
        /// Element selector
        selector: String,
    },

    /// Click an element
    Click {
        /// Element selector
        selector: String,

        /// Mouse button (left, right, middle)
        #[arg(long)]
        button: Option<String>,

        /// Double-click instead of single click
        #[arg(long)]
        double: bool,
    },

    /// Double-click an element
    DoubleClick {
        /// Element selector
        selector: String,
    },

    /// Right-click an element
    RightClick {
        /// Element selector
        selector: String,
    },

    /// Clear and fill text in an element
    Fill {
        /// Element selector
        selector: String,

        /// Text to fill
        text: String,
    },

    /// Clear text from an element
    Clear {
        /// Element selector
        selector: String,
    },

    /// Select an item in a ComboBox or ListBox
    Select {
        /// Element selector
        selector: String,

        /// Item to select
        item: String,
    },

    /// Check or uncheck a CheckBox or RadioButton
    Check {
        /// Element selector
        selector: String,

        /// Uncheck instead of check
        #[arg(long)]
        uncheck: bool,
    },

    /// Scroll an element
    Scroll {
        /// Element selector
        selector: String,

        /// Scroll direction (up, down, left, right)
        #[arg(long)]
        direction: Option<String>,

        /// Scroll amount
        #[arg(long)]
        amount: Option<i32>,

        /// Child selector to scroll into view
        #[arg(long)]
        to_child: Option<String>,
    },

    /// Window operations
    Window {
        /// Action: list, focus, maximize, minimize, restore, close
        action: String,

        /// Window selector (optional)
        selector: Option<String>,
    },

    /// Run a PowerShell command
    Run {
        /// Command to run
        command: String,

        /// Command arguments
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,

        /// Wait for command to complete
        #[arg(long)]
        wait: bool,

        /// Run with hidden window
        #[arg(long)]
        hidden: bool,
    },

    /// Wait for an element to reach a state
    WaitFor {
        /// Element selector
        selector: String,

        /// Timeout in milliseconds
        #[arg(long)]
        timeout: Option<u64>,

        /// State to wait for (visible, enabled, gone)
        #[arg(long)]
        state: Option<String>,
    },

    /// Get automation agent status
    Status,
}

/// Locate command arguments (OCR-based text location).
#[derive(Parser)]
pub struct LocateArgs {
    /// Text to search for on screen (searches within full lines)
    #[arg(required_unless_present = "all")]
    pub text: Option<String>,

    /// Use pattern matching (glob-style: * and ?)
    #[arg(long, short = 'p')]
    pub pattern: bool,

    /// Case-sensitive matching (default is case-insensitive)
    #[arg(long, short = 'c')]
    pub case_sensitive: bool,

    /// Return all text lines on screen (ignores search text)
    #[arg(long, short = 'a')]
    pub all: bool,
}
