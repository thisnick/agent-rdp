//! Clipboard handler.
//!
//! Uses CLIPRDR to sync clipboard with remote Windows machine.

use std::sync::Arc;

use agent_rdp_protocol::{ClipboardRequest, ErrorCode, Response, ResponseData};
use tokio::sync::Mutex;

use crate::rdp_session::RdpSession;

/// Handle a clipboard request using the RDP session's CLIPRDR integration.
pub async fn handle(
    rdp_session: &Arc<Mutex<Option<RdpSession>>>,
    action: ClipboardRequest,
) -> Response {
    let session = rdp_session.lock().await;

    let Some(ref rdp) = *session else {
        return Response::error(ErrorCode::NotConnected, "Not connected to an RDP server");
    };

    match action {
        ClipboardRequest::Get => {
            match rdp.clipboard_get().await {
                Ok(Some(text)) => Response::success(ResponseData::Clipboard { text }),
                Ok(None) => Response::success(ResponseData::Clipboard { text: String::new() }),
                Err(e) => Response::error(ErrorCode::ClipboardError, format!("Failed to get clipboard: {}", e)),
            }
        }

        ClipboardRequest::Set { text } => {
            match rdp.clipboard_set(text).await {
                Ok(()) => Response::ok(),
                Err(e) => Response::error(ErrorCode::ClipboardError, format!("Failed to set clipboard: {}", e)),
            }
        }
    }
}
