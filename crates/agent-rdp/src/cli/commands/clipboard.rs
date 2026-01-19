//! Clipboard command implementation.

use agent_rdp_protocol::{ClipboardRequest, Request};

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

    let clipboard_request = match args.action {
        ClipboardAction::Get => ClipboardRequest::Get,
        ClipboardAction::Set { text } => ClipboardRequest::Set { text },
    };

    let request = Request::Clipboard(clipboard_request);
    let response = client.send(&request, timeout_ms).await?;
    output.print_response(&response);

    if !response.success {
        std::process::exit(1);
    }

    Ok(())
}
