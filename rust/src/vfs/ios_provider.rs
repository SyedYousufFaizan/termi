//! iOS file provider
//!
//! Placeholder for future iOS implementation using UIDocumentPickerViewController.
//! This module is only compiled when the "ios" feature is enabled.

#![cfg(feature = "ios")]

use crate::utils::error::{VfsError, VfsResult};
use crate::vfs::provider::{DirEntry, FileMetadata, FsProvider};
use std::path::Path;

/// iOS file provider (placeholder)
pub struct IosProvider {
    /// Base URL for this provider
    base_url: String,
}

impl IosProvider {
    pub fn new(base_url: String) -> Self {
        Self { base_url }
    }
}

impl FsProvider for IosProvider {
    fn read_file(&self, path: &Path) -> VfsResult<Vec<u8>> {
        Err(VfsError::NotFound("iOS provider not implemented".into()))
    }

    fn write_file(&self, path: &Path, _contents: &[u8]) -> VfsResult<()> {
        Err(VfsError::PermissionDenied("iOS provider not implemented".into()))
    }

    fn metadata(&self, path: &Path) -> VfsResult<FileMetadata> {
        Err(VfsError::NotFound("iOS provider not implemented".into()))
    }

    fn list_dir(&self, path: &Path) -> VfsResult<Vec<DirEntry>> {
        Err(VfsError::NotFound("iOS provider not implemented".into()))
    }

    fn create_dir(&self, path: &Path) -> VfsResult<()> {
        Err(VfsError::PermissionDenied("iOS provider not implemented".into()))
    }

    fn delete(&self, path: &Path) -> VfsResult<()> {
        Err(VfsError::PermissionDenied("iOS provider not implemented".into()))
    }

    fn rename(&self, from: &Path, to: &Path) -> VfsResult<()> {
        Err(VfsError::PermissionDenied("iOS provider not implemented".into()))
    }

    fn exists(&self, _path: &Path) -> bool {
        false
    }

    fn is_dir(&self, _path: &Path) -> bool {
        false
    }
}
