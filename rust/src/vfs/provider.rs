//! Filesystem provider interface
//!
//! Abstracts over different filesystem backends (internal, SAF, etc.)

use crate::utils::error::{VfsError, VfsResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// File metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// File name
    pub name: String,
    /// Full path
    pub path: String,
    /// Is this a directory?
    pub is_dir: bool,
    /// File size in bytes
    pub size: u64,
    /// Last modified timestamp (Unix epoch seconds)
    pub modified: u64,
    /// Last accessed timestamp (Unix epoch seconds)
    pub accessed: u64,
    /// Is the file readable?
    pub readable: bool,
    /// Is the file writable?
    pub writable: bool,
    /// MIME type (if known)
    pub mime_type: Option<String>,
}

impl FileMetadata {
    /// Create metadata for a directory
    pub fn directory(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            is_dir: true,
            size: 0,
            modified: 0,
            accessed: 0,
            readable: true,
            writable: true,
            mime_type: None,
        }
    }

    /// Create metadata for a file
    pub fn file(name: impl Into<String>, path: impl Into<String>, size: u64) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            is_dir: false,
            size,
            modified: 0,
            accessed: 0,
            readable: true,
            writable: true,
            mime_type: None,
        }
    }
}

/// Directory entry for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    /// Entry name
    pub name: String,
    /// Is directory
    pub is_dir: bool,
    /// Size (0 for directories)
    pub size: u64,
}

/// Filesystem provider trait
/// Implemented by internal storage, SAF, etc.
pub trait FsProvider: Send + Sync {
    /// Read file contents
    fn read_file(&self, path: &Path) -> VfsResult<Vec<u8>>;

    /// Write file contents
    fn write_file(&self, path: &Path, contents: &[u8]) -> VfsResult<()>;

    /// Get file metadata
    fn metadata(&self, path: &Path) -> VfsResult<FileMetadata>;

    /// List directory contents
    fn list_dir(&self, path: &Path) -> VfsResult<Vec<DirEntry>>;

    /// Create a directory
    fn create_dir(&self, path: &Path) -> VfsResult<()>;

    /// Delete a file or empty directory
    fn delete(&self, path: &Path) -> VfsResult<()>;

    /// Rename/move a file
    fn rename(&self, from: &Path, to: &Path) -> VfsResult<()>;

    /// Check if path exists
    fn exists(&self, path: &Path) -> bool;

    /// Check if path is a directory
    fn is_dir(&self, path: &Path) -> bool;

    // ------------------------------------------------------------------
    // Phase 1d additions: capability-gated operations.
    //
    // These default to "not supported" rather than being left off the
    // trait entirely. Before this change, `SafProvider` simply had no
    // chmod/symlink methods at all — which meant the capability system in
    // `vfs::capabilities` (which *knows* SAF can't chmod) had no actual
    // operation to block. The capability check and the operation now live
    // on the same trait, so `VfsService` (see `vfs::service`) can enforce
    // one against the other instead of the two systems silently drifting
    // apart. `InternalProvider` overrides these with real implementations;
    // `SafProvider` intentionally does not override them, since SAF has no
    // equivalent primitive — the inherited default IS the correct behavior.
    // ------------------------------------------------------------------

    /// Change file permissions. Most providers don't support this — the
    /// default returns `OperationNotSupported`. Only providers backed by a
    /// real Unix filesystem (`InternalProvider`) should override this.
    fn chmod(&self, path: &Path, _mode: u32) -> VfsResult<()> {
        Err(crate::utils::error::unsupported_operation(
            crate::vfs::capabilities::VfsOperation::Chmod,
            &path.to_string_lossy(),
        ))
    }

    /// Create a symbolic link at `link` pointing to `target`. Default:
    /// not supported (true for SAF and FAT-backed storage).
    fn symlink(&self, target: &Path, link: &Path) -> VfsResult<()> {
        let _ = target;
        Err(crate::utils::error::unsupported_operation(
            crate::vfs::capabilities::VfsOperation::Symlink,
            &link.to_string_lossy(),
        ))
    }

    /// Read the target of a symbolic link. Default: not supported.
    fn readlink(&self, path: &Path) -> VfsResult<std::path::PathBuf> {
        Err(crate::utils::error::unsupported_operation(
            crate::vfs::capabilities::VfsOperation::Symlink,
            &path.to_string_lossy(),
        ))
    }
}

/// Internal storage provider (direct filesystem access)
pub struct InternalProvider {
    /// Base path for internal storage
    base_path: std::path::PathBuf,
}

impl InternalProvider {
    pub fn new(base_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    fn resolve_path(&self, path: &Path) -> std::path::PathBuf {
        // Already a real path under this provider's root.
        if path.is_absolute() && path.starts_with(&self.base_path) {
            return path.to_path_buf();
        }

        // `Path::join` replaces the left-hand side when the right-hand side
        // is absolute, so a virtual path like `/index.js` would otherwise
        // resolve to `/index.js` on the host filesystem instead of
        // `$base/index.js`. Strip a leading `/` so virtual-absolute and
        // relative paths both land under the provider root.
        let relative = path.strip_prefix("/").unwrap_or(path);
        self.base_path.join(relative)
    }
}

impl FsProvider for InternalProvider {
    fn read_file(&self, path: &Path) -> VfsResult<Vec<u8>> {
        let full_path = self.resolve_path(path);
        std::fs::read(&full_path)
            .map_err(|e| VfsError::NotFound(format!("{}: {}", full_path.display(), e)))
    }

    fn write_file(&self, path: &Path, contents: &[u8]) -> VfsResult<()> {
        let full_path = self.resolve_path(path);

        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| VfsError::PermissionDenied(e.to_string()))?;
        }

        std::fs::write(&full_path, contents)
            .map_err(|e| VfsError::PermissionDenied(format!("{}: {}", full_path.display(), e)))
    }

    fn metadata(&self, path: &Path) -> VfsResult<FileMetadata> {
        let full_path = self.resolve_path(path);
        let meta = std::fs::metadata(&full_path)
            .map_err(|e| VfsError::NotFound(format!("{}: {}", full_path.display(), e)))?;

        let name = full_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        Ok(FileMetadata {
            name,
            path: full_path.to_string_lossy().to_string(),
            is_dir: meta.is_dir(),
            size: meta.len(),
            modified: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
            accessed: meta
                .accessed()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
            readable: true,
            writable: !meta.permissions().readonly(),
            mime_type: None,
        })
    }

    fn list_dir(&self, path: &Path) -> VfsResult<Vec<DirEntry>> {
        let full_path = self.resolve_path(path);
        let entries = std::fs::read_dir(&full_path)
            .map_err(|e| VfsError::NotFound(format!("{}: {}", full_path.display(), e)))?;

        let mut result = Vec::new();
        for entry in entries.flatten() {
            let meta = entry.metadata().ok();
            result.push(DirEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                is_dir: meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
            });
        }

        Ok(result)
    }

    fn create_dir(&self, path: &Path) -> VfsResult<()> {
        let full_path = self.resolve_path(path);
        std::fs::create_dir_all(&full_path)
            .map_err(|e| VfsError::PermissionDenied(format!("{}: {}", full_path.display(), e)))
    }

    fn delete(&self, path: &Path) -> VfsResult<()> {
        let full_path = self.resolve_path(path);

        if full_path.is_dir() {
            std::fs::remove_dir(&full_path)
        } else {
            std::fs::remove_file(&full_path)
        }
        .map_err(|e| VfsError::PermissionDenied(format!("{}: {}", full_path.display(), e)))
    }

    fn rename(&self, from: &Path, to: &Path) -> VfsResult<()> {
        let from_path = self.resolve_path(from);
        let to_path = self.resolve_path(to);

        std::fs::rename(&from_path, &to_path).map_err(|e| {
            VfsError::PermissionDenied(format!(
                "{} -> {}: {}",
                from_path.display(),
                to_path.display(),
                e
            ))
        })
    }

    fn exists(&self, path: &Path) -> bool {
        self.resolve_path(path).exists()
    }

    fn is_dir(&self, path: &Path) -> bool {
        self.resolve_path(path).is_dir()
    }

    fn chmod(&self, path: &Path, mode: u32) -> VfsResult<()> {
        // Real chmod, since internal storage is a genuine Unix filesystem.
        // This is exactly the operation that's a no-op-or-error everywhere
        // else in the VFS layer, which is the whole reason the capability
        // system exists — see vfs::capabilities and vfs::service.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let full_path = self.resolve_path(path);
            let perms = std::fs::Permissions::from_mode(mode);
            std::fs::set_permissions(&full_path, perms)
                .map_err(|e| VfsError::PermissionDenied(format!("{}: {}", full_path.display(), e)))
        }
        #[cfg(not(unix))]
        {
            let _ = mode;
            Err(crate::utils::error::unsupported_operation(
                crate::vfs::capabilities::VfsOperation::Chmod,
                &path.to_string_lossy(),
            ))
        }
    }

    fn symlink(&self, target: &Path, link: &Path) -> VfsResult<()> {
        #[cfg(unix)]
        {
            let full_link = self.resolve_path(link);
            std::os::unix::fs::symlink(target, &full_link)
                .map_err(|e| VfsError::PermissionDenied(format!("{}: {}", full_link.display(), e)))
        }
        #[cfg(not(unix))]
        {
            let _ = target;
            Err(crate::utils::error::unsupported_operation(
                crate::vfs::capabilities::VfsOperation::Symlink,
                &link.to_string_lossy(),
            ))
        }
    }

    fn readlink(&self, path: &Path) -> VfsResult<std::path::PathBuf> {
        let full_path = self.resolve_path(path);
        std::fs::read_link(&full_path)
            .map_err(|e| VfsError::NotFound(format!("{}: {}", full_path.display(), e)))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_internal_provider() {
        let temp_dir = env::temp_dir().join("vfs_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let provider = InternalProvider::new(&temp_dir);

        // Write and read
        provider
            .write_file(Path::new("test.txt"), b"hello")
            .unwrap();
        let content = provider.read_file(Path::new("test.txt")).unwrap();
        assert_eq!(content, b"hello");

        // Metadata
        let meta = provider.metadata(Path::new("test.txt")).unwrap();
        assert!(!meta.is_dir);
        assert_eq!(meta.size, 5);

        // Directory operations
        provider.create_dir(Path::new("subdir")).unwrap();
        assert!(provider.is_dir(Path::new("subdir")));

        // List
        let entries = provider.list_dir(Path::new("")).unwrap();
        assert!(entries.len() >= 2);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_virtual_absolute_path_stays_under_provider_root() {
        // Regression: Path::join with an absolute RHS replaces the base, so
        // write_file("/abs.txt") used to target the host's /abs.txt.
        let temp_dir = env::temp_dir().join(format!("vfs_test_abs_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let provider = InternalProvider::new(&temp_dir);
        provider
            .write_file(Path::new("/abs.txt"), b"under-root")
            .unwrap();

        assert_eq!(
            std::fs::read(temp_dir.join("abs.txt")).unwrap(),
            b"under-root"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[cfg(unix)]
    fn test_internal_provider_chmod_and_symlink() {
        let temp_dir = env::temp_dir().join("vfs_test_chmod_symlink");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let provider = InternalProvider::new(&temp_dir);
        provider.write_file(Path::new("real.txt"), b"data").unwrap();

        // chmod should actually change permissions on internal storage —
        // this is the exact operation that's a no-op everywhere else.
        provider.chmod(Path::new("real.txt"), 0o600).unwrap();
        let meta = std::fs::metadata(temp_dir.join("real.txt")).unwrap();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);

        // symlink + readlink round-trip
        provider
            .symlink(Path::new("real.txt"), Path::new("link.txt"))
            .unwrap();
        let target = provider.readlink(Path::new("link.txt")).unwrap();
        assert_eq!(target, Path::new("real.txt"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// A minimal fake provider used to prove the *default* trait methods
    /// (chmod/symlink/readlink) return `OperationNotSupported` for any
    /// provider that doesn't override them — this is the behavior SAF
    /// relies on without writing any code of its own for it.
    struct BareMinimumProvider;

    impl FsProvider for BareMinimumProvider {
        fn read_file(&self, _path: &Path) -> VfsResult<Vec<u8>> {
            Ok(vec![])
        }
        fn write_file(&self, _path: &Path, _contents: &[u8]) -> VfsResult<()> {
            Ok(())
        }
        fn metadata(&self, _path: &Path) -> VfsResult<FileMetadata> {
            Ok(FileMetadata::file("x", "x", 0))
        }
        fn list_dir(&self, _path: &Path) -> VfsResult<Vec<DirEntry>> {
            Ok(vec![])
        }
        fn create_dir(&self, _path: &Path) -> VfsResult<()> {
            Ok(())
        }
        fn delete(&self, _path: &Path) -> VfsResult<()> {
            Ok(())
        }
        fn rename(&self, _from: &Path, _to: &Path) -> VfsResult<()> {
            Ok(())
        }
        fn exists(&self, _path: &Path) -> bool {
            true
        }
        fn is_dir(&self, _path: &Path) -> bool {
            false
        }
    }

    #[test]
    fn test_default_chmod_symlink_are_unsupported() {
        let provider = BareMinimumProvider;
        assert!(matches!(
            provider.chmod(Path::new("/x"), 0o644),
            Err(VfsError::OperationNotSupported { .. })
        ));
        assert!(matches!(
            provider.symlink(Path::new("/a"), Path::new("/b")),
            Err(VfsError::OperationNotSupported { .. })
        ));
        assert!(matches!(
            provider.readlink(Path::new("/b")),
            Err(VfsError::OperationNotSupported { .. })
        ));
    }
}
