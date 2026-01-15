//! Terminal Core Library
//!
//! Cross-platform terminal emulator core providing:
//! - PTY management (process spawning, I/O)
//! - Virtual filesystem (SAF bridge for Android)
//! - Terminal emulation (ANSI parsing, screen buffer)
//! - Session state management (checkpointing, restore)
//!
//! ## Safety
//!
//! This crate provides safe wrappers for JNI operations. All JNI code
//! MUST use the wrappers in `jni_safe` module. Never use unwrap() on
//! JNI boundaries - always handle errors explicitly.

// Core safety modules (Week 0 foundation)
pub mod jni_safe;
pub mod session_state;
pub mod vfs_capabilities;

// Utility modules
pub mod utils;

// Feature modules
pub mod pty;
pub mod terminal;
pub mod vfs;

// Package management (Month 3)
pub mod package;

// Platform-specific JNI exports
#[cfg(feature = "android")]
pub mod android_jni;

// Re-exports for convenience
pub use jni_safe::JniErrorCode;
pub use session_state::{CheckpointManager, SessionState, TerminalState};
pub use vfs_capabilities::{VfsCapabilities, VfsOperation};
pub use utils::error::{TerminalError, Result};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the library
/// Call this once when the library is loaded
pub fn init() {
    utils::logger::init_or_ignore();
    jni_safe::install_panic_hook();
    log::info!("Terminal core v{} initialized", VERSION);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        init();
        // Should not panic on multiple calls
        init();
    }

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
