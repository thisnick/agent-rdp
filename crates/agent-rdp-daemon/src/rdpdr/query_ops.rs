//! Query operations for RDPDR: query_information, query_volume_information, query_directory.

use std::fs::{self, ReadDir};
use std::path::PathBuf;

use ironrdp::pdu::PduResult;
use ironrdp_rdpdr::pdu::efs::*;
use ironrdp_rdpdr::pdu::RdpdrPdu;
use ironrdp_svc::SvcMessage;
use tracing::{debug, warn};

use super::helpers::{
    get_creation_time, get_disk_space, get_file_attributes, get_last_access_time,
    get_last_write_time,
};
use super::MultiDriveBackend;

/// State for directory iteration.
#[derive(Debug)]
pub struct DirIterState {
    pub iter: ReadDir,
    #[allow(dead_code)]
    pub base_path: PathBuf,
}

/// Handle query information request.
pub fn query_information(
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
            // Prefer file handle metadata (works even if file deleted locally on Unix)
            // Fall back to path-based metadata for directories
            let meta_result = if let Some(Some(file)) = backend.file_map.get(&file_id) {
                file.metadata()
            } else {
                fs::metadata(path)
            };
            match meta_result {
                Ok(meta) => {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    let file_attribute = get_file_attributes(&meta, name);

                    if FileInformationClassLevel::FILE_BASIC_INFORMATION
                        == req_inner.file_info_class_lvl
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
                        // Windows may request various file info classes; returning UNSUCCESSFUL is valid
                        debug!(
                            "unsupported file info class: {:?}",
                            req_inner.file_info_class_lvl
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
            warn!(
                "query_information: no such file_id={} in file_path_map",
                file_id
            );
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

/// Handle query volume information request.
pub fn query_volume_information(
    backend: &mut MultiDriveBackend,
    req_inner: ServerDriveQueryVolumeInformationRequest,
) -> PduResult<Vec<SvcMessage>> {
    match backend
        .file_path_map
        .get(&req_inner.device_io_request.file_id)
    {
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
                    bytes_per_sector,
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
                // Prefer file handle metadata (works even if file deleted locally on Unix)
                let creation_time = if let Some(Some(file)) =
                    backend.file_map.get(&req_inner.device_io_request.file_id)
                {
                    file.metadata()
                        .map(|m| get_creation_time(&m))
                        .unwrap_or(0)
                } else {
                    fs::metadata(path)
                        .map(|m| get_creation_time(&m))
                        .unwrap_or(0)
                };

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
                                    total_alloc_units: i64::try_from(total_units)
                                        .unwrap_or(i64::MAX),
                                    available_alloc_units: i64::try_from(free_units)
                                        .unwrap_or(i64::MAX),
                                    sectors_per_alloc_unit: sectors_per_unit,
                                    bytes_per_sector,
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

/// Handle query directory request.
pub fn query_directory(
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
            debug!(
                "query_directory: file_id={} -> parent_path={:?}",
                file_id, _parent_path
            );
            let mut find_file_path = None;

            // Get base path for this device
            let base_path = match backend.get_base_path(device_id) {
                Some(p) => p.clone(),
                None => {
                    warn!("No base path for device {}", device_id);
                    return Ok(vec![SvcMessage::from(
                        RdpdrPdu::ClientDriveQueryDirectoryResponse(
                            ClientDriveQueryDirectoryResponse {
                                device_io_reply: DeviceIoResponse::new(
                                    req_inner.device_io_request,
                                    NtStatus::NO_SUCH_FILE,
                                ),
                                buffer: None,
                            },
                        ),
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

fn make_query_dir_resp(
    find_file_path: Option<PathBuf>,
    device_io_request: DeviceIoRequest,
    file_class: FileInformationClassLevel,
    initial_query: bool,
) -> PduResult<Vec<SvcMessage>> {
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
                        // Windows may request various file info classes; NOT_SUPPORTED is a valid response
                        debug!(
                            "unsupported file class for query directory: {:?}",
                            file_class
                        );
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
                    // File may have been deleted between listing and metadata fetch (normal for IPC)
                    debug!(%error, "Get metadata error (file may have been deleted)");
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
