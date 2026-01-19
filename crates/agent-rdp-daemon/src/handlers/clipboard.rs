//! Clipboard handler.
//!
//! Uses CLIPRDR to sync clipboard with remote Windows machine.

use std::sync::Arc;

use agent_rdp_protocol::{ClipboardRequest, ErrorCode, Response, ResponseData, SetFileSource};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
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
        ClipboardRequest::GetText => {
            match rdp.clipboard_get().await {
                Ok(Some(text)) => Response::success(ResponseData::Clipboard { text }),
                Ok(None) => Response::success(ResponseData::Clipboard { text: String::new() }),
                Err(e) => Response::error(ErrorCode::ClipboardError, format!("Failed to get clipboard: {}", e)),
            }
        }

        ClipboardRequest::SetText { text } => {
            match rdp.clipboard_set(text).await {
                Ok(()) => Response::ok(),
                Err(e) => Response::error(ErrorCode::ClipboardError, format!("Failed to set clipboard: {}", e)),
            }
        }

        ClipboardRequest::GetFile => {
            match rdp.clipboard_get_file().await {
                Ok(Some(file)) => {
                    let data = BASE64.encode(&file.data);
                    Response::success(ResponseData::ClipboardFile {
                        name: file.name,
                        size: file.data.len() as u64,
                        data,
                    })
                }
                Ok(None) => Response::error(ErrorCode::ClipboardError, "No file on clipboard"),
                Err(e) => Response::error(ErrorCode::ClipboardError, format!("Failed to get file: {}", e)),
            }
        }

        ClipboardRequest::SetFile(SetFileSource::Path { path }) => {
            // Daemon stores path, reads on-demand when server requests
            match rdp.clipboard_set_file_path(path).await {
                Ok(()) => Response::ok(),
                Err(e) => Response::error(ErrorCode::ClipboardError, format!("Failed to set file: {}", e)),
            }
        }

        ClipboardRequest::SetFile(SetFileSource::Data { name, data }) => {
            // From stdin - decode and store in memory
            let bytes = match BASE64.decode(&data) {
                Ok(b) => b,
                Err(e) => return Response::error(ErrorCode::InvalidRequest, format!("Invalid base64: {}", e)),
            };
            match rdp.clipboard_set_file_data(name, bytes).await {
                Ok(()) => Response::ok(),
                Err(e) => Response::error(ErrorCode::ClipboardError, format!("Failed to set file: {}", e)),
            }
        }
    }
}
