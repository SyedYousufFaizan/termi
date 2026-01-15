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
        if path.is_absolute() && path.starts_with(&self.base_path) {
            path.to_path_buf()
        } else {
            self.base_path.join(path)
        }
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
            modified: meta.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
            accessed: meta.accessed()
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
        }.map_err(|e| VfsError::PermissionDenied(format!("{}: {}", full_path.display(), e)))
    }

    fn rename(&self, from: &Path, to: &Path) -> VfsResult<()> {
        let from_path = self.resolve_path(from);
        let to_path = self.resolve_path(to);
        
        std::fs::rename(&from_path, &to_path)
            .map_err(|e| VfsError::PermissionDenied(format!("{} -> {}: {}", from_path.display(), to_path.display(), e)))
    }

    fn exists(&self, path: &Path) -> bool {
        self.resolve_path(path).exists()
    }

    fn is_dir(&self, path: &Path) -> bool {
        self.resolve_path(path).is_dir()
    }
}

#[cfg(test)]
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
        provider.write_file(Path::new("test.txt"), b"hello").unwrap();
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
}
