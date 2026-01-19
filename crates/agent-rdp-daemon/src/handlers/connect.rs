//! Connection handler.

use std::sync::Arc;

use agent_rdp_protocol::{ConnectRequest, ErrorCode, Response, ResponseData};
use tokio::sync::Mutex;
use tracing::info;

use crate::rdp_session::{DisconnectNotify, RdpConfig, RdpSession};

/// Handle a connect request.
pub async fn handle(
    rdp_session: &Arc<Mutex<Option<RdpSession>>>,
    params: ConnectRequest,
    disconnect_notify: DisconnectNotify,
) -> Response {
    let mut session = rdp_session.lock().await;

    // Auto-disconnect if already connected (handles stale/dropped connections)
    if let Some(old_session) = session.take() {
        info!("Disconnecting existing session before new connection");
        if let Err(e) = old_session.disconnect().await {
            // Log but don't fail - the old connection might already be dead
            info!("Previous disconnect returned error (may be expected): {}", e);
        }
    }

    // Build configuration
    let config = RdpConfig {
        host: params.host.clone(),
        port: params.port,
        username: params.username,
        password: params.password,
        domain: params.domain,
        width: params.width,
        height: params.height,
        drives: params.drives,
    };

    // Attempt connection
    match RdpSession::connect(config, Some(disconnect_notify)).await {
        Ok(rdp) => {
            let host = rdp.host();
            let width = rdp.width();
            let height = rdp.height();

            *session = Some(rdp);

            info!("Connected to {} ({}x{})", host, width, height);

            Response::success(ResponseData::Connected {
                host,
                width,
                height,
            })
        }
        Err(e) => {
            let code = match &e {
                crate::rdp_session::RdpError::AuthenticationFailed => ErrorCode::AuthenticationFailed,
                _ => ErrorCode::ConnectionFailed,
            };

            Response::error(code, e.to_string())
        }
    }
}

/// Handle a disconnect request.
pub async fn handle_disconnect(
    rdp_session: &Arc<Mutex<Option<RdpSession>>>,
) -> Response {
    let mut session = rdp_session.lock().await;

    match session.take() {
        Some(rdp) => {
            if let Err(e) = rdp.disconnect().await {
                return Response::error(ErrorCode::InternalError, format!("Disconnect error: {}", e));
            }
            Response::ok()
        }
        None => {
            Response::error(ErrorCode::NotConnected, "Not connected to an RDP server")
        }
    }
}
