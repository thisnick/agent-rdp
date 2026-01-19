//! Mouse command implementation.

use agent_rdp_protocol::{MouseRequest, Request};

use crate::cli::{MouseAction, MouseArgs};
use crate::output::Output;
use crate::session_manager::SessionManager;

pub async fn run(
    session: &str,
    args: MouseArgs,
    output: &Output,
    timeout_ms: u64,
) -> anyhow::Result<()> {
    let manager = SessionManager::new(session.to_string());

    if !manager.is_daemon_alive() {
        output.print_error("daemon_not_running", "No daemon running for this session");
        std::process::exit(1);
    }

    let mut client = manager.ensure_daemon().await?;

    let mouse_request = match args.action {
        MouseAction::Click { x, y } => MouseRequest::Click { x, y },
        MouseAction::RightClick { x, y } => MouseRequest::RightClick { x, y },
        MouseAction::DoubleClick { x, y } => MouseRequest::DoubleClick { x, y },
        MouseAction::Move { x, y } => MouseRequest::Move { x, y },
        MouseAction::Drag { x1, y1, x2, y2 } => MouseRequest::Drag {
            from_x: x1,
            from_y: y1,
            to_x: x2,
            to_y: y2,
        },
    };

    let request = Request::Mouse(mouse_request);
    let response = client.send(&request, timeout_ms).await?;
    output.print_response(&response);

    if !response.success {
        std::process::exit(1);
    }

    Ok(())
}
