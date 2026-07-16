use std::ffi::OsString;
use std::fs::Metadata;
use std::path::{Path, PathBuf};

use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityNodeKind {
    File,
    Folder,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileIdentity {
    pub kind: &'static str,
    pub key: Vec<u8>,
    pub link_count: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityError {
    PlatformMetadataUnavailable,
    InvalidPathEncoding,
}

pub fn platform_identity(
    path: &Path,
    metadata: &Metadata,
    kind: IdentityNodeKind,
) -> Result<FileIdentity, IdentityError> {
    platform_identity_impl(path, metadata, kind)
        .map_err(|_| IdentityError::PlatformMetadataUnavailable)
}

pub fn fallback_identity(path_key: &str, kind: IdentityNodeKind) -> FileIdentity {
    let mut key = Vec::with_capacity(path_key.len() + 1);
    key.push(node_kind_byte(kind));
    key.extend_from_slice(path_key.as_bytes());
    FileIdentity {
        kind: "path_fallback",
        key,
        link_count: None,
    }
}

pub fn comparison_key(path: &Path) -> String {
    let normalized: String = path.to_string_lossy().nfc().collect();
    if cfg!(windows) {
        normalized.to_lowercase()
    } else {
        normalized
    }
}

#[cfg(unix)]
pub fn path_to_raw(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
pub fn path_to_raw(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
pub fn path_to_raw(path: &Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

#[cfg(unix)]
pub fn path_from_raw(raw: &[u8]) -> Result<PathBuf, IdentityError> {
    use std::os::unix::ffi::OsStringExt;
    Ok(PathBuf::from(OsString::from_vec(raw.to_vec())))
}

#[cfg(windows)]
pub fn path_from_raw(raw: &[u8]) -> Result<PathBuf, IdentityError> {
    use std::os::windows::ffi::OsStringExt;
    if !raw.len().is_multiple_of(2) {
        return Err(IdentityError::InvalidPathEncoding);
    }
    let wide = raw
        .chunks_exact(2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
        .collect::<Vec<_>>();
    Ok(PathBuf::from(OsString::from_wide(&wide)))
}

#[cfg(not(any(unix, windows)))]
pub fn path_from_raw(raw: &[u8]) -> Result<PathBuf, IdentityError> {
    String::from_utf8(raw.to_vec())
        .map(PathBuf::from)
        .map_err(|_| IdentityError::InvalidPathEncoding)
}

#[cfg(windows)]
pub fn is_symlink_or_reparse_point(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
pub fn is_symlink_or_reparse_point(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(unix)]
fn platform_identity_impl(
    _path: &Path,
    metadata: &Metadata,
    kind: IdentityNodeKind,
) -> Result<FileIdentity, std::io::Error> {
    use std::os::unix::fs::MetadataExt;

    let mut key = Vec::with_capacity(17);
    key.push(node_kind_byte(kind));
    key.extend_from_slice(&metadata.dev().to_le_bytes());
    key.extend_from_slice(&metadata.ino().to_le_bytes());
    Ok(FileIdentity {
        kind: "unix_device_inode",
        key,
        link_count: Some(metadata.nlink()),
    })
}

#[cfg(windows)]
fn platform_identity_impl(
    path: &Path,
    _metadata: &Metadata,
    kind: IdentityNodeKind,
) -> Result<FileIdentity, std::io::Error> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileInformationByHandle, OPEN_EXISTING,
    };

    let mut wide_path: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide_path.push(0);
    // SAFETY: `wide_path` is NUL terminated and lives for the call. Null security and template
    // pointers are allowed by CreateFileW. The returned owned handle is closed below.
    let handle = unsafe {
        CreateFileW(
            wide_path.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error());
    }

    struct OwnedHandle(windows_sys::Win32::Foundation::HANDLE);
    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // SAFETY: this CreateFileW handle is closed exactly once by its owner.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    let handle = OwnedHandle(handle);
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: the handle is valid and `information` is a correctly sized writable output value.
    if unsafe { GetFileInformationByHandle(handle.0, &mut information) } == 0 {
        return Err(std::io::Error::last_os_error());
    }

    let file_index =
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow);
    let mut key = Vec::with_capacity(13);
    key.push(node_kind_byte(kind));
    key.extend_from_slice(&information.dwVolumeSerialNumber.to_le_bytes());
    key.extend_from_slice(&file_index.to_le_bytes());
    Ok(FileIdentity {
        kind: "windows_volume_file_index",
        key,
        link_count: Some(u64::from(information.nNumberOfLinks)),
    })
}

#[cfg(not(any(unix, windows)))]
fn platform_identity_impl(
    _path: &Path,
    _metadata: &Metadata,
    _kind: IdentityNodeKind,
) -> Result<FileIdentity, std::io::Error> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "stable filesystem identity unavailable",
    ))
}

fn node_kind_byte(kind: IdentityNodeKind) -> u8 {
    match kind {
        IdentityNodeKind::File => b'f',
        IdentityNodeKind::Folder => b'd',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_comparison_keys_use_nfc() {
        assert_eq!(
            comparison_key(Path::new("caf\u{e9}.txt")),
            comparison_key(Path::new("cafe\u{301}.txt"))
        );
    }

    #[test]
    fn raw_path_round_trip_preserves_platform_path() {
        let path = Path::new("folder").join("file.txt");
        assert_eq!(
            path_from_raw(&path_to_raw(&path)).expect("path should decode"),
            path
        );
    }
}
