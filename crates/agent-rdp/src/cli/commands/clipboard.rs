//! Clipboard command implementation.

use std::fs::File;
use std::io::{self, Read, Write};

use agent_rdp_protocol::{ClipboardRequest, Request, ResponseData, SetFileSource};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;

use crate::cli::{ClipboardAction, ClipboardArgs};
use crate::output::Output;
use crate::session_manager::SessionManager;

pub async fn run(
    session: &str,
    args: ClipboardArgs,
    output: &Output,
    timeout_ms: u64,
) -> anyhow::Result<()> {
    let manager = SessionManager::new(session.to_string());

    if !manager.is_daemon_alive() {
        output.print_error("daemon_not_running", "No daemon running for this session");
        std::process::exit(1);
    }

    let mut client = manager.ensure_daemon().await?;

    let clipboard_request = match &args.action {
        ClipboardAction::GetText => ClipboardRequest::GetText,
        ClipboardAction::SetText { text } => ClipboardRequest::SetText { text: text.clone() },
        ClipboardAction::GetFile { .. } => ClipboardRequest::GetFile,
        ClipboardAction::SetFile { path, base64, name } => {
            if *base64 {
                // Read base64 from stdin
                let name = name.clone().ok_or_else(|| {
                    anyhow::anyhow!("--name is required when using --base64")
                })?;
                let mut data = String::new();
                io::stdin().read_to_string(&mut data)?;
                ClipboardRequest::SetFile(SetFileSource::Data {
                    name,
                    data: data.trim().to_string(),
                })
            } else {
                // Read from file path
                let path = path.clone().ok_or_else(|| {
                    anyhow::anyhow!("File path is required (or use --base64 to read from stdin)")
                })?;
                ClipboardRequest::SetFile(SetFileSource::Path { path })
            }
        }
    };

    let request = Request::Clipboard(clipboard_request);
    let response = client.send(&request, timeout_ms).await?;

    // Handle get-file special output
    if let ClipboardAction::GetFile { output: file_output, base64: use_base64 } = &args.action {
        if response.success {
            if let Some(ResponseData::ClipboardFile { name, size, data }) = &response.data {
                if let Some(output_path) = file_output {
                    // Write to file
                    let bytes = BASE64.decode(data)?;
                    let mut file = File::create(output_path)?;
                    file.write_all(&bytes)?;

                    if output.is_json() {
                        // Print JSON with file info
                        let saved_response = agent_rdp_protocol::Response::success(
                            ResponseData::ClipboardFileSaved {
                                name: name.clone(),
                                size: *size,
                                path: output_path.clone(),
                            }
                        );
                        output.print_response(&saved_response);
                    } else {
                        println!("Saved {} ({} bytes) to {}", name, size, output_path);
                    }
                } else if *use_base64 {
                    // Output base64 to stdout
                    if output.is_json() {
                        output.print_response(&response);
                    } else {
                        println!("{}", data);
                    }
                } else {
                    // Neither --output nor --base64 specified
                    output.print_error("invalid_request", "Must specify --output or --base64 for get-file");
                    std::process::exit(1);
                }
            } else {
                output.print_response(&response);
            }
        } else {
            output.print_response(&response);
            std::process::exit(1);
        }
    } else {
        // Normal response handling for text operations
        output.print_response(&response);
        if !response.success {
            std::process::exit(1);
        }
    }

    Ok(())
}
