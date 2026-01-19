//! Drive mapping command implementation.

use agent_rdp_protocol::{DriveRequest, Request};

use crate::cli::{DriveAction, DriveArgs};
use crate::output::Output;
use crate::session_manager::SessionManager;

pub async fn run(
    session: &str,
    args: DriveArgs,
    output: &Output,
    timeout_ms: u64,
) -> anyhow::Result<()> {
    let manager = SessionManager::new(session.to_string());

    if !manager.is_daemon_alive() {
        output.print_error("daemon_not_running", "No daemon running for this session");
        std::process::exit(1);
    }

    let mut client = manager.ensure_daemon().await?;

    let drive_request = match args.action {
        DriveAction::Map { path, name } => DriveRequest::Map { path, name },
        DriveAction::Unmap { name } => DriveRequest::Unmap { name },
        DriveAction::List => DriveRequest::List,
    };

    let request = Request::Drive(drive_request);
    let response = client.send(&request, timeout_ms).await?;
    output.print_response(&response);

    if !response.success {
        std::process::exit(1);
    }

    Ok(())
}
