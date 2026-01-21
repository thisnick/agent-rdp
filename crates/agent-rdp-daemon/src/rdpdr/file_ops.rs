//! File operations for RDPDR: create, read, write, close.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};

use ironrdp::pdu::PduResult;
use ironrdp_rdpdr::pdu::efs::*;
use ironrdp_rdpdr::pdu::RdpdrPdu;
use ironrdp_svc::SvcMessage;
use tracing::{debug, warn};

use super::MultiDriveBackend;

/// Handle device write request.
pub fn write_device(
    backend: &mut MultiDriveBackend,
    req_inner: DeviceWriteRequest,
) -> PduResult<Vec<SvcMessage>> {
    process_dependent_file(
        backend,
        req_inner.device_io_request,
        |request| {
            let res = RdpdrPdu::DeviceWriteResponse(DeviceWriteResponse {
                device_io_reply: DeviceIoResponse::new(request, NtStatus::NO_SUCH_FILE),
                length: 0u32,
            });
            Ok(vec![SvcMessage::from(res)])
        },
        |file, request| {
            match write_inner(file, req_inner.offset, &req_inner.write_data) {
                Ok(length) => {
                    if length == req_inner.write_data.len() {
                        Ok(vec![SvcMessage::from(RdpdrPdu::DeviceWriteResponse(
                            DeviceWriteResponse {
                                device_io_reply: DeviceIoResponse::new(request, NtStatus::SUCCESS),
                                length: u32::try_from(req_inner.write_data.len()).unwrap(),
                            },
                        ))])
                    } else {
                        warn!(
                            "Written content len:{} is not equal to {}",
                            length,
                            req_inner.write_data.len()
                        );
                        let res = RdpdrPdu::DeviceWriteResponse(DeviceWriteResponse {
                            device_io_reply: DeviceIoResponse::new(request, NtStatus::UNSUCCESSFUL),
                            length: 0u32,
                        });
                        Ok(vec![SvcMessage::from(res)])
                    }
                }
                Err(error) => {
                    warn!(%error, "Write error");
                    let res = RdpdrPdu::DeviceWriteResponse(DeviceWriteResponse {
                        device_io_reply: DeviceIoResponse::new(request, NtStatus::UNSUCCESSFUL),
                        length: 0u32,
                    });
                    Ok(vec![SvcMessage::from(res)])
                }
            }
        },
    )
}

fn write_inner(file: &mut File, offset: u64, write_data: &[u8]) -> std::io::Result<usize> {
    file.seek(SeekFrom::Start(offset))?;
    let length = file.write(write_data)?;
    file.flush()?;
    Ok(length)
}

/// Handle device read request.
pub fn read_device(
    backend: &mut MultiDriveBackend,
    req_inner: DeviceReadRequest,
) -> PduResult<Vec<SvcMessage>> {
    process_dependent_file(
        backend,
        req_inner.device_io_request,
        |request| {
            let res = RdpdrPdu::DeviceReadResponse(DeviceReadResponse {
                device_io_reply: DeviceIoResponse::new(request, NtStatus::NO_SUCH_FILE),
                read_data: Vec::new(),
            });
            Ok(vec![SvcMessage::from(res)])
        },
        |file, request| {
            match read_inner(file, req_inner.offset, usize::try_from(req_inner.length).unwrap()) {
                Ok(buf) => {
                    let res = RdpdrPdu::DeviceReadResponse(DeviceReadResponse {
                        device_io_reply: DeviceIoResponse::new(request, NtStatus::SUCCESS),
                        read_data: buf,
                    });
                    Ok(vec![SvcMessage::from(res)])
                }
                Err(error) => {
                    warn!(?error, "Read error");
                    let res = RdpdrPdu::DeviceReadResponse(DeviceReadResponse {
                        device_io_reply: DeviceIoResponse::new(request, NtStatus::UNSUCCESSFUL),
                        read_data: Vec::new(),
                    });
                    Ok(vec![SvcMessage::from(res)])
                }
            }
        },
    )
}

fn read_inner(file: &mut File, offset: u64, length: usize) -> std::io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0; length];
    let length = file.read(&mut buf)?;
    buf.resize(length, 0u8);
    Ok(buf)
}

/// Handle device close request.
pub fn close_device(
    backend: &mut MultiDriveBackend,
    req_inner: DeviceCloseRequest,
) -> PduResult<Vec<SvcMessage>> {
    let file_id = req_inner.device_io_request.file_id;

    // Sync file to disk before closing to ensure visibility for subsequent operations
    // This is critical for RDPDR where Windows may immediately try to access/rename the file
    if let Some(Some(file)) = backend.file_map.get(&file_id) {
        if let Err(e) = file.sync_all() {
            warn!("Failed to sync file on close: {:?}", e);
        }
    }

    // Check if file was marked for deletion
    let should_delete = backend.delete_on_close.remove(&file_id).unwrap_or(false);

    // Get path before removing from maps (needed for deletion)
    let file_path = backend.file_path_map.get(&file_id).cloned();

    // Clean up all mappings
    backend.file_map.remove(&file_id);
    backend.file_path_map.remove(&file_id);
    backend.file_device_map.remove(&file_id);
    backend.file_dir_map.remove(&file_id);

    // Perform actual deletion after closing handle and cleaning up maps
    if should_delete {
        if let Some(path) = file_path {
            debug!("Deleting file on close: {:?}", path);
            if let Err(e) = fs::remove_file(&path) {
                // Try removing as directory if file removal fails
                if let Err(e2) = fs::remove_dir(&path) {
                    warn!("Failed to delete {:?}: file={:?}, dir={:?}", path, e, e2);
                }
            }
        }
    }

    let res = RdpdrPdu::DeviceCloseResponse(DeviceCloseResponse {
        device_io_response: DeviceIoResponse::new(req_inner.device_io_request, NtStatus::SUCCESS),
    });
    Ok(vec![SvcMessage::from(res)])
}

/// Handle device create request (open/create file or directory).
pub fn create_drive(
    backend: &mut MultiDriveBackend,
    req_inner: DeviceCreateRequest,
) -> PduResult<Vec<SvcMessage>> {
    let file_id = backend.next_file_id();

    let device_id = req_inner.device_io_request.device_id;
    debug!(
        "create_drive request: device_id={}, req_path={:?}, registered_devices={:?}",
        device_id,
        req_inner.path,
        backend.drive_paths_keys()
    );

    // Get base path for this device
    let base_path = match backend.get_base_path(device_id) {
        Some(p) => p.clone(),
        None => {
            warn!(
                "No base path for device {}. Registered: {:?}",
                device_id,
                backend.drive_paths_debug()
            );
            let io_response =
                DeviceIoResponse::new(req_inner.device_io_request, NtStatus::UNSUCCESSFUL);
            let res = RdpdrPdu::DeviceCreateResponse(DeviceCreateResponse {
                device_io_reply: io_response,
                file_id,
                information: Information::empty(),
            });
            return Ok(vec![SvcMessage::from(res)]);
        }
    };

    // Convert backslashes and strip leading slashes to prevent join from replacing base path
    let req_path = req_inner.path.replace('\\', "/");
    let req_path = req_path.trim_start_matches('/');
    let path = if req_path.is_empty() {
        base_path.clone()
    } else {
        base_path.join(req_path)
    };
    debug!("create_drive resolved: base={:?}, full_path={:?}", base_path, path);

    // First process directory
    match fs::metadata(&path) {
        Ok(meta) => {
            if meta.is_dir() {
                if req_inner.create_disposition == CreateDisposition::FILE_CREATE {
                    warn!("Attempt to create directory, but it exists");
                    let io_response =
                        DeviceIoResponse::new(req_inner.device_io_request, NtStatus::UNSUCCESSFUL);
                    let res = RdpdrPdu::DeviceCreateResponse(DeviceCreateResponse {
                        device_io_reply: io_response,
                        file_id,
                        information: Information::empty(),
                    });
                    return Ok(vec![SvcMessage::from(res)]);
                }
                if req_inner.create_options.bits() & CreateOptions::FILE_NON_DIRECTORY_FILE.bits()
                    != 0
                {
                    warn!("Attempt to create a file, but it is a directory");
                    let io_response =
                        DeviceIoResponse::new(req_inner.device_io_request, NtStatus::UNSUCCESSFUL);
                    let res = RdpdrPdu::DeviceCreateResponse(DeviceCreateResponse {
                        device_io_reply: io_response,
                        file_id,
                        information: Information::empty(),
                    });
                    return Ok(vec![SvcMessage::from(res)]);
                }
                // Success case: opening existing directory
                debug!("Opening existing directory file_id:{}, path:{:?}", file_id, path);
                backend.insert_directory(file_id, device_id, path.clone());
                return make_create_drive_resp(
                    req_inner.device_io_request,
                    req_inner.create_disposition,
                    file_id,
                );
            } else if req_inner.create_options.bits() & CreateOptions::FILE_DIRECTORY_FILE.bits()
                != 0
            {
                warn!("Attempt to create a directory, but it is a file");
                let io_response =
                    DeviceIoResponse::new(req_inner.device_io_request, NtStatus::NOT_A_DIRECTORY);
                let res = RdpdrPdu::DeviceCreateResponse(DeviceCreateResponse {
                    device_io_reply: io_response,
                    file_id,
                    information: Information::empty(),
                });
                return Ok(vec![SvcMessage::from(res)]);
            }
        }
        Err(_) => {
            if req_inner.create_options.bits() & CreateOptions::FILE_DIRECTORY_FILE.bits() != 0 {
                if (req_inner.create_disposition == CreateDisposition::FILE_CREATE
                    || req_inner.create_disposition == CreateDisposition::FILE_OPEN_IF)
                    && fs::create_dir_all(&path).is_ok()
                {
                    // Successfully created directory
                    debug!("Created directory file_id:{}, path:{:?}", file_id, path);
                    backend.insert_directory(file_id, device_id, path.clone());
                    return make_create_drive_resp(
                        req_inner.device_io_request,
                        req_inner.create_disposition,
                        file_id,
                    );
                }
                let io_response =
                    DeviceIoResponse::new(req_inner.device_io_request, NtStatus::UNSUCCESSFUL);
                let res = RdpdrPdu::DeviceCreateResponse(DeviceCreateResponse {
                    device_io_reply: io_response,
                    file_id,
                    information: Information::empty(),
                });
                return Ok(vec![SvcMessage::from(res)]);
            }
        }
    }

    debug!(
        "create_drive file: disposition={:?}, options={:?}, path={:?}",
        req_inner.create_disposition,
        req_inner.create_options,
        path
    );

    let mut fs_opts = fs::OpenOptions::new();
    match req_inner.create_disposition {
        CreateDisposition::FILE_OPEN_IF => {
            fs_opts.create(true).write(true).read(true);
        }
        CreateDisposition::FILE_CREATE => {
            fs_opts.create_new(true).write(true).read(true);
        }
        CreateDisposition::FILE_SUPERSEDE => {
            fs_opts.create(true).write(true).append(true).read(true);
        }
        CreateDisposition::FILE_OPEN => {
            fs_opts.read(true);
        }
        CreateDisposition::FILE_OVERWRITE => {
            fs_opts.write(true).truncate(true).read(true);
        }
        CreateDisposition::FILE_OVERWRITE_IF => {
            fs_opts.write(true).truncate(true).create(true).read(true);
        }
        _ => {}
    }

    match fs_opts.open(&path) {
        Ok(file) => {
            debug!("create drive file_id:{}, device_id:{}, path:{:?}", file_id, device_id, path);
            backend.insert_file(file_id, device_id, path.clone(), file);
            make_create_drive_resp(
                req_inner.device_io_request,
                req_inner.create_disposition,
                file_id,
            )
        }
        Err(error) => {
            warn!(?error, "Open file error for path:{:?}", path);
            let io_response =
                DeviceIoResponse::new(req_inner.device_io_request, NtStatus::UNSUCCESSFUL);
            let res = RdpdrPdu::DeviceCreateResponse(DeviceCreateResponse {
                device_io_reply: io_response,
                file_id,
                information: Information::empty(),
            });
            Ok(vec![SvcMessage::from(res)])
        }
    }
}

fn make_create_drive_resp(
    device_io_request: DeviceIoRequest,
    create_disposition: CreateDisposition,
    file_id: u32,
) -> PduResult<Vec<SvcMessage>> {
    let io_response = DeviceIoResponse::new(device_io_request, NtStatus::SUCCESS);
    let information = match create_disposition {
        CreateDisposition::FILE_CREATE
        | CreateDisposition::FILE_SUPERSEDE
        | CreateDisposition::FILE_OPEN
        | CreateDisposition::FILE_OVERWRITE => Information::FILE_SUPERSEDED,
        CreateDisposition::FILE_OPEN_IF => Information::FILE_OPENED,
        CreateDisposition::FILE_OVERWRITE_IF => Information::FILE_OVERWRITTEN,
        _ => Information::empty(),
    };
    let res = RdpdrPdu::DeviceCreateResponse(DeviceCreateResponse {
        device_io_reply: io_response,
        file_id,
        information,
    });
    Ok(vec![SvcMessage::from(res)])
}

/// Helper to process operations that require an open file handle.
pub fn process_dependent_file(
    backend: &mut MultiDriveBackend,
    request: DeviceIoRequest,
    error_fx: impl Fn(DeviceIoRequest) -> PduResult<Vec<SvcMessage>>,
    fx: impl Fn(&mut File, DeviceIoRequest) -> PduResult<Vec<SvcMessage>>,
) -> PduResult<Vec<SvcMessage>> {
    match backend.file_map.get_mut(&request.file_id) {
        Some(Some(file)) => fx(file, request),
        _ => error_fx(request), // None or Some(None) for directories
    }
}
