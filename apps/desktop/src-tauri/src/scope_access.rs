use std::path::{Path, PathBuf};

const DEVELOPMENT_SELECTION_RECEIPT: &[u8] = b"deskgraph-development-native-selection-v1";
#[cfg(target_os = "macos")]
const MACOS_BOOKMARK_PREFIX: &[u8] = b"deskgraph-macos-security-scoped-bookmark-v1\0";

pub(crate) struct PreparedScopeAccess {
    pub(crate) platform: &'static str,
    pub(crate) opaque_grant: Vec<u8>,
    pub(crate) resolved_path: PathBuf,
    pub(crate) access: ActiveScopeAccess,
}

pub(crate) struct RestoredScopeAccess {
    pub(crate) resolved_path: Option<PathBuf>,
    pub(crate) refreshed_grant: Option<Vec<u8>>,
    pub(crate) access: ActiveScopeAccess,
}

#[cfg(target_os = "macos")]
pub(crate) struct ActiveScopeAccess {
    security_scoped_url: Option<objc2::rc::Retained<objc2_foundation::NSURL>>,
}

#[cfg(not(target_os = "macos"))]
pub(crate) struct ActiveScopeAccess;

#[cfg(target_os = "macos")]
impl Drop for ActiveScopeAccess {
    fn drop(&mut self) {
        if let Some(url) = self.security_scoped_url.take() {
            // SAFETY: `restore_macos_bookmark` records the URL only after a
            // successful start call. This is its single balanced stop call.
            unsafe { url.stopAccessingSecurityScopedResource() };
        }
    }
}

pub(crate) fn prepare_selected_scope(
    selected_path: &Path,
) -> Result<PreparedScopeAccess, &'static str> {
    #[cfg(target_os = "macos")]
    {
        match prepare_macos_bookmark(selected_path) {
            Ok(prepared) => Ok(prepared),
            Err(_) if cfg!(debug_assertions) => Ok(PreparedScopeAccess {
                platform: std::env::consts::OS,
                opaque_grant: DEVELOPMENT_SELECTION_RECEIPT.to_vec(),
                resolved_path: selected_path.to_path_buf(),
                access: ActiveScopeAccess {
                    security_scoped_url: None,
                },
            }),
            Err(_) => Err("scope_bookmark_create_failed"),
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        if cfg!(debug_assertions) {
            Ok(PreparedScopeAccess {
                platform: std::env::consts::OS,
                opaque_grant: DEVELOPMENT_SELECTION_RECEIPT.to_vec(),
                resolved_path: selected_path.to_path_buf(),
                access: ActiveScopeAccess,
            })
        } else {
            Err("scope_access_grant_unavailable")
        }
    }
}

pub(crate) fn restore_scope_access(
    platform: &str,
    opaque_grant: &[u8],
) -> Result<RestoredScopeAccess, &'static str> {
    #[cfg(target_os = "macos")]
    {
        if platform != std::env::consts::OS {
            return Err("scope_access_grant_unavailable");
        }
        if cfg!(debug_assertions) && opaque_grant == DEVELOPMENT_SELECTION_RECEIPT {
            return Ok(RestoredScopeAccess {
                resolved_path: None,
                refreshed_grant: None,
                access: ActiveScopeAccess {
                    security_scoped_url: None,
                },
            });
        }
        restore_macos_bookmark(opaque_grant)
    }

    #[cfg(not(target_os = "macos"))]
    {
        if platform != std::env::consts::OS
            || !cfg!(debug_assertions)
            || opaque_grant != DEVELOPMENT_SELECTION_RECEIPT
        {
            return Err("scope_access_grant_unavailable");
        }
        Ok(RestoredScopeAccess {
            resolved_path: None,
            refreshed_grant: None,
            access: ActiveScopeAccess,
        })
    }
}

#[cfg(target_os = "macos")]
fn prepare_macos_bookmark(selected_path: &Path) -> Result<PreparedScopeAccess, &'static str> {
    use objc2_foundation::NSURLBookmarkCreationOptions;

    let selected_url = file_url(selected_path)?;
    let bookmark = selected_url
        .bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(
            NSURLBookmarkCreationOptions::WithSecurityScope,
            None,
            None,
        )
        .map_err(|_| "scope_bookmark_create_failed")?;
    let bookmark_bytes = bookmark.to_vec();
    if bookmark_bytes.is_empty() {
        return Err("scope_bookmark_create_failed");
    }
    let opaque_grant = wrap_macos_bookmark(&bookmark_bytes);

    let restored = restore_macos_bookmark(&opaque_grant)?;
    let resolved_path = restored
        .resolved_path
        .clone()
        .ok_or("scope_bookmark_resolve_failed")?;
    Ok(PreparedScopeAccess {
        platform: std::env::consts::OS,
        opaque_grant,
        resolved_path,
        access: restored.access,
    })
}

#[cfg(target_os = "macos")]
fn restore_macos_bookmark(opaque_grant: &[u8]) -> Result<RestoredScopeAccess, &'static str> {
    use objc2::runtime::Bool;
    use objc2_foundation::{NSData, NSURL, NSURLBookmarkResolutionOptions};

    let bookmark_bytes = opaque_grant
        .strip_prefix(MACOS_BOOKMARK_PREFIX)
        .filter(|bytes| !bytes.is_empty())
        .ok_or("scope_bookmark_format_unsupported")?;
    let bookmark = NSData::with_bytes(bookmark_bytes);
    let mut is_stale = Bool::NO;
    let options = NSURLBookmarkResolutionOptions::WithSecurityScope
        | NSURLBookmarkResolutionOptions::WithoutImplicitStartAccessing;
    // SAFETY: `is_stale` is a valid writable pointer for the duration of the
    // call and the retained NSData outlives the Objective-C invocation.
    let url = unsafe {
        NSURL::URLByResolvingBookmarkData_options_relativeToURL_bookmarkDataIsStale_error(
            &bookmark,
            options,
            None,
            &mut is_stale,
        )
    }
    .map_err(|_| "scope_bookmark_resolve_failed")?;
    // SAFETY: Foundation owns the retained URL. A true result is balanced by
    // `ActiveScopeAccess::drop`; false creates no retained access claim.
    if !unsafe { url.startAccessingSecurityScopedResource() } {
        return Err("scope_bookmark_access_denied");
    }

    let access = ActiveScopeAccess {
        security_scoped_url: Some(url),
    };
    let active_url = access
        .security_scoped_url
        .as_deref()
        .ok_or("scope_bookmark_access_denied")?;
    let resolved_path = path_from_url(active_url)?;
    let refreshed_grant = if is_stale.as_bool() {
        let refreshed = active_url
            .bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(
                objc2_foundation::NSURLBookmarkCreationOptions::WithSecurityScope,
                None,
                None,
            )
            .map_err(|_| "scope_bookmark_refresh_failed")?
            .to_vec();
        if refreshed.is_empty() {
            return Err("scope_bookmark_refresh_failed");
        }
        Some(wrap_macos_bookmark(&refreshed))
    } else {
        None
    };

    Ok(RestoredScopeAccess {
        resolved_path: Some(resolved_path),
        refreshed_grant,
        access,
    })
}

#[cfg(target_os = "macos")]
fn wrap_macos_bookmark(bookmark_bytes: &[u8]) -> Vec<u8> {
    let mut opaque_grant = Vec::with_capacity(MACOS_BOOKMARK_PREFIX.len() + bookmark_bytes.len());
    opaque_grant.extend_from_slice(MACOS_BOOKMARK_PREFIX);
    opaque_grant.extend_from_slice(bookmark_bytes);
    opaque_grant
}

#[cfg(target_os = "macos")]
fn file_url(path: &Path) -> Result<objc2::rc::Retained<objc2_foundation::NSURL>, &'static str> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::ptr::NonNull;

    let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| "scope_path_invalid")?;
    let pointer = NonNull::new(path.as_ptr().cast_mut()).ok_or("scope_path_invalid")?;
    // SAFETY: CString guarantees a valid NUL-terminated filesystem byte
    // representation for the duration of this class-method call.
    Ok(unsafe {
        objc2_foundation::NSURL::fileURLWithFileSystemRepresentation_isDirectory_relativeToURL(
            pointer, true, None,
        )
    })
}

#[cfg(target_os = "macos")]
fn path_from_url(url: &objc2_foundation::NSURL) -> Result<PathBuf, &'static str> {
    use std::ffi::{CStr, OsString};
    use std::os::unix::ffi::OsStringExt;

    let pointer = url.fileSystemRepresentation();
    // SAFETY: Foundation documents this as a NUL-terminated filesystem
    // representation whose lifetime is tied to `url`, borrowed above.
    let bytes = unsafe { CStr::from_ptr(pointer.as_ptr()) }.to_bytes();
    if bytes.is_empty() {
        return Err("scope_bookmark_resolve_failed");
    }
    Ok(PathBuf::from(OsString::from_vec(bytes.to_vec())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_grants_fail_closed_for_unknown_platforms() {
        let error = restore_scope_access("unknown", b"opaque")
            .err()
            .expect("unknown grant should fail closed");
        assert_eq!(error, "scope_access_grant_unavailable");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_grants_require_a_versioned_nonempty_bookmark_envelope() {
        for opaque_grant in [b"unversioned-bookmark".as_slice(), MACOS_BOOKMARK_PREFIX] {
            let error = restore_scope_access(std::env::consts::OS, opaque_grant)
                .err()
                .expect("unsupported bookmark envelope should fail closed");
            assert_eq!(error, "scope_bookmark_format_unsupported");
        }
    }

    #[cfg(all(not(target_os = "macos"), debug_assertions))]
    #[test]
    fn non_macos_development_receipt_is_debug_only_and_path_free() {
        let restored = restore_scope_access(std::env::consts::OS, DEVELOPMENT_SELECTION_RECEIPT)
            .expect("development selection receipt should restore in debug builds");
        assert!(restored.resolved_path.is_none());
        assert!(restored.refreshed_grant.is_none());
    }

    #[cfg(all(target_os = "macos", debug_assertions))]
    #[test]
    fn development_receipt_is_explicitly_debug_only_and_path_free() {
        let restored = restore_scope_access(std::env::consts::OS, DEVELOPMENT_SELECTION_RECEIPT)
            .expect("debug build should restore its development-only receipt");
        assert!(restored.resolved_path.is_none());
        assert!(restored.refreshed_grant.is_none());
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn release_build_rejects_development_receipts() {
        assert!(restore_scope_access(std::env::consts::OS, DEVELOPMENT_SELECTION_RECEIPT).is_err());
    }
}
