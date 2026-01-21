//! Automate command implementation for Windows UI Automation.

use agent_rdp_protocol::{
    AutomateRequest, AutomationMouseButton, AutomationScrollDirection, Request, SnapshotScope,
    WaitState, WindowAction,
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
            refs,
            scope,
            window,
            max_depth,
        } => {
            let scope = if scope.as_deref() == Some("window") {
                SnapshotScope::Window
            } else {
                SnapshotScope::Desktop
            };
            AutomateRequest::Snapshot {
                include_refs: refs,
                scope,
                window,
                max_depth: max_depth.unwrap_or(10),
            }
        }

        AutomateAction::Get { selector, property } => AutomateRequest::Get { selector, property },

        AutomateAction::Focus { selector } => AutomateRequest::Focus { selector },

        AutomateAction::Click {
            selector,
            button,
            double,
        } => {
            let button = match button.as_deref() {
                Some("right") => AutomationMouseButton::Right,
                Some("middle") => AutomationMouseButton::Middle,
                _ => AutomationMouseButton::Left,
            };
            AutomateRequest::Click {
                selector,
                button,
                double,
            }
        }

        AutomateAction::DoubleClick { selector } => AutomateRequest::DoubleClick { selector },

        AutomateAction::RightClick { selector } => AutomateRequest::RightClick { selector },

        AutomateAction::Fill { selector, text } => AutomateRequest::Fill { selector, text },

        AutomateAction::Clear { selector } => AutomateRequest::Clear { selector },

        AutomateAction::Select { selector, item } => AutomateRequest::Select { selector, item },

        AutomateAction::Check { selector, uncheck } => AutomateRequest::Check { selector, uncheck },

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
        } => AutomateRequest::Run {
            command,
            args: cmd_args,
            wait,
            hidden,
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
