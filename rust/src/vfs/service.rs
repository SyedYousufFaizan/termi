//! Capability-aware VFS service facade — Phase 1d
//!
//! ## Why this file exists
//!
//! Before this pass, `vfs::capabilities` had a fully worked-out capability
//! model (`check_operation`, `VfsCapabilities::saf_external()`, etc.) and
//! `vfs::mount::MountTable` knew which capabilities applied to which path —
//! but **nothing in the codebase actually called `check_operation` before
//! dispatching to a provider.** `SafProvider` just hard-failed each method
//! individually with its own hand-written error message. The capability
//! system and the actual filesystem calls were two parallel systems that
//! happened to agree, by construction, rather than one system enforcing
//! the other.
//!
//! `VfsService` is the missing wiring: every operation goes
//! capability-check → then, and only then, provider dispatch. This is also
//! the concrete mechanism behind the roadmap item "surface inline warnings
//! in the terminal itself instead of failing silently" — `VfsOutcome`
//! carries a `hint` field that the terminal/UI layer can render directly
//! as a banner, instead of the user seeing a bare `Err` and having to go
//! read `docs/LIMITATIONS.md` to understand why `chmod` did nothing.
//!
//! ## What's NOT here yet
//!
//! `VfsService` operates on a `MountTable` plus a provider registry that
//! the caller populates. The actual Android SAF provider it will be wired
//! to (`vfs::android_saf::SafProvider`) still returns "not implemented" for
//! everything at the JNI layer — that part requires a live JNI environment
//! and is out of scope for a host-testable pass. What's tested here is the
//! *policy layer*: given any provider (a real one, or the `MockProvider`
//! below standing in for SAF), does the capability check correctly block,
//! degrade, or allow the operation, and does it produce a hint a UI could
//! show? That policy layer is 100% of what makes the "tell the user
//! upfront" UX work, regardless of which provider is plugged in underneath.

use crate::utils::error::{VfsError, VfsResult};
use crate::vfs::capabilities::{check_operation, OperationCheck, VfsOperation};
use crate::vfs::mount::{MountPoint, MountTable};
use crate::vfs::provider::{DirEntry, FileMetadata, FsProvider};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The result of attempting a VFS operation through the capability layer.
///
/// This is deliberately a three-way outcome rather than a plain
/// `Result<T, E>`: `Degraded` exists because some operations (notably
/// rename on SAF) *work* but with a caveat the user should see — collapsing
/// that into a boolean success/failure would either hide the caveat or
/// incorrectly treat it as a failure.
#[derive(Debug)]
pub enum VfsOutcome<T> {
    /// Operation succeeded with no caveats.
    Ok(T),
    /// Operation was not attempted because the capability check failed.
    /// `hint` is a suggested next step suitable for showing directly in
    /// the terminal UI (e.g. "move this project to internal storage").
    Blocked {
        operation: VfsOperation,
        reason: String,
        hint: Option<String>,
    },
    /// Operation was attempted and succeeded, but with a caveat the user
    /// should know about (e.g. "rename is not atomic here").
    Degraded { value: T, caveat: String },
}

impl<T> VfsOutcome<T> {
    /// Collapse into a plain `Result`, discarding the hint/caveat text.
    /// Useful for callers that only care about pass/fail (e.g. internal
    /// plumbing), not for anything that's about to talk to a user.
    pub fn into_result(self) -> VfsResult<T> {
        match self {
            VfsOutcome::Ok(v) => Ok(v),
            VfsOutcome::Degraded { value, .. } => Ok(value),
            VfsOutcome::Blocked {
                operation, reason, ..
            } => Err(VfsError::OperationNotSupported { operation, reason }),
        }
    }

    pub fn is_blocked(&self) -> bool {
        matches!(self, VfsOutcome::Blocked { .. })
    }

    pub fn is_degraded(&self) -> bool {
        matches!(self, VfsOutcome::Degraded { .. })
    }
}

/// Ties a [`MountTable`] (path → capabilities) to a registry of live
/// [`FsProvider`] instances (path → actual I/O implementation), and
/// enforces the former before invoking the latter.
pub struct VfsService {
    mounts: MountTable,
    providers: HashMap<PathBuf, Arc<dyn FsProvider>>,
}

impl VfsService {
    /// Create a new service with the given internal storage provider
    /// mounted at `/`.
    pub fn new(internal_root: impl Into<PathBuf>, internal_provider: Arc<dyn FsProvider>) -> Self {
        let root = internal_root.into();
        let mut providers = HashMap::new();
        providers.insert(PathBuf::from("/"), internal_provider);
        Self {
            mounts: MountTable::new(root),
            providers,
        }
    }

    /// Mount an additional provider (e.g. a SAF-backed external storage
    /// mount) at the given virtual path.
    pub fn mount_provider(
        &mut self,
        mount: MountPoint,
        provider: Arc<dyn FsProvider>,
    ) -> VfsResult<()> {
        let path = mount.virtual_path.clone();
        self.mounts.mount(mount)?;
        self.providers.insert(path, provider);
        Ok(())
    }

    /// Remove a mount and its associated provider.
    pub fn unmount(&mut self, virtual_path: &Path) -> VfsResult<()> {
        self.mounts.unmount(virtual_path)?;
        self.providers.remove(virtual_path);
        Ok(())
    }

    /// Expose the underlying mount table read-only (used by `vfs::health`
    /// to scan all mounts for stale permissions).
    pub fn mounts(&self) -> &MountTable {
        &self.mounts
    }

    fn resolve_provider(
        &self,
        virtual_path: &Path,
    ) -> VfsResult<(&MountPoint, Arc<dyn FsProvider>, PathBuf)> {
        let (mount, relative) = self.mounts.resolve(virtual_path)?;
        let provider = self
            .providers
            .get(&mount.virtual_path)
            .cloned()
            .ok_or_else(|| VfsError::NotMounted(virtual_path.to_string_lossy().into()))?;
        Ok((mount, provider, relative))
    }

    /// Suggested next step for operations that commonly block on SAF.
    /// Kept centralized so the wording is consistent everywhere it surfaces
    /// (terminal inline warning, settings screen, health-check prompt).
    fn hint_for(op: VfsOperation) -> Option<String> {
        match op {
            VfsOperation::Chmod | VfsOperation::Chown => Some(
                "Permissions can't be changed on external storage. Move this file to internal \
                 storage if the tool you're using requires chmod to work."
                    .into(),
            ),
            VfsOperation::Symlink | VfsOperation::Hardlink => Some(
                "Symbolic links aren't supported on external storage — this will break npm, \
                 yarn, and Python venv. Run this project from internal storage instead."
                    .into(),
            ),
            VfsOperation::Watch => Some(
                "File watching isn't supported here, so live-reload tools won't fire. \
                 Move the project to internal storage if you need file watching."
                    .into(),
            ),
            VfsOperation::Mmap | VfsOperation::Lock => Some(
                "This operation needs direct file-descriptor access, which external storage \
                 doesn't provide. Internal storage supports it."
                    .into(),
            ),
            _ => None,
        }
    }

    fn dispatch<T>(
        &self,
        path: &Path,
        op: VfsOperation,
        f: impl FnOnce(&dyn FsProvider, &Path) -> VfsResult<T>,
    ) -> VfsOutcome<T> {
        let (mount, provider, relative) = match self.resolve_provider(path) {
            Ok(v) => v,
            Err(e) => {
                return VfsOutcome::Blocked {
                    operation: op,
                    reason: e.to_string(),
                    hint: None,
                }
            }
        };

        match check_operation(path, op, &mount.capabilities) {
            OperationCheck::NotSupported { operation, reason } => VfsOutcome::Blocked {
                hint: Self::hint_for(operation),
                operation,
                reason,
            },
            OperationCheck::Supported => match f(provider.as_ref(), &relative) {
                Ok(v) => VfsOutcome::Ok(v),
                Err(e) => VfsOutcome::Blocked {
                    operation: op,
                    reason: e.to_string(),
                    hint: None,
                },
            },
            OperationCheck::PartialSupport { operation, caveat } => {
                match f(provider.as_ref(), &relative) {
                    Ok(v) => VfsOutcome::Degraded { value: v, caveat },
                    Err(e) => VfsOutcome::Blocked {
                        operation,
                        reason: e.to_string(),
                        hint: None,
                    },
                }
            }
        }
    }

    pub fn read_file(&self, path: &Path) -> VfsOutcome<Vec<u8>> {
        self.dispatch(path, VfsOperation::Read, |p, rel| p.read_file(rel))
    }

    pub fn write_file(&self, path: &Path, contents: &[u8]) -> VfsOutcome<()> {
        self.dispatch(path, VfsOperation::Write, |p, rel| {
            p.write_file(rel, contents)
        })
    }

    pub fn metadata(&self, path: &Path) -> VfsOutcome<FileMetadata> {
        self.dispatch(path, VfsOperation::Stat, |p, rel| p.metadata(rel))
    }

    pub fn list_dir(&self, path: &Path) -> VfsOutcome<Vec<DirEntry>> {
        self.dispatch(path, VfsOperation::ListDir, |p, rel| p.list_dir(rel))
    }

    pub fn create_dir(&self, path: &Path) -> VfsOutcome<()> {
        self.dispatch(path, VfsOperation::Mkdir, |p, rel| p.create_dir(rel))
    }

    pub fn delete(&self, path: &Path) -> VfsOutcome<()> {
        self.dispatch(path, VfsOperation::Delete, |p, rel| p.delete(rel))
    }

    pub fn chmod(&self, path: &Path, mode: u32) -> VfsOutcome<()> {
        self.dispatch(path, VfsOperation::Chmod, |p, rel| p.chmod(rel, mode))
    }

    pub fn readlink(&self, path: &Path) -> VfsOutcome<PathBuf> {
        self.dispatch(path, VfsOperation::Symlink, |p, rel| p.readlink(rel))
    }

    /// Create a symlink at `link` pointing to `target`. Capability is
    /// checked against `link`'s mount (the filesystem the link itself will
    /// live on) — the target may live elsewhere and that's fine, same as
    /// real Unix symlinks.
    pub fn symlink(&self, target: &Path, link: &Path) -> VfsOutcome<()> {
        let (mount, provider, relative_link) = match self.resolve_provider(link) {
            Ok(v) => v,
            Err(e) => {
                return VfsOutcome::Blocked {
                    operation: VfsOperation::Symlink,
                    reason: e.to_string(),
                    hint: None,
                }
            }
        };

        match check_operation(link, VfsOperation::Symlink, &mount.capabilities) {
            OperationCheck::NotSupported { operation, reason } => VfsOutcome::Blocked {
                hint: Self::hint_for(operation),
                operation,
                reason,
            },
            OperationCheck::Supported => match provider.symlink(target, &relative_link) {
                Ok(()) => VfsOutcome::Ok(()),
                Err(e) => VfsOutcome::Blocked {
                    operation: VfsOperation::Symlink,
                    reason: e.to_string(),
                    hint: None,
                },
            },
            OperationCheck::PartialSupport { operation, caveat } => {
                match provider.symlink(target, &relative_link) {
                    Ok(()) => VfsOutcome::Degraded { value: (), caveat },
                    Err(e) => VfsOutcome::Blocked {
                        operation,
                        reason: e.to_string(),
                        hint: None,
                    },
                }
            }
        }
    }

    /// Rename/move a file. Capability + provider dispatch happen against
    /// `from`'s mount. Cross-mount rename (e.g. internal → SAF) is not yet
    /// supported and returns `Blocked` — this is a known Phase 2+ gap
    /// (would need a copy+delete fallback), not a silent failure.
    pub fn rename(&self, from: &Path, to: &Path) -> VfsOutcome<()> {
        let (from_mount, from_provider, from_rel) = match self.resolve_provider(from) {
            Ok(v) => v,
            Err(e) => {
                return VfsOutcome::Blocked {
                    operation: VfsOperation::Rename,
                    reason: e.to_string(),
                    hint: None,
                }
            }
        };
        let (to_mount, _, to_rel) = match self.resolve_provider(to) {
            Ok(v) => v,
            Err(e) => {
                return VfsOutcome::Blocked {
                    operation: VfsOperation::Rename,
                    reason: e.to_string(),
                    hint: None,
                }
            }
        };

        if from_mount.virtual_path != to_mount.virtual_path {
            return VfsOutcome::Blocked {
                operation: VfsOperation::Rename,
                reason: "Cross-mount rename is not supported yet (from and to are on different \
                         filesystems)."
                    .into(),
                hint: Some(
                    "Copy the file to the destination and delete the original instead.".into(),
                ),
            };
        }

        match check_operation(from, VfsOperation::Rename, &from_mount.capabilities) {
            OperationCheck::NotSupported { operation, reason } => VfsOutcome::Blocked {
                hint: Self::hint_for(operation),
                operation,
                reason,
            },
            OperationCheck::Supported => match from_provider.rename(&from_rel, &to_rel) {
                Ok(()) => VfsOutcome::Ok(()),
                Err(e) => VfsOutcome::Blocked {
                    operation: VfsOperation::Rename,
                    reason: e.to_string(),
                    hint: None,
                },
            },
            OperationCheck::PartialSupport { operation, caveat } => {
                match from_provider.rename(&from_rel, &to_rel) {
                    Ok(()) => VfsOutcome::Degraded { value: (), caveat },
                    Err(e) => VfsOutcome::Blocked {
                        operation,
                        reason: e.to_string(),
                        hint: None,
                    },
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::vfs::provider::InternalProvider;
    use std::sync::Mutex;

    /// In-memory stand-in for `SafProvider` — behaves like real SAF
    /// external storage from the capability system's point of view (no
    /// chmod/symlink support, since it inherits the trait defaults) without
    /// needing a JNI environment. This is what lets the capability-blocking
    /// *policy* be fully tested on host even though the real SAF bridge
    /// can't be.
    #[derive(Default)]
    struct MockSafProvider {
        files: Mutex<HashMap<PathBuf, Vec<u8>>>,
    }

    impl FsProvider for MockSafProvider {
        fn read_file(&self, path: &Path) -> VfsResult<Vec<u8>> {
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| VfsError::NotFound(path.to_string_lossy().into()))
        }
        fn write_file(&self, path: &Path, contents: &[u8]) -> VfsResult<()> {
            self.files
                .lock()
                .unwrap()
                .insert(path.to_path_buf(), contents.to_vec());
            Ok(())
        }
        fn metadata(&self, path: &Path) -> VfsResult<FileMetadata> {
            let files = self.files.lock().unwrap();
            let contents = files
                .get(path)
                .ok_or_else(|| VfsError::NotFound(path.to_string_lossy().into()))?;
            Ok(FileMetadata::file(
                path.file_name().unwrap().to_string_lossy(),
                path.to_string_lossy(),
                contents.len() as u64,
            ))
        }
        fn list_dir(&self, _path: &Path) -> VfsResult<Vec<DirEntry>> {
            Ok(vec![])
        }
        fn create_dir(&self, _path: &Path) -> VfsResult<()> {
            Ok(())
        }
        fn delete(&self, path: &Path) -> VfsResult<()> {
            self.files.lock().unwrap().remove(path);
            Ok(())
        }
        fn rename(&self, from: &Path, to: &Path) -> VfsResult<()> {
            let mut files = self.files.lock().unwrap();
            let v = files
                .remove(from)
                .ok_or_else(|| VfsError::NotFound(from.to_string_lossy().into()))?;
            files.insert(to.to_path_buf(), v);
            Ok(())
        }
        fn exists(&self, path: &Path) -> bool {
            self.files.lock().unwrap().contains_key(path)
        }
        fn is_dir(&self, _path: &Path) -> bool {
            false
        }
        // chmod/symlink/readlink deliberately NOT overridden — this proves
        // the trait defaults (OperationNotSupported) are what a real SAF
        // provider would inherit too.
    }

    fn service_with_saf_mount() -> VfsService {
        // Unique dir per call: several tests in this module share this helper
        // and used to race on a single `/tmp/vfs_service_test` (one test's
        // `remove_dir_all` wiping another's just-written file).
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let temp_dir = std::env::temp_dir().join(format!(
            "vfs_service_test_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let internal = Arc::new(InternalProvider::new(&temp_dir));
        let mut svc = VfsService::new(&temp_dir, internal);

        let saf_mount = MountPoint::saf("/mnt/sdcard", "content://fake/tree/sdcard", "SD Card");
        let saf_provider: Arc<dyn FsProvider> = Arc::new(MockSafProvider::default());
        svc.mount_provider(saf_mount, saf_provider).unwrap();
        svc
    }

    #[test]
    fn test_write_read_on_internal_is_ok() {
        let svc = service_with_saf_mount();
        let outcome = svc.write_file(Path::new("/hello.txt"), b"hi");
        assert!(matches!(outcome, VfsOutcome::Ok(())));

        match svc.read_file(Path::new("/hello.txt")) {
            VfsOutcome::Ok(data) => assert_eq!(data, b"hi"),
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    #[test]
    fn test_chmod_blocked_on_saf_mount_with_hint() {
        let svc = service_with_saf_mount();
        svc.write_file(Path::new("/mnt/sdcard/project/index.js"), b"x")
            .into_result()
            .unwrap();

        match svc.chmod(Path::new("/mnt/sdcard/project/index.js"), 0o755) {
            VfsOutcome::Blocked {
                operation, hint, ..
            } => {
                assert_eq!(operation, VfsOperation::Chmod);
                // This hint is exactly what the terminal UI is supposed to
                // render inline instead of a bare failure — see the
                // roadmap item this file implements.
                assert!(hint.is_some());
                assert!(hint.unwrap().contains("internal storage"));
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn test_chmod_succeeds_on_internal_mount() {
        let svc = service_with_saf_mount();
        svc.write_file(Path::new("/index.js"), b"x")
            .into_result()
            .unwrap();

        match svc.chmod(Path::new("/index.js"), 0o755) {
            VfsOutcome::Ok(()) => {}
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    #[test]
    fn test_symlink_blocked_on_saf_with_npm_specific_hint() {
        let svc = service_with_saf_mount();

        match svc.symlink(
            Path::new("/mnt/sdcard/node_modules/.bin/real"),
            Path::new("/mnt/sdcard/project/node_modules/.bin/tool"),
        ) {
            VfsOutcome::Blocked {
                operation, hint, ..
            } => {
                assert_eq!(operation, VfsOperation::Symlink);
                assert!(hint.unwrap().contains("npm"));
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn test_rename_on_saf_is_degraded_not_blocked() {
        // Rename IS supported on SAF, but non-atomically — this must come
        // back as Degraded (succeeds + caveat), not Blocked, since treating
        // a working-but-caveated operation as a hard failure would be
        // wrong and would make the terminal lie about what happened.
        let svc = service_with_saf_mount();
        svc.write_file(Path::new("/mnt/sdcard/a.txt"), b"data")
            .into_result()
            .unwrap();

        match svc.rename(
            Path::new("/mnt/sdcard/a.txt"),
            Path::new("/mnt/sdcard/b.txt"),
        ) {
            VfsOutcome::Degraded { caveat, .. } => {
                assert!(caveat.to_lowercase().contains("atomic"));
            }
            other => panic!("expected Degraded, got {other:?}"),
        }

        // And the rename actually happened.
        assert!(matches!(
            svc.read_file(Path::new("/mnt/sdcard/b.txt")),
            VfsOutcome::Ok(_)
        ));
    }

    #[test]
    fn test_cross_mount_rename_is_blocked_not_silently_wrong() {
        let svc = service_with_saf_mount();
        svc.write_file(Path::new("/a.txt"), b"data")
            .into_result()
            .unwrap();

        match svc.rename(Path::new("/a.txt"), Path::new("/mnt/sdcard/a.txt")) {
            VfsOutcome::Blocked { reason, hint, .. } => {
                assert!(reason.contains("Cross-mount"));
                assert!(hint.is_some());
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn test_unmounted_path_is_blocked_after_explicit_unmount() {
        let mut svc = service_with_saf_mount();
        svc.write_file(Path::new("/mnt/sdcard/a.txt"), b"data")
            .into_result()
            .unwrap();

        svc.unmount(Path::new("/mnt/sdcard")).unwrap();

        // After unmounting, "/mnt/sdcard/a.txt" falls back to the internal
        // root provider (which has no such file) rather than silently
        // resolving to the removed SAF mount.
        match svc.read_file(Path::new("/mnt/sdcard/a.txt")) {
            VfsOutcome::Blocked { .. } => {}
            other => panic!("expected Blocked, got {other:?}"),
        }
    }
}
