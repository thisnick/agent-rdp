//! IPC server for CLI communication.

use std::path::Path;
use std::io;

use thiserror::Error;
use tokio::io::{ReadHalf, WriteHalf};
use tracing::info;

#[derive(Error, Debug)]
pub enum IpcError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Socket path already exists")]
    SocketExists,
}

/// Platform-specific IPC stream type alias.
#[cfg(unix)]
pub type IpcStream = tokio::net::UnixStream;

#[cfg(windows)]
pub type IpcStream = tokio::net::TcpStream;

/// IPC server that listens for CLI connections.
pub struct IpcServer {
    #[cfg(unix)]
    listener: tokio::net::UnixListener,

    #[cfg(windows)]
    listener: tokio::net::TcpListener,

    /// Path or address for display purposes.
    address: String,
}

impl IpcServer {
    /// Bind to the given socket path (Unix) or derive a port (Windows).
    #[cfg(unix)]
    pub async fn bind(path: &Path) -> Result<Self, IpcError> {
        // Remove existing socket file if present
        if path.exists() {
            std::fs::remove_file(path)?;
        }

        let listener = tokio::net::UnixListener::bind(path)?;
        let address = path.display().to_string();

        info!("IPC server listening on {}", address);

        Ok(Self { listener, address })
    }

    #[cfg(windows)]
    pub async fn bind(path: &Path) -> Result<Self, IpcError> {
        // On Windows, extract session name and use TCP
        let session = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("default");

        let port = crate::get_session_port(session);
        let addr = format!("127.0.0.1:{}", port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;

        info!("IPC server listening on {}", addr);

        Ok(Self {
            listener,
            address: addr,
        })
    }

    /// Accept a new client connection.
    #[cfg(unix)]
    pub async fn accept(&self) -> Result<IpcStream, IpcError> {
        let (stream, _) = self.listener.accept().await?;
        Ok(stream)
    }

    #[cfg(windows)]
    pub async fn accept(&self) -> Result<IpcStream, IpcError> {
        let (stream, _) = self.listener.accept().await?;
        Ok(stream)
    }

    /// Get the address this server is listening on.
    pub fn address(&self) -> &str {
        &self.address
    }
}

/// Extension trait to split an IPC stream into read/write halves.
pub trait IpcStreamExt {
    type Read;
    type Write;

    fn split(self) -> (Self::Read, Self::Write);
}

#[cfg(unix)]
impl IpcStreamExt for IpcStream {
    type Read = ReadHalf<tokio::net::UnixStream>;
    type Write = WriteHalf<tokio::net::UnixStream>;

    fn split(self) -> (Self::Read, Self::Write) {
        tokio::io::split(self)
    }
}

#[cfg(windows)]
impl IpcStreamExt for IpcStream {
    type Read = ReadHalf<tokio::net::TcpStream>;
    type Write = WriteHalf<tokio::net::TcpStream>;

    fn split(self) -> (Self::Read, Self::Write) {
        tokio::io::split(self)
    }
}
