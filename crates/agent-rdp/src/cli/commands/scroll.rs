//! Scroll command implementation.

use agent_rdp_protocol::{Request, ScrollDirection as ProtoScrollDirection, ScrollRequest};

use crate::cli::{ScrollArgs, ScrollDirection};
use crate::output::Output;
use crate::session_manager::SessionManager;

pub async fn run(
    session: &str,
    args: ScrollArgs,
    output: &Output,
    timeout_ms: u64,
) -> anyhow::Result<()> {
    let manager = SessionManager::new(session.to_string());

    if !manager.is_daemon_alive() {
        output.print_error("daemon_not_running", "No daemon running for this session");
        std::process::exit(1);
    }

    let mut client = manager.ensure_daemon().await?;

    let (direction, amount, at) = match args.direction {
        ScrollDirection::Up { amount, at } => (ProtoScrollDirection::Up, amount, at),
        ScrollDirection::Down { amount, at } => (ProtoScrollDirection::Down, amount, at),
        ScrollDirection::Left { amount, at } => (ProtoScrollDirection::Left, amount, at),
        ScrollDirection::Right { amount, at } => (ProtoScrollDirection::Right, amount, at),
    };

    let (x, y) = match at {
        Some(coords) if coords.len() == 2 => (Some(coords[0]), Some(coords[1])),
        _ => (None, None),
    };

    let request = Request::Scroll(ScrollRequest {
        direction,
        amount,
        x,
        y,
    });

    let response = client.send(&request, timeout_ms).await?;
    output.print_response(&response);

    if !response.success {
        std::process::exit(1);
    }

    Ok(())
}
