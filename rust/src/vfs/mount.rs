//! Mount point management
//!
//! Tracks virtual mount points that map Unix paths to SAF URIs.

use crate::vfs::capabilities::{VfsCapabilities, VfsOperation};
use crate::utils::error::{VfsError, VfsResult};
use log::{info, warn};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A virtual mount point
#[derive(Debug, Clone)]
pub struct MountPoint {
    /// Virtual Unix-style path (e.g., /mnt/downloads)
    pub virtual_path: PathBuf,
    /// Source URI or path (SAF URI for external, real path for internal)
    pub source: MountSource,
    /// Filesystem capabilities
    pub capabilities: VfsCapabilities,
    /// Whether the mount is currently active
    pub active: bool,
    /// User-friendly display name
    pub display_name: String,
}

/// Source of a mount point
#[derive(Debug, Clone)]
pub enum MountSource {
    /// Internal storage path (direct filesystem access)
    Internal(PathBuf),
    /// SAF URI (content:// URI)
    SafUri(String),
}

impl MountPoint {
    /// Create a new internal storage mount
    pub fn internal(virtual_path: impl Into<PathBuf>, real_path: impl Into<PathBuf>) -> Self {
        let vp = virtual_path.into();
        let rp = real_path.into();
        
        Self {
            display_name: vp.to_string_lossy().to_string(),
            virtual_path: vp,
            source: MountSource::Internal(rp),
            capabilities: VfsCapabilities::internal_storage(),
            active: true,
        }
    }

    /// Create a new SAF mount
    pub fn saf(virtual_path: impl Into<PathBuf>, uri: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            virtual_path: virtual_path.into(),
            source: MountSource::SafUri(uri.into()),
            capabilities: VfsCapabilities::saf_external(),
            active: true,
            display_name: display_name.into(),
        }
    }

    /// Check if an operation is supported
    pub fn supports(&self, op: VfsOperation) -> bool {
        self.capabilities.supports(op)
    }

    /// Get limitation warning for this mount
    pub fn limitation_warning(&self) -> Option<String> {
        self.capabilities.limitation_warning()
    }
}

/// Mount table managing all virtual mounts
pub struct MountTable {
    /// Map from virtual path to mount point
    mounts: HashMap<PathBuf, MountPoint>,
    /// Default internal storage mount path
    internal_root: PathBuf,
}

impl MountTable {
    /// Create a new mount table
    pub fn new(internal_root: impl Into<PathBuf>) -> Self {
        let root = internal_root.into();
        let mut table = Self {
            mounts: HashMap::new(),
            internal_root: root.clone(),
        };

        // Always mount internal storage at root
        let _ = table.mount(MountPoint::internal("/", &root));
        
        table
    }

    /// The real filesystem path backing the root ("/") mount. Useful for
    /// diagnostics and for `vfs::health` to know which path is exempt from
    /// SAF-style permission checks.
    pub fn internal_root(&self) -> &Path {
        &self.internal_root
    }

    /// Add a mount point
    pub fn mount(&mut self, mount: MountPoint) -> VfsResult<()> {
        let path = mount.virtual_path.clone();
        
        if self.mounts.contains_key(&path) {
            warn!("Mount point {} already exists, unmounting first", path.display());
            self.unmount(&path)?;
        }

        info!("Mounting {} -> {:?}", path.display(), mount.source);
        
        // Show limitation warning if applicable
        if let Some(warning) = mount.limitation_warning() {
            warn!("{}", warning);
        }

        self.mounts.insert(path, mount);
        Ok(())
    }

    /// Remove a mount point
    pub fn unmount(&mut self, virtual_path: &Path) -> VfsResult<()> {
        if virtual_path == Path::new("/") {
            return Err(VfsError::PermissionDenied(
                "Cannot unmount root filesystem".into()
            ));
        }

        if self.mounts.remove(virtual_path).is_some() {
            info!("Unmounted {}", virtual_path.display());
            Ok(())
        } else {
            Err(VfsError::NotMounted(virtual_path.to_string_lossy().into()))
        }
    }

    /// Find mount point for a virtual path
    pub fn find_mount(&self, virtual_path: &Path) -> Option<&MountPoint> {
        // Find the longest matching mount point prefix
        let mut best_match: Option<&MountPoint> = None;
        let mut best_len = 0;

        for (mount_path, mount) in &self.mounts {
            if virtual_path.starts_with(mount_path) {
                let len = mount_path.components().count();
                if len > best_len {
                    best_len = len;
                    best_match = Some(mount);
                }
            }
        }

        best_match
    }

    /// Resolve a virtual path to its mount point and relative path
    pub fn resolve(&self, virtual_path: &Path) -> VfsResult<(&MountPoint, PathBuf)> {
        let mount = self.find_mount(virtual_path)
            .ok_or_else(|| VfsError::NotMounted(virtual_path.to_string_lossy().into()))?;

        let relative = virtual_path
            .strip_prefix(&mount.virtual_path)
            .unwrap_or(virtual_path)
            .to_path_buf();

        Ok((mount, relative))
    }

    /// Get capabilities for a path
    pub fn get_capabilities(&self, virtual_path: &Path) -> VfsCapabilities {
        self.find_mount(virtual_path)
            .map(|m| m.capabilities.clone())
            .unwrap_or_else(VfsCapabilities::saf_external)
    }

    /// Check if an operation is supported for a path
    pub fn supports_operation(&self, virtual_path: &Path, op: VfsOperation) -> bool {
        self.find_mount(virtual_path)
            .map(|m| m.supports(op))
            .unwrap_or(false)
    }

    /// List all mount points
    pub fn list_mounts(&self) -> Vec<&MountPoint> {
        self.mounts.values().collect()
    }

    /// Get mount point by virtual path
    pub fn get_mount(&self, virtual_path: &Path) -> Option<&MountPoint> {
        self.mounts.get(virtual_path)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_mount_table() {
        let table = MountTable::new("/data/data/com.terminal/files");
        
        // Root should be mounted
        assert!(table.find_mount(Path::new("/")).is_some());
        assert!(table.supports_operation(Path::new("/"), VfsOperation::Chmod));
    }

    #[test]
    fn test_saf_mount() {
        let mut table = MountTable::new("/data/data/com.terminal/files");
        
        let saf_mount = MountPoint::saf(
            "/mnt/downloads",
            "content://com.android.providers.downloads.documents/tree/downloads",
            "Downloads"
        );
        
        table.mount(saf_mount).unwrap();
        
        let mount = table.find_mount(Path::new("/mnt/downloads/test.txt"));
        assert!(mount.is_some());
        assert!(mount.unwrap().capabilities.is_saf);
        assert!(!table.supports_operation(Path::new("/mnt/downloads"), VfsOperation::Chmod));
    }

    #[test]
    fn test_path_resolution() {
        let mut table = MountTable::new("/data/data/com.terminal/files");
        
        table.mount(MountPoint::saf("/mnt/sd", "content://...", "SD Card")).unwrap();
        
        let (_mount, relative) = table.resolve(Path::new("/mnt/sd/Documents/file.txt")).unwrap();
        assert_eq!(relative, PathBuf::from("Documents/file.txt"));
    }

    #[test]
    fn test_cannot_unmount_root() {
        let mut table = MountTable::new("/data");
        let result = table.unmount(Path::new("/"));
        assert!(result.is_err());
    }
}
