//! Android Storage Access Framework provider
//!
//! Implements the FsProvider trait using SAF via JNI calls to Kotlin.
//! This module is only compiled when the "android" feature is enabled.

#![cfg(feature = "android")]

use crate::jni_safe::{safe_call_bool_method, JniErrorCode, JniResult};
use crate::utils::error::{VfsError, VfsResult};
use crate::vfs::provider::{DirEntry, FileMetadata, FsProvider};
use jni::objects::{GlobalRef, JObject, JValue};
use jni::JNIEnv;
use log::{error, warn};
use std::path::Path;

/// SAF provider that bridges to Kotlin via JNI
pub struct SafProvider {
    /// Global reference to the Kotlin SafHelper instance
    helper_ref: GlobalRef,
    /// Base URI for this provider
    base_uri: String,
}

impl SafProvider {
    /// Create a new SAF provider
    /// SAFETY: Must be called from a thread attached to the JVM
    pub fn new(env: &mut JNIEnv, helper: JObject, uri: String) -> JniResult<Self> {
        let helper_ref = env.new_global_ref(&helper).map_err(|e| {
            error!("Failed to create global ref for SafHelper: {:?}", e);
            JniErrorCode::OutOfMemory
        })?;

        Ok(Self {
            helper_ref,
            base_uri: uri,
        })
    }

    /// Call a method on the SafHelper that returns a boolean
    fn call_bool_method(&self, env: &mut JNIEnv, method: &str, path: &str) -> VfsResult<bool> {
        let path_jstring = env.new_string(path).map_err(|_| {
            VfsError::NotFound(format!("Failed to create JNI string for path: {}", path))
        })?;

        let path_obj: JObject = path_jstring.into();
        
        safe_call_bool_method(
            env,
            self.helper_ref.as_obj(),
            method,
            "(Ljava/lang/String;)Z",
            &[JValue::Object(&path_obj)],
        )
        .map_err(|e| VfsError::NotFound(format!("JNI error calling {}: {:?}", method, e)))
    }
}

impl FsProvider for SafProvider {
    fn read_file(&self, path: &Path) -> VfsResult<Vec<u8>> {
        // TODO: Implement via JNI call to SafHelper.readFile()
        // For now, return placeholder error
        warn!("SafProvider.read_file not yet implemented");
        Err(VfsError::NotFound(format!(
            "SAF read not implemented: {}",
            path.display()
        )))
    }

    fn write_file(&self, path: &Path, _contents: &[u8]) -> VfsResult<()> {
        // TODO: Implement via JNI call to SafHelper.writeFile()
        warn!("SafProvider.write_file not yet implemented");
        Err(VfsError::PermissionDenied(format!(
            "SAF write not implemented: {}",
            path.display()
        )))
    }

    fn metadata(&self, path: &Path) -> VfsResult<FileMetadata> {
        // TODO: Implement via JNI call to SafHelper.getMetadata()
        warn!("SafProvider.metadata not yet implemented");
        Err(VfsError::NotFound(format!(
            "SAF metadata not implemented: {}",
            path.display()
        )))
    }

    fn list_dir(&self, path: &Path) -> VfsResult<Vec<DirEntry>> {
        // TODO: Implement via JNI call to SafHelper.listDirectory()
        warn!("SafProvider.list_dir not yet implemented");
        Err(VfsError::NotFound(format!(
            "SAF list_dir not implemented: {}",
            path.display()
        )))
    }

    fn create_dir(&self, path: &Path) -> VfsResult<()> {
        // TODO: Implement via JNI call to SafHelper.createDirectory()
        warn!("SafProvider.create_dir not yet implemented");
        Err(VfsError::PermissionDenied(format!(
            "SAF create_dir not implemented: {}",
            path.display()
        )))
    }

    fn delete(&self, path: &Path) -> VfsResult<()> {
        // TODO: Implement via JNI call to SafHelper.delete()
        warn!("SafProvider.delete not yet implemented");
        Err(VfsError::PermissionDenied(format!(
            "SAF delete not implemented: {}",
            path.display()
        )))
    }

    fn rename(&self, from: &Path, to: &Path) -> VfsResult<()> {
        // TODO: Implement via JNI call to SafHelper.rename()
        // NOTE: This is NOT atomic on SAF!
        warn!("SafProvider.rename not yet implemented");
        Err(VfsError::PermissionDenied(format!(
            "SAF rename not implemented: {} -> {}",
            from.display(),
            to.display()
        )))
    }

    fn exists(&self, _path: &Path) -> bool {
        // TODO: Implement via JNI call to SafHelper.exists()
        warn!("SafProvider.exists not yet implemented");
        false
    }

    fn is_dir(&self, _path: &Path) -> bool {
        // TODO: Implement via JNI call to SafHelper.isDirectory()
        warn!("SafProvider.is_dir not yet implemented");
        false
    }
}
