//! Set operations for RDPDR: set_information (rename, delete disposition, etc.).

use std::fs;

use ironrdp::pdu::{encode_err, PduResult};
use ironrdp_rdpdr::pdu::efs::*;
use ironrdp_rdpdr::pdu::RdpdrPdu;
use ironrdp_svc::SvcMessage;
use tracing::{debug, warn};

use super::MultiDriveBackend;

/// Handle set information request (rename, delete, truncate, etc.).
pub fn set_information(
    backend: &mut MultiDriveBackend,
    req_inner: ServerDriveSetInformationRequest,
) -> PduResult<Vec<SvcMessage>> {
    let device_id = req_inner.device_io_request.device_id;
    let file_id = req_inner.device_io_request.file_id;

    debug!(
        "set_information: device_id={}, file_id={}, buffer_type={:?}",
        device_id,
        file_id,
        std::mem::discriminant(&req_inner.set_buffer)
    );

    match backend.file_path_map.get(&file_id) {
        Some(file_path) => {
            match &req_inner.set_buffer {
                FileInformationClass::Rename(info) => {
                    debug!(
                        "set_information RENAME: file_id={}, from={:?}, to={}",
                        file_id, file_path, info.file_name
                    );
                    // Get the base path for this device to build the new path
                    let base_path = match backend.get_base_path(device_id) {
                        Some(p) => p.clone(),
                        None => {
                            warn!("No base path for device {}", device_id);
                            let res = RdpdrPdu::ClientDriveSetInformationResponse(
                                ClientDriveSetInformationResponse::new(
                                    &req_inner,
                                    NtStatus::UNSUCCESSFUL,
                                )
                                .map_err(|e| encode_err!(e))?,
                            );
                            return Ok(vec![SvcMessage::from(res)]);
                        }
                    };

                    let new_path = info.file_name.replace('\\', "/");
                    let new_path = new_path.trim_start_matches('/');
                    let to = base_path.join(new_path);

                    if let Err(error) = fs::rename(file_path, &to) {
                        warn!(
                            "set_information RENAME FAILED: from={:?}, to={:?}, error={:?}",
                            file_path, to, error
                        );
                        let res = RdpdrPdu::ClientDriveSetInformationResponse(
                            ClientDriveSetInformationResponse::new(
                                &req_inner,
                                NtStatus::UNSUCCESSFUL,
                            )
                            .map_err(|e| encode_err!(e))?,
                        );
                        return Ok(vec![SvcMessage::from(res)]);
                    } else {
                        debug!(
                            "set_information RENAME SUCCESS: from={:?}, to={:?}",
                            file_path, to
                        );
                        // CRITICAL: Update file_path_map to point to the new path
                        // Otherwise subsequent queries will fail with "file not found"
                        backend.file_path_map.insert(file_id, to);
                    }
                }
                FileInformationClass::Allocation(_) => {
                    // nothing to do
                }
                FileInformationClass::Disposition(info) => {
                    // Mark file for deletion on close (per Windows semantics)
                    // Don't delete immediately - that corrupts the state if file is still in use
                    let should_delete = info.delete_pending != 0;
                    debug!(
                        "set_information DISPOSITION: file_id={}, delete_on_close={}",
                        file_id, should_delete
                    );
                    if should_delete {
                        backend.delete_on_close.insert(file_id, true);
                    } else {
                        // Can unmark if delete_pending is false
                        backend.delete_on_close.remove(&file_id);
                    }
                }
                FileInformationClass::EndOfFile(info) => {
                    if let Some(Some(file)) =
                        backend.file_map.get(&req_inner.device_io_request.file_id)
                    {
                        if let Err(error) = file.set_len(info.end_of_file as u64) {
                            warn!(%error, "Failed to set end of file");
                            let res = RdpdrPdu::ClientDriveSetInformationResponse(
                                ClientDriveSetInformationResponse::new(
                                    &req_inner,
                                    NtStatus::UNSUCCESSFUL,
                                )
                                .map_err(|e| encode_err!(e))?,
                            );
                            return Ok(vec![SvcMessage::from(res)]);
                        }
                    } else {
                        warn!("no such file or is a directory");
                        let res = RdpdrPdu::ClientDriveSetInformationResponse(
                            ClientDriveSetInformationResponse::new(
                                &req_inner,
                                NtStatus::NO_SUCH_FILE,
                            )
                            .map_err(|e| encode_err!(e))?,
                        );
                        return Ok(vec![SvcMessage::from(res)]);
                    }
                }
                _ => {
                    // TODO: handle other cases
                }
            }
        }
        None => {
            warn!("no such file");
            let res = RdpdrPdu::ClientDriveSetInformationResponse(
                ClientDriveSetInformationResponse::new(&req_inner, NtStatus::NO_SUCH_FILE)
                    .map_err(|e| encode_err!(e))?,
            );
            return Ok(vec![SvcMessage::from(res)]);
        }
    }
    Ok(vec![SvcMessage::from(
        RdpdrPdu::ClientDriveSetInformationResponse(
            ClientDriveSetInformationResponse::new(&req_inner, NtStatus::SUCCESS)
                .map_err(|e| encode_err!(e))?,
        ),
    )])
}
