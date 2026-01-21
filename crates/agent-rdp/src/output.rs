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
            ResponseData::Snapshot(snapshot) => {
                println!("Snapshot ID: {}", snapshot.snapshot_id);
                println!("Elements with refs: {}", snapshot.ref_count);
                // For non-JSON output, print a summary of the root element
                println!("Root: {} ({})",
                    snapshot.root.role,
                    snapshot.root.name.as_deref().unwrap_or("-"));
            }
            ResponseData::Element(element) => {
                if let Some(ref name) = element.name {
                    println!("Name: {}", name);
                }
                if let Some(ref value) = element.value {
                    println!("Value: {}", value);
                }
                if !element.states.is_empty() {
                    println!("States: {}", element.states.join(", "));
                }
                if let Some(ref bounds) = element.bounds {
                    println!("Bounds: {}x{} at ({}, {})",
                        bounds.width, bounds.height, bounds.x, bounds.y);
                }
            }
            ResponseData::WindowList { windows } => {
                if windows.is_empty() {
                    println!("No windows found");
                } else {
                    for window in windows {
                        let process = window.process_name.as_deref().unwrap_or("-");
                        println!("{}: {} ({})", window.title, process,
                            if window.minimized { "minimized" }
                            else if window.maximized { "maximized" }
                            else { "normal" });
                    }
                }
            }
            ResponseData::AutomationStatus(status) => {
                println!("Agent running: {}", status.agent_running);
                if let Some(pid) = status.agent_pid {
                    println!("Agent PID: {}", pid);
                }
                if let Some(ref version) = status.version {
                    println!("Version: {}", version);
                }
                if !status.capabilities.is_empty() {
                    println!("Capabilities: {}", status.capabilities.join(", "));
                }
            }
            ResponseData::RunResult(result) => {
                if let Some(code) = result.exit_code {
                    println!("Exit code: {}", code);
                }
                if let Some(ref stdout) = result.stdout {
                    if !stdout.is_empty() {
                        println!("{}", stdout);
                    }
                }
                if let Some(ref stderr) = result.stderr {
                    if !stderr.is_empty() {
                        eprintln!("{}", stderr);
                    }
                }
                if let Some(pid) = result.pid {
                    println!("Process ID: {}", pid);
                }
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
