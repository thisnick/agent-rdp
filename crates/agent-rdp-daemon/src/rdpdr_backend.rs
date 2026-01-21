//! Cross-platform RDPDR backend with multi-drive support.
//!
//! This module provides drive redirection for RDP sessions, supporting multiple
//! drives mapped to different local directories.

use std::collections::HashMap;
use std::fs::{self, File, ReadDir};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use ironrdp::pdu::{encode_err, PduResult};
use ironrdp_rdpdr::pdu::efs::*;
use ironrdp_rdpdr::pdu::esc::{ScardCall, ScardIoCtlCode};
use ironrdp_rdpdr::pdu::RdpdrPdu;
use ironrdp_rdpdr::RdpdrBackend;
use ironrdp_svc::{impl_as_any, SvcMessage};
use tracing::{debug, info, warn};

/// Cross-platform RDPDR backend supporting multiple drives.
#[derive(Debug, Default)]
pub struct MultiDriveBackend {
    /// Next file ID to assign.
    file_id: u32,
    /// Mapping from device_id to base path for each drive.
    drive_paths: HashMap<u32, PathBuf>,
    /// File handles - None for directories.
    file_map: HashMap<u32, Option<File>>,
    /// File ID to full path mapping.
    file_path_map: HashMap<u32, PathBuf>,
    /// File ID to device ID mapping (to look up base path for volume queries).
    file_device_map: HashMap<u32, u32>,
    /// Directory iteration state.
    file_dir_map: HashMap<u32, DirIterState>,
}

/// State for directory iteration.
#[derive(Debug)]
struct DirIterState {
    iter: ReadDir,
    #[allow(dead_code)]
    base_path: PathBuf,
}

impl MultiDriveBackend {
    /// Create a new multi-drive backend.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a drive mapping.
    ///
    /// The device_id should match the ID used when registering drives with Rdpdr::with_drives().
    pub fn add_drive(&mut self, device_id: u32, path: PathBuf) {
        info!("Adding drive mapping: device_id={} -> {:?}", device_id, path);
        self.drive_paths.insert(device_id, path);
    }

    /// Get the base path for a device.
    fn get_base_path(&self, device_id: u32) -> Option<&PathBuf> {
        self.drive_paths.get(&device_id)
    }

    /// Get the base path for a file (via file_id -> device_id lookup).
    fn get_base_path_for_file(&self, file_id: u32) -> Option<&PathBuf> {
        self.file_device_map
            .get(&file_id)
            .and_then(|device_id| self.drive_paths.get(device_id))
    }
}

impl_as_any!(MultiDriveBackend);

impl RdpdrBackend for MultiDriveBackend {
    fn handle_server_device_announce_response(
        &mut self,
        _pdu: ServerDeviceAnnounceResponse,
    ) -> PduResult<()> {
        Ok(())
    }

    fn handle_scard_call(
        &mut self,
        _req: DeviceControlRequest<ScardIoCtlCode>,
        _call: ScardCall,
    ) -> PduResult<()> {
        Ok(())
    }

    fn handle_drive_io_request(&mut self, req: ServerDriveIoRequest) -> PduResult<Vec<SvcMessage>> {
        debug!("handle_drive_io_request:{:?}", req);
        match req {
            ServerDriveIoRequest::DeviceWriteRequest(req_inner) => write_device(self, req_inner),
            ServerDriveIoRequest::ServerCreateDriveRequest(req_inner) => create_drive(self, req_inner),
            ServerDriveIoRequest::DeviceReadRequest(req_inner) => read_device(self, req_inner),
            ServerDriveIoRequest::DeviceCloseRequest(req_inner) => close_device(self, req_inner),
            ServerDriveIoRequest::ServerDriveNotifyChangeDirectoryRequest(_) => {
                // TODO: implement directory change notifications
                Ok(Vec::new())
            }
            ServerDriveIoRequest::ServerDriveQueryDirectoryRequest(req_inner) => {
                query_directory(self, req_inner)
            }
            ServerDriveIoRequest::ServerDriveQueryInformationRequest(req_inner) => {
                query_information(self, req_inner)
            }
            ServerDriveIoRequest::ServerDriveQueryVolumeInformationRequest(req_inner) => {
                query_volume_information(self, req_inner)
            }
            ServerDriveIoRequest::ServerDriveSetInformationRequest(req_inner) => {
                set_information(self, req_inner)
            }
            ServerDriveIoRequest::DeviceControlRequest(req_inner) => Ok(vec![SvcMessage::from(
                RdpdrPdu::DeviceControlResponse(DeviceControlResponse {
                    device_io_reply: DeviceIoResponse::new(req_inner.header, NtStatus::SUCCESS),
                    output_buffer: None,
                }),
            )]),
            ServerDriveIoRequest::ServerDriveLockControlRequest(_) => {
                // TODO: implement file locking
                Ok(Vec::new())
            }
        }
    }
}

fn write_device(
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

fn read_device(
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

fn close_device(
    backend: &mut MultiDriveBackend,
    req_inner: DeviceCloseRequest,
) -> PduResult<Vec<SvcMessage>> {
    backend.file_map.remove(&req_inner.device_io_request.file_id);
    backend.file_path_map.remove(&req_inner.device_io_request.file_id);
    backend.file_device_map.remove(&req_inner.device_io_request.file_id);
    backend.file_dir_map.remove(&req_inner.device_io_request.file_id);
    let res = RdpdrPdu::DeviceCloseResponse(DeviceCloseResponse {
        device_io_response: DeviceIoResponse::new(req_inner.device_io_request, NtStatus::SUCCESS),
    });
    Ok(vec![SvcMessage::from(res)])
}

fn query_information(
    backend: &mut MultiDriveBackend,
    req_inner: ServerDriveQueryInformationRequest,
) -> PduResult<Vec<SvcMessage>> {
    let file_id = req_inner.device_io_request.file_id;
    debug!(
        "query_information: file_id={}, class={:?}",
        file_id, req_inner.file_info_class_lvl
    );

    match backend.file_path_map.get(&file_id) {
        Some(path) => {
            debug!("query_information: file_id={} -> path={:?}", file_id, path);
            match fs::metadata(path) {
            Ok(meta) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                let file_attribute = get_file_attributes(&meta, name);

                if FileInformationClassLevel::FILE_BASIC_INFORMATION == req_inner.file_info_class_lvl
                {
                    let basic_info = FileBasicInformation {
                        creation_time: get_creation_time(&meta),
                        last_access_time: get_last_access_time(&meta),
                        last_write_time: get_last_write_time(&meta),
                        change_time: get_last_write_time(&meta),
                        file_attributes: file_attribute,
                    };
                    let res = RdpdrPdu::ClientDriveQueryInformationResponse(
                        ClientDriveQueryInformationResponse {
                            device_io_response: DeviceIoResponse::new(
                                req_inner.device_io_request,
                                NtStatus::SUCCESS,
                            ),
                            buffer: Some(FileInformationClass::Basic(basic_info)),
                        },
                    );
                    Ok(vec![SvcMessage::from(res)])
                } else if FileInformationClassLevel::FILE_STANDARD_INFORMATION
                    == req_inner.file_info_class_lvl
                {
                    let dir = if meta.is_dir() {
                        Boolean::True
                    } else {
                        Boolean::False
                    };
                    let standard_info = FileStandardInformation {
                        allocation_size: i64::try_from(meta.len()).unwrap_or(0),
                        end_of_file: i64::try_from(meta.len()).unwrap_or(0),
                        number_of_links: 1,
                        delete_pending: Boolean::False,
                        directory: dir,
                    };
                    let res = RdpdrPdu::ClientDriveQueryInformationResponse(
                        ClientDriveQueryInformationResponse {
                            device_io_response: DeviceIoResponse::new(
                                req_inner.device_io_request,
                                NtStatus::SUCCESS,
                            ),
                            buffer: Some(FileInformationClass::Standard(standard_info)),
                        },
                    );
                    Ok(vec![SvcMessage::from(res)])
                } else if FileInformationClassLevel::FILE_ATTRIBUTE_TAG_INFORMATION
                    == req_inner.file_info_class_lvl
                {
                    let info = FileAttributeTagInformation {
                        file_attributes: file_attribute,
                        reparse_tag: 0,
                    };
                    let res = RdpdrPdu::ClientDriveQueryInformationResponse(
                        ClientDriveQueryInformationResponse {
                            device_io_response: DeviceIoResponse::new(
                                req_inner.device_io_request,
                                NtStatus::SUCCESS,
                            ),
                            buffer: Some(FileInformationClass::AttributeTag(info)),
                        },
                    );
                    Ok(vec![SvcMessage::from(res)])
                } else {
                    warn!("unsupported file class");
                    let res = RdpdrPdu::ClientDriveQueryInformationResponse(
                        ClientDriveQueryInformationResponse {
                            device_io_response: DeviceIoResponse::new(
                                req_inner.device_io_request,
                                NtStatus::UNSUCCESSFUL,
                            ),
                            buffer: None,
                        },
                    );
                    Ok(vec![SvcMessage::from(res)])
                }
            }
            Err(error) => {
                warn!(
                    "query_information: Get file metadata error for file_id={}, path={:?}, error={:?}",
                    file_id, path, error
                );
                let res = RdpdrPdu::ClientDriveQueryInformationResponse(
                    ClientDriveQueryInformationResponse {
                        device_io_response: DeviceIoResponse::new(
                            req_inner.device_io_request,
                            NtStatus::UNSUCCESSFUL,
                        ),
                        buffer: None,
                    },
                );
                Ok(vec![SvcMessage::from(res)])
            }
            }
        }
        None => {
            warn!("query_information: no such file_id={} in file_path_map", file_id);
            let res = RdpdrPdu::ClientDriveQueryInformationResponse(
                ClientDriveQueryInformationResponse {
                    device_io_response: DeviceIoResponse::new(
                        req_inner.device_io_request,
                        NtStatus::NO_SUCH_FILE,
                    ),
                    buffer: None,
                },
            );
            Ok(vec![SvcMessage::from(res)])
        }
    }
}

fn query_volume_information(
    backend: &mut MultiDriveBackend,
    req_inner: ServerDriveQueryVolumeInformationRequest,
) -> PduResult<Vec<SvcMessage>> {
    match backend.file_path_map.get(&req_inner.device_io_request.file_id) {
        Some(path) => {
            // Get the base path for this file's device to query disk space
            let base_path = backend
                .get_base_path_for_file(req_inner.device_io_request.file_id)
                .cloned()
                .unwrap_or_else(|| path.clone());

            let (total_bytes, free_bytes) = match get_disk_space(&base_path) {
                Ok(info) => info,
                Err(e) => {
                    warn!(?e, "Failed to get disk space");
                    // Return sensible defaults
                    (100 * 1024 * 1024 * 1024u64, 50 * 1024 * 1024 * 1024u64)
                }
            };

            // Use 4KB allocation units (typical for NTFS)
            let bytes_per_sector = 512u32;
            let sectors_per_unit = 8u32;
            let bytes_per_unit = (bytes_per_sector * sectors_per_unit) as u64;
            let total_units = total_bytes / bytes_per_unit;
            let free_units = free_bytes / bytes_per_unit;

            if FileSystemInformationClassLevel::FILE_FS_FULL_SIZE_INFORMATION
                == req_inner.fs_info_class_lvl
            {
                let info = FileFsFullSizeInformation {
                    total_alloc_units: i64::try_from(total_units).unwrap_or(i64::MAX),
                    caller_available_alloc_units: i64::try_from(free_units).unwrap_or(i64::MAX),
                    actual_available_alloc_units: i64::try_from(free_units).unwrap_or(i64::MAX),
                    sectors_per_alloc_unit: sectors_per_unit,
                    bytes_per_sector: bytes_per_sector,
                };

                Ok(vec![SvcMessage::from(
                    RdpdrPdu::ClientDriveQueryVolumeInformationResponse(
                        ClientDriveQueryVolumeInformationResponse {
                            device_io_reply: DeviceIoResponse::new(
                                req_inner.device_io_request,
                                NtStatus::SUCCESS,
                            ),
                            buffer: Some(FileSystemInformationClass::FileFsFullSizeInformation(
                                info,
                            )),
                        },
                    ),
                )])
            } else if FileSystemInformationClassLevel::FILE_FS_ATTRIBUTE_INFORMATION
                == req_inner.fs_info_class_lvl
            {
                Ok(vec![SvcMessage::from(
                    RdpdrPdu::ClientDriveQueryVolumeInformationResponse(
                        ClientDriveQueryVolumeInformationResponse {
                            device_io_reply: DeviceIoResponse::new(
                                req_inner.device_io_request,
                                NtStatus::SUCCESS,
                            ),
                            buffer: Some(FileSystemInformationClass::FileFsAttributeInformation(
                                FileFsAttributeInformation {
                                    file_system_attributes:
                                        FileSystemAttributes::FILE_CASE_SENSITIVE_SEARCH
                                            | FileSystemAttributes::FILE_CASE_PRESERVED_NAMES
                                            | FileSystemAttributes::FILE_UNICODE_ON_DISK,
                                    max_component_name_len: 260,
                                    file_system_name: "NTFS".to_owned(),
                                },
                            )),
                        },
                    ),
                )])
            } else if FileSystemInformationClassLevel::FILE_FS_VOLUME_INFORMATION
                == req_inner.fs_info_class_lvl
            {
                let creation_time = fs::metadata(path)
                    .map(|m| get_creation_time(&m))
                    .unwrap_or(0);

                Ok(vec![SvcMessage::from(
                    RdpdrPdu::ClientDriveQueryVolumeInformationResponse(
                        ClientDriveQueryVolumeInformationResponse {
                            device_io_reply: DeviceIoResponse::new(
                                req_inner.device_io_request,
                                NtStatus::SUCCESS,
                            ),
                            buffer: Some(FileSystemInformationClass::FileFsVolumeInformation(
                                FileFsVolumeInformation {
                                    volume_creation_time: creation_time,
                                    volume_serial_number: 0x12345678,
                                    supports_objects: Boolean::False,
                                    volume_label: "AGENT_RDP".to_owned(),
                                },
                            )),
                        },
                    ),
                )])
            } else if FileSystemInformationClassLevel::FILE_FS_SIZE_INFORMATION
                == req_inner.fs_info_class_lvl
            {
                Ok(vec![SvcMessage::from(
                    RdpdrPdu::ClientDriveQueryVolumeInformationResponse(
                        ClientDriveQueryVolumeInformationResponse {
                            device_io_reply: DeviceIoResponse::new(
                                req_inner.device_io_request,
                                NtStatus::SUCCESS,
                            ),
                            buffer: Some(FileSystemInformationClass::FileFsSizeInformation(
                                FileFsSizeInformation {
                                    total_alloc_units: i64::try_from(total_units).unwrap_or(i64::MAX),
                                    available_alloc_units: i64::try_from(free_units)
                                        .unwrap_or(i64::MAX),
                                    sectors_per_alloc_unit: sectors_per_unit,
                                    bytes_per_sector: bytes_per_sector,
                                },
                            )),
                        },
                    ),
                )])
            } else {
                warn!("unsupported volume class");
                Ok(vec![SvcMessage::from(
                    RdpdrPdu::ClientDriveQueryVolumeInformationResponse(
                        ClientDriveQueryVolumeInformationResponse {
                            device_io_reply: DeviceIoResponse::new(
                                req_inner.device_io_request,
                                NtStatus::UNSUCCESSFUL,
                            ),
                            buffer: None,
                        },
                    ),
                )])
            }
        }
        None => {
            warn!("no such file");
            let res = RdpdrPdu::ClientDriveQueryInformationResponse(
                ClientDriveQueryInformationResponse {
                    device_io_response: DeviceIoResponse::new(
                        req_inner.device_io_request,
                        NtStatus::NO_SUCH_FILE,
                    ),
                    buffer: None,
                },
            );
            Ok(vec![SvcMessage::from(res)])
        }
    }
}

fn set_information(
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
                                ClientDriveSetInformationResponse::new(&req_inner, NtStatus::UNSUCCESSFUL)
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
                            ClientDriveSetInformationResponse::new(&req_inner, NtStatus::UNSUCCESSFUL)
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
                FileInformationClass::Disposition(_) => {
                    if let Err(error) = fs::remove_file(file_path) {
                        warn!(?error, "Remove file error");
                        let res = RdpdrPdu::ClientDriveSetInformationResponse(
                            ClientDriveSetInformationResponse::new(&req_inner, NtStatus::UNSUCCESSFUL)
                                .map_err(|e| encode_err!(e))?,
                        );
                        return Ok(vec![SvcMessage::from(res)]);
                    }
                }
                FileInformationClass::EndOfFile(info) => {
                    if let Some(Some(file)) = backend.file_map.get(&req_inner.device_io_request.file_id) {
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
                            ClientDriveSetInformationResponse::new(&req_inner, NtStatus::NO_SUCH_FILE)
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

fn get_file_attributes(meta: &fs::Metadata, file_name: &str) -> FileAttributes {
    let mut file_attribute = FileAttributes::empty();

    if meta.is_dir() {
        file_attribute |= FileAttributes::FILE_ATTRIBUTE_DIRECTORY;
    }
    if file_attribute.is_empty() {
        file_attribute |= FileAttributes::FILE_ATTRIBUTE_ARCHIVE;
    }

    // Check for hidden files (starting with .)
    if file_name.len() > 1 && file_name.starts_with('.') && !file_name.starts_with("..") {
        file_attribute |= FileAttributes::FILE_ATTRIBUTE_HIDDEN;
    }

    // Platform-specific file attributes
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        let win_attrs = meta.file_attributes();
        if win_attrs & 0x1 != 0 {
            file_attribute |= FileAttributes::FILE_ATTRIBUTE_READONLY;
        }
        if win_attrs & 0x2 != 0 {
            file_attribute |= FileAttributes::FILE_ATTRIBUTE_HIDDEN;
        }
        if win_attrs & 0x4 != 0 {
            file_attribute |= FileAttributes::FILE_ATTRIBUTE_SYSTEM;
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode();
        // Check if file is read-only (no write bits)
        if mode & 0o222 == 0 {
            file_attribute |= FileAttributes::FILE_ATTRIBUTE_READONLY;
        }
    }

    file_attribute
}

fn make_query_dir_resp(
    backend: &MultiDriveBackend,
    find_file_path: Option<PathBuf>,
    device_io_request: DeviceIoRequest,
    file_class: FileInformationClassLevel,
    initial_query: bool,
) -> PduResult<Vec<SvcMessage>> {
    let _ = backend; // Silence unused warning
    let not_found_status = if initial_query {
        NtStatus::NO_SUCH_FILE
    } else {
        NtStatus::NO_MORE_FILES
    };

    match find_file_path {
        None => Ok(vec![SvcMessage::from(
            RdpdrPdu::ClientDriveQueryDirectoryResponse(ClientDriveQueryDirectoryResponse {
                device_io_reply: DeviceIoResponse::new(device_io_request, not_found_status),
                buffer: None,
            }),
        )]),
        Some(file_full_path) => {
            let file_name = file_full_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            match fs::metadata(&file_full_path) {
                Ok(meta) => {
                    let file_attribute = get_file_attributes(&meta, file_name);
                    if file_class == FileInformationClassLevel::FILE_BOTH_DIRECTORY_INFORMATION {
                        let info = FileBothDirectoryInformation::new(
                            get_creation_time(&meta),
                            get_last_write_time(&meta),
                            get_last_access_time(&meta),
                            get_last_write_time(&meta),
                            i64::try_from(meta.len()).unwrap_or(0),
                            file_attribute,
                            file_name.to_owned(),
                        );
                        let info2 = FileInformationClass::BothDirectory(info);
                        Ok(vec![SvcMessage::from(
                            RdpdrPdu::ClientDriveQueryDirectoryResponse(
                                ClientDriveQueryDirectoryResponse {
                                    device_io_reply: DeviceIoResponse::new(
                                        device_io_request,
                                        NtStatus::SUCCESS,
                                    ),
                                    buffer: Some(info2),
                                },
                            ),
                        )])
                    } else {
                        warn!("unsupported file class for query directory");
                        Ok(vec![SvcMessage::from(
                            RdpdrPdu::ClientDriveQueryDirectoryResponse(
                                ClientDriveQueryDirectoryResponse {
                                    device_io_reply: DeviceIoResponse::new(
                                        device_io_request,
                                        NtStatus::NOT_SUPPORTED,
                                    ),
                                    buffer: None,
                                },
                            ),
                        )])
                    }
                }
                Err(error) => {
                    warn!(%error, "Get metadata error");
                    Ok(vec![SvcMessage::from(
                        RdpdrPdu::ClientDriveQueryDirectoryResponse(
                            ClientDriveQueryDirectoryResponse {
                                device_io_reply: DeviceIoResponse::new(
                                    device_io_request,
                                    not_found_status,
                                ),
                                buffer: None,
                            },
                        ),
                    )])
                }
            }
        }
    }
}

fn query_directory(
    backend: &mut MultiDriveBackend,
    req_inner: ServerDriveQueryDirectoryRequest,
) -> PduResult<Vec<SvcMessage>> {
    let device_id = req_inner.device_io_request.device_id;
    let file_id = req_inner.device_io_request.file_id;

    debug!(
        "query_directory: device_id={}, file_id={}, path={:?}, initial_query={}",
        device_id, file_id, req_inner.path, req_inner.initial_query
    );

    match backend.file_path_map.get(&file_id) {
        Some(_parent_path) => {
            debug!("query_directory: file_id={} -> parent_path={:?}", file_id, _parent_path);
            let mut find_file_path = None;

            // Get base path for this device
            let base_path = match backend.get_base_path(device_id) {
                Some(p) => p.clone(),
                None => {
                    warn!("No base path for device {}", device_id);
                    return Ok(vec![SvcMessage::from(
                        RdpdrPdu::ClientDriveQueryDirectoryResponse(ClientDriveQueryDirectoryResponse {
                            device_io_reply: DeviceIoResponse::new(
                                req_inner.device_io_request,
                                NtStatus::NO_SUCH_FILE,
                            ),
                            buffer: None,
                        }),
                    )]);
                }
            };

            if req_inner.initial_query > 0 {
                if req_inner.path.ends_with('*') {
                    // Wildcard query - list directory contents
                    let query_path = req_inner.path.replace('\\', "/");
                    let len = query_path.len();
                    // Strip the trailing * and any leading slashes
                    let dir_path_str = query_path[..len - 1].trim_start_matches('/');
                    let dir_path = if dir_path_str.is_empty() {
                        base_path.clone()
                    } else {
                        base_path.join(dir_path_str)
                    };

                    if let Ok(read_dir) = fs::read_dir(&dir_path) {
                        let mut iter = read_dir;
                        // Find first non-. and non-.. entry
                        while let Some(Ok(entry)) = iter.next() {
                            let name = entry.file_name();
                            let name_str = name.to_string_lossy();
                            if name_str != "." && name_str != ".." {
                                find_file_path = Some(entry.path());
                                break;
                            }
                        }
                        backend.file_dir_map.insert(
                            req_inner.device_io_request.file_id,
                            DirIterState {
                                iter,
                                base_path: dir_path,
                            },
                        );
                    }
                } else {
                    // Specific file query
                    let query_path = req_inner.path.replace('\\', "/");
                    let query_path = query_path.trim_start_matches('/');
                    let full_path = if query_path.is_empty() {
                        base_path.clone()
                    } else {
                        base_path.join(query_path)
                    };
                    find_file_path = Some(full_path);
                }

                make_query_dir_resp(
                    backend,
                    find_file_path,
                    req_inner.device_io_request,
                    req_inner.file_info_class_lvl,
                    true,
                )
            } else {
                // Continuation query
                if let Some(dir_state) = backend
                    .file_dir_map
                    .get_mut(&req_inner.device_io_request.file_id)
                {
                    if let Some(Ok(entry)) = dir_state.iter.next() {
                        find_file_path = Some(entry.path());
                    }
                }

                make_query_dir_resp(
                    backend,
                    find_file_path,
                    req_inner.device_io_request,
                    req_inner.file_info_class_lvl,
                    false,
                )
            }
        }
        None => {
            warn!(
                "query_directory: no file_id={} in file_path_map, registered_files={:?}",
                file_id,
                backend.file_path_map.keys().collect::<Vec<_>>()
            );
            Ok(vec![SvcMessage::from(
                RdpdrPdu::ClientDriveQueryDirectoryResponse(ClientDriveQueryDirectoryResponse {
                    device_io_reply: DeviceIoResponse::new(
                        req_inner.device_io_request,
                        NtStatus::NO_SUCH_FILE,
                    ),
                    buffer: None,
                }),
            )])
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

fn create_drive(
    backend: &mut MultiDriveBackend,
    req_inner: DeviceCreateRequest,
) -> PduResult<Vec<SvcMessage>> {
    let file_id = backend.file_id;
    backend.file_id += 1;

    let device_id = req_inner.device_io_request.device_id;
    debug!(
        "create_drive request: device_id={}, req_path={:?}, registered_devices={:?}",
        device_id,
        req_inner.path,
        backend.drive_paths.keys().collect::<Vec<_>>()
    );

    // Get base path for this device
    let base_path = match backend.get_base_path(device_id) {
        Some(p) => p.clone(),
        None => {
            warn!(
                "No base path for device {}. Registered: {:?}",
                device_id,
                backend.drive_paths
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
                backend.file_map.insert(file_id, None);
                backend.file_path_map.insert(file_id, path.clone());
                backend.file_device_map.insert(file_id, device_id);
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
                    backend.file_map.insert(file_id, None);
                    backend.file_path_map.insert(file_id, path.clone());
                    backend.file_device_map.insert(file_id, device_id);
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
            backend.file_map.insert(file_id, Some(file));
            backend.file_path_map.insert(file_id, path.clone());
            backend.file_device_map.insert(file_id, device_id);
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

fn process_dependent_file(
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

// ============ Platform-specific helpers ============

/// Get creation time as Windows FILETIME (100-nanosecond intervals since 1601).
#[cfg(windows)]
fn get_creation_time(meta: &fs::Metadata) -> i64 {
    use std::os::windows::fs::MetadataExt;
    meta.creation_time() as i64
}

#[cfg(unix)]
fn get_creation_time(meta: &fs::Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    // Unix doesn't have creation time, use ctime (status change time)
    // Convert Unix timestamp to Windows FILETIME
    unix_to_filetime(meta.ctime())
}

/// Get last access time as Windows FILETIME.
#[cfg(windows)]
fn get_last_access_time(meta: &fs::Metadata) -> i64 {
    use std::os::windows::fs::MetadataExt;
    meta.last_access_time() as i64
}

#[cfg(unix)]
fn get_last_access_time(meta: &fs::Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    unix_to_filetime(meta.atime())
}

/// Get last write time as Windows FILETIME.
#[cfg(windows)]
fn get_last_write_time(meta: &fs::Metadata) -> i64 {
    use std::os::windows::fs::MetadataExt;
    meta.last_write_time() as i64
}

#[cfg(unix)]
fn get_last_write_time(meta: &fs::Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    unix_to_filetime(meta.mtime())
}

/// Convert Unix timestamp to Windows FILETIME.
/// Windows FILETIME is 100-nanosecond intervals since January 1, 1601.
/// Unix timestamp is seconds since January 1, 1970.
#[cfg(unix)]
fn unix_to_filetime(unix_secs: i64) -> i64 {
    // Difference between 1601 and 1970 in seconds: 11644473600
    const UNIX_TO_FILETIME_OFFSET: i64 = 116444736000000000;
    // Convert seconds to 100-nanosecond intervals
    unix_secs * 10_000_000 + UNIX_TO_FILETIME_OFFSET
}

/// Get disk space information for a path.
#[cfg(windows)]
fn get_disk_space(path: &Path) -> std::io::Result<(u64, u64)> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let path_wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut free_bytes_available: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut total_free_bytes: u64 = 0;

    let result = unsafe {
        GetDiskFreeSpaceExW(
            path_wide.as_ptr(),
            &mut free_bytes_available,
            &mut total_bytes,
            &mut total_free_bytes,
        )
    };

    if result != 0 {
        Ok((total_bytes, free_bytes_available))
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(unix)]
fn get_disk_space(path: &Path) -> std::io::Result<(u64, u64)> {
    use std::ffi::CString;
    use std::mem::MaybeUninit;

    let path_cstr = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid path"))?;

    let mut stat: MaybeUninit<libc::statvfs> = MaybeUninit::uninit();
    let result = unsafe { libc::statvfs(path_cstr.as_ptr(), stat.as_mut_ptr()) };

    if result == 0 {
        let stat = unsafe { stat.assume_init() };
        let block_size = stat.f_frsize as u64;
        let total_bytes = stat.f_blocks as u64 * block_size;
        let free_bytes = stat.f_bavail as u64 * block_size;
        Ok((total_bytes, free_bytes))
    } else {
        Err(std::io::Error::last_os_error())
    }
}
