//! Unified error types for the terminal core
//!
//! This module defines all error types used throughout the crate.
//! Follows the "make invalid states unrepresentable" principle.

use thiserror::Error;
use crate::jni_safe::JniErrorCode;
use crate::session_state::CheckpointError;
use crate::vfs_capabilities::VfsOperation;

/// Top-level error type for the terminal core
#[derive(Error, Debug)]
pub enum TerminalError {
    /// PTY-related errors
    #[error("PTY error: {0}")]
    Pty(#[from] PtyError),

    /// VFS/filesystem errors
    #[error("VFS error: {0}")]
    Vfs(#[from] VfsError),

    /// Session/state errors
    #[error("Session error: {0}")]
    Session(#[from] SessionError),

    /// JNI bridge errors
    #[error("JNI error: {0}")]
    Jni(#[from] JniError),

    /// Generic I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// PTY-specific errors
#[derive(Error, Debug)]
pub enum PtyError {
    /// Failed to spawn PTY process
    #[error("Failed to spawn PTY: {0}")]
    SpawnFailed(String),

    /// PTY process has exited
    #[error("PTY process exited with code {0}")]
    ProcessExited(i32),

    /// Failed to read from PTY
    #[error("Failed to read from PTY: {0}")]
    ReadFailed(String),

    /// Failed to write to PTY
    #[error("Failed to write to PTY: {0}")]
    WriteFailed(String),

    /// Failed to resize PTY
    #[error("Failed to resize PTY: {0}")]
    ResizeFailed(String),

    /// PTY handle is invalid
    #[error("Invalid PTY handle")]
    InvalidHandle,

    /// PTY is not initialized
    #[error("PTY not initialized")]
    NotInitialized,

    /// Signal sending failed
    #[error("Failed to send signal {signal}: {reason}")]
    SignalFailed { signal: i32, reason: String },
}

/// VFS/filesystem-specific errors
#[derive(Error, Debug)]
pub enum VfsError {
    /// Path not found
    #[error("Path not found: {0}")]
    NotFound(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Operation not supported on this filesystem
    #[error("Operation {operation:?} not supported: {reason}")]
    OperationNotSupported {
        operation: VfsOperation,
        reason: String,
    },

    /// Path is not mounted
    #[error("Path not mounted: {0}")]
    NotMounted(String),

    /// Mount failed
    #[error("Failed to mount {path}: {reason}")]
    MountFailed { path: String, reason: String },

    /// SAF URI is invalid
    #[error("Invalid SAF URI: {0}")]
    InvalidUri(String),

    /// SAF permission lost
    #[error("SAF permission lost for: {0}")]
    PermissionLost(String),

    /// Cache error
    #[error("Cache error: {0}")]
    CacheError(String),

    /// File already exists
    #[error("File already exists: {0}")]
    AlreadyExists(String),

    /// Not a directory
    #[error("Not a directory: {0}")]
    NotADirectory(String),

    /// Not a file
    #[error("Not a file: {0}")]
    NotAFile(String),

    /// Directory not empty
    #[error("Directory not empty: {0}")]
    DirectoryNotEmpty(String),
}

/// Session/state-specific errors
#[derive(Error, Debug)]
pub enum SessionError {
    /// Session not found
    #[error("Session not found: {0}")]
    NotFound(String),

    /// Session already exists
    #[error("Session already exists: {0}")]
    AlreadyExists(String),

    /// Invalid session state for operation
    #[error("Invalid state {current:?} for operation {operation}")]
    InvalidState {
        current: crate::session_state::SessionState,
        operation: String,
    },

    /// Checkpoint failed
    #[error("Checkpoint error: {0}")]
    Checkpoint(#[from] CheckpointError),

    /// Restore failed
    #[error("Failed to restore session: {0}")]
    RestoreFailed(String),

    /// Session limit reached
    #[error("Maximum session limit ({0}) reached")]
    LimitReached(usize),
}

/// JNI bridge errors
#[derive(Error, Debug)]
pub enum JniError {
    /// Error code from JNI layer
    #[error("JNI error code: {0:?}")]
    Code(JniErrorCode),

    /// Java exception was thrown
    #[error("Java exception: {0}")]
    JavaException(String),

    /// Invalid string encoding
    #[error("Invalid string encoding")]
    InvalidEncoding,

    /// Object was garbage collected
    #[error("Java object was garbage collected")]
    ObjectCollected,

    /// Thread attachment failed
    #[error("Failed to attach to JVM thread")]
    ThreadAttachFailed,
}

impl From<JniErrorCode> for JniError {
    fn from(code: JniErrorCode) -> Self {
        JniError::Code(code)
    }
}

impl From<JniErrorCode> for TerminalError {
    fn from(code: JniErrorCode) -> Self {
        TerminalError::Jni(JniError::Code(code))
    }
}

/// Result type alias for terminal operations
pub type Result<T> = std::result::Result<T, TerminalError>;

/// Result type alias for PTY operations
pub type PtyResult<T> = std::result::Result<T, PtyError>;

/// Result type alias for VFS operations
pub type VfsResult<T> = std::result::Result<T, VfsError>;

/// Result type alias for session operations
pub type SessionResult<T> = std::result::Result<T, SessionError>;

/// Extension trait for converting Option to Result with meaningful errors
pub trait OptionExt<T> {
    /// Convert Option to Result with PTY error
    fn ok_or_pty(self, msg: impl Into<String>) -> PtyResult<T>;
    
    /// Convert Option to Result with VFS error
    fn ok_or_vfs_not_found(self, path: impl Into<String>) -> VfsResult<T>;
}

impl<T> OptionExt<T> for Option<T> {
    fn ok_or_pty(self, msg: impl Into<String>) -> PtyResult<T> {
        self.ok_or_else(|| PtyError::SpawnFailed(msg.into()))
    }

    fn ok_or_vfs_not_found(self, path: impl Into<String>) -> VfsResult<T> {
        self.ok_or_else(|| VfsError::NotFound(path.into()))
    }
}

/// Helper to create "operation not supported" VFS errors
pub fn unsupported_operation(op: VfsOperation, path: &str) -> VfsError {
    VfsError::OperationNotSupported {
        operation: op,
        reason: format!(
            "Operation {:?} is not supported on path: {}. This is likely a SAF limitation.",
            op, path
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = PtyError::ProcessExited(1);
        assert!(err.to_string().contains("exited"));

        let err = VfsError::NotFound("/some/path".into());
        assert!(err.to_string().contains("/some/path"));
    }

    #[test]
    fn test_error_conversion() {
        let pty_err = PtyError::InvalidHandle;
        let terminal_err: TerminalError = pty_err.into();
        assert!(matches!(terminal_err, TerminalError::Pty(_)));
    }

    #[test]
    fn test_option_ext() {
        let none: Option<i32> = None;
        let result = none.ok_or_pty("test error");
        assert!(matches!(result, Err(PtyError::SpawnFailed(_))));

        let some = Some(42);
        let result = some.ok_or_pty("should not error");
        assert_eq!(result.unwrap(), 42);
    }
}
