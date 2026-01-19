//! Output formatting for CLI responses.

use agent_rdp_protocol::Response;

/// Output formatter.
pub struct Output {
    json: bool,
}

impl Output {
    /// Create a new output formatter.
    pub fn new(json: bool) -> Self {
        Self { json }
    }

    /// Whether JSON output is enabled.
    pub fn is_json(&self) -> bool {
        self.json
    }

    /// Print a response.
    pub fn print_response(&self, response: &Response) {
        if self.json {
            println!("{}", serde_json::to_string(response).unwrap());
        } else {
            if response.success {
                if let Some(ref data) = response.data {
                    self.print_data(data);
                } else {
                    println!("OK");
                }
            } else if let Some(ref error) = response.error {
                eprintln!("Error [{}]: {}", error.code, error.message);
            }
        }
    }

    /// Print response data in human-readable format.
    fn print_data(&self, data: &agent_rdp_protocol::ResponseData) {
        use agent_rdp_protocol::ResponseData;

        match data {
            ResponseData::Ok => {
                println!("OK");
            }
            ResponseData::Connected { host, width, height } => {
                println!("Connected to {} ({}x{})", host, width, height);
            }
            ResponseData::Screenshot { width, height, format, base64 } => {
                println!("Screenshot: {}x{} ({})", width, height, format);
                if self.json {
                    // In JSON mode, base64 is included in the output
                } else {
                    println!("Base64 data: {} bytes", base64.len());
                }
            }
            ResponseData::Clipboard { text } => {
                println!("{}", text);
            }
            ResponseData::SessionInfo(info) => {
                println!("Session: {}", info.name);
                println!("State: {:?}", info.state);
                if let Some(ref host) = info.host {
                    println!("Host: {}", host);
                }
                if let (Some(w), Some(h)) = (info.width, info.height) {
                    println!("Resolution: {}x{}", w, h);
                }
                println!("PID: {}", info.pid);
                println!("Uptime: {}s", info.uptime_secs);
            }
            ResponseData::DriveList { drives } => {
                if drives.is_empty() {
                    println!("No drives mapped");
                } else {
                    for drive in drives {
                        println!("{}: {}", drive.name, drive.path);
                    }
                }
            }
            ResponseData::SessionList { sessions } => {
                if sessions.is_empty() {
                    println!("No active sessions");
                } else {
                    for session in sessions {
                        let host = session.host.as_deref().unwrap_or("-");
                        println!("{}: {:?} ({})", session.name, session.state, host);
                    }
                }
            }
            ResponseData::Pong => {
                println!("Pong");
            }
        }
    }

    /// Print an error message.
    pub fn print_error(&self, code: &str, message: &str) {
        if self.json {
            let response = agent_rdp_protocol::Response {
                success: false,
                data: None,
                error: Some(agent_rdp_protocol::ErrorInfo {
                    code: agent_rdp_protocol::ErrorCode::InternalError,
                    message: message.to_string(),
                }),
            };
            println!("{}", serde_json::to_string(&response).unwrap());
        } else {
            eprintln!("Error [{}]: {}", code, message);
        }
    }
}
