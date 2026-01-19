//! Drive mapping handler.
//!
//! Drives are configured at connect time using the --drive flag.

use std::sync::Arc;

use agent_rdp_protocol::{DriveRequest, ErrorCode, MappedDrive, Response, ResponseData};
use tokio::sync::Mutex;

use crate::rdp_session::RdpSession;

/// Handle a drive request.
pub async fn handle(
    rdp_session: &Arc<Mutex<Option<RdpSession>>>,
    action: DriveRequest,
) -> Response {
    let session = rdp_session.lock().await;

    let rdp = match session.as_ref() {
        Some(rdp) => rdp,
        None => {
            return Response::error(ErrorCode::NotConnected, "Not connected to an RDP server");
        }
    };

    match action {
        DriveRequest::List => {
            let drives = rdp
                .get_drives()
                .into_iter()
                .map(|d| MappedDrive {
                    name: d.name,
                    path: d.path,
                })
                .collect();
            Response::success(ResponseData::DriveList { drives })
        }
    }
}
