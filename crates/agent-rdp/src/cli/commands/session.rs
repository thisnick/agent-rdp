//! Session management command implementation.

use agent_rdp_protocol::{Request, ResponseData, SessionSummary, ConnectionState};

use crate::cli::{SessionAction, SessionArgs};
use crate::output::Output;
use crate::session_manager::SessionManager;

pub async fn run(
    session: &str,
    args: SessionArgs,
    output: &Output,
    timeout_ms: u64,
) -> anyhow::Result<()> {
    match args.action {
        SessionAction::List => {
            list_sessions(output).await
        }
        SessionAction::Info => {
            session_info(session, output, timeout_ms).await
        }
        SessionAction::Daemon => {
            run_daemon(session).await
        }
    }
}

async fn list_sessions(output: &Output) -> anyhow::Result<()> {
    let sessions = SessionManager::list_sessions();

    let mut summaries = Vec::new();
    for session_name in sessions {
        let manager = SessionManager::new(session_name.clone());

        let state = if manager.is_daemon_alive() {
            // Try to get session info
            if let Ok(mut client) = crate::ipc_client::try_connect(
                &manager.socket_path(),
                1,
                100,
            ).await {
                if let Ok(response) = client.send(&Request::SessionInfo, 5000).await {
                    if let Some(ResponseData::SessionInfo(info)) = response.data {
                        summaries.push(SessionSummary {
                            name: session_name,
                            state: info.state,
                            host: info.host,
                        });
                        continue;
                    }
                }
            }
            ConnectionState::Disconnected
        } else {
            ConnectionState::Disconnected
        };

        summaries.push(SessionSummary {
            name: session_name,
            state,
            host: None,
        });
    }

    let response = agent_rdp_protocol::Response::success(ResponseData::SessionList {
        sessions: summaries,
    });

    output.print_response(&response);
    Ok(())
}

async fn session_info(session: &str, output: &Output, timeout_ms: u64) -> anyhow::Result<()> {
    let manager = SessionManager::new(session.to_string());

    if !manager.is_daemon_alive() {
        output.print_error("daemon_not_running", "No daemon running for this session");
        std::process::exit(1);
    }

    let mut client = manager.ensure_daemon().await?;
    let response = client.send(&Request::SessionInfo, timeout_ms).await?;
    output.print_response(&response);

    Ok(())
}

/// Run as the background daemon (called by session manager).
async fn run_daemon(session: &str) -> anyhow::Result<()> {
    agent_rdp_daemon::run_server(session).await
}
