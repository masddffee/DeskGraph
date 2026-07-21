//! Windows scope-fence opener.
//!
//! Every operation with a create side effect is relative to an already pinned
//! and identity-verified directory handle. This avoids turning a path
//! validation into authority over a directory that was swapped afterwards.

use std::fs::File;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::{Component, Path};
use std::ptr::{null, null_mut};

use deskgraph_identity::{IdentityNodeKind, platform_identity_for_open_file};
use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_IF, FILE_OPEN_REPARSE_POINT,
    NTCREATEFILE_CREATE_DISPOSITION, NTCREATEFILE_CREATE_OPTIONS, NtCreateFile,
};
use windows_sys::Win32::Foundation::{
    GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE, OBJ_CASE_INSENSITIVE,
    RtlNtStatusToDosError, UNICODE_STRING,
};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileInformationByHandle,
    OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

use super::{
    DatabaseError, ManifestDatabase, ScopeFilesystemFenceDomain, ScopeFilesystemFenceRole,
};

const FENCE_ROOT_NAME: &str = "scope-read-fences-v1";

/// Opens the two per-scope fence files without resolving any create-capable
/// pathname after the database parent has been pinned and verified.
///
/// The parent module should call this only for a file-backed manifest:
///
/// ```ignore
/// #[cfg(windows)]
/// {
///     return fence_windows::open_scope_filesystem_fence_files(
///         self,
///         scope_id,
///         &database_path,
///         database_parent,
///     )
///     .map(Some);
/// }
/// ```
pub(super) fn open_scope_filesystem_fence_files(
    database: &ManifestDatabase,
    scope_id: i64,
    database_path: &Path,
    database_parent: &Path,
) -> Result<(File, File), DatabaseError> {
    if scope_id <= 0 {
        return Err(DatabaseError::ScopeFilesystemFenceInvalid);
    }

    // This pathname open has no create side effect. FILE_FLAG_OPEN_REPARSE_POINT
    // pins the leaf itself, so validation below cannot accidentally validate a
    // reparse target in place of the database parent.
    let parent = open_existing_path_no_reparse(database_parent, true)?;
    validate_open_handle(&parent, HandleKind::Directory)?;

    // Before creating the fence root, prove that this pinned directory contains
    // the exact database file whose immutable identity defines this
    // ManifestDatabase's fence domain.
    let database_name = database_path
        .file_name()
        .ok_or(DatabaseError::ScopeFilesystemFenceInvalid)?;
    let database_file = nt_open_relative(
        &parent,
        database_name,
        FILE_OPEN,
        FILE_NON_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT,
        GENERIC_READ,
        FILE_ATTRIBUTE_NORMAL,
    )?;
    validate_open_handle(&database_file, HandleKind::File)?;
    let database_identity =
        identity_for_handle(&database_file, database_path, IdentityNodeKind::File)?;
    if database_identity.link_count.is_some_and(|links| links != 1)
        || !matches!(
            &database.fence_domain,
            ScopeFilesystemFenceDomain::File {
                identity_kind,
                identity_key,
            } if *identity_kind == database_identity.kind
                && identity_key == &database_identity.key
        )
    {
        return Err(DatabaseError::ScopeFilesystemFenceInvalid);
    }

    // NtCreateFile with RootDirectory makes FILE_OPEN_IF relative to the
    // verified parent handle. A concurrently replaced pathname cannot redirect
    // creation to a different directory.
    let root = nt_open_relative(
        &parent,
        FENCE_ROOT_NAME.as_ref(),
        FILE_OPEN_IF,
        FILE_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT,
        GENERIC_READ | GENERIC_WRITE,
        FILE_ATTRIBUTE_DIRECTORY,
    )?;
    validate_open_handle(&root, HandleKind::Directory)?;
    let fence_root_path = database_parent.join(FENCE_ROOT_NAME);
    let root_identity = identity_for_handle(&root, &fence_root_path, IdentityNodeKind::Folder)?;
    if root_identity.link_count.is_some_and(|links| links != 1) {
        return Err(DatabaseError::ScopeFilesystemFenceInvalid);
    }
    database.bind_scope_filesystem_fence_identity(
        scope_id,
        ScopeFilesystemFenceRole::Root,
        root_identity.kind,
        &root_identity.key,
    )?;

    let gate_name = format!("scope-{scope_id}.gate");
    let data_name = format!("scope-{scope_id}.lock");
    let gate = open_fence_file(
        database,
        &root,
        scope_id,
        ScopeFilesystemFenceRole::Gate,
        &gate_name,
        &fence_root_path,
    )?;
    let data = open_fence_file(
        database,
        &root,
        scope_id,
        ScopeFilesystemFenceRole::Data,
        &data_name,
        &fence_root_path,
    )?;
    Ok((gate, data))
}

fn open_fence_file(
    database: &ManifestDatabase,
    root: &File,
    scope_id: i64,
    role: ScopeFilesystemFenceRole,
    name: &str,
    fence_root_path: &Path,
) -> Result<File, DatabaseError> {
    let file = nt_open_relative(
        root,
        name.as_ref(),
        FILE_OPEN_IF,
        FILE_NON_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT,
        GENERIC_READ | GENERIC_WRITE,
        FILE_ATTRIBUTE_NORMAL,
    )?;
    validate_open_handle(&file, HandleKind::File)?;
    let identity = identity_for_handle(&file, &fence_root_path.join(name), IdentityNodeKind::File)?;
    if identity.link_count.is_some_and(|links| links != 1) {
        return Err(DatabaseError::ScopeFilesystemFenceInvalid);
    }
    database.bind_scope_filesystem_fence_identity(scope_id, role, identity.kind, &identity.key)?;
    Ok(file)
}

fn open_existing_path_no_reparse(path: &Path, directory: bool) -> Result<File, DatabaseError> {
    let wide = nul_terminated_wide(path)?;
    let flags = FILE_FLAG_OPEN_REPARSE_POINT
        | if directory {
            FILE_FLAG_BACKUP_SEMANTICS
        } else {
            0
        };
    // SAFETY: `wide` is NUL terminated and lives through the call. No security
    // descriptor or template is supplied. A successful handle is transferred
    // exactly once to File below.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            flags,
            null_mut(),
        )
    };
    owned_file(handle)
}

fn nt_open_relative(
    root: &File,
    name: &std::ffi::OsStr,
    disposition: NTCREATEFILE_CREATE_DISPOSITION,
    create_options: NTCREATEFILE_CREATE_OPTIONS,
    desired_access: u32,
    attributes: u32,
) -> Result<File, DatabaseError> {
    let mut name_wide = validated_relative_name(name)?;
    let byte_length = name_wide
        .len()
        .checked_mul(size_of::<u16>())
        .and_then(|length| u16::try_from(length).ok())
        .ok_or(DatabaseError::ScopeFilesystemFenceInvalid)?;
    let unicode_name = UNICODE_STRING {
        Length: byte_length,
        MaximumLength: byte_length,
        Buffer: name_wide.as_mut_ptr(),
    };
    let object_attributes = OBJECT_ATTRIBUTES {
        Length: u32::try_from(size_of::<OBJECT_ATTRIBUTES>())
            .map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?,
        RootDirectory: root.as_raw_handle(),
        ObjectName: &unicode_name,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: null(),
        SecurityQualityOfService: null(),
    };
    let mut io_status = IO_STATUS_BLOCK::default();
    let mut handle: HANDLE = INVALID_HANDLE_VALUE;
    // SAFETY: RootDirectory is borrowed from a live File; ObjectName points to
    // a one-component UTF-16 buffer and every stack value outlives the call.
    // The returned handle, on success, is transferred exactly once to File.
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            desired_access,
            &object_attributes,
            &mut io_status,
            null(),
            attributes,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            disposition,
            create_options,
            null(),
            0,
        )
    };
    if status < 0 {
        // SAFETY: conversion has no pointer arguments or ownership effects.
        let windows_error = unsafe { RtlNtStatusToDosError(status) };
        return Err(std::io::Error::from_raw_os_error(
            i32::try_from(windows_error).unwrap_or(i32::MAX),
        )
        .into());
    }
    owned_file(handle)
}

fn owned_file(handle: HANDLE) -> Result<File, DatabaseError> {
    if handle == INVALID_HANDLE_VALUE || handle.is_null() {
        return Err(DatabaseError::ScopeFilesystemFenceInvalid);
    }
    // SAFETY: each successful Windows open returns a newly owned handle. This
    // conversion transfers that sole ownership to File.
    Ok(unsafe { File::from_raw_handle(handle) })
}

#[derive(Clone, Copy)]
enum HandleKind {
    Directory,
    File,
}

fn validate_open_handle(file: &File, expected: HandleKind) -> Result<(), DatabaseError> {
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: `file` owns a live handle and `information` is a correctly sized
    // writable output buffer. The call borrows and never closes the handle.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) } == 0 {
        return Err(DatabaseError::ScopeFilesystemFenceInvalid);
    }
    let attributes = information.dwFileAttributes;
    let is_directory = attributes & FILE_ATTRIBUTE_DIRECTORY != 0;
    let is_reparse = attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    let kind_matches = match expected {
        HandleKind::Directory => is_directory,
        HandleKind::File => !is_directory,
    };
    if is_reparse || !kind_matches || information.nNumberOfLinks != 1 {
        return Err(DatabaseError::ScopeFilesystemFenceInvalid);
    }
    Ok(())
}

fn identity_for_handle(
    file: &File,
    diagnostic_path: &Path,
    kind: IdentityNodeKind,
) -> Result<deskgraph_identity::FileIdentity, DatabaseError> {
    let metadata = file
        .metadata()
        .map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)?;
    platform_identity_for_open_file(file, diagnostic_path, &metadata, kind)
        .map_err(|_| DatabaseError::ScopeFilesystemFenceInvalid)
}

fn validated_relative_name(name: &std::ffi::OsStr) -> Result<Vec<u16>, DatabaseError> {
    let path = Path::new(name);
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(DatabaseError::ScopeFilesystemFenceInvalid);
    }
    let wide: Vec<u16> = name.encode_wide().collect();
    if wide.is_empty()
        || wide
            .iter()
            .any(|unit| *unit == 0 || *unit == b'/' as u16 || *unit == b'\\' as u16)
    {
        return Err(DatabaseError::ScopeFilesystemFenceInvalid);
    }
    Ok(wide)
}

fn nul_terminated_wide(path: &Path) -> Result<Vec<u16>, DatabaseError> {
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.is_empty() || wide.contains(&0) {
        return Err(DatabaseError::ScopeFilesystemFenceInvalid);
    }
    wide.push(0);
    Ok(wide)
}
