package com.terminal.ui.viewmodels

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.terminal.TerminalApplication
import com.terminal.core.Session
import com.terminal.core.SessionManager
import com.terminal.core.TerminalEngine
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.launch
import timber.log.Timber
import java.io.File

/**
 * ViewModel for the terminal screen.
 * 
 * Manages:
 * - Session lifecycle
 * - Terminal output buffering
 * - User input handling
 */
class TerminalViewModel(application: Application) : AndroidViewModel(application) {
    
    private val sessionManager: SessionManager
    
    private val _uiState = MutableStateFlow(TerminalUiState())
    val uiState: StateFlow<TerminalUiState> = _uiState.asStateFlow()
    
    private var currentSession: Session? = null
    
    /** Output buffer - stores lines for display */
    private val outputBuffer = mutableListOf<String>()
    private val maxBufferLines = 10_000
    
    /** Partial line buffer for incomplete UTF-8 sequences */
    private var partialLine = StringBuilder()
    /** Last [outputBuffer] row is a live prompt, not a committed line. */
    private var hasOpenLine = false
    /** Chunk ended with CR; next chunk may complete a CRLF pair. */
    private var pendingCr = false
    
    init {
        val checkpointDir = File(application.filesDir, "checkpoints")
        sessionManager = SessionManager(checkpointDir, viewModelScope)
        
        // Auto-create a session on launch
        createSession()
    }
    
    /**
     * Create a new terminal session.
     */
    fun createSession() {
        viewModelScope.launch(Dispatchers.IO) {
            _uiState.update { it.copy(isLoading = true, error = null) }
            
            sessionManager.createSession()
                .onSuccess { session ->
                    currentSession = session
                    synchronized(outputBuffer) {
                        hasOpenLine = false
                        pendingCr = false
                        partialLine.clear()
                    }

                    // Update UI state
                    _uiState.update { 
                        it.copy(
                            isConnected = true,
                            isLoading = false,
                            sessionState = session.stateString
                        )
                    }
                    
                    // Add welcome message
                    addOutputLine("Terminal session started")
                    if (!TerminalApplication.isNativeLibraryLoaded) {
                        addOutputLine("--- MOCK MODE ACTIVE ---")
                        addOutputLine("Type commands to see them echoed back.")
                    } else {
                        addOutputLine("Shell: ${session.shellPath}")
                    }
                    addOutputLine("")
                    
                    // Start collecting output
                    collectSessionOutput(session)
                }
                .onFailure { e ->
                    Timber.e(e, "Failed to create session")
                    _uiState.update { 
                        it.copy(
                            isLoading = false,
                            error = e.message ?: "Failed to create session"
                        )
                    }
                }
        }
    }
    
    /**
     * Close the current session.
     */
    fun closeSession() {
        val session = currentSession ?: return
        
        viewModelScope.launch(Dispatchers.IO) {
            sessionManager.closeSession(session.id)
                .onSuccess {
                    currentSession = null
                    _uiState.update { 
                        it.copy(
                            isConnected = false,
                            sessionState = ""
                        )
                    }
                    addOutputLine("")
                    addOutputLine("[Session closed]")
                }
                .onFailure { e ->
                    Timber.e(e, "Failed to close session")
                    _uiState.update { it.copy(error = "Failed to close: ${e.message}") }
                }
        }
    }
    
    /**
     * Update input text.
     */
    fun updateInput(text: String) {
        _uiState.update { it.copy(inputText = text) }
    }
    
    /**
     * Send the current input as a command.
     */
    fun sendCommand() {
        val session = currentSession
        val command = _uiState.value.inputText.trim()
        
        if (session == null || command.isEmpty()) return
        
        // Clear input
        _uiState.update { it.copy(inputText = "") }
        
        // If mock mode, echo manually
        if (!TerminalApplication.isNativeLibraryLoaded) {
            addOutputLine("$ $command")
            addOutputLine("Executed: $command (Mock)")
            return
        }

        addOutputLine("$ $command")
        
        viewModelScope.launch(Dispatchers.IO) {
            sessionManager.writeCommand(session.id, command)
                .onFailure { e ->
                    Timber.e(e, "Failed to send command")
                    _uiState.update { it.copy(error = "Failed to send: ${e.message}") }
                }
        }
    }
    
    /**
     * Send raw bytes to the PTY (toolbar keys, Ctrl sequences).
     */
    fun sendRaw(bytes: ByteArray) {
        val session = currentSession ?: return
        if (bytes.isEmpty()) return
        viewModelScope.launch(Dispatchers.IO) {
            sessionManager.writeToSession(session.id, bytes)
                .onFailure { e ->
                    Timber.e(e, "Failed to write to PTY")
                    _uiState.update { it.copy(error = "Failed to send: ${e.message}") }
                }
        }
    }
    fun sendCtrlC() {
        sendRaw(byteArrayOf(0x03))
        val session = currentSession ?: return
        viewModelScope.launch(Dispatchers.IO) {
            sessionManager.signalSession(session.id, 2) // SIGINT; no-op if no CTTY
        }
    }
    
    /**
     * Send Ctrl+D (EOF).
     */
    fun sendCtrlD() {
        val session = currentSession ?: return
        
        viewModelScope.launch(Dispatchers.IO) {
            // Send EOF character
            sessionManager.writeToSession(session.id, byteArrayOf(0x04))
        }
    }
    
    /**
     * Clear error message.
     */
    fun clearError() {
        _uiState.update { it.copy(error = null) }
    }
    
    /**
     * Collect output from a session.
     */
    private fun collectSessionOutput(session: Session) {
        viewModelScope.launch {
            session.output.collect { data ->
                processOutput(data)
            }
        }
    }
    
    /**
     * Process raw output bytes into display lines.
     *
     * Prompts have no trailing newline. The old path always *replaced* the
     * last buffer row with that prompt, which ate one-line commands
     * (`echo hello`) while leaving multi-line `ps` mostly visible.
     */
    private fun processOutput(data: ByteArray) {
        val text = try {
            String(data, Charsets.UTF_8)
        } catch (e: Exception) {
            String(data, Charsets.US_ASCII)
        }

        synchronized(outputBuffer) {
            val chars = text.toCharArray()
            var i = 0
            if (pendingCr) {
                pendingCr = false
                if (chars.isEmpty() || chars[0] != '\n') {
                    restartOpenLine()
                }
            }
            while (i < chars.size) {
                when (val char = chars[i]) {
                    '\n' -> {
                        commitLine(partialLine.toString())
                        partialLine.clear()
                    }
                    '\r' -> {
                        // PTY ONLCR turns NL into CR-LF. Treat CRLF as one
                        // newline. A lone CR (or CR split across reads) is
                        // "overwrite this line" — the old code always did
                        // that, which deleted `echo`/`mkdir` output.
                        if (i + 1 < chars.size && chars[i + 1] == '\n') {
                            // skip; \n handles commit
                        } else if (i + 1 == chars.size) {
                            pendingCr = true
                        } else {
                            restartOpenLine()
                        }
                    }
                    '\t' -> partialLine.append("    ")
                    else -> {
                        if (char.code >= 32 || char == '\u001b') {
                            partialLine.append(char)
                        }
                    }
                }
                i++
            }
            if (partialLine.isNotEmpty()) {
                showOpenLine(partialLine.toString())
            }
            publishLinesLocked()
        }

        currentSession?.let { session ->
            _uiState.update { it.copy(sessionState = session.stateString) }
        }
    }

    /**
     * Add a completed line (welcome text, "[Session closed]"). Inserts
     * above an in-progress prompt so we don't bury it.
     */
    private fun addOutputLine(line: String) {
        synchronized(outputBuffer) {
            val cleaned = stripAnsiCodes(line)
            if (hasOpenLine && outputBuffer.isNotEmpty()) {
                outputBuffer.add(outputBuffer.size - 1, cleaned)
            } else {
                outputBuffer.add(cleaned)
            }
            trimBufferLocked()
            publishLinesLocked()
        }
    }

    private fun commitLine(line: String) {
        val cleaned = stripAnsiCodes(line)
        if (hasOpenLine && outputBuffer.isNotEmpty()) {
            outputBuffer[outputBuffer.size - 1] = cleaned
            hasOpenLine = false
        } else {
            outputBuffer.add(cleaned)
        }
        trimBufferLocked()
    }

    private fun showOpenLine(line: String) {
        val cleaned = stripAnsiCodes(line)
        if (hasOpenLine && outputBuffer.isNotEmpty()) {
            outputBuffer[outputBuffer.size - 1] = cleaned
        } else {
            outputBuffer.add(cleaned)
            hasOpenLine = true
        }
        trimBufferLocked()
    }

    private fun trimBufferLocked() {
        while (outputBuffer.size > maxBufferLines) {
            outputBuffer.removeAt(0)
        }
    }

    private fun publishLinesLocked() {
        _uiState.update { it.copy(outputLines = outputBuffer.toList()) }
    }
    
    /**
     * Strip ANSI escape codes for display.
     * TODO: Parse and render colors properly in future
     */
    private fun restartOpenLine() {
        partialLine.clear()
        if (hasOpenLine && outputBuffer.isNotEmpty()) {
            outputBuffer[outputBuffer.size - 1] = ""
        }
    }

    private fun stripAnsiCodes(text: String): String {
        return text
            .replace(Regex("\u001B\\][^\u0007\u001B]*(?:\u0007|\u001B\\\\)"), "")
            .replace(Regex("\u001B\\[[?][0-9;]*[A-Za-z]"), "")
            .replace(Regex("\u001B\\[[0-9;]*[A-Za-z]"), "")
            .replace(Regex("\u001B."), "")
    }
    
    override fun onCleared() {
        super.onCleared()
        
        // Checkpoint before clearing
        viewModelScope.launch {
            sessionManager.checkpointAll()
        }
    }
}

/**
 * UI state for the terminal screen.
 */
data class TerminalUiState(
    val isConnected: Boolean = false,
    val isLoading: Boolean = false,
    val sessionState: String = "",
    val inputText: String = "",
    val outputLines: List<String> = emptyList(),
    val error: String? = null
)
