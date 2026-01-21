//! Cross-platform RDPDR backend with multi-drive support.
//!
//! This module provides drive redirection for RDP sessions, supporting multiple
//! drives mapped to different local directories.

mod file_ops;
mod helpers;
mod query_ops;
mod set_ops;

use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;

use ironrdp::pdu::PduResult;
use ironrdp_rdpdr::pdu::efs::*;
use ironrdp_rdpdr::pdu::esc::{ScardCall, ScardIoCtlCode};
use ironrdp_rdpdr::pdu::RdpdrPdu;
use ironrdp_rdpdr::RdpdrBackend;
use ironrdp_svc::{impl_as_any, SvcMessage};
use tracing::{debug, info};

use file_ops::{close_device, create_drive, read_device, write_device};
use query_ops::{query_directory, query_information, query_volume_information, DirIterState};
use set_ops::set_information;

/// Cross-platform RDPDR backend supporting multiple drives.
#[derive(Debug, Default)]
pub struct MultiDriveBackend {
    /// Next file ID to assign.
    file_id: u32,
    /// Mapping from device_id to base path for each drive.
    pub(crate) drive_paths: HashMap<u32, PathBuf>,
    /// File handles - None for directories.
    pub(crate) file_map: HashMap<u32, Option<File>>,
    /// File ID to full path mapping.
    pub(crate) file_path_map: HashMap<u32, PathBuf>,
    /// File ID to device ID mapping (to look up base path for volume queries).
    pub(crate) file_device_map: HashMap<u32, u32>,
    /// Directory iteration state.
    pub(crate) file_dir_map: HashMap<u32, DirIterState>,
    /// Files marked for deletion on close (set via FileDispositionInformation).
    pub(crate) delete_on_close: HashMap<u32, bool>,
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
    pub(crate) fn get_base_path(&self, device_id: u32) -> Option<&PathBuf> {
        self.drive_paths.get(&device_id)
    }

    /// Get the base path for a file (via file_id -> device_id lookup).
    pub(crate) fn get_base_path_for_file(&self, file_id: u32) -> Option<&PathBuf> {
        self.file_device_map
            .get(&file_id)
            .and_then(|device_id| self.drive_paths.get(device_id))
    }

    /// Get next file ID and increment counter.
    pub(crate) fn next_file_id(&mut self) -> u32 {
        let id = self.file_id;
        self.file_id += 1;
        id
    }

    /// Get drive paths keys for debug logging.
    pub(crate) fn drive_paths_keys(&self) -> Vec<&u32> {
        self.drive_paths.keys().collect()
    }

    /// Get drive paths for debug logging.
    pub(crate) fn drive_paths_debug(&self) -> &HashMap<u32, PathBuf> {
        &self.drive_paths
    }

    /// Insert a directory entry (no file handle).
    pub(crate) fn insert_directory(&mut self, file_id: u32, device_id: u32, path: PathBuf) {
        self.file_map.insert(file_id, None);
        self.file_path_map.insert(file_id, path);
        self.file_device_map.insert(file_id, device_id);
    }

    /// Insert a file entry with handle.
    pub(crate) fn insert_file(&mut self, file_id: u32, device_id: u32, path: PathBuf, file: File) {
        self.file_map.insert(file_id, Some(file));
        self.file_path_map.insert(file_id, path);
        self.file_device_map.insert(file_id, device_id);
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
            ServerDriveIoRequest::ServerCreateDriveRequest(req_inner) => {
                create_drive(self, req_inner)
            }
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
