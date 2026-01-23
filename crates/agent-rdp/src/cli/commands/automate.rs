//! Automate command implementation for Windows UI Automation.

use agent_rdp_protocol::{
    AutomateRequest, AutomationScrollDirection, Request, WaitState, WindowAction,
};

use crate::cli::{AutomateAction, AutomateArgs};
use crate::output::Output;
use crate::session_manager::SessionManager;

pub async fn run(
    session: &str,
    args: AutomateArgs,
    output: &Output,
    timeout_ms: u64,
) -> anyhow::Result<()> {
    let manager = SessionManager::new(session.to_string());

    if !manager.is_daemon_alive() {
        output.print_error("daemon_not_running", "No daemon running for this session");
        std::process::exit(1);
    }

    let mut client = manager.ensure_daemon().await?;

    let automate_request = match args.action {
        AutomateAction::Snapshot {
            interactive,
            compact,
            depth,
            selector,
            focused,
        } => AutomateRequest::Snapshot {
            interactive_only: interactive,
            compact,
            max_depth: depth,
            selector,
            focused,
        },

        AutomateAction::Get { selector, property } => AutomateRequest::Get { selector, property },

        AutomateAction::Focus { selector } => AutomateRequest::Focus { selector },

        AutomateAction::Click { selector, double_click } => AutomateRequest::Click { selector, double_click },

        AutomateAction::Select { selector, item } => AutomateRequest::Select { selector, item },

        AutomateAction::Toggle { selector, state } => {
            let state = state.map(|s| matches!(s.as_str(), "on" | "true" | "1"));
            AutomateRequest::Toggle { selector, state }
        }

        AutomateAction::Expand { selector } => AutomateRequest::Expand { selector },

        AutomateAction::Collapse { selector } => AutomateRequest::Collapse { selector },

        AutomateAction::ContextMenu { selector } => AutomateRequest::ContextMenu { selector },

        AutomateAction::Fill { selector, text } => AutomateRequest::Fill { selector, text },

        AutomateAction::Clear { selector } => AutomateRequest::Clear { selector },

        AutomateAction::Scroll {
            selector,
            direction,
            amount,
            to_child,
        } => {
            let direction = direction.map(|d| match d.as_str() {
                "up" => AutomationScrollDirection::Up,
                "down" => AutomationScrollDirection::Down,
                "left" => AutomationScrollDirection::Left,
                "right" => AutomationScrollDirection::Right,
                _ => AutomationScrollDirection::Down,
            });
            AutomateRequest::Scroll {
                selector,
                direction,
                amount,
                to_child,
            }
        }

        AutomateAction::Window { action, selector } => {
            let action = match action.as_str() {
                "list" => WindowAction::List,
                "focus" => WindowAction::Focus,
                "maximize" => WindowAction::Maximize,
                "minimize" => WindowAction::Minimize,
                "restore" => WindowAction::Restore,
                "close" => WindowAction::Close,
                _ => {
                    output.print_error(
                        "invalid_request",
                        &format!("Unknown window action: {}", action),
                    );
                    std::process::exit(1);
                }
            };
            AutomateRequest::Window { action, selector }
        }

        AutomateAction::Run {
            command,
            args: cmd_args,
            wait,
            hidden,
            process_timeout,
        } => AutomateRequest::Run {
            command,
            args: cmd_args,
            wait,
            hidden,
            timeout_ms: process_timeout.unwrap_or(10000),
        },

        AutomateAction::WaitFor {
            selector,
            timeout,
            state,
        } => {
            let state = match state.as_deref() {
                Some("enabled") => WaitState::Enabled,
                Some("gone") => WaitState::Gone,
                _ => WaitState::Visible,
            };
            AutomateRequest::WaitFor {
                selector,
                timeout_ms: timeout.unwrap_or(30000),
                state,
            }
        }

        AutomateAction::Status => AutomateRequest::Status,
    };

    let request = Request::Automate(automate_request);
    let response = client.send(&request, timeout_ms).await?;
    output.print_response(&response);

    if !response.success {
        std::process::exit(1);
    }

    Ok(())
}
