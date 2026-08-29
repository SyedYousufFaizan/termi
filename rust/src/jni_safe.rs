//! Safe JNI wrapper module - MANDATORY safety layer for all JNI operations
//!
//! This module provides safe wrappers around raw JNI calls to prevent:
//! - Crashes from unchecked exceptions
//! - Memory corruption from invalid handles
//! - Panics across FFI boundaries
//!
//! CRITICAL: NEVER use raw JNI calls outside this module. Always use these wrappers.
//!
//! ## Module layout (Phase 1b restructure)
//!
//! This module is split into two halves so the handle-management logic —
//! which is pure Rust and has nothing to do with the `jni` crate specifically —
//! can be compiled and unit-tested on a plain host toolchain with no Android
//! NDK, no `jni` crate, and no JVM anywhere in sight:
//!
//! - **Always compiled** (below): `JniErrorCode`, handle box/unbox helpers,
//!   jboolean conversion, panic hook installation. Zero dependency on the
//!   `jni` crate. Fully covered by `cargo test --no-default-features`.
//! - **`#[cfg(any(feature = "android", target_os = "android"))]`** (bottom of file): the actual
//!   `JNIEnv`-based call wrappers. These require the `jni` crate and can only
//!   be meaningfully exercised on-device or with `--features android`.
//!
//! Run `cargo test` with no flags for the fast, NDK-free path used by CI and
//! by Cursor's local dev loop. Run `cargo check --features android` to verify
//! the JNI-boundary code still type-checks against the real `jni` crate.

#![allow(dead_code)]

use log::{error, warn};
use std::sync::atomic::{AtomicBool, Ordering};

/// Error codes returned to Kotlin (never panic across FFI)
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JniErrorCode {
    Success = 0,
    NullPointer = -1,
    InvalidHandle = -2,
    JavaException = -3,
    InvalidUtf8 = -4,
    InvalidArgument = -5,
    OutOfMemory = -6,
    PtyError = -7,
    VfsError = -8,
    IoError = -9,
    Unknown = -99,
}

impl From<JniErrorCode> for i32 {
    fn from(code: JniErrorCode) -> i32 {
        code as i32
    }
}

/// Result type for JNI operations
pub type JniResult<T> = Result<T, JniErrorCode>;

/// Global flag to track if panic hook is installed
static PANIC_HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Install panic hook to prevent unwinding across FFI boundary
/// Call this ONCE at library initialization
pub fn install_panic_hook() {
    if PANIC_HOOK_INSTALLED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        std::panic::set_hook(Box::new(|panic_info| {
            // Log the panic but don't propagate it
            error!(
                "PANIC in native code (caught at FFI boundary): {}",
                panic_info
            );

            // In release builds with panic=abort, this won't even run
            // But in debug builds, this prevents unwinding across FFI
        }));
    }
}

// ============================================================================
// JNI-crate-dependent wrappers below this line.
//
// Everything above this line is pure Rust with no `jni` crate dependency and
// is exercised by `cargo test` with no feature flags. Everything below
// requires `--features android` to compile, because it touches `JNIEnv`,
// `JObject`, `JString`, and friends. It can only be meaningfully *run*
// on-device (or under a JVM test harness we don't have set up yet) — but it
// should still type-check in CI via `cargo check --features android` so a
// signature typo doesn't silently rot until someone builds for a phone.
// ============================================================================
#[cfg(any(feature = "android", target_os = "android"))]
use jni::objects::{GlobalRef, JObject, JString};
#[cfg(any(feature = "android", target_os = "android"))]
use jni::JNIEnv;

/// Check for and clear Java exceptions after JNI calls
/// Returns true if an exception was pending (and is now cleared)
#[cfg(any(feature = "android", target_os = "android"))]
#[inline]
pub fn check_and_clear_exception(env: &mut JNIEnv) -> bool {
    match env.exception_check() {
        Ok(true) => {
            // Log the exception details before clearing
            if let Err(e) = env.exception_describe() {
                error!("Failed to describe exception: {:?}", e);
            }
            if let Err(e) = env.exception_clear() {
                error!("Failed to clear exception: {:?}", e);
            }
            true
        }
        Ok(false) => false,
        Err(e) => {
            error!("Failed to check for exception: {:?}", e);
            // Assume there might be an exception and try to clear
            let _ = env.exception_clear();
            true
        }
    }
}

/// Safe wrapper for calling Java void methods
#[cfg(any(feature = "android", target_os = "android"))]
pub fn safe_call_void_method(
    env: &mut JNIEnv,
    obj: &JObject,
    method_name: &str,
    sig: &str,
    args: &[jni::objects::JValue],
) -> JniResult<()> {
    if obj.is_null() {
        error!(
            "safe_call_void_method: null object for method {}",
            method_name
        );
        return Err(JniErrorCode::NullPointer);
    }

    let result = env.call_method(obj, method_name, sig, args);

    if check_and_clear_exception(env) {
        error!("Java exception during call to {}{}", method_name, sig);
        return Err(JniErrorCode::JavaException);
    }

    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("JNI call_method failed for {}: {:?}", method_name, e);
            Err(JniErrorCode::Unknown)
        }
    }
}

/// Safe wrapper for calling Java methods that return int
#[cfg(any(feature = "android", target_os = "android"))]
pub fn safe_call_int_method(
    env: &mut JNIEnv,
    obj: &JObject,
    method_name: &str,
    sig: &str,
    args: &[jni::objects::JValue],
) -> JniResult<i32> {
    if obj.is_null() {
        error!(
            "safe_call_int_method: null object for method {}",
            method_name
        );
        return Err(JniErrorCode::NullPointer);
    }

    let result = env.call_method(obj, method_name, sig, args);

    if check_and_clear_exception(env) {
        error!("Java exception during call to {}{}", method_name, sig);
        return Err(JniErrorCode::JavaException);
    }

    match result {
        Ok(val) => val.i().map_err(|e| {
            error!("Failed to extract int from JValue: {:?}", e);
            JniErrorCode::InvalidArgument
        }),
        Err(e) => {
            error!("JNI call_method failed for {}: {:?}", method_name, e);
            Err(JniErrorCode::Unknown)
        }
    }
}

/// Safe wrapper for calling Java methods that return boolean
#[cfg(any(feature = "android", target_os = "android"))]
pub fn safe_call_bool_method(
    env: &mut JNIEnv,
    obj: &JObject,
    method_name: &str,
    sig: &str,
    args: &[jni::objects::JValue],
) -> JniResult<bool> {
    if obj.is_null() {
        error!(
            "safe_call_bool_method: null object for method {}",
            method_name
        );
        return Err(JniErrorCode::NullPointer);
    }

    let result = env.call_method(obj, method_name, sig, args);

    if check_and_clear_exception(env) {
        error!("Java exception during call to {}{}", method_name, sig);
        return Err(JniErrorCode::JavaException);
    }

    match result {
        Ok(val) => val.z().map_err(|e| {
            error!("Failed to extract boolean from JValue: {:?}", e);
            JniErrorCode::InvalidArgument
        }),
        Err(e) => {
            error!("JNI call_method failed for {}: {:?}", method_name, e);
            Err(JniErrorCode::Unknown)
        }
    }
}

/// Safe wrapper for calling Java methods that return long
#[cfg(any(feature = "android", target_os = "android"))]
pub fn safe_call_long_method(
    env: &mut JNIEnv,
    obj: &JObject,
    method_name: &str,
    sig: &str,
    args: &[jni::objects::JValue],
) -> JniResult<i64> {
    if obj.is_null() {
        error!(
            "safe_call_long_method: null object for method {}",
            method_name
        );
        return Err(JniErrorCode::NullPointer);
    }

    let result = env.call_method(obj, method_name, sig, args);

    if check_and_clear_exception(env) {
        error!("Java exception during call to {}{}", method_name, sig);
        return Err(JniErrorCode::JavaException);
    }

    match result {
        Ok(val) => val.j().map_err(|e| {
            error!("Failed to extract long from JValue: {:?}", e);
            JniErrorCode::InvalidArgument
        }),
        Err(e) => {
            error!("JNI call_method failed for {}: {:?}", method_name, e);
            Err(JniErrorCode::Unknown)
        }
    }
}

/// Safely convert a JString to a Rust String
/// Returns None if the string is null or invalid UTF-8
#[cfg(any(feature = "android", target_os = "android"))]
pub fn safe_get_string(env: &mut JNIEnv, s: &JString) -> JniResult<String> {
    if s.is_null() {
        return Err(JniErrorCode::NullPointer);
    }

    let java_str = env.get_string(s).map_err(|e| {
        error!("Failed to get string from JString: {:?}", e);
        JniErrorCode::InvalidUtf8
    })?;

    Ok(java_str.into())
}

/// Safely create a new JString from a Rust string
#[cfg(any(feature = "android", target_os = "android"))]
pub fn safe_new_string<'a>(env: &mut JNIEnv<'a>, s: &str) -> JniResult<JString<'a>> {
    env.new_string(s).map_err(|e| {
        error!("Failed to create JString from '{}': {:?}", s, e);
        JniErrorCode::OutOfMemory
    })
}

/// Safely convert a jlong handle to a pointer
///
/// Uses plain `i64` rather than `jni::sys::jlong` so this function has zero
/// dependency on the `jni` crate — `jlong` is a type alias for `i64` anyway,
/// so this is a no-op change at the JNI boundary but lets this file compile
/// and unit-test on host without the `android` feature enabled.
///
/// CRITICAL: Always validate handles before dereferencing
pub fn handle_to_ptr<T>(handle: i64) -> JniResult<*mut T> {
    if handle == 0 {
        error!("Invalid handle: null pointer");
        return Err(JniErrorCode::InvalidHandle);
    }

    let ptr = handle as *mut T;

    // We can't fully validate the pointer, but we can check alignment
    if !(ptr as usize).is_multiple_of(std::mem::align_of::<T>()) {
        error!("Invalid handle: misaligned pointer");
        return Err(JniErrorCode::InvalidHandle);
    }

    Ok(ptr)
}

/// Convert a pointer to a jlong handle for passing to Java
pub fn ptr_to_handle<T>(ptr: *mut T) -> i64 {
    ptr as i64
}

/// Safe wrapper for getting a reference from a handle.
/// Use this when you need to read from a handle.
///
/// # Safety
///
/// `handle` must have been produced by [`handle_box`] for a live value of
/// type `T`, and no mutable alias may exist for the duration of `'a`.
pub unsafe fn handle_to_ref<'a, T>(handle: i64) -> JniResult<&'a T> {
    let ptr = handle_to_ptr::<T>(handle)?;

    // SAFETY: Caller guarantees the handle points to valid, initialized memory
    // and will not be accessed mutably during the lifetime 'a
    Ok(&*ptr)
}

/// Safe wrapper for getting a mutable reference from a handle.
/// Use this when you need to modify through a handle.
///
/// # Safety
///
/// `handle` must have been produced by [`handle_box`] for a live value of
/// type `T`, and no other alias (mutable or shared) may exist for the
/// duration of `'a`.
pub unsafe fn handle_to_mut<'a, T>(handle: i64) -> JniResult<&'a mut T> {
    let ptr = handle_to_ptr::<T>(handle)?;

    // SAFETY: Caller guarantees the handle points to valid, initialized memory
    // and will not be accessed (mutably or immutably) during the lifetime 'a
    Ok(&mut *ptr)
}

/// Safely box a value and return its handle
/// The handle MUST be freed with handle_drop later
pub fn handle_box<T>(value: T) -> i64 {
    let boxed = Box::new(value);
    ptr_to_handle(Box::into_raw(boxed))
}

/// Safely drop a boxed value from its handle.
/// CRITICAL: Only call this once per handle.
///
/// # Safety
///
/// `handle` must have been produced by [`handle_box`] for a value of type
/// `T`, must not have been dropped already, and no outstanding references
/// to that value may exist.
pub unsafe fn handle_drop<T>(handle: i64) -> JniResult<()> {
    if handle == 0 {
        warn!("handle_drop called with null handle (already freed?)");
        return Ok(());
    }

    let ptr = handle_to_ptr::<T>(handle)?;

    // SAFETY: Caller guarantees:
    // 1. handle was created by handle_box
    // 2. handle has not been freed before
    // 3. no references to the value exist
    let _ = Box::from_raw(ptr);

    Ok(())
}

/// Create a GlobalRef that persists beyond the current JNI call
/// Use this for callbacks or storing Java objects in Rust
#[cfg(any(feature = "android", target_os = "android"))]
pub fn create_global_ref<'a>(env: &mut JNIEnv<'a>, obj: &JObject<'a>) -> JniResult<GlobalRef> {
    if obj.is_null() {
        return Err(JniErrorCode::NullPointer);
    }

    env.new_global_ref(obj).map_err(|e| {
        error!("Failed to create global ref: {:?}", e);
        JniErrorCode::OutOfMemory
    })
}

/// Macro for safely executing JNI code blocks with automatic exception handling
/// Usage:
/// ```ignore
/// jni_safe_block!(env, {
///     // Your JNI code here
///     Ok(result)
/// })
/// ```
#[macro_export]
macro_rules! jni_safe_block {
    ($env:expr, $block:expr) => {{
        use $crate::jni_safe::{check_and_clear_exception, JniErrorCode};

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $block));

        match result {
            Ok(inner_result) => {
                // Check for any uncaught exceptions
                if check_and_clear_exception($env) {
                    Err(JniErrorCode::JavaException)
                } else {
                    inner_result
                }
            }
            Err(panic) => {
                log::error!("Panic in JNI block: {:?}", panic);
                Err(JniErrorCode::Unknown)
            }
        }
    }};
}

/// Convert boolean for JNI (jboolean is u8, not bool)
///
/// Uses plain `u8` rather than `jni::sys::jboolean` (a type alias for `u8`)
/// for the same host-testability reason as the handle helpers above.
#[inline]
pub const fn to_jboolean(b: bool) -> u8 {
    if b {
        1
    } else {
        0
    }
}

/// Convert jboolean to Rust bool
#[inline]
pub const fn from_jboolean(b: u8) -> bool {
    b != 0
}

/// Trait for types that can be safely passed across JNI boundary
pub trait JniSafe: Sized + Send + 'static {}

// Implement JniSafe for common types
impl JniSafe for String {}
impl JniSafe for Vec<u8> {}
impl JniSafe for i32 {}
impl JniSafe for i64 {}
impl JniSafe for bool {}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        // These numeric values are part of the JNI contract with
        // `TerminalEngine.ErrorCode` on the Kotlin side. Changing one
        // without changing the other mislabels every native error.
        assert_eq!(JniErrorCode::Success as i32, 0);
        assert_eq!(JniErrorCode::NullPointer as i32, -1);
        assert_eq!(JniErrorCode::InvalidHandle as i32, -2);
        assert_eq!(JniErrorCode::JavaException as i32, -3);
        assert_eq!(JniErrorCode::InvalidUtf8 as i32, -4);
        assert_eq!(JniErrorCode::InvalidArgument as i32, -5);
        assert_eq!(JniErrorCode::OutOfMemory as i32, -6);
        assert_eq!(JniErrorCode::PtyError as i32, -7);
        assert_eq!(JniErrorCode::VfsError as i32, -8);
        assert_eq!(JniErrorCode::IoError as i32, -9);
        assert_eq!(JniErrorCode::Unknown as i32, -99);
    }

    #[test]
    fn test_handle_roundtrip() {
        let value = Box::new(42i32);
        let handle = ptr_to_handle(Box::into_raw(value));
        assert!(handle != 0);

        unsafe {
            let ptr = handle_to_ptr::<i32>(handle).unwrap();
            assert_eq!(*ptr, 42);
            let _ = Box::from_raw(ptr); // Clean up
        }
    }

    #[test]
    fn test_null_handle() {
        let result = handle_to_ptr::<i32>(0);
        assert_eq!(result, Err(JniErrorCode::InvalidHandle));
    }

    #[test]
    fn test_jboolean_conversion() {
        assert_eq!(to_jboolean(true), 1);
        assert_eq!(to_jboolean(false), 0);
        assert!(from_jboolean(1));
        assert!(from_jboolean(255)); // Any non-zero is true
        assert!(!from_jboolean(0));
    }

    #[test]
    fn test_handle_box_and_drop() {
        // Exercises handle_box/handle_drop, which previously had no direct
        // test coverage even though they're the primary way session/PTY
        // handles cross the JNI boundary.
        let handle = handle_box(String::from("hello"));
        assert!(handle != 0);

        unsafe {
            let s: &String = handle_to_ref(handle).unwrap();
            assert_eq!(s, "hello");
        }

        unsafe {
            handle_drop::<String>(handle).unwrap();
        }
    }

    #[test]
    fn test_handle_drop_null_is_noop() {
        // handle_drop(0) must be safe (no-op) since Kotlin may call this
        // defensively on a handle it thinks might already be freed.
        unsafe {
            assert!(handle_drop::<i32>(0).is_ok());
        }
    }

    #[test]
    fn test_panic_hook_install_is_idempotent() {
        // install_panic_hook uses a CAS guard; calling it repeatedly must
        // never panic or double-install.
        install_panic_hook();
        install_panic_hook();
        install_panic_hook();
    }
}
