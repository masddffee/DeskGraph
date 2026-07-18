use std::ffi::OsStr;
use std::fmt;

use deskgraph_identity::{ActionBindingError, ActionFileBinding, ActionFileObservation};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformRenameError {
    Binding(ActionBindingError),
    Unsupported,
    DestinationConflict,
    RenameFailed,
    DirectorySyncFailed,
    DestinationVerificationFailed,
}

impl PlatformRenameError {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Binding(error) => error.code(),
            Self::Unsupported => "action_platform_rename_unsupported",
            Self::DestinationConflict => "action_platform_destination_conflict",
            Self::RenameFailed => "action_platform_rename_failed",
            Self::DirectorySyncFailed => "action_platform_directory_sync_failed",
            Self::DestinationVerificationFailed => {
                "action_platform_destination_verification_failed"
            }
        }
    }
}

impl fmt::Display for PlatformRenameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for PlatformRenameError {}

impl From<ActionBindingError> for PlatformRenameError {
    fn from(error: ActionBindingError) -> Self {
        Self::Binding(error)
    }
}

pub fn rename_same_parent_no_replace(
    binding: &mut ActionFileBinding,
    destination_name: &OsStr,
) -> Result<ActionFileObservation, PlatformRenameError> {
    #[cfg(test)]
    {
        rename_same_parent_no_replace_impl(binding, destination_name)
    }
    #[cfg(not(test))]
    {
        let _ = binding;
        let _ = destination_name;
        Err(PlatformRenameError::Unsupported)
    }
}

/// The Unix leaf-name prototype is deliberately test-only. Neither macOS nor
/// Linux exposes an accepted atomic "rename this exact open inode" primitive;
/// production stays unavailable until a superseding platform decision closes
/// that race. Windows is independently unavailable pending its handle adapter.
pub const fn direct_rename_supported() -> bool {
    cfg!(all(
        test,
        any(
            target_os = "macos",
            all(target_os = "linux", target_env = "gnu")
        )
    ))
}

/// Makes an already-observed namespace state durable before recovery records
/// a terminal journal event. This is intentionally descriptor-bound; recovery
/// never reopens or syncs an arbitrary path supplied by a caller.
pub fn sync_action_parent(binding: &ActionFileBinding) -> Result<(), PlatformRenameError> {
    #[cfg(test)]
    {
        sync_action_parent_impl(binding)
    }
    #[cfg(not(test))]
    {
        let _ = binding;
        Err(PlatformRenameError::Unsupported)
    }
}

#[cfg(all(test, target_os = "macos"))]
fn rename_same_parent_no_replace_impl(
    binding: &mut ActionFileBinding,
    destination_name: &OsStr,
) -> Result<ActionFileObservation, PlatformRenameError> {
    use std::ffi::CString;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;

    let target = binding.prepare_absent_destination(destination_name)?;
    binding.revalidate_for_rename(&target)?;
    let source = CString::new(binding.current_leaf().as_bytes())
        .map_err(|_| PlatformRenameError::RenameFailed)?;
    let destination =
        CString::new(target.leaf().as_bytes()).map_err(|_| PlatformRenameError::RenameFailed)?;
    let parent_descriptor = binding.parent_file().as_raw_fd();
    // Darwin declares this in sys/stdio.h, but libc 0.2.186 does not expose it.
    const RENAME_NOFOLLOW_ANY: libc::c_uint = 0x0000_0010;

    // SAFETY: both leaf names are NUL terminated, both are resolved relative to the held and
    // identity-validated same-parent descriptor, and RENAME_EXCL prevents replacement. The
    // no-follow flag makes a concurrent symlink substitution fail rather than be traversed.
    let result = unsafe {
        libc::renameatx_np(
            parent_descriptor,
            source.as_ptr(),
            parent_descriptor,
            destination.as_ptr(),
            libc::RENAME_EXCL | RENAME_NOFOLLOW_ANY,
        )
    };
    if result != 0 {
        return Err(map_unix_rename_error(std::io::Error::last_os_error()));
    }

    let sync_result = sync_parent(binding);
    let observation = binding
        .observe_renamed_to(target)
        .map_err(|_| PlatformRenameError::DestinationVerificationFailed)?;
    sync_result?;
    Ok(observation)
}

#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
fn rename_same_parent_no_replace_impl(
    binding: &mut ActionFileBinding,
    destination_name: &OsStr,
) -> Result<ActionFileObservation, PlatformRenameError> {
    use std::ffi::CString;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;

    let target = binding.prepare_absent_destination(destination_name)?;
    binding.revalidate_for_rename(&target)?;
    let source = CString::new(binding.current_leaf().as_bytes())
        .map_err(|_| PlatformRenameError::RenameFailed)?;
    let destination =
        CString::new(target.leaf().as_bytes()).map_err(|_| PlatformRenameError::RenameFailed)?;
    let parent_descriptor = binding.parent_file().as_raw_fd();

    // SAFETY: both leaf names are NUL terminated and resolved relative to the held and
    // identity-validated same-parent descriptor. RENAME_NOREPLACE is the only accepted Linux
    // operation; unsupported kernels/filesystems fail closed below.
    let result = unsafe {
        libc::renameat2(
            parent_descriptor,
            source.as_ptr(),
            parent_descriptor,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result != 0 {
        return Err(map_unix_rename_error(std::io::Error::last_os_error()));
    }

    let sync_result = sync_parent(binding);
    let observation = binding
        .observe_renamed_to(target)
        .map_err(|_| PlatformRenameError::DestinationVerificationFailed)?;
    sync_result?;
    Ok(observation)
}

#[cfg(all(
    test,
    any(target_os = "macos", all(target_os = "linux", target_env = "gnu"))
))]
fn sync_parent(binding: &ActionFileBinding) -> Result<(), PlatformRenameError> {
    use std::os::fd::AsRawFd;

    // SAFETY: the descriptor is owned by binding and remains live for the call.
    if unsafe { libc::fsync(binding.parent_file().as_raw_fd()) } != 0 {
        return Err(PlatformRenameError::DirectorySyncFailed);
    }
    Ok(())
}

#[cfg(all(
    test,
    any(target_os = "macos", all(target_os = "linux", target_env = "gnu"))
))]
fn sync_action_parent_impl(binding: &ActionFileBinding) -> Result<(), PlatformRenameError> {
    sync_parent(binding)
}

#[cfg(all(
    test,
    not(any(target_os = "macos", all(target_os = "linux", target_env = "gnu")))
))]
fn sync_action_parent_impl(_binding: &ActionFileBinding) -> Result<(), PlatformRenameError> {
    Err(PlatformRenameError::Unsupported)
}

#[cfg(all(
    test,
    any(target_os = "macos", all(target_os = "linux", target_env = "gnu"))
))]
fn map_unix_rename_error(error: std::io::Error) -> PlatformRenameError {
    match error.raw_os_error() {
        Some(libc::EEXIST) => PlatformRenameError::DestinationConflict,
        Some(libc::ENOSYS) | Some(libc::EINVAL) | Some(libc::ENOTSUP) => {
            PlatformRenameError::Unsupported
        }
        _ => PlatformRenameError::RenameFailed,
    }
}

#[cfg(all(
    test,
    not(any(target_os = "macos", all(target_os = "linux", target_env = "gnu")))
))]
fn rename_same_parent_no_replace_impl(
    _binding: &mut ActionFileBinding,
    _destination_name: &OsStr,
) -> Result<ActionFileObservation, PlatformRenameError> {
    // Windows intentionally remains fail closed here. A future implementation must construct
    // the source with DELETE | FILE_READ_ATTRIBUTES and FILE_FLAG_OPEN_REPARSE_POINT, retain an
    // identity-validated parent HANDLE, and use SetFileInformationByHandle(FileRenameInfo) with
    // ReplaceIfExists=false and RootDirectory=parent. No path-based fallback is permitted.
    Err(PlatformRenameError::Unsupported)
}

#[cfg(all(
    test,
    any(target_os = "macos", all(target_os = "linux", target_env = "gnu"))
))]
mod tests {
    use super::*;
    use deskgraph_identity::{
        ActionBindingError, ActionEntryObservation, IdentityExpectation, IdentityNodeKind,
        bind_action_file, platform_identity_for_open_file,
    };
    use std::ffi::CString;
    use std::fs::{self, File};
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    struct Fixture {
        _directory: tempfile::TempDir,
        root: PathBuf,
        parent: PathBuf,
        source: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().expect("fixture should create");
            let requested_root = directory.path().join("authorized");
            let requested_parent = requested_root.join("inbox");
            fs::create_dir(&requested_root).expect("root should create");
            fs::create_dir(&requested_parent).expect("parent should create");
            fs::write(
                requested_parent.join("Draft.txt"),
                "identity-bound rename fixture",
            )
            .expect("source should write");
            let root = fs::canonicalize(&requested_root).expect("root should canonicalize");
            let parent = root.join("inbox");
            let source = parent.join("Draft.txt");
            Self {
                _directory: directory,
                root,
                parent,
                source,
            }
        }

        fn binding(&self) -> Result<ActionFileBinding, ActionBindingError> {
            let root_identity = identity_for_path(&self.root, IdentityNodeKind::Folder);
            let parent_identity = identity_for_path(&self.parent, IdentityNodeKind::Folder);
            let source_identity = identity_for_path(&self.source, IdentityNodeKind::File);
            bind_action_file(
                &self.root,
                &self.source,
                IdentityExpectation::from_identity(&root_identity),
                IdentityExpectation::from_identity(&parent_identity),
                IdentityExpectation::from_identity(&source_identity),
            )
        }
    }

    fn identity_for_path(path: &Path, kind: IdentityNodeKind) -> deskgraph_identity::FileIdentity {
        let file = File::open(path).expect("fixture path should open");
        let metadata = file.metadata().expect("fixture metadata should load");
        platform_identity_for_open_file(&file, path, &metadata, kind)
            .expect("fixture identity should load")
    }

    fn fixture_rename(source: &Path, destination: &Path) {
        let source =
            CString::new(source.as_os_str().as_bytes()).expect("source path should encode");
        let destination = CString::new(destination.as_os_str().as_bytes())
            .expect("destination path should encode");
        // SAFETY: both fixture paths are valid NUL-terminated strings and destination is absent.
        assert_eq!(
            unsafe { libc::rename(source.as_ptr(), destination.as_ptr()) },
            0
        );
    }

    #[test]
    fn direct_no_replace_rename_observes_destination_and_supports_inverse() {
        let fixture = Fixture::new();
        let mut binding = fixture.binding().expect("binding should succeed");
        let destination = fixture.parent.join("Final.txt");

        let observation = rename_same_parent_no_replace(&mut binding, OsStr::new("Final.txt"))
            .expect("rename should succeed");
        assert_eq!(observation.current, ActionEntryObservation::Missing);
        assert_eq!(
            observation.alternate,
            ActionEntryObservation::ExpectedIdentity
        );
        assert!(!fixture.source.exists());
        assert_eq!(
            fs::read_to_string(&destination).expect("destination should remain readable"),
            "identity-bound rename fixture"
        );
        assert_eq!(
            binding
                .observe_current_and(OsStr::new("Draft.txt"))
                .expect("post-rename observation should succeed"),
            ActionFileObservation {
                current: ActionEntryObservation::ExpectedIdentity,
                alternate: ActionEntryObservation::Missing,
            }
        );

        rename_same_parent_no_replace(&mut binding, OsStr::new("Draft.txt"))
            .expect("inverse rename should succeed");
        assert!(fixture.source.exists());
        assert!(!destination.exists());
    }

    #[test]
    fn destination_created_after_binding_is_never_replaced() {
        let fixture = Fixture::new();
        let mut binding = fixture.binding().expect("binding should succeed");
        let destination = fixture.parent.join("Final.txt");
        fs::write(&destination, "unrelated destination").expect("conflict should write");

        let error = rename_same_parent_no_replace(&mut binding, OsStr::new("Final.txt"))
            .expect_err("destination conflict should fail closed");
        assert!(matches!(
            error,
            PlatformRenameError::Binding(ActionBindingError::DestinationConflict)
                | PlatformRenameError::DestinationConflict
        ));
        assert_eq!(
            fs::read_to_string(&destination).expect("conflict should remain"),
            "unrelated destination"
        );
        assert!(fixture.source.exists());
    }

    #[test]
    fn hard_link_and_case_only_actions_are_denied() {
        let fixture = Fixture::new();
        fs::hard_link(&fixture.source, fixture.parent.join("alias.txt"))
            .expect("hard link should create");
        assert_eq!(
            fixture.binding().expect_err("hard link should fail closed"),
            ActionBindingError::SourceHasMultipleLinks
        );

        let single = Fixture::new();
        let mut binding = single.binding().expect("binding should succeed");
        assert_eq!(
            rename_same_parent_no_replace(&mut binding, OsStr::new("draft.TXT"))
                .expect_err("case-only action should fail")
                .code(),
            "action_binding_case_only_rename_denied"
        );
    }

    #[test]
    fn symlink_parent_and_leaf_are_never_followed() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        let parent_alias = fixture.root.join("parent-alias");
        symlink(&fixture.parent, &parent_alias).expect("parent symlink should create");
        let root_identity = identity_for_path(&fixture.root, IdentityNodeKind::Folder);
        let parent_identity = identity_for_path(&fixture.parent, IdentityNodeKind::Folder);
        let source_identity = identity_for_path(&fixture.source, IdentityNodeKind::File);
        assert_eq!(
            bind_action_file(
                &fixture.root,
                &parent_alias.join("Draft.txt"),
                IdentityExpectation::from_identity(&root_identity),
                IdentityExpectation::from_identity(&parent_identity),
                IdentityExpectation::from_identity(&source_identity),
            )
            .expect_err("symlinked parent should fail closed"),
            ActionBindingError::ParentUnavailable
        );

        let leaf_alias = fixture.parent.join("leaf-alias.txt");
        symlink(&fixture.source, &leaf_alias).expect("leaf symlink should create");
        assert_eq!(
            bind_action_file(
                &fixture.root,
                &leaf_alias,
                IdentityExpectation::from_identity(&root_identity),
                IdentityExpectation::from_identity(&parent_identity),
                IdentityExpectation::from_identity(&source_identity),
            )
            .expect_err("symlinked leaf should fail closed"),
            ActionBindingError::SourceUnavailable
        );
    }

    #[test]
    fn source_parent_and_root_replacement_are_detected_before_mutation() {
        let source_fixture = Fixture::new();
        let mut source_binding = source_fixture.binding().expect("binding should succeed");
        let displaced_source = source_fixture.parent.join("displaced.txt");
        fixture_rename(&source_fixture.source, &displaced_source);
        fs::write(&source_fixture.source, "replacement").expect("replacement should write");
        assert_eq!(
            rename_same_parent_no_replace(&mut source_binding, OsStr::new("Final.txt"))
                .expect_err("source replacement should fail")
                .code(),
            "action_binding_source_identity_changed"
        );

        let parent_fixture = Fixture::new();
        let mut parent_binding = parent_fixture.binding().expect("binding should succeed");
        let displaced_parent = parent_fixture.root.join("displaced-parent");
        fixture_rename(&parent_fixture.parent, &displaced_parent);
        fs::create_dir(&parent_fixture.parent).expect("replacement parent should create");
        assert_eq!(
            rename_same_parent_no_replace(&mut parent_binding, OsStr::new("Final.txt"))
                .expect_err("parent replacement should fail")
                .code(),
            "action_binding_parent_identity_changed"
        );
        assert!(displaced_parent.join("Draft.txt").exists());

        let root_fixture = Fixture::new();
        let mut root_binding = root_fixture.binding().expect("binding should succeed");
        let displaced_root = root_fixture.root.with_extension("displaced");
        fixture_rename(&root_fixture.root, &displaced_root);
        fs::create_dir(&root_fixture.root).expect("replacement root should create");
        assert_eq!(
            rename_same_parent_no_replace(&mut root_binding, OsStr::new("Final.txt"))
                .expect_err("root replacement should fail")
                .code(),
            "action_binding_root_identity_changed"
        );
        assert!(displaced_root.join("inbox/Draft.txt").exists());
    }
}
