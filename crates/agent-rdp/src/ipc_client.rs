//! IPC client for communicating with the daemon.

use std::io;
use std::path::Path;
use std::time::Duration;

use agent_rdp_protocol::{Request, Response};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::timeout;

/// IPC client for daemon communication.
pub struct IpcClient {
    #[cfg(unix)]
    stream: tokio::net::UnixStream,
    #[cfg(windows)]
    stream: tokio::net::TcpStream,
}

impl IpcClient {
    /// Connect to the daemon for the given session.
    #[cfg(unix)]
    pub async fn connect(socket_path: &Path) -> io::Result<Self> {
        let stream = tokio::net::UnixStream::connect(socket_path).await?;
        Ok(Self { stream })
    }

    #[cfg(windows)]
    pub async fn connect(socket_path: &Path) -> io::Result<Self> {
        // On Windows, derive port from session name
        let session = socket_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("default");

        let port = agent_rdp_daemon::get_session_port(session);
        let addr = format!("127.0.0.1:{}", port);
        let stream = tokio::net::TcpStream::connect(&addr).await?;
        Ok(Self { stream })
    }

    /// Send a request and receive a response.
    pub async fn send(&mut self, request: &Request, timeout_ms: u64) -> anyhow::Result<Response> {
        let json = serde_json::to_string(request)? + "\n";

        // Write request and flush to ensure it's sent immediately
        self.stream.write_all(json.as_bytes()).await?;
        self.stream.flush().await?;

        // Read response with timeout
        let response = timeout(
            Duration::from_millis(timeout_ms),
            self.read_response(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Request timed out"))??;

        Ok(response)
    }

    /// Read a response from the stream.
    async fn read_response(&mut self) -> anyhow::Result<Response> {
        let mut reader = BufReader::new(&mut self.stream);
        let mut line = String::new();
        reader.read_line(&mut line).await?;

        let response: Response = serde_json::from_str(line.trim())?;
        Ok(response)
    }
}

/// Try to connect to an existing daemon, with optional retries.
pub async fn try_connect(socket_path: &Path, retries: u32, delay_ms: u64) -> io::Result<IpcClient> {
    let mut last_error = io::Error::new(io::ErrorKind::Other, "No connection attempts made");

    for _ in 0..retries {
        match IpcClient::connect(socket_path).await {
            Ok(client) => return Ok(client),
            Err(e) => {
                last_error = e;
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
        }
    }

    Err(last_error)
}
