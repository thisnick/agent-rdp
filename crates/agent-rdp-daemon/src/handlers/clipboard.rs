//! Clipboard handler.
//!
//! Note: Full clipboard support requires ironrdp-cliprdr which implements
//! the RDP clipboard virtual channel. This is a placeholder implementation.

use std::sync::Arc;

use agent_rdp_protocol::{ClipboardRequest, ErrorCode, Response};
use tokio::sync::Mutex;

use crate::rdp_session::RdpSession;

/// Handle a clipboard request.
pub async fn handle(
    rdp_session: &Arc<Mutex<Option<RdpSession>>>,
    action: ClipboardRequest,
) -> Response {
    let session = rdp_session.lock().await;

    if session.is_none() {
        return Response::error(ErrorCode::NotConnected, "Not connected to an RDP server");
    }

    match action {
        ClipboardRequest::Get => {
            // TODO: Implement clipboard get via CLIPRDR virtual channel
            // This requires setting up the clipboard virtual channel during connection
            // and handling the format list/data request PDUs
            Response::error(
                ErrorCode::NotSupported,
                "Clipboard get not yet implemented. Requires CLIPRDR virtual channel.",
            )
        }

        ClipboardRequest::Set { text: _ } => {
            // TODO: Implement clipboard set via CLIPRDR virtual channel
            // This requires sending a format list PDU and responding to format data requests
            Response::error(
                ErrorCode::NotSupported,
                "Clipboard set not yet implemented. Requires CLIPRDR virtual channel.",
            )
        }
    }
}
