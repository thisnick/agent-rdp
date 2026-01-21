//! Platform-specific helper functions for RDPDR.

use std::fs;
use std::path::Path;

use ironrdp_rdpdr::pdu::efs::FileAttributes;

/// Get file attributes from metadata.
pub fn get_file_attributes(meta: &fs::Metadata, file_name: &str) -> FileAttributes {
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

/// Get creation time as Windows FILETIME (100-nanosecond intervals since 1601).
#[cfg(windows)]
pub fn get_creation_time(meta: &fs::Metadata) -> i64 {
    use std::os::windows::fs::MetadataExt;
    meta.creation_time() as i64
}

#[cfg(unix)]
pub fn get_creation_time(meta: &fs::Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    // Unix doesn't have creation time, use ctime (status change time)
    // Convert Unix timestamp to Windows FILETIME
    unix_to_filetime(meta.ctime())
}

/// Get last access time as Windows FILETIME.
#[cfg(windows)]
pub fn get_last_access_time(meta: &fs::Metadata) -> i64 {
    use std::os::windows::fs::MetadataExt;
    meta.last_access_time() as i64
}

#[cfg(unix)]
pub fn get_last_access_time(meta: &fs::Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    unix_to_filetime(meta.atime())
}

/// Get last write time as Windows FILETIME.
#[cfg(windows)]
pub fn get_last_write_time(meta: &fs::Metadata) -> i64 {
    use std::os::windows::fs::MetadataExt;
    meta.last_write_time() as i64
}

#[cfg(unix)]
pub fn get_last_write_time(meta: &fs::Metadata) -> i64 {
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
pub fn get_disk_space(path: &Path) -> std::io::Result<(u64, u64)> {
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
pub fn get_disk_space(path: &Path) -> std::io::Result<(u64, u64)> {
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
