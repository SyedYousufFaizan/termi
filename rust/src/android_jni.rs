//! Android JNI exports
//!
//! This module exposes Rust functions to Kotlin via JNI.
//! All functions use the safe wrappers from jni_safe.rs.
//!
//! SAFETY: Every function MUST:
//! 1. Use jni_safe wrappers (never raw unwrap)
//! 2. Check exceptions after JNI calls
//! 3. Validate handles before use
//! 4. Return error codes on failure

#![cfg(feature = "android")]

use crate::jni_safe::{
    handle_box, handle_drop, handle_to_mut, handle_to_ref, safe_get_string, safe_new_string,
    JniErrorCode,
};
use crate::pty::PtySession;
use crate::session_state::SessionState;
use crate::vfs::capabilities::{VfsCapabilities, VfsOperation};

use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jboolean, jint, jlong};
use jni::JNIEnv;
use log::{debug, error, info};

// ============================================================================
// Library Initialization
// ============================================================================

/// Initialize the native library
/// Call once from Application.onCreate()
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeInit(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    crate::init();
    info!("Terminal core initialized via JNI");
    JniErrorCode::Success.into()
}

/// Get library version
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeGetVersion<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    safe_new_string(&mut env, crate::VERSION).unwrap_or_default()
}

// ============================================================================
// PTY Session Management
// ============================================================================

/// Create a new PTY session
/// Returns: handle (>0) on success, error code (<0) on failure
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeCreateSession<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: JString<'local>,
) -> jlong {
    // Get session ID string
    let session_id = match safe_get_string(&mut env, &session_id) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to get session ID: {:?}", e);
            return e as jlong;
        }
    };

    // Create session
    match PtySession::new(&session_id) {
        Ok(session) => {
            let handle = handle_box(session);
            info!(
                "Created PTY session '{}' with handle {}",
                session_id, handle
            );
            handle
        }
        Err(e) => {
            error!("Failed to create session: {:?}", e);
            JniErrorCode::PtyError as jlong
        }
    }
}

/// Destroy a PTY session
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeDestroySession(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    if handle <= 0 {
        return JniErrorCode::InvalidHandle.into();
    }

    unsafe {
        match handle_drop::<PtySession>(handle) {
            Ok(_) => {
                info!("Destroyed PTY session handle {}", handle);
                JniErrorCode::Success.into()
            }
            Err(e) => {
                error!("Failed to destroy session: {:?}", e);
                e.into()
            }
        }
    }
}

/// Spawn a shell in the PTY
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeSpawnShell<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
    shell_path: JString<'local>,
) -> jint {
    // Validate handle
    let session = match unsafe { handle_to_mut::<PtySession>(handle) } {
        Ok(s) => s,
        Err(e) => return e.into(),
    };

    // Get shell path
    let shell_path = match safe_get_string(&mut env, &shell_path) {
        Ok(s) => s,
        Err(e) => return e.into(),
    };

    // Spawn shell
    match session.spawn_shell(&shell_path) {
        Ok(_) => JniErrorCode::Success.into(),
        Err(e) => {
            error!("Failed to spawn shell: {:?}", e);
            JniErrorCode::PtyError.into()
        }
    }
}

/// Write data to PTY (user input)
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeWrite<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
    data: JByteArray<'local>,
) -> jint {
    // Validate handle
    let session = match unsafe { handle_to_mut::<PtySession>(handle) } {
        Ok(s) => s,
        Err(e) => return e.into(),
    };

    // Get byte array
    let data_vec = match env.convert_byte_array(data) {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to convert byte array: {:?}", e);
            return JniErrorCode::InvalidArgument.into();
        }
    };

    // Write to PTY
    match session.write(&data_vec) {
        Ok(n) => n as jint,
        Err(e) => {
            error!("PTY write failed: {:?}", e);
            JniErrorCode::IoError.into()
        }
    }
}

/// Read data from PTY (terminal output)
/// Returns number of bytes read, or error code
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeRead<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
    buffer: JByteArray<'local>,
) -> jint {
    // Validate handle
    let session = match unsafe { handle_to_mut::<PtySession>(handle) } {
        Ok(s) => s,
        Err(e) => return e.into(),
    };

    // Get buffer size
    let buf_len = match env.get_array_length(&buffer) {
        Ok(len) => len as usize,
        Err(e) => {
            error!("Failed to get buffer length: {:?}", e);
            return JniErrorCode::InvalidArgument.into();
        }
    };

    // Read from PTY
    let mut buf = vec![0u8; buf_len];
    match session.read(&mut buf) {
        Ok(n) => {
            // Copy data to Java array
            if n > 0 {
                if let Err(e) = env.set_byte_array_region(&buffer, 0, unsafe {
                    std::slice::from_raw_parts(buf.as_ptr() as *const i8, n)
                }) {
                    error!("Failed to copy to byte array: {:?}", e);
                    return JniErrorCode::IoError.into();
                }
            }
            n as jint
        }
        Err(e) => {
            error!("PTY read failed: {:?}", e);
            JniErrorCode::IoError.into()
        }
    }
}

/// Resize the PTY
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeResize(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    cols: jint,
    rows: jint,
) -> jint {
    // Validate handle
    let session = match unsafe { handle_to_mut::<PtySession>(handle) } {
        Ok(s) => s,
        Err(e) => return e.into(),
    };

    match session.resize(cols as u16, rows as u16) {
        Ok(_) => JniErrorCode::Success.into(),
        Err(e) => {
            error!("PTY resize failed: {:?}", e);
            JniErrorCode::PtyError.into()
        }
    }
}

/// Close the PTY session
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeClose(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    // Validate handle
    let session = match unsafe { handle_to_mut::<PtySession>(handle) } {
        Ok(s) => s,
        Err(e) => return e.into(),
    };

    match session.close() {
        Ok(_) => JniErrorCode::Success.into(),
        Err(e) => {
            error!("PTY close failed: {:?}", e);
            JniErrorCode::PtyError.into()
        }
    }
}

/// Check if PTY is running
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeIsRunning(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jboolean {
    match unsafe { handle_to_ref::<PtySession>(handle) } {
        Ok(session) => session.is_running() as jboolean,
        Err(_) => 0,
    }
}

/// Get session state (Active=0, Checkpointed=1, Restored=2, Failed=3)
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeGetSessionState(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    match unsafe { handle_to_ref::<PtySession>(handle) } {
        Ok(session) => session.session_state() as jint,
        Err(_) => SessionState::Failed as jint,
    }
}

/// Get exit code (-1 if still running)
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeGetExitCode(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    match unsafe { handle_to_ref::<PtySession>(handle) } {
        Ok(session) => session.exit_code().unwrap_or(-1),
        Err(_) => -1,
    }
}

/// Send signal to PTY (SIGINT=2, SIGQUIT=3, etc.)
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeSignal(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    signal: jint,
) -> jint {
    let session = match unsafe { handle_to_mut::<PtySession>(handle) } {
        Ok(s) => s,
        Err(e) => return e.into(),
    };

    match session.signal(signal) {
        Ok(_) => JniErrorCode::Success.into(),
        Err(e) => {
            error!("Signal failed: {:?}", e);
            JniErrorCode::PtyError.into()
        }
    }
}

// ============================================================================
// VFS Capabilities
// ============================================================================

/// Check if operation is supported on path
/// Operation codes: Read=0, Write=1, ..., Chmod=5, Symlink=7, etc.
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeSupportsOperation<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    path: JString<'local>,
    operation: jint,
    is_saf: jboolean,
) -> jboolean {
    let _path = match safe_get_string(&mut env, &path) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let caps = if is_saf != 0 {
        VfsCapabilities::saf_external()
    } else {
        VfsCapabilities::internal_storage()
    };

    let op = match operation {
        0 => VfsOperation::Read,
        1 => VfsOperation::Write,
        2 => VfsOperation::Create,
        3 => VfsOperation::Delete,
        4 => VfsOperation::Rename,
        5 => VfsOperation::Chmod,
        6 => VfsOperation::Chown,
        7 => VfsOperation::Symlink,
        8 => VfsOperation::Hardlink,
        9 => VfsOperation::ListDir,
        10 => VfsOperation::Mkdir,
        _ => return 0,
    };

    caps.supports(op) as jboolean
}

/// Get limitation warning for SAF path (returns null if no warning)
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeGetLimitationWarning<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    is_saf: jboolean,
) -> JString<'local> {
    let caps = if is_saf != 0 {
        VfsCapabilities::saf_external()
    } else {
        VfsCapabilities::internal_storage()
    };

    match caps.limitation_warning() {
        Some(warning) => safe_new_string(&mut env, &warning).unwrap_or_default(),
        None => JString::default(),
    }
}

// ============================================================================
// Checkpointing
// ============================================================================

// PtySession owns the parser/screen and snapshots them into TerminalState.
// Kotlin only triggers checkpoint/restore — it must not assemble the buffer
// itself. nativeRestore returns a *new* handle: after process death the old
// one is gone. The restored session is not running; call nativeSpawnShell
// if a live PTY is needed. Screen contents are what the user sees as restored.

/// Trigger checkpoint of session state (including parsed screen contents)
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeCheckpoint<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
    checkpoint_dir: JString<'local>,
) -> jint {
    let session = match unsafe { handle_to_ref::<PtySession>(handle) } {
        Ok(s) => s,
        Err(e) => return e.into(),
    };

    let dir = match safe_get_string(&mut env, &checkpoint_dir) {
        Ok(s) => s,
        Err(e) => return e.into(),
    };

    match session.checkpoint(&dir) {
        Ok(_) => {
            info!("Checkpoint saved to {}", dir);
            JniErrorCode::Success.into()
        }
        Err(e) => {
            error!("Checkpoint failed: {:?}", e);
            JniErrorCode::IoError.into()
        }
    }
}

/// Restore a session from disk. Returns a new handle (>0) or an error code (<0).
///
/// The restored session is display-only until nativeSpawnShell is called.
/// This function is type-checked with `--features android`; it is not
/// exercised by `cargo test` (no JVM).
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeRestore<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    session_id: JString<'local>,
    checkpoint_dir: JString<'local>,
) -> jlong {
    let session_id = match safe_get_string(&mut env, &session_id) {
        Ok(s) => s,
        Err(e) => return e as jlong,
    };

    let dir = match safe_get_string(&mut env, &checkpoint_dir) {
        Ok(s) => s,
        Err(e) => return e as jlong,
    };

    match PtySession::restore_from_disk(&session_id, &dir) {
        Ok(session) => {
            let handle = handle_box(session);
            info!(
                "Restored session '{}' from {} with handle {}",
                session_id, dir, handle
            );
            handle
        }
        Err(e) => {
            error!("Restore failed for '{}': {:?}", session_id, e);
            JniErrorCode::IoError as jlong
        }
    }
}

// ============================================================================
// Logging (for debugging)
// ============================================================================

/// Log a message from Kotlin side
#[no_mangle]
pub extern "system" fn Java_com_terminal_core_TerminalEngine_nativeLog<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    level: jint,
    message: JString<'local>,
) {
    let msg = match safe_get_string(&mut env, &message) {
        Ok(s) => s,
        Err(_) => return,
    };

    match level {
        0 => debug!("[Kotlin] {}", msg),
        1 => info!("[Kotlin] {}", msg),
        2 => log::warn!("[Kotlin] {}", msg),
        3 => error!("[Kotlin] {}", msg),
        _ => debug!("[Kotlin] {}", msg),
    }
}
