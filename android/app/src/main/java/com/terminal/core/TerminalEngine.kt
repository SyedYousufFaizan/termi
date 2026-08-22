package com.terminal.core

import timber.log.Timber

/**
 * JNI bridge to the Rust terminal core library.
 * 
 * All native functions use safe wrappers on the Rust side.
 * Error handling: negative return values indicate errors.
 * 
 * SAFETY: This class must only be used after [loadNativeLibrary] succeeds.
 * MOCK MODE: If the library is not loaded, this class returns mock data to prevent crashes.
 */
object TerminalEngine {
    
    private var isLoaded = false
    
    /**
     * Error codes returned by native functions
     */
    object ErrorCode {
        // Must match rust/src/jni_safe.rs `JniErrorCode` exactly.
        const val SUCCESS = 0
        const val NULL_POINTER = -1
        const val INVALID_HANDLE = -2
        const val JAVA_EXCEPTION = -3
        const val INVALID_UTF8 = -4
        const val INVALID_ARGUMENT = -5
        const val OUT_OF_MEMORY = -6
        const val PTY_ERROR = -7
        const val VFS_ERROR = -8
        const val IO_ERROR = -9
        const val UNKNOWN = -99
        
        fun isError(code: Int): Boolean = code < 0
        
        fun describe(code: Int): String = when (code) {
            SUCCESS -> "Success"
            NULL_POINTER -> "Null pointer"
            INVALID_HANDLE -> "Invalid session handle"
            JAVA_EXCEPTION -> "JNI exception"
            INVALID_UTF8 -> "Invalid UTF-8"
            INVALID_ARGUMENT -> "Invalid argument"
            OUT_OF_MEMORY -> "Out of memory"
            PTY_ERROR -> "PTY error"
            VFS_ERROR -> "VFS error"
            IO_ERROR -> "I/O error"
            UNKNOWN -> "Unknown error"
            else -> "Error code: $code"
        }
    }
    
    /**
     * VFS operation codes for capability checks
     */
    object VfsOperation {
        const val READ = 0
        const val WRITE = 1
        const val CREATE = 2
        const val DELETE = 3
        const val RENAME = 4
        const val CHMOD = 5
        const val CHOWN = 6
        const val SYMLINK = 7
        const val HARDLINK = 8
        const val LIST_DIR = 9
        const val MKDIR = 10
    }
    
    /**
     * Session state values
     */
    object SessionState {
        const val ACTIVE = 0
        const val CHECKPOINTED = 1
        const val RESTORED = 2
        const val FAILED = 3
        
        fun describe(state: Int): String = when (state) {
            ACTIVE -> "Active"
            CHECKPOINTED -> "Checkpointed"
            RESTORED -> "Restored"
            FAILED -> "Failed"
            else -> "Unknown"
        }
    }
    
    /**
     * Load the native library. Must be called before any other method.
     * @return true if loaded successfully
     */
    @Synchronized
    fun loadNativeLibrary(): Boolean {
        if (isLoaded) return true
        
        return try {
            System.loadLibrary("terminal_core")
            isLoaded = true
            Timber.i("Native library loaded successfully")
            true
        } catch (e: UnsatisfiedLinkError) {
            Timber.e(e, "Failed to load native library - MOCK MODE ACTIVE")
            false
        }
    }
    
    /**
     * Initialize the native library. Call once after loading.
     */
    fun initialize(): Result<Unit> {
        if (!isLoaded) return Result.success(Unit) // Mock success
        
        val result = nativeInit()
        return if (result == ErrorCode.SUCCESS) {
            Timber.i("Terminal engine initialized")
            Result.success(Unit)
        } else {
            Result.failure(TerminalException(result, "Initialization failed"))
        }
    }
    
    /**
     * Get the native library version.
     */
    fun getVersion(): String {
        if (!isLoaded) return "0.0.0-mock"
        return nativeGetVersion() ?: "unknown"
    }
    
    // ========================================================================
    // Session Management
    // ========================================================================
    
    /**
     * Create a new PTY session.
     * @param sessionId Unique identifier for this session
     * @return Handle (>0) on success, or error
     */
    fun createSession(sessionId: String): Result<Long> {
        if (!isLoaded) return Result.success(999L) // Mock handle
        
        val handle = nativeCreateSession(sessionId)
        return if (handle > 0) {
            Timber.d("Created session '$sessionId' with handle $handle")
            Result.success(handle)
        } else {
            Result.failure(TerminalException(handle.toInt(), "Failed to create session"))
        }
    }
    
    /**
     * Destroy a PTY session and free resources.
     */
    fun destroySession(handle: Long): Result<Unit> {
        if (!isLoaded) return Result.success(Unit)
        
        val result = nativeDestroySession(handle)
        return if (result == ErrorCode.SUCCESS) {
            Timber.d("Destroyed session handle $handle")
            Result.success(Unit)
        } else {
            Result.failure(TerminalException(result, "Failed to destroy session"))
        }
    }
    
    /**
     * Spawn a shell in the PTY.
     * @param handle Session handle from [createSession]
     * @param shellPath Path to shell (e.g., "/system/bin/sh")
     */
    fun spawnShell(handle: Long, shellPath: String = "/system/bin/sh"): Result<Unit> {
        if (!isLoaded) return Result.success(Unit)
        
        val result = nativeSpawnShell(handle, shellPath)
        return if (result == ErrorCode.SUCCESS) {
            Timber.d("Spawned shell: $shellPath")
            Result.success(Unit)
        } else {
            Result.failure(TerminalException(result, "Failed to spawn shell"))
        }
    }
    
    /**
     * Write data to the PTY (user input).
     * @return Number of bytes written, or error
     */
    fun write(handle: Long, data: ByteArray): Result<Int> {
        if (!isLoaded) return Result.success(data.size)
        
        val result = nativeWrite(handle, data)
        return if (result >= 0) {
            Result.success(result)
        } else {
            Result.failure(TerminalException(result, "Write failed"))
        }
    }
    
    /**
     * Write a string to the PTY.
     */
    fun writeString(handle: Long, text: String): Result<Int> {
        return write(handle, text.toByteArray(Charsets.UTF_8))
    }
    
    /**
     * Read data from the PTY (terminal output).
     * @param buffer Buffer to read into
     * @return Number of bytes read (0 if no data available), or error
     */
    fun read(handle: Long, buffer: ByteArray): Result<Int> {
        if (!isLoaded) return Result.success(0) // No output in mock mode by default
        
        val result = nativeRead(handle, buffer)
        return if (result >= 0) {
            Result.success(result)
        } else {
            Result.failure(TerminalException(result, "Read failed"))
        }
    }
    
    /**
     * Resize the PTY.
     */
    fun resize(handle: Long, cols: Int, rows: Int): Result<Unit> {
        if (!isLoaded) return Result.success(Unit)
        
        val result = nativeResize(handle, cols, rows)
        return if (result == ErrorCode.SUCCESS) {
            Timber.d("Resized to ${cols}x${rows}")
            Result.success(Unit)
        } else {
            Result.failure(TerminalException(result, "Resize failed"))
        }
    }
    
    /**
     * Close the PTY (terminate the shell).
     */
    fun close(handle: Long): Result<Unit> {
        if (!isLoaded) return Result.success(Unit)
        
        val result = nativeClose(handle)
        return if (result == ErrorCode.SUCCESS) {
            Result.success(Unit)
        } else {
            Result.failure(TerminalException(result, "Close failed"))
        }
    }
    
    /**
     * Check if the PTY is running.
     */
    fun isRunning(handle: Long): Boolean {
        if (!isLoaded) return true
        return nativeIsRunning(handle)
    }
    
    /**
     * Get the session state.
     */
    fun getSessionState(handle: Long): Int {
        if (!isLoaded) return SessionState.ACTIVE
        return nativeGetSessionState(handle)
    }
    
    /**
     * Get the exit code (-1 if still running).
     */
    fun getExitCode(handle: Long): Int {
        if (!isLoaded) return -1
        return nativeGetExitCode(handle)
    }
    
    /**
     * Send a signal to the PTY process.
     * Common signals: SIGINT=2, SIGQUIT=3, SIGKILL=9, SIGTERM=15
     */
    fun signal(handle: Long, signal: Int): Result<Unit> {
        if (!isLoaded) return Result.success(Unit)
        
        val result = nativeSignal(handle, signal)
        return if (result == ErrorCode.SUCCESS) {
            Result.success(Unit)
        } else {
            Result.failure(TerminalException(result, "Signal failed"))
        }
    }
    
    // ========================================================================
    // VFS Capabilities
    // ========================================================================
    
    /**
     * Check if an operation is supported on a path.
     */
    fun supportsOperation(path: String, operation: Int, isSaf: Boolean): Boolean {
        if (!isLoaded) return true
        return nativeSupportsOperation(path, operation, isSaf)
    }
    
    /**
     * Get a limitation warning for SAF paths (null if none).
     */
    fun getLimitationWarning(isSaf: Boolean): String? {
        if (!isLoaded) return null
        val warning = nativeGetLimitationWarning(isSaf)
        return if (warning.isNullOrEmpty()) null else warning
    }
    
    // ========================================================================
    // Checkpointing
    // ========================================================================
    
    /**
     * Checkpoint the session state to disk.
     */
    fun checkpoint(handle: Long, checkpointDir: String): Result<Unit> {
        if (!isLoaded) return Result.success(Unit)
        
        val result = nativeCheckpoint(handle, checkpointDir)
        return if (result == ErrorCode.SUCCESS) {
            Timber.d("Checkpoint saved to $checkpointDir")
            Result.success(Unit)
        } else {
            Result.failure(TerminalException(result, "Checkpoint failed"))
        }
    }
    
    // ========================================================================
    // Logging
    // ========================================================================
    
    /**
     * Log levels for native logging
     */
    object LogLevel {
        const val DEBUG = 0
        const val INFO = 1
        const val WARN = 2
        const val ERROR = 3
    }
    
    /**
     * Log a message to the native logger.
     */
    fun log(level: Int, message: String) {
        if (isLoaded) {
            nativeLog(level, message)
        }
    }
    
    // ========================================================================
    // Native Methods
    // ========================================================================
    
    // Library initialization
    private external fun nativeInit(): Int
    private external fun nativeGetVersion(): String?
    
    // Session management
    private external fun nativeCreateSession(sessionId: String): Long
    private external fun nativeDestroySession(handle: Long): Int
    private external fun nativeSpawnShell(handle: Long, shellPath: String): Int
    private external fun nativeWrite(handle: Long, data: ByteArray): Int
    private external fun nativeRead(handle: Long, buffer: ByteArray): Int
    private external fun nativeResize(handle: Long, cols: Int, rows: Int): Int
    private external fun nativeClose(handle: Long): Int
    private external fun nativeIsRunning(handle: Long): Boolean
    private external fun nativeGetSessionState(handle: Long): Int
    private external fun nativeGetExitCode(handle: Long): Int
    private external fun nativeSignal(handle: Long, signal: Int): Int
    
    // VFS capabilities
    private external fun nativeSupportsOperation(path: String, operation: Int, isSaf: Boolean): Boolean
    private external fun nativeGetLimitationWarning(isSaf: Boolean): String?
    
    // Checkpointing
    private external fun nativeCheckpoint(handle: Long, checkpointDir: String): Int
    
    // Logging
    private external fun nativeLog(level: Int, message: String)
}

/**
 * Exception thrown by terminal operations.
 */
class TerminalException(
    val errorCode: Int,
    message: String
) : Exception("$message: ${TerminalEngine.ErrorCode.describe(errorCode)}")
