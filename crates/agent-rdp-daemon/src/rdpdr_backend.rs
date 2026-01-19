//! Platform-specific RDPDR backend implementations.
//!
//! This module provides drive redirection backends for both Unix and Windows platforms.

#[cfg(unix)]
pub use ironrdp_rdpdr_native::backend::NixRdpdrBackend as PlatformRdpdrBackend;

#[cfg(windows)]
pub use self::win::WinRdpdrBackend as PlatformRdpdrBackend;

#[cfg(windows)]
mod win {
    use std::collections::HashMap;
    use std::fs::{self, File, ReadDir};
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::MetadataExt;
    use std::path::{Path, PathBuf};

    use ironrdp::pdu::{encode_err, PduResult};
    use ironrdp_rdpdr::pdu::efs::*;
    use ironrdp_rdpdr::pdu::esc::{ScardCall, ScardIoCtlCode};
    use ironrdp_rdpdr::pdu::RdpdrPdu;
    use ironrdp_rdpdr::RdpdrBackend;
    use ironrdp_svc::{impl_as_any, SvcMessage};
    use tracing::{debug, warn};

    /// Windows implementation of the RDPDR backend for drive redirection.
    #[derive(Debug, Default)]
    pub struct WinRdpdrBackend {
        file_id: u32,
        file_base: PathBuf,
        file_map: HashMap<u32, File>,
        file_path_map: HashMap<u32, PathBuf>,
        file_dir_map: HashMap<u32, DirIterState>,
    }

    /// State for directory iteration.
    #[derive(Debug)]
    struct DirIterState {
        iter: ReadDir,
        base_path: PathBuf,
    }

    impl WinRdpdrBackend {
        pub fn new(file_base: String) -> Self {
            Self {
                file_base: PathBuf::from(file_base),
                ..Default::default()
            }
        }
    }

    impl_as_any!(WinRdpdrBackend);

    impl RdpdrBackend for WinRdpdrBackend {
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
        backend: &mut WinRdpdrBackend,
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
        backend: &mut WinRdpdrBackend,
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
        backend: &mut WinRdpdrBackend,
        req_inner: DeviceCloseRequest,
    ) -> PduResult<Vec<SvcMessage>> {
        backend.file_map.remove(&req_inner.device_io_request.file_id);
        backend.file_path_map.remove(&req_inner.device_io_request.file_id);
        backend.file_dir_map.remove(&req_inner.device_io_request.file_id);
        let res = RdpdrPdu::DeviceCloseResponse(DeviceCloseResponse {
            device_io_response: DeviceIoResponse::new(req_inner.device_io_request, NtStatus::SUCCESS),
        });
        Ok(vec![SvcMessage::from(res)])
    }

    fn query_information(
        backend: &mut WinRdpdrBackend,
        req_inner: ServerDriveQueryInformationRequest,
    ) -> PduResult<Vec<SvcMessage>> {
        match backend.file_map.get(&req_inner.device_io_request.file_id) {
            Some(file) => match file.metadata() {
                Ok(meta) => {
                    let path = backend
                        .file_path_map
                        .get(&req_inner.device_io_request.file_id)
                        .cloned()
                        .unwrap_or_default();
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");
                    let file_attribute = get_file_attributes(&meta, name);

                    if FileInformationClassLevel::FILE_BASIC_INFORMATION == req_inner.file_info_class_lvl
                    {
                        let basic_info = FileBasicInformation {
                            creation_time: windows_filetime_to_rdp(meta.creation_time()),
                            last_access_time: windows_filetime_to_rdp(meta.last_access_time()),
                            last_write_time: windows_filetime_to_rdp(meta.last_write_time()),
                            change_time: windows_filetime_to_rdp(meta.last_write_time()),
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
                            allocation_size: i64::try_from(meta.file_size()).unwrap(),
                            end_of_file: i64::try_from(meta.file_size()).unwrap(),
                            number_of_links: 1, // Windows doesn't expose this easily
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
                    warn!(?error, "Get file metadata error");
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
            },
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

    fn query_volume_information(
        backend: &mut WinRdpdrBackend,
        req_inner: ServerDriveQueryVolumeInformationRequest,
    ) -> PduResult<Vec<SvcMessage>> {
        match backend.file_map.get(&req_inner.device_io_request.file_id) {
            Some(file) => {
                // Get disk space information using Windows API
                let (total_bytes, free_bytes) = match get_disk_space(&backend.file_base) {
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
                    let creation_time = file
                        .metadata()
                        .map(|m| windows_filetime_to_rdp(m.creation_time()))
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
        backend: &mut WinRdpdrBackend,
        req_inner: ServerDriveSetInformationRequest,
    ) -> PduResult<Vec<SvcMessage>> {
        match backend.file_path_map.get(&req_inner.device_io_request.file_id) {
            Some(file_path) => {
                match &req_inner.set_buffer {
                    FileInformationClass::Rename(info) => {
                        let mut to = backend.file_base.clone();
                        let new_path = info.file_name.replace('\\', "/");
                        to.push(&new_path);
                        if let Err(error) = fs::rename(file_path, &to) {
                            warn!(?error, "Rename file error");
                            let res = RdpdrPdu::ClientDriveSetInformationResponse(
                                ClientDriveSetInformationResponse::new(&req_inner, NtStatus::UNSUCCESSFUL)
                                    .map_err(|e| encode_err!(e))?,
                            );
                            return Ok(vec![SvcMessage::from(res)]);
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
                        if let Some(file) = backend.file_map.get(&req_inner.device_io_request.file_id) {
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
                            warn!("no such file");
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

    /// Convert Windows FILETIME (100-nanosecond intervals since 1601) to RDP format.
    /// Windows MetadataExt already returns FILETIME values, so we just cast.
    fn windows_filetime_to_rdp(filetime: u64) -> i64 {
        filetime as i64
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

        // Use Windows file attributes if available
        let win_attrs = meta.file_attributes();
        if win_attrs & 0x1 != 0 {
            // FILE_ATTRIBUTE_READONLY
            file_attribute |= FileAttributes::FILE_ATTRIBUTE_READONLY;
        }
        if win_attrs & 0x2 != 0 {
            // FILE_ATTRIBUTE_HIDDEN
            file_attribute |= FileAttributes::FILE_ATTRIBUTE_HIDDEN;
        }
        if win_attrs & 0x4 != 0 {
            // FILE_ATTRIBUTE_SYSTEM
            file_attribute |= FileAttributes::FILE_ATTRIBUTE_SYSTEM;
        }

        file_attribute
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
                                windows_filetime_to_rdp(meta.creation_time()),
                                windows_filetime_to_rdp(meta.last_write_time()),
                                windows_filetime_to_rdp(meta.last_access_time()),
                                windows_filetime_to_rdp(meta.last_write_time()),
                                i64::try_from(meta.file_size()).unwrap(),
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
        backend: &mut WinRdpdrBackend,
        req_inner: ServerDriveQueryDirectoryRequest,
    ) -> PduResult<Vec<SvcMessage>> {
        match backend
            .file_path_map
            .get(&req_inner.device_io_request.file_id)
        {
            Some(parent_path) => {
                let mut find_file_path = None;

                if req_inner.initial_query > 0 {
                    if req_inner.path.ends_with('*') {
                        // Wildcard query - list directory contents
                        let query_path = req_inner.path.replace('\\', "/");
                        let len = query_path.len();
                        let dir_path = backend.file_base.join(&query_path[..len - 1]);

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
                        let full_path = backend.file_base.join(&query_path);
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
                warn!("no file to query directory");
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
        backend: &mut WinRdpdrBackend,
        req_inner: DeviceCreateRequest,
    ) -> PduResult<Vec<SvcMessage>> {
        let file_id = backend.file_id;
        backend.file_id += 1;

        let req_path = req_inner.path.replace('\\', "/");
        let path = backend.file_base.join(&req_path);

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
                        match File::open(&path) {
                            Ok(file) => {
                                debug!("create drive file_id:{},path:{:?}", file_id, path);
                                backend.file_map.insert(file_id, file);
                                backend.file_path_map.insert(file_id, path.clone());
                                return make_create_drive_resp(
                                    req_inner.device_io_request,
                                    req_inner.create_disposition,
                                    file_id,
                                );
                            }
                            Err(error) => {
                                warn!(%error, "Open file dir error");
                            }
                        }
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
                debug!("create drive file_id:{},path:{:?}", file_id, path);
                backend.file_map.insert(file_id, file);
                backend.file_path_map.insert(file_id, path.clone());
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
        backend: &mut WinRdpdrBackend,
        request: DeviceIoRequest,
        error_fx: impl Fn(DeviceIoRequest) -> PduResult<Vec<SvcMessage>>,
        fx: impl Fn(&mut File, DeviceIoRequest) -> PduResult<Vec<SvcMessage>>,
    ) -> PduResult<Vec<SvcMessage>> {
        match backend.file_map.get_mut(&request.file_id) {
            None => error_fx(request),
            Some(file) => fx(file, request),
        }
    }

    /// Get disk space information for a path using Windows API.
    fn get_disk_space(path: &Path) -> std::io::Result<(u64, u64)> {
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
}
