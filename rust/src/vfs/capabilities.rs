//! VFS Capability System - MANDATORY check before any filesystem operation
//!
//! Android's Storage Access Framework (SAF) does NOT support all Unix filesystem operations.
//! This module provides capability checking to handle limitations gracefully.
//!
//! CRITICAL: ALWAYS check capabilities before attempting operations on VFS paths.
//! Failing to do so will cause silent failures, data corruption, or crashes.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// Filesystem operations that may or may not be supported depending on path
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum VfsOperation {
    /// Read file contents
    Read = 0,
    /// Write file contents
    Write = 1,
    /// Create new files
    Create = 2,
    /// Delete files/directories
    Delete = 3,
    /// Rename/move files
    Rename = 4,
    /// Change file permissions (chmod)
    Chmod = 5,
    /// Change file ownership (chown)
    Chown = 6,
    /// Create symbolic links
    Symlink = 7,
    /// Create hard links
    Hardlink = 8,
    /// List directory contents
    ListDir = 9,
    /// Create directories
    Mkdir = 10,
    /// Get file metadata (stat)
    Stat = 11,
    /// Set file timestamps (touch/utime)
    SetTimestamp = 12,
    /// Watch for file changes (inotify)
    Watch = 13,
    /// Atomic operations (rename within same filesystem)
    AtomicRename = 14,
    /// Memory-mapped I/O
    Mmap = 15,
    /// File locking
    Lock = 16,
    /// Extended attributes
    Xattr = 17,
}

/// Capabilities of a filesystem at a specific path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VfsCapabilities {
    /// Set of supported operations
    pub supported: HashSet<VfsOperation>,
    /// Is this a SAF-backed path?
    pub is_saf: bool,
    /// Human-readable filesystem type
    pub fs_type: FsType,
    /// Maximum filename length
    pub max_filename_len: usize,
    /// Maximum path length
    pub max_path_len: usize,
    /// Is the filesystem read-only?
    pub read_only: bool,
    /// Does the filesystem preserve case?
    pub case_sensitive: bool,
}

/// Types of filesystems we interact with
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FsType {
    /// App's internal storage (full Unix semantics)
    Internal,
    /// SAF-backed external storage (limited)
    SafExternal,
    /// FAT32/exFAT external storage (very limited)
    FatExternal,
    /// Unknown filesystem type
    Unknown,
}

impl FsType {
    /// Human-readable name for UI display
    pub fn display_name(&self) -> &'static str {
        match self {
            FsType::Internal => "Internal Storage",
            FsType::SafExternal => "External Storage (SAF)",
            FsType::FatExternal => "External Storage (FAT)",
            FsType::Unknown => "Unknown Storage",
        }
    }
}

impl VfsCapabilities {
    /// Create capabilities for internal storage (full Unix support)
    pub fn internal_storage() -> Self {
        let mut supported = HashSet::new();
        supported.insert(VfsOperation::Read);
        supported.insert(VfsOperation::Write);
        supported.insert(VfsOperation::Create);
        supported.insert(VfsOperation::Delete);
        supported.insert(VfsOperation::Rename);
        supported.insert(VfsOperation::Chmod);
        supported.insert(VfsOperation::Chown);
        supported.insert(VfsOperation::Symlink);
        supported.insert(VfsOperation::Hardlink);
        supported.insert(VfsOperation::ListDir);
        supported.insert(VfsOperation::Mkdir);
        supported.insert(VfsOperation::Stat);
        supported.insert(VfsOperation::SetTimestamp);
        supported.insert(VfsOperation::Watch);
        supported.insert(VfsOperation::AtomicRename);
        supported.insert(VfsOperation::Mmap);
        supported.insert(VfsOperation::Lock);
        supported.insert(VfsOperation::Xattr);

        Self {
            supported,
            is_saf: false,
            fs_type: FsType::Internal,
            max_filename_len: 255,
            max_path_len: 4096,
            read_only: false,
            case_sensitive: true,
        }
    }

    /// Create capabilities for SAF external storage (limited Unix support)
    /// WARNING: Many operations will fail silently or behave unexpectedly
    pub fn saf_external() -> Self {
        let mut supported = HashSet::new();
        // Basic operations work
        supported.insert(VfsOperation::Read);
        supported.insert(VfsOperation::Write);
        supported.insert(VfsOperation::Create);
        supported.insert(VfsOperation::Delete);
        supported.insert(VfsOperation::ListDir);
        supported.insert(VfsOperation::Mkdir);
        supported.insert(VfsOperation::Stat);

        // Rename works but is NOT atomic
        supported.insert(VfsOperation::Rename);

        // These operations DO NOT WORK on SAF:
        // - Chmod: silently ignored
        // - Chown: not supported
        // - Symlink: not supported
        // - Hardlink: not supported
        // - Watch (inotify): not supported
        // - AtomicRename: not guaranteed
        // - Mmap: not supported
        // - Lock: not supported
        // - Xattr: not supported

        Self {
            supported,
            is_saf: true,
            fs_type: FsType::SafExternal,
            max_filename_len: 255,
            max_path_len: 4096,
            read_only: false,
            case_sensitive: false, // Depends on underlying FS, assume worst case
        }
    }

    /// Create capabilities for FAT32/exFAT external storage
    pub fn fat_external() -> Self {
        let mut supported = HashSet::new();
        // Very limited operations
        supported.insert(VfsOperation::Read);
        supported.insert(VfsOperation::Write);
        supported.insert(VfsOperation::Create);
        supported.insert(VfsOperation::Delete);
        supported.insert(VfsOperation::Rename);
        supported.insert(VfsOperation::ListDir);
        supported.insert(VfsOperation::Mkdir);
        supported.insert(VfsOperation::Stat);

        Self {
            supported,
            is_saf: true, // On modern Android, FAT is accessed via SAF
            fs_type: FsType::FatExternal,
            max_filename_len: 255, // Actually limited by 8.3 in some cases
            max_path_len: 260,     // Windows-style path limit
            read_only: false,
            case_sensitive: false,
        }
    }

    /// Check if a specific operation is supported
    #[inline]
    pub fn supports(&self, op: VfsOperation) -> bool {
        self.supported.contains(&op)
    }

    /// Get list of unsupported operations (for UI warnings)
    pub fn unsupported_operations(&self) -> Vec<VfsOperation> {
        use VfsOperation::*;
        let all_ops = [
            Read, Write, Create, Delete, Rename, Chmod, Chown,
            Symlink, Hardlink, ListDir, Mkdir, Stat, SetTimestamp,
            Watch, AtomicRename, Mmap, Lock, Xattr,
        ];
        
        all_ops
            .iter()
            .filter(|op| !self.supported.contains(op))
            .copied()
            .collect()
    }

    /// Generate user-friendly warning message about limitations
    pub fn limitation_warning(&self) -> Option<String> {
        if !self.is_saf {
            return None;
        }

        let mut warnings = Vec::new();

        if !self.supports(VfsOperation::Chmod) {
            warnings.push("chmod does not work (permissions cannot be changed)");
        }
        if !self.supports(VfsOperation::Symlink) {
            warnings.push("symlinks are not supported (npm/yarn/venv may fail)");
        }
        if !self.supports(VfsOperation::AtomicRename) {
            warnings.push("atomic renames not guaranteed (file operations may conflict)");
        }
        if !self.supports(VfsOperation::Watch) {
            warnings.push("file watching not supported (live reload may not work)");
        }

        if warnings.is_empty() {
            None
        } else {
            Some(format!(
                "⚠️ External Storage Limitations:\n• {}",
                warnings.join("\n• ")
            ))
        }
    }
}

/// Result of checking an operation
#[derive(Debug)]
pub enum OperationCheck {
    /// Operation is supported
    Supported,
    /// Operation is not supported - includes reason
    NotSupported { operation: VfsOperation, reason: String },
    /// Operation is partially supported with caveats
    PartialSupport { operation: VfsOperation, caveat: String },
}

/// Check if an operation is safe to perform on a path
pub fn check_operation(path: &Path, op: VfsOperation, caps: &VfsCapabilities) -> OperationCheck {
    if caps.supports(op) {
        // Check for partial support cases
        if caps.is_saf && op == VfsOperation::Rename {
            return OperationCheck::PartialSupport {
                operation: op,
                caveat: "Rename is not atomic on external storage. Files may be in inconsistent state if interrupted.".into(),
            };
        }
        OperationCheck::Supported
    } else {
        let reason = match op {
            VfsOperation::Chmod => {
                "chmod is not supported on external storage. File permissions cannot be changed via SAF."
            }
            VfsOperation::Chown => {
                "chown is not supported on external storage."
            }
            VfsOperation::Symlink => {
                "Symbolic links are not supported on external storage. This will break npm, yarn, and Python venv."
            }
            VfsOperation::Hardlink => {
                "Hard links are not supported on external storage."
            }
            VfsOperation::Watch => {
                "File watching (inotify) is not supported on external storage. Live reload and file watchers will not work."
            }
            VfsOperation::AtomicRename => {
                "Atomic renames are not guaranteed on external storage. Concurrent file operations may cause data corruption."
            }
            VfsOperation::Mmap => {
                "Memory-mapped files are not supported on external storage."
            }
            VfsOperation::Lock => {
                "File locking is not supported on external storage."
            }
            VfsOperation::Xattr => {
                "Extended attributes are not supported on external storage."
            }
            _ => "This operation is not supported on the current filesystem.",
        };

        OperationCheck::NotSupported {
            operation: op,
            reason: format!("{}\nPath: {}", reason, path.display()),
        }
    }
}

/// Determine capabilities based on path
/// This is called when mounting or accessing a path
pub fn get_capabilities_for_path(path: &Path) -> VfsCapabilities {
    let path_str = path.to_string_lossy();

    // Internal storage paths (app's own directories)
    if path_str.starts_with("/data/") 
        || path_str.starts_with("/data/data/")
        || path_str.contains("/files/")
        || path_str.contains("/cache/")
    {
        return VfsCapabilities::internal_storage();
    }

    // SAF-mounted paths (our virtual mount point)
    if path_str.starts_with("/mnt/") || path_str.starts_with("/storage/") {
        // Could be SAF or FAT depending on what's mounted
        // Default to SAF limitations since that's the safer assumption
        return VfsCapabilities::saf_external();
    }

    // Default to most restrictive for unknown paths
    VfsCapabilities::saf_external()
}

/// Tools that have known issues on SAF filesystems
pub struct ToolCompatibility;

impl ToolCompatibility {
    /// Check if a tool is compatible with the filesystem capabilities
    pub fn check(tool: &str, caps: &VfsCapabilities) -> ToolCompatibilityResult {
        if !caps.is_saf {
            return ToolCompatibilityResult::FullyCompatible;
        }

        match tool.to_lowercase().as_str() {
            "git" => ToolCompatibilityResult::PartiallyCompatible {
                tool: tool.into(),
                issues: vec![
                    "Rename detection may be unreliable".into(),
                    "Performance is slower due to SAF overhead".into(),
                    "Permissions are not preserved in clones".into(),
                ],
                recommendation: "For best results, clone repositories to internal storage.".into(),
            },
            "npm" | "yarn" | "pnpm" => ToolCompatibilityResult::NotCompatible {
                tool: tool.into(),
                reason: "npm/yarn use symlinks extensively for node_modules. These will not work on external storage.".into(),
                recommendation: "Use internal storage for Node.js projects.".into(),
            },
            "python" | "pip" => ToolCompatibilityResult::PartiallyCompatible {
                tool: tool.into(),
                issues: vec![
                    "venv may fail due to symlink requirements".into(),
                    "Some packages expect chmod to work".into(),
                ],
                recommendation: "Create virtual environments in internal storage.".into(),
            },
            "make" | "cmake" => ToolCompatibilityResult::PartiallyCompatible {
                tool: tool.into(),
                issues: vec![
                    "Timestamp-based rebuilds may be unreliable".into(),
                    "Build artifacts may have wrong permissions".into(),
                ],
                recommendation: "Build in internal storage, copy results to external.".into(),
            },
            "tar" | "zip" => ToolCompatibilityResult::PartiallyCompatible {
                tool: tool.into(),
                issues: vec![
                    "Cannot preserve permissions".into(),
                    "Cannot create symlinks".into(),
                ],
                recommendation: "Archive permissions will not be restored correctly.".into(),
            },
            "rsync" => ToolCompatibilityResult::PartiallyCompatible {
                tool: tool.into(),
                issues: vec![
                    "Cannot preserve permissions, ownership, or symlinks".into(),
                    "Partial transfer recovery may not work correctly".into(),
                ],
                recommendation: "Use for data-only transfers, not full backups.".into(),
            },
            _ => ToolCompatibilityResult::Unknown,
        }
    }
}

/// Result of tool compatibility check
#[derive(Debug, Clone)]
pub enum ToolCompatibilityResult {
    /// Tool works perfectly on this filesystem
    FullyCompatible,
    /// Tool works with some limitations
    PartiallyCompatible {
        tool: String,
        issues: Vec<String>,
        recommendation: String,
    },
    /// Tool will not work correctly
    NotCompatible {
        tool: String,
        reason: String,
        recommendation: String,
    },
    /// Unknown tool, cannot determine compatibility
    Unknown,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_internal_storage_has_all_ops() {
        let caps = VfsCapabilities::internal_storage();
        assert!(caps.supports(VfsOperation::Chmod));
        assert!(caps.supports(VfsOperation::Symlink));
        assert!(caps.supports(VfsOperation::Watch));
        assert!(!caps.is_saf);
    }

    #[test]
    fn test_saf_limitations() {
        let caps = VfsCapabilities::saf_external();
        assert!(caps.is_saf);
        assert!(!caps.supports(VfsOperation::Chmod));
        assert!(!caps.supports(VfsOperation::Symlink));
        assert!(!caps.supports(VfsOperation::Watch));
        assert!(caps.supports(VfsOperation::Read));
        assert!(caps.supports(VfsOperation::Write));
    }

    #[test]
    fn test_tool_compatibility() {
        let saf_caps = VfsCapabilities::saf_external();
        
        let npm_result = ToolCompatibility::check("npm", &saf_caps);
        assert!(matches!(npm_result, ToolCompatibilityResult::NotCompatible { .. }));

        let git_result = ToolCompatibility::check("git", &saf_caps);
        assert!(matches!(git_result, ToolCompatibilityResult::PartiallyCompatible { .. }));

        let internal_caps = VfsCapabilities::internal_storage();
        let npm_internal = ToolCompatibility::check("npm", &internal_caps);
        assert!(matches!(npm_internal, ToolCompatibilityResult::FullyCompatible));
    }

    #[test]
    fn test_limitation_warning() {
        let saf_caps = VfsCapabilities::saf_external();
        let warning = saf_caps.limitation_warning();
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("chmod"));

        let internal_caps = VfsCapabilities::internal_storage();
        assert!(internal_caps.limitation_warning().is_none());
    }
}
