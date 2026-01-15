//! Safe JNI wrapper module - MANDATORY safety layer for all JNI operations
//!
//! This module provides safe wrappers around raw JNI calls to prevent:
//! - Crashes from unchecked exceptions
//! - Memory corruption from invalid handles
//! - Panics across FFI boundaries
//!
//! CRITICAL: NEVER use raw JNI calls outside this module. Always use these wrappers.

#![allow(dead_code)]

use jni::objects::{GlobalRef, JObject, JString};
use jni::sys::{jboolean, jint, jlong};
use jni::JNIEnv;
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

impl From<JniErrorCode> for jint {
    fn from(code: JniErrorCode) -> jint {
        code as jint
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
            error!("PANIC in native code (caught at FFI boundary): {}", panic_info);
            
            // In release builds with panic=abort, this won't even run
            // But in debug builds, this prevents unwinding across FFI
        }));
    }
}

/// Check for and clear Java exceptions after JNI calls
/// Returns true if an exception was pending (and is now cleared)
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
pub fn safe_call_void_method(
    env: &mut JNIEnv,
    obj: &JObject,
    method_name: &str,
    sig: &str,
    args: &[jni::objects::JValue],
) -> JniResult<()> {
    if obj.is_null() {
        error!("safe_call_void_method: null object for method {}", method_name);
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
pub fn safe_call_int_method(
    env: &mut JNIEnv,
    obj: &JObject,
    method_name: &str,
    sig: &str,
    args: &[jni::objects::JValue],
) -> JniResult<i32> {
    if obj.is_null() {
        error!("safe_call_int_method: null object for method {}", method_name);
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
pub fn safe_call_bool_method(
    env: &mut JNIEnv,
    obj: &JObject,
    method_name: &str,
    sig: &str,
    args: &[jni::objects::JValue],
) -> JniResult<bool> {
    if obj.is_null() {
        error!("safe_call_bool_method: null object for method {}", method_name);
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
pub fn safe_call_long_method(
    env: &mut JNIEnv,
    obj: &JObject,
    method_name: &str,
    sig: &str,
    args: &[jni::objects::JValue],
) -> JniResult<i64> {
    if obj.is_null() {
        error!("safe_call_long_method: null object for method {}", method_name);
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
pub fn safe_new_string<'a>(env: &mut JNIEnv<'a>, s: &str) -> JniResult<JString<'a>> {
    env.new_string(s).map_err(|e| {
        error!("Failed to create JString from '{}': {:?}", s, e);
        JniErrorCode::OutOfMemory
    })
}

/// Safely convert a jlong handle to a pointer
/// CRITICAL: Always validate handles before dereferencing
pub fn handle_to_ptr<T>(handle: jlong) -> JniResult<*mut T> {
    if handle == 0 {
        error!("Invalid handle: null pointer");
        return Err(JniErrorCode::InvalidHandle);
    }

    let ptr = handle as *mut T;
    
    // We can't fully validate the pointer, but we can check alignment
    if (ptr as usize) % std::mem::align_of::<T>() != 0 {
        error!("Invalid handle: misaligned pointer");
        return Err(JniErrorCode::InvalidHandle);
    }

    Ok(ptr)
}

/// Convert a pointer to a jlong handle for passing to Java
pub fn ptr_to_handle<T>(ptr: *mut T) -> jlong {
    ptr as jlong
}

/// Safe wrapper for getting a reference from a handle
/// Use this when you need to read from a handle
pub unsafe fn handle_to_ref<'a, T>(handle: jlong) -> JniResult<&'a T> {
    let ptr = handle_to_ptr::<T>(handle)?;
    
    // SAFETY: Caller guarantees the handle points to valid, initialized memory
    // and will not be accessed mutably during the lifetime 'a
    Ok(&*ptr)
}

/// Safe wrapper for getting a mutable reference from a handle
/// Use this when you need to modify through a handle
pub unsafe fn handle_to_mut<'a, T>(handle: jlong) -> JniResult<&'a mut T> {
    let ptr = handle_to_ptr::<T>(handle)?;
    
    // SAFETY: Caller guarantees the handle points to valid, initialized memory
    // and will not be accessed (mutably or immutably) during the lifetime 'a
    Ok(&mut *ptr)
}

/// Safely box a value and return its handle
/// The handle MUST be freed with handle_drop later
pub fn handle_box<T>(value: T) -> jlong {
    let boxed = Box::new(value);
    ptr_to_handle(Box::into_raw(boxed))
}

/// Safely drop a boxed value from its handle
/// CRITICAL: Only call this once per handle
pub unsafe fn handle_drop<T>(handle: jlong) -> JniResult<()> {
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
#[inline]
pub const fn to_jboolean(b: bool) -> jboolean {
    if b { 1 } else { 0 }
}

/// Convert jboolean to Rust bool
#[inline]
pub const fn from_jboolean(b: jboolean) -> bool {
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
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        assert_eq!(JniErrorCode::Success as i32, 0);
        assert_eq!(JniErrorCode::NullPointer as i32, -1);
        assert_eq!(JniErrorCode::InvalidHandle as i32, -2);
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
}
