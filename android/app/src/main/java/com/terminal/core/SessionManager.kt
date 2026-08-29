package com.terminal.core

import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import timber.log.Timber
import java.io.File
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong

/**
 * Manages terminal sessions and their lifecycle.
 * 
 * Handles:
 * - Session creation and destruction
 * - I/O loop for reading PTY output
 * - Checkpointing on lifecycle events
 * - Session restoration
 */
class SessionManager(
    private val checkpointDir: File,
    private val scope: CoroutineScope = CoroutineScope(Dispatchers.IO + SupervisorJob())
) {
    
    /** Active sessions by ID */
    private val sessions = ConcurrentHashMap<String, Session>()
    
    /** Counter for generating unique session IDs */
    private val sessionCounter = AtomicLong(0)
    
    /** Flow of session events */
    private val _events = MutableSharedFlow<SessionEvent>(replay = 0, extraBufferCapacity = 64)
    val events: SharedFlow<SessionEvent> = _events.asSharedFlow()
    
    init {
        // Ensure checkpoint directory exists
        checkpointDir.mkdirs()
        Timber.d("SessionManager initialized with checkpoint dir: $checkpointDir")
    }
    
    /**
     * Create a new terminal session.
     */
    suspend fun createSession(shellPath: String = "/system/bin/sh"): Result<Session> {
        val sessionId = "session_${sessionCounter.incrementAndGet()}"
        
        return withContext(Dispatchers.IO) {
            try {
                // Create native session
                val handle = TerminalEngine.createSession(sessionId).getOrThrow()
                
                // Spawn shell in the app's files dir (writable; "/" is not)
                val cwd = checkpointDir.parent ?: checkpointDir.absolutePath
                TerminalEngine.spawnShell(handle, shellPath, cwd).getOrThrow()
                
                // Create session wrapper
                val session = Session(
                    id = sessionId,
                    handle = handle,
                    shellPath = shellPath
                )
                
                sessions[sessionId] = session
                
                // Start I/O loop
                startReadLoop(session)
                
                // Emit event
                _events.emit(SessionEvent.Created(sessionId))
                
                Timber.i("Created session $sessionId")
                Result.success(session)
                
            } catch (e: Exception) {
                Timber.e(e, "Failed to create session")
                Result.failure(e)
            }
        }
    }
    
    /**
     * Get a session by ID.
     */
    fun getSession(sessionId: String): Session? = sessions[sessionId]
    
    /**
     * Get all active sessions.
     */
    fun getAllSessions(): List<Session> = sessions.values.toList()
    
    /**
     * Write input to a session.
     */
    suspend fun writeToSession(sessionId: String, data: ByteArray): Result<Int> {
        val session = sessions[sessionId]
            ?: return Result.failure(IllegalArgumentException("Session not found: $sessionId"))
        
        return withContext(Dispatchers.IO) {
            TerminalEngine.write(session.handle, data)
        }
    }
    
    /**
     * Write a string to a session (with newline).
     */
    suspend fun writeCommand(sessionId: String, command: String): Result<Int> {
        return writeToSession(sessionId, "$command\n".toByteArray(Charsets.UTF_8))
    }
    
    /**
     * Resize a session's PTY.
     */
    suspend fun resizeSession(sessionId: String, cols: Int, rows: Int): Result<Unit> {
        val session = sessions[sessionId]
            ?: return Result.failure(IllegalArgumentException("Session not found: $sessionId"))
        
        return withContext(Dispatchers.IO) {
            TerminalEngine.resize(session.handle, cols, rows)
        }
    }
    
    /**
     * Send a signal to a session (e.g., SIGINT for Ctrl+C).
     */
    suspend fun signalSession(sessionId: String, signal: Int): Result<Unit> {
        val session = sessions[sessionId]
            ?: return Result.failure(IllegalArgumentException("Session not found: $sessionId"))
        
        return withContext(Dispatchers.IO) {
            TerminalEngine.signal(session.handle, signal)
        }
    }
    
    /**
     * Close a session.
     */
    suspend fun closeSession(sessionId: String): Result<Unit> {
        val session = sessions.remove(sessionId)
            ?: return Result.failure(IllegalArgumentException("Session not found: $sessionId"))
        
        return withContext(Dispatchers.IO) {
            try {
                // Cancel read job
                session.readJob?.cancel()
                
                // Close native session
                TerminalEngine.close(session.handle)
                TerminalEngine.destroySession(session.handle)
                
                // Emit event
                _events.emit(SessionEvent.Closed(sessionId))
                
                Timber.i("Closed session $sessionId")
                Result.success(Unit)
                
            } catch (e: Exception) {
                Timber.e(e, "Error closing session $sessionId")
                Result.failure(e)
            }
        }
    }
    
    /**
     * Checkpoint all sessions (call on Activity.onStop).
     */
    suspend fun checkpointAll() {
        withContext(Dispatchers.IO) {
            sessions.values.forEach { session ->
                try {
                    val dir = File(checkpointDir, session.id)
                    dir.mkdirs()
                    TerminalEngine.checkpoint(session.handle, dir.absolutePath)
                    Timber.d("Checkpointed session ${session.id}")
                } catch (e: Exception) {
                    Timber.e(e, "Failed to checkpoint session ${session.id}")
                }
            }
        }
    }
    
    /**
     * Close all sessions (call on service destroy).
     */
    suspend fun closeAll() {
        val sessionIds = sessions.keys.toList()
        sessionIds.forEach { closeSession(it) }
        scope.cancel()
        Timber.i("All sessions closed")
    }
    
    /**
     * Start the read loop for a session.
     *
     * MUST run on [Dispatchers.IO]. [viewModelScope] uses Main, and
     * `nativeRead` used to block the UI thread until the next PTY byte —
     * which is why the X/keyboard froze and the app died at ~20s (ANR).
     */
    private fun startReadLoop(session: Session) {
        session.readJob = scope.launch(Dispatchers.IO) {
            val buffer = ByteArray(8192)
            
            while (isActive && TerminalEngine.isRunning(session.handle)) {
                try {
                    val result = TerminalEngine.read(session.handle, buffer)
                    
                    result.onSuccess { bytesRead ->
                        if (bytesRead > 0) {
                            val data = buffer.copyOf(bytesRead)
                            session._output.emit(data)
                            _events.emit(SessionEvent.Output(session.id, data))
                        }
                    }.onFailure { e ->
                        Timber.w(e, "Read error in session ${session.id}")
                    }
                    
                    // Small delay to prevent busy-waiting
                    delay(10)
                    
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    Timber.e(e, "Read loop error in session ${session.id}")
                    delay(100)
                }
            }
            
            // Session ended
            val exitCode = TerminalEngine.getExitCode(session.handle)
            _events.emit(SessionEvent.Exited(session.id, exitCode))
            Timber.i("Session ${session.id} read loop ended (exit code: $exitCode)")
        }
    }
}

/**
 * Represents an active terminal session.
 */
class Session(
    val id: String,
    val handle: Long,
    val shellPath: String,
    internal var readJob: Job? = null
) {
    /** Flow of output data from the PTY */
    internal val _output = MutableSharedFlow<ByteArray>(replay = 0, extraBufferCapacity = 256)
    val output: SharedFlow<ByteArray> = _output.asSharedFlow()
    
    /** Check if running */
    val isRunning: Boolean
        get() = TerminalEngine.isRunning(handle)
    
    /** Get session state */
    val state: Int
        get() = TerminalEngine.getSessionState(handle)
    
    /** Get state as string */
    val stateString: String
        get() = TerminalEngine.SessionState.describe(state)
}

/**
 * Events emitted by SessionManager.
 */
sealed class SessionEvent {
    data class Created(val sessionId: String) : SessionEvent()
    data class Output(val sessionId: String, val data: ByteArray) : SessionEvent()
    data class Exited(val sessionId: String, val exitCode: Int) : SessionEvent()
    data class Closed(val sessionId: String) : SessionEvent()
    data class StateChanged(val sessionId: String, val state: Int) : SessionEvent()
}
