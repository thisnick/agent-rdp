//! Drive mapping handler.
//!
//! Drive mapping is configured at connect time using the --drive flag.
//! Dynamic drive mapping (post-connection) is not supported.

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
        DriveRequest::Map { path: _, name: _ } => {
            // Dynamic drive mapping is not supported - drives must be configured at connect time
            Response::error(
                ErrorCode::NotSupported,
                "Dynamic drive mapping is not supported. Use --drive flag with the connect command instead. \
                 Example: agent-rdp connect --host <ip> -u <user> -p <pass> --drive /path:DriveName",
            )
        }

        DriveRequest::Unmap { name: _ } => {
            // Dynamic drive unmapping is not supported
            Response::error(
                ErrorCode::NotSupported,
                "Dynamic drive unmapping is not supported. Drives are configured at connect time. \
                 Disconnect and reconnect without the drive to remove it.",
            )
        }

        DriveRequest::List => {
            // Return the drives that were mapped at connect time
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
