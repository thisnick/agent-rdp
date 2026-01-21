//! Locate command implementation (OCR-based text location).

use agent_rdp_protocol::{LocateRequest, Request, ResponseData};

use crate::cli::LocateArgs;
use crate::output::Output;
use crate::session_manager::SessionManager;

pub async fn run(
    session: &str,
    args: LocateArgs,
    output: &Output,
    timeout_ms: u64,
) -> anyhow::Result<()> {
    let manager = SessionManager::new(session.to_string());

    if !manager.is_daemon_alive() {
        output.print_error("daemon_not_running", "No daemon running for this session");
        std::process::exit(1);
    }

    let mut client = manager.ensure_daemon().await?;

    let search_text = args.text.clone().unwrap_or_default();

    let request = Request::Locate(LocateRequest {
        text: search_text.clone(),
        pattern: args.pattern,
        ignore_case: !args.case_sensitive,
        all: args.all,
    });

    let response = client.send(&request, timeout_ms).await?;

    if !response.success {
        output.print_response(&response);
        std::process::exit(1);
    }

    // Handle the locate result
    if let Some(ResponseData::LocateResult(result)) = response.data {
        if output.is_json() {
            // Output full JSON result
            println!("{}", serde_json::to_string(&serde_json::json!({
                "success": true,
                "data": {
                    "matches": result.matches,
                    "total_lines": result.total_words
                }
            }))?);
        } else if args.all {
            // Show all lines
            println!("Found {} text lines on screen:", result.matches.len());
            for m in &result.matches {
                println!("  '{}' at ({}, {}) size {}x{} - center: ({}, {})",
                    m.text, m.x, m.y, m.width, m.height, m.center_x, m.center_y);
            }
        } else {
            // Search mode
            if result.matches.is_empty() {
                println!("No lines containing '{}' found ({} lines detected)", search_text, result.total_words);
            } else {
                println!("Found {} line(s) containing '{}' ({} lines detected):",
                    result.matches.len(), search_text, result.total_words);
                for m in &result.matches {
                    println!("  '{}' at ({}, {}) size {}x{} - center: ({}, {})",
                        m.text, m.x, m.y, m.width, m.height, m.center_x, m.center_y);
                }
                // Show a helpful hint for clicking
                if let Some(first) = result.matches.first() {
                    println!("\nTo click the first match: agent-rdp mouse click {} {}",
                        first.center_x, first.center_y);
                }
            }
        }
    }

    Ok(())
}
