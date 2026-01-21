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
        } else if response.success {
            if let Some(ref data) = response.data {
                self.print_data(data);
            } else {
                println!("OK");
            }
        } else {
            // Error case - always print something
            if let Some(ref error) = response.error {
                eprintln!("Error [{}]: {}", error.code, error.message);
            } else {
                eprintln!("Error: Command failed (no details provided)");
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
            ResponseData::Screenshot { width, height, format, .. } => {
                println!("Screenshot: {}x{} ({})", width, height, format);
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
                // Print full accessibility tree like agent-browser
                println!("Snapshot ID: {}", snapshot.snapshot_id);
                println!("Elements: {}", snapshot.ref_count);
                println!();
                self.print_element_tree(&snapshot.root, 0);
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

    /// Print an element tree in compact Playwright-like aria format.
    /// Format: - role "name" [ref=eN, id=..., ...]
    fn print_element_tree(&self, element: &agent_rdp_protocol::AccessibilityElement, depth: usize) {
        let indent = "  ".repeat(depth);

        // Build the main line: - role "name"
        let mut line = format!("{}- {}", indent, element.role);

        // Add name if present
        if let Some(ref name) = element.name {
            if !name.is_empty() {
                // Truncate long names (use chars to handle Unicode correctly)
                let display_name = if name.chars().count() > 40 {
                    format!("{}...", name.chars().take(37).collect::<String>())
                } else {
                    name.clone()
                };
                line.push_str(&format!(" \"{}\"", display_name));
            }
        }

        // Build attributes in brackets
        let mut attrs = Vec::new();

        // Ref is always first attribute (with "e" prefix)
        if let Some(r) = element.r#ref {
            attrs.push(format!("ref=e{}", r));
        }

        if let Some(ref auto_id) = element.automation_id {
            if !auto_id.is_empty() {
                attrs.push(format!("id={}", auto_id));
            }
        }

        if let Some(ref class) = element.class_name {
            if !class.is_empty() {
                attrs.push(format!("class={}", class));
            }
        }

        if let Some(ref value) = element.value {
            if !value.is_empty() {
                // Use chars to handle Unicode correctly
                let display_value = if value.chars().count() > 30 {
                    format!("{}...", value.chars().take(27).collect::<String>())
                } else {
                    value.clone()
                };
                attrs.push(format!("value=\"{}\"", display_value));
            }
        }

        if !attrs.is_empty() {
            line.push_str(&format!(" [{}]", attrs.join(", ")));
        }

        println!("{}", line);

        // Recurse into children
        for child in &element.children {
            self.print_element_tree(child, depth + 1);
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
