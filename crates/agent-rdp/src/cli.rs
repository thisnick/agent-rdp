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

    /// Session management
    Session(SessionArgs),

    /// Wait for specified milliseconds
    Wait {
        /// Milliseconds to wait
        ms: u64,
    },
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

    /// Map a local directory as a drive (format: /path:DriveName)
    #[arg(long = "drive", value_name = "PATH:NAME")]
    pub drive: Option<String>,
}

/// Screenshot command arguments.
#[derive(Parser)]
pub struct ScreenshotArgs {
    /// Save to file path
    #[arg(long, short = 'o', default_value = "./screenshot.png")]
    pub output: String,

    /// Output base64 to stdout instead of file
    #[arg(long)]
    pub base64: bool,

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

    /// Press a key combination (e.g., "ctrl+c", "alt+tab")
    Press {
        /// Key combination
        keys: String,
    },

    /// Press and release a single key
    Key {
        /// Key name
        key: String,
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

    /// Run as daemon (internal use)
    #[command(hide = true)]
    Daemon,
}
