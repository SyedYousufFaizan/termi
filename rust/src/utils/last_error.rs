//! Process-wide last-error string for the JNI boundary.
//!
//! JNI integer error codes (`JniErrorCode::PtyError`, etc.) collapse the
//! useful `PtyError` text that actually tells a user *why* session create
//! failed. Kotlin reads this after a failed native call and shows it in
//! the banner instead of a generic "PTY error".

use crate::utils::sync_ext::LockExt;
use std::sync::Mutex;

static LAST_ERROR: Mutex<String> = Mutex::new(String::new());

/// Store a human-readable error for the next JNI/Kotlin caller to pick up.
pub fn set_last_error(msg: impl Into<String>) {
    let mut guard = LAST_ERROR.lock_safe();
    *guard = msg.into();
}

/// Take the stored error (empty if none). Clears the slot.
pub fn take_last_error() -> String {
    let mut guard = LAST_ERROR.lock_safe();
    std::mem::take(&mut *guard)
}

/// Peek without clearing (tests).
#[cfg(test)]
pub fn peek_last_error() -> String {
    LAST_ERROR.lock_safe().clone()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_last_error_roundtrip() {
        set_last_error("posix_openpt failed: Permission denied");
        assert!(peek_last_error().contains("posix_openpt"));
        let taken = take_last_error();
        assert!(taken.contains("Permission denied"));
        assert!(take_last_error().is_empty());
    }
}
