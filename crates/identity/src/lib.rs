use std::ffi::OsString;
use std::fs::{File, Metadata};
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::time::UNIX_EPOCH;

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

#[derive(Clone, Copy, Debug)]
pub struct IdentityExpectation<'a> {
    pub kind: &'a str,
    pub key: &'a [u8],
}

impl<'a> IdentityExpectation<'a> {
    #[must_use]
    pub fn from_identity(identity: &'a FileIdentity) -> Self {
        Self {
            kind: identity.kind,
            key: &identity.key,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionBindingError {
    UnsupportedPlatform,
    InvalidPath,
    WeakIdentity,
    RootUnavailable,
    RootIdentityChanged,
    ParentUnavailable,
    ParentIdentityChanged,
    SourceUnavailable,
    SourceNotRegularFile,
    SourceIdentityChanged,
    SourceMetadataChanged,
    SourceHasMultipleLinks,
    InvalidDestinationName,
    CaseOnlyRenameDenied,
    DestinationConflict,
    DestinationIdentityChanged,
}

impl ActionBindingError {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "action_platform_rename_unsupported",
            Self::InvalidPath => "action_binding_path_invalid",
            Self::WeakIdentity => "action_binding_identity_weak",
            Self::RootUnavailable => "action_binding_root_unavailable",
            Self::RootIdentityChanged => "action_binding_root_identity_changed",
            Self::ParentUnavailable => "action_binding_parent_unavailable",
            Self::ParentIdentityChanged => "action_binding_parent_identity_changed",
            Self::SourceUnavailable => "action_binding_source_unavailable",
            Self::SourceNotRegularFile => "action_binding_source_not_regular_file",
            Self::SourceIdentityChanged => "action_binding_source_identity_changed",
            Self::SourceMetadataChanged => "action_binding_source_metadata_changed",
            Self::SourceHasMultipleLinks => "action_binding_source_hard_link_denied",
            Self::InvalidDestinationName => "action_binding_destination_name_invalid",
            Self::CaseOnlyRenameDenied => "action_binding_case_only_rename_denied",
            Self::DestinationConflict => "action_binding_destination_conflict",
            Self::DestinationIdentityChanged => "action_binding_destination_identity_changed",
        }
    }
}

impl std::fmt::Display for ActionBindingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ActionBindingError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionEntryObservation {
    Missing,
    ExpectedIdentity,
    OtherEntry,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionFileObservation {
    pub current: ActionEntryObservation,
    pub alternate: ActionEntryObservation,
}

#[cfg(unix)]
#[derive(Debug)]
pub struct ActionFileBinding {
    root: File,
    parent: File,
    source: File,
    root_path: PathBuf,
    parent_path: PathBuf,
    current_leaf: OsString,
    root_identity: FileIdentity,
    parent_identity: FileIdentity,
    source_identity: FileIdentity,
    source_size_bytes: u64,
    source_modified_unix_ns: Option<i64>,
}

#[cfg(not(unix))]
#[derive(Debug)]
pub struct ActionFileBinding {
    _private: (),
}

#[cfg(unix)]
#[derive(Debug)]
pub struct ActionRenameTarget {
    leaf: OsString,
}

#[cfg(not(unix))]
#[derive(Debug)]
pub struct ActionRenameTarget {
    _private: (),
}

pub fn bind_action_file(
    canonical_authorized_root: &Path,
    source_path: &Path,
    expected_root: IdentityExpectation<'_>,
    expected_parent: IdentityExpectation<'_>,
    expected_source: IdentityExpectation<'_>,
) -> Result<ActionFileBinding, ActionBindingError> {
    bind_action_file_impl(
        canonical_authorized_root,
        source_path,
        expected_root,
        expected_parent,
        expected_source,
    )
}

pub fn platform_identity(
    path: &Path,
    metadata: &Metadata,
    kind: IdentityNodeKind,
) -> Result<FileIdentity, IdentityError> {
    platform_identity_impl(path, metadata, kind)
        .map_err(|_| IdentityError::PlatformMetadataUnavailable)
}

pub fn platform_identity_for_open_file(
    file: &File,
    path: &Path,
    metadata: &Metadata,
    kind: IdentityNodeKind,
) -> Result<FileIdentity, IdentityError> {
    platform_identity_for_open_file_impl(file, path, metadata, kind)
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

#[cfg(windows)]
pub fn has_hidden_or_system_attribute(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM};

    metadata.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
}

#[cfg(target_os = "macos")]
pub fn has_hidden_or_system_attribute(metadata: &Metadata) -> bool {
    use std::os::macos::fs::MetadataExt;

    const UF_HIDDEN: u32 = 0x0000_8000;
    metadata.st_flags() & UF_HIDDEN != 0
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn has_hidden_or_system_attribute(_metadata: &Metadata) -> bool {
    false
}

#[cfg(unix)]
impl ActionFileBinding {
    /// Returns the already-open, read-only source handle captured by the binding.
    ///
    /// Callers that need file content (for example, a streaming digest) must read this handle
    /// instead of reopening the source by path. The handle is intentionally borrowed so callers
    /// cannot outlive or replace the identity-bound capability.
    #[must_use]
    pub fn source_file(&self) -> &File {
        &self.source
    }

    #[must_use]
    pub fn root_identity(&self) -> &FileIdentity {
        &self.root_identity
    }

    #[must_use]
    pub fn parent_identity(&self) -> &FileIdentity {
        &self.parent_identity
    }

    #[must_use]
    pub fn source_identity(&self) -> &FileIdentity {
        &self.source_identity
    }

    #[must_use]
    pub fn source_size_bytes(&self) -> u64 {
        self.source_size_bytes
    }

    #[must_use]
    pub fn source_modified_unix_ns(&self) -> Option<i64> {
        self.source_modified_unix_ns
    }

    pub fn prepare_absent_destination(
        &self,
        destination_name: &std::ffi::OsStr,
    ) -> Result<ActionRenameTarget, ActionBindingError> {
        validate_leaf_name(destination_name)?;
        if destination_name == self.current_leaf {
            return Err(ActionBindingError::InvalidDestinationName);
        }
        if destination_name.to_string_lossy().to_lowercase()
            == self.current_leaf.to_string_lossy().to_lowercase()
        {
            return Err(ActionBindingError::CaseOnlyRenameDenied);
        }
        let target = ActionRenameTarget {
            leaf: destination_name.to_owned(),
        };
        self.revalidate_for_rename(&target)?;
        Ok(target)
    }

    pub fn revalidate_for_rename(
        &self,
        target: &ActionRenameTarget,
    ) -> Result<(), ActionBindingError> {
        self.revalidate_namespace()?;
        self.revalidate_source_entry()?;
        match inspect_entry(&self.parent, &target.leaf, &self.source_identity)? {
            ActionEntryObservation::Missing => Ok(()),
            ActionEntryObservation::ExpectedIdentity => {
                Err(ActionBindingError::CaseOnlyRenameDenied)
            }
            ActionEntryObservation::OtherEntry => Err(ActionBindingError::DestinationConflict),
        }
    }

    /// Revalidates the held namespace, leaf entry, identity, link count, size,
    /// and modification timestamp without preparing a mutation.
    pub fn revalidate_bound_source(&self) -> Result<(), ActionBindingError> {
        self.revalidate_namespace()?;
        self.revalidate_source_entry()
    }

    pub fn observe_renamed_to(
        &mut self,
        target: ActionRenameTarget,
    ) -> Result<ActionFileObservation, ActionBindingError> {
        self.revalidate_namespace()?;
        let observation = self.observe_current_and(&target.leaf)?;
        if observation.current != ActionEntryObservation::Missing
            || observation.alternate != ActionEntryObservation::ExpectedIdentity
        {
            return Err(ActionBindingError::DestinationIdentityChanged);
        }
        self.revalidate_open_source_metadata()?;
        self.current_leaf = target.leaf;
        Ok(observation)
    }

    pub fn observe_current_and(
        &self,
        alternate_name: &std::ffi::OsStr,
    ) -> Result<ActionFileObservation, ActionBindingError> {
        validate_leaf_name(alternate_name)?;
        self.revalidate_namespace()?;
        Ok(ActionFileObservation {
            current: inspect_entry(&self.parent, &self.current_leaf, &self.source_identity)?,
            alternate: inspect_entry(&self.parent, alternate_name, &self.source_identity)?,
        })
    }

    #[must_use]
    pub fn parent_file(&self) -> &File {
        &self.parent
    }

    #[must_use]
    pub fn current_leaf(&self) -> &std::ffi::OsStr {
        &self.current_leaf
    }

    fn revalidate_namespace(&self) -> Result<(), ActionBindingError> {
        let current_root = open_absolute_directory_without_links(&self.root_path)
            .map_err(|_| ActionBindingError::RootUnavailable)?;
        validate_open_identity(
            &current_root,
            &self.root_path,
            IdentityNodeKind::Folder,
            &self.root_identity,
            ActionBindingError::RootIdentityChanged,
        )?;
        validate_open_identity(
            &self.root,
            &self.root_path,
            IdentityNodeKind::Folder,
            &self.root_identity,
            ActionBindingError::RootIdentityChanged,
        )?;

        let relative_parent = self
            .parent_path
            .strip_prefix(&self.root_path)
            .map_err(|_| ActionBindingError::ParentUnavailable)?;
        let current_parent = open_relative_directory(&current_root, relative_parent)
            .map_err(|_| ActionBindingError::ParentUnavailable)?;
        validate_open_identity(
            &current_parent,
            &self.parent_path,
            IdentityNodeKind::Folder,
            &self.parent_identity,
            ActionBindingError::ParentIdentityChanged,
        )?;
        validate_open_identity(
            &self.parent,
            &self.parent_path,
            IdentityNodeKind::Folder,
            &self.parent_identity,
            ActionBindingError::ParentIdentityChanged,
        )
    }

    fn revalidate_source_entry(&self) -> Result<(), ActionBindingError> {
        let source = open_regular_file_at(&self.parent, &self.current_leaf)
            .map_err(|_| ActionBindingError::SourceUnavailable)?;
        validate_open_identity(
            &source,
            &self.parent_path.join(&self.current_leaf),
            IdentityNodeKind::File,
            &self.source_identity,
            ActionBindingError::SourceIdentityChanged,
        )?;
        self.revalidate_open_source_metadata()
    }

    fn revalidate_open_source_metadata(&self) -> Result<(), ActionBindingError> {
        let metadata = self
            .source
            .metadata()
            .map_err(|_| ActionBindingError::SourceUnavailable)?;
        if !metadata.is_file() {
            return Err(ActionBindingError::SourceNotRegularFile);
        }
        let identity = platform_identity_for_open_file(
            &self.source,
            &self.parent_path.join(&self.current_leaf),
            &metadata,
            IdentityNodeKind::File,
        )
        .map_err(|_| ActionBindingError::SourceIdentityChanged)?;
        if identity.kind != self.source_identity.kind || identity.key != self.source_identity.key {
            return Err(ActionBindingError::SourceIdentityChanged);
        }
        if identity.link_count != Some(1) {
            return Err(ActionBindingError::SourceHasMultipleLinks);
        }
        if metadata.len() != self.source_size_bytes
            || modified_unix_ns(&metadata) != self.source_modified_unix_ns
        {
            return Err(ActionBindingError::SourceMetadataChanged);
        }
        Ok(())
    }
}

#[cfg(unix)]
impl ActionRenameTarget {
    #[must_use]
    pub fn leaf(&self) -> &std::ffi::OsStr {
        &self.leaf
    }
}

#[cfg(unix)]
fn bind_action_file_impl(
    canonical_authorized_root: &Path,
    source_path: &Path,
    expected_root: IdentityExpectation<'_>,
    expected_parent: IdentityExpectation<'_>,
    expected_source: IdentityExpectation<'_>,
) -> Result<ActionFileBinding, ActionBindingError> {
    if !is_strict_absolute_path(canonical_authorized_root)
        || !is_strict_absolute_path(source_path)
        || source_path == canonical_authorized_root
    {
        return Err(ActionBindingError::InvalidPath);
    }
    validate_strong_expectation(expected_root, "unix_device_inode")?;
    validate_strong_expectation(expected_parent, "unix_device_inode")?;
    validate_strong_expectation(expected_source, "unix_device_inode")?;

    let parent_path = source_path
        .parent()
        .ok_or(ActionBindingError::InvalidPath)?;
    let relative_parent = parent_path
        .strip_prefix(canonical_authorized_root)
        .map_err(|_| ActionBindingError::InvalidPath)?;
    let source_leaf = source_path
        .file_name()
        .ok_or(ActionBindingError::InvalidPath)?
        .to_owned();
    validate_leaf_name(&source_leaf)?;

    let root = open_absolute_directory_without_links(canonical_authorized_root)
        .map_err(|_| ActionBindingError::RootUnavailable)?;
    let root_identity = identity_for_open_file(
        &root,
        canonical_authorized_root,
        IdentityNodeKind::Folder,
        ActionBindingError::RootUnavailable,
    )?;
    validate_expected_identity(
        &root_identity,
        expected_root,
        ActionBindingError::RootIdentityChanged,
    )?;

    let parent = open_relative_directory(&root, relative_parent)
        .map_err(|_| ActionBindingError::ParentUnavailable)?;
    let parent_identity = identity_for_open_file(
        &parent,
        parent_path,
        IdentityNodeKind::Folder,
        ActionBindingError::ParentUnavailable,
    )?;
    validate_expected_identity(
        &parent_identity,
        expected_parent,
        ActionBindingError::ParentIdentityChanged,
    )?;

    let source = open_regular_file_at(&parent, &source_leaf)
        .map_err(|_| ActionBindingError::SourceUnavailable)?;
    let source_metadata = source
        .metadata()
        .map_err(|_| ActionBindingError::SourceUnavailable)?;
    if !source_metadata.is_file() || is_symlink_or_reparse_point(&source_metadata) {
        return Err(ActionBindingError::SourceNotRegularFile);
    }
    let source_identity = platform_identity_for_open_file(
        &source,
        source_path,
        &source_metadata,
        IdentityNodeKind::File,
    )
    .map_err(|_| ActionBindingError::SourceIdentityChanged)?;
    validate_expected_identity(
        &source_identity,
        expected_source,
        ActionBindingError::SourceIdentityChanged,
    )?;
    if source_identity.link_count != Some(1) {
        return Err(ActionBindingError::SourceHasMultipleLinks);
    }

    Ok(ActionFileBinding {
        root,
        parent,
        source,
        root_path: canonical_authorized_root.to_owned(),
        parent_path: parent_path.to_owned(),
        current_leaf: source_leaf,
        root_identity,
        parent_identity,
        source_identity,
        source_size_bytes: source_metadata.len(),
        source_modified_unix_ns: modified_unix_ns(&source_metadata),
    })
}

#[cfg(not(unix))]
fn bind_action_file_impl(
    _canonical_authorized_root: &Path,
    _source_path: &Path,
    _expected_root: IdentityExpectation<'_>,
    _expected_parent: IdentityExpectation<'_>,
    _expected_source: IdentityExpectation<'_>,
) -> Result<ActionFileBinding, ActionBindingError> {
    Err(ActionBindingError::UnsupportedPlatform)
}

#[cfg(unix)]
fn validate_strong_expectation(
    expectation: IdentityExpectation<'_>,
    required_kind: &str,
) -> Result<(), ActionBindingError> {
    if expectation.kind != required_kind || expectation.key.is_empty() {
        return Err(ActionBindingError::WeakIdentity);
    }
    Ok(())
}

#[cfg(unix)]
fn validate_expected_identity(
    actual: &FileIdentity,
    expected: IdentityExpectation<'_>,
    mismatch: ActionBindingError,
) -> Result<(), ActionBindingError> {
    if actual.kind != expected.kind || actual.key != expected.key {
        return Err(mismatch);
    }
    Ok(())
}

#[cfg(unix)]
fn identity_for_open_file(
    file: &File,
    path: &Path,
    kind: IdentityNodeKind,
    unavailable: ActionBindingError,
) -> Result<FileIdentity, ActionBindingError> {
    let metadata = file.metadata().map_err(|_| unavailable)?;
    platform_identity_for_open_file(file, path, &metadata, kind).map_err(|_| unavailable)
}

#[cfg(unix)]
fn validate_open_identity(
    file: &File,
    path: &Path,
    kind: IdentityNodeKind,
    expected: &FileIdentity,
    mismatch: ActionBindingError,
) -> Result<(), ActionBindingError> {
    let actual = identity_for_open_file(file, path, kind, mismatch)?;
    if actual.kind != expected.kind || actual.key != expected.key {
        return Err(mismatch);
    }
    Ok(())
}

#[cfg(unix)]
fn is_strict_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        })
}

#[cfg(unix)]
fn validate_leaf_name(name: &std::ffi::OsStr) -> Result<(), ActionBindingError> {
    use std::os::unix::ffi::OsStrExt;

    let mut components = Path::new(name).components();
    let valid_component = matches!(components.next(), Some(std::path::Component::Normal(_)))
        && components.next().is_none();
    if !valid_component || name.as_bytes().contains(&0) {
        return Err(ActionBindingError::InvalidDestinationName);
    }
    Ok(())
}

#[cfg(unix)]
fn open_absolute_directory_without_links(path: &Path) -> Result<File, std::io::Error> {
    use std::ffi::CString;
    use std::os::fd::{FromRawFd, RawFd};
    use std::os::unix::ffi::OsStrExt;

    if !is_strict_absolute_path(path) {
        return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
    }
    let slash = CString::new("/").expect("static path contains no NUL");
    // SAFETY: the static path is NUL terminated and the returned descriptor is owned below.
    let root_fd = unsafe { libc::open(slash.as_ptr(), directory_open_flags()) };
    if root_fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: root_fd is newly owned and is wrapped exactly once.
    let mut current = unsafe { File::from_raw_fd(root_fd as RawFd) };
    for component in path.components() {
        let std::path::Component::Normal(name) = component else {
            continue;
        };
        let name = CString::new(name.as_bytes())
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
        current = open_directory_at(&current, &name)?;
    }
    Ok(current)
}

#[cfg(unix)]
fn open_relative_directory(root: &File, relative: &Path) -> Result<File, std::io::Error> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let current_component = CString::new(".").expect("static path contains no NUL");
    let mut current = open_directory_at(root, &current_component)?;
    for component in relative.components() {
        let std::path::Component::Normal(name) = component else {
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
        };
        let name = CString::new(name.as_bytes())
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
        current = open_directory_at(&current, &name)?;
    }
    Ok(current)
}

#[cfg(unix)]
fn open_directory_at(parent: &File, name: &std::ffi::CStr) -> Result<File, std::io::Error> {
    use std::os::fd::{AsRawFd, FromRawFd};

    // SAFETY: parent remains open, name is NUL terminated, and the new descriptor is owned below.
    let descriptor =
        unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), directory_open_flags()) };
    if descriptor < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: descriptor is newly owned and is wrapped exactly once.
    Ok(unsafe { File::from_raw_fd(descriptor) })
}

#[cfg(unix)]
fn open_regular_file_at(parent: &File, leaf: &std::ffi::OsStr) -> Result<File, std::io::Error> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;

    let leaf = CString::new(leaf.as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    // SAFETY: parent remains open, leaf is NUL terminated, and the new descriptor is owned below.
    let descriptor =
        unsafe { libc::openat(parent.as_raw_fd(), leaf.as_ptr(), regular_file_open_flags()) };
    if descriptor < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: descriptor is newly owned and is wrapped exactly once.
    let file = unsafe { File::from_raw_fd(descriptor) };
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
    }
    Ok(file)
}

#[cfg(unix)]
fn inspect_entry(
    parent: &File,
    leaf: &std::ffi::OsStr,
    expected: &FileIdentity,
) -> Result<ActionEntryObservation, ActionBindingError> {
    use std::io::ErrorKind;

    match open_regular_file_at(parent, leaf) {
        Ok(file) => {
            let path_hint = Path::new(leaf);
            let identity = identity_for_open_file(
                &file,
                path_hint,
                IdentityNodeKind::File,
                ActionBindingError::DestinationConflict,
            )?;
            if identity.kind == expected.kind && identity.key == expected.key {
                Ok(ActionEntryObservation::ExpectedIdentity)
            } else {
                Ok(ActionEntryObservation::OtherEntry)
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(ActionEntryObservation::Missing),
        Err(_) => Ok(ActionEntryObservation::OtherEntry),
    }
}

#[cfg(unix)]
const fn directory_open_flags() -> libc::c_int {
    libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_NOCTTY
}

#[cfg(unix)]
const fn regular_file_open_flags() -> libc::c_int {
    libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NONBLOCK | libc::O_NOFOLLOW | libc::O_NOCTTY
}

#[cfg(unix)]
fn modified_unix_ns(metadata: &Metadata) -> Option<i64> {
    let duration = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_nanos()).ok()
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
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
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
    windows_identity_from_handle(handle.0, kind)
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

#[cfg(unix)]
fn platform_identity_for_open_file_impl(
    _file: &File,
    path: &Path,
    metadata: &Metadata,
    kind: IdentityNodeKind,
) -> Result<FileIdentity, std::io::Error> {
    platform_identity_impl(path, metadata, kind)
}

#[cfg(windows)]
fn platform_identity_for_open_file_impl(
    file: &File,
    _path: &Path,
    _metadata: &Metadata,
    kind: IdentityNodeKind,
) -> Result<FileIdentity, std::io::Error> {
    use std::os::windows::io::AsRawHandle;

    windows_identity_from_handle(file.as_raw_handle(), kind)
}

#[cfg(not(any(unix, windows)))]
fn platform_identity_for_open_file_impl(
    _file: &File,
    path: &Path,
    metadata: &Metadata,
    kind: IdentityNodeKind,
) -> Result<FileIdentity, std::io::Error> {
    platform_identity_impl(path, metadata, kind)
}

#[cfg(windows)]
fn windows_identity_from_handle(
    handle: windows_sys::Win32::Foundation::HANDLE,
    kind: IdentityNodeKind,
) -> Result<FileIdentity, std::io::Error> {
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: the caller supplies a live file handle and `information` is a correctly sized
    // writable output value. This helper borrows the handle and never closes it.
    if unsafe { GetFileInformationByHandle(handle, &mut information) } == 0 {
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

    #[cfg(unix)]
    #[test]
    fn action_binding_exposes_only_held_source_and_bound_facts() {
        use std::fs;
        use std::io::Read;

        let directory = tempfile::tempdir().expect("fixture should create");
        let requested_root = directory.path().join("authorized");
        let requested_parent = requested_root.join("inbox");
        fs::create_dir_all(&requested_parent).expect("fixture directories should create");
        let requested_source = requested_parent.join("Draft.txt");
        let contents = b"digest exactly these held bytes";
        fs::write(&requested_source, contents).expect("fixture source should write");

        // macOS commonly exposes /var through /private/var; the production API intentionally
        // requires its authorized root to have already been canonicalized.
        let root = fs::canonicalize(&requested_root).expect("root should canonicalize");
        let parent = root.join("inbox");
        let source = parent.join("Draft.txt");
        let root_identity = test_identity_for_path(&root, IdentityNodeKind::Folder);
        let parent_identity = test_identity_for_path(&parent, IdentityNodeKind::Folder);
        let source_identity = test_identity_for_path(&source, IdentityNodeKind::File);
        let source_metadata = fs::metadata(&source).expect("source metadata should load");

        let binding = bind_action_file(
            &root,
            &source,
            IdentityExpectation::from_identity(&root_identity),
            IdentityExpectation::from_identity(&parent_identity),
            IdentityExpectation::from_identity(&source_identity),
        )
        .expect("strong binding should succeed");

        let mut held_source = binding.source_file();
        let mut held_contents = Vec::new();
        held_source
            .read_to_end(&mut held_contents)
            .expect("held source should remain readable");

        assert_eq!(held_contents, contents);
        assert_eq!(binding.root_identity(), &root_identity);
        assert_eq!(binding.parent_identity(), &parent_identity);
        assert_eq!(binding.source_identity(), &source_identity);
        assert_eq!(binding.source_size_bytes(), contents.len() as u64);
        assert_eq!(
            binding.source_modified_unix_ns(),
            modified_unix_ns(&source_metadata)
        );
    }

    #[cfg(unix)]
    fn test_identity_for_path(path: &Path, kind: IdentityNodeKind) -> FileIdentity {
        let file = File::open(path).expect("fixture path should open");
        let metadata = file.metadata().expect("fixture metadata should load");
        platform_identity_for_open_file(&file, path, &metadata, kind)
            .expect("fixture identity should load")
    }
}
