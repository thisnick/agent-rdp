//! Daemon process for agent-rdp RDP session management.
//!
//! This crate implements the background daemon that maintains RDP connections
//! and handles commands from CLI instances via IPC.

pub mod daemon;
pub mod handlers;
pub mod ipc_server;
pub mod rdp_session;

pub use daemon::Daemon;
pub use ipc_server::IpcServer;
pub use rdp_session::RdpSession;

/// Get the session directory path.
pub fn get_session_dir(session: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("/tmp/agent-rdp/{}", session))
}

/// Get the socket path for a session.
pub fn get_socket_path(session: &str) -> std::path::PathBuf {
    #[cfg(unix)]
    {
        get_session_dir(session).join("socket")
    }
    #[cfg(windows)]
    {
        // On Windows, we use a named pipe path
        std::path::PathBuf::from(format!("\\\\.\\pipe\\agent-rdp-{}", session))
    }
}

/// Get the PID file path for a session.
pub fn get_pid_path(session: &str) -> std::path::PathBuf {
    get_session_dir(session).join("pid")
}

/// Get the TCP port for a session (Windows fallback).
/// Uses a deterministic hash of the session name to derive a port in the range 49152-65535.
pub fn get_session_port(session: &str) -> u16 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    session.hash(&mut hasher);
    let hash = hasher.finish();
    // Map to ephemeral port range: 49152-65535 (16384 ports)
    49152 + (hash % 16384) as u16
}

/// Clean up a session directory.
pub fn cleanup_session(session: &str) {
    let dir = get_session_dir(session);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Run the daemon server for the given session.
/// This is the main entry point called by `agent-rdp session daemon`.
pub async fn run_server(session: &str) -> anyhow::Result<()> {
    use std::io::Write;

    // Create session directory
    let session_dir = get_session_dir(session);
    std::fs::create_dir_all(&session_dir)?;

    // Write PID file
    let pid_path = get_pid_path(session);
    let mut pid_file = std::fs::File::create(&pid_path)?;
    writeln!(pid_file, "{}", std::process::id())?;
    drop(pid_file);

    // Create and run daemon
    let mut daemon = Daemon::new(session.to_string()).await?;
    let result = daemon.run().await;

    // Cleanup on exit
    cleanup_session(session);

    result
}
