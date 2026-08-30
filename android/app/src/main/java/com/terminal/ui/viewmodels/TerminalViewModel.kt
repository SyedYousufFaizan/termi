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
    
    /** Output buffer - stores completed/committed lines for display */
    private val outputBuffer = mutableListOf<String>()
    private val maxBufferLines = 10_000
    
    /** Partial line buffer for currently streaming output */
    private var partialLine = StringBuilder()

    /** Streaming ANSI escape parser states */
    private enum class AnsiState {
        NORMAL, ESC, CSI, OSC, OSC_ESC
    }

    private var ansiState = AnsiState.NORMAL
    private var pendingUtf8 = ByteArray(0)
    
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
                        ansiState = AnsiState.NORMAL
                        pendingUtf8 = ByteArray(0)
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
        
        // Clear input field
        _uiState.update { it.copy(inputText = "") }
        
        // If mock mode, echo manually
        if (!TerminalApplication.isNativeLibraryLoaded) {
            addOutputLine("$ $command")
            addOutputLine("Executed: $command (Mock)")
            return
        }

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
     * Process raw output bytes into display lines using a streaming state machine.
     */
    private fun processOutput(data: ByteArray) {
        val bytesToProcess = if (pendingUtf8.isNotEmpty()) {
            val combined = ByteArray(pendingUtf8.size + data.size)
            System.arraycopy(pendingUtf8, 0, combined, 0, pendingUtf8.size)
            System.arraycopy(data, 0, combined, pendingUtf8.size, data.size)
            pendingUtf8 = ByteArray(0)
            combined
        } else {
            data
        }

        val validLength = getValidUtf8Length(bytesToProcess)
        if (validLength < bytesToProcess.size) {
            pendingUtf8 = bytesToProcess.copyOfRange(validLength, bytesToProcess.size)
        }
        val textToProcess = String(bytesToProcess, 0, validLength, Charsets.UTF_8)

        synchronized(outputBuffer) {
            var i = 0
            val len = textToProcess.length
            while (i < len) {
                val c = textToProcess[i]
                when (ansiState) {
                    AnsiState.NORMAL -> {
                        when (c) {
                            '\u001b' -> ansiState = AnsiState.ESC
                            '\n' -> {
                                commitLine(partialLine.toString())
                                partialLine.clear()
                            }
                            '\r' -> {
                                // Skip carriage return; line breaks are driven by \n
                            }
                            '\b' -> {
                                if (partialLine.isNotEmpty()) {
                                    partialLine.deleteCharAt(partialLine.length - 1)
                                }
                            }
                            '\t' -> {
                                val spaces = 4 - (partialLine.length % 4)
                                partialLine.append(" ".repeat(spaces))
                            }
                            else -> {
                                if (c.code >= 32) {
                                    partialLine.append(c)
                                }
                            }
                        }
                    }
                    AnsiState.ESC -> {
                        when (c) {
                            '[' -> ansiState = AnsiState.CSI
                            ']' -> ansiState = AnsiState.OSC
                            '(', ')' -> { /* wait for designation char */ }
                            else -> ansiState = AnsiState.NORMAL
                        }
                    }
                    AnsiState.CSI -> {
                        when (c) {
                            in '0'..'9', ';', '?', '<', '=', '>', ' ' -> {}
                            'J' -> {
                                partialLine.clear()
                                ansiState = AnsiState.NORMAL
                            }
                            else -> {
                                ansiState = AnsiState.NORMAL
                            }
                        }
                    }
                    AnsiState.OSC -> {
                        when (c) {
                            '\u0007' -> ansiState = AnsiState.NORMAL
                            '\u001b' -> ansiState = AnsiState.OSC_ESC
                            else -> {}
                        }
                    }
                    AnsiState.OSC_ESC -> {
                        when (c) {
                            '\\' -> ansiState = AnsiState.NORMAL
                            else -> ansiState = AnsiState.OSC
                        }
                    }
                }
                i++
            }

            publishLinesLocked()
        }

        currentSession?.let { session ->
            _uiState.update { it.copy(sessionState = session.stateString) }
        }
    }

    private fun getValidUtf8Length(bytes: ByteArray): Int {
        val len = bytes.size
        if (len == 0) return 0
        var i = len - 1
        var needed = 0
        while (i >= 0 && i >= len - 4) {
            val b = bytes[i].toInt() and 0xFF
            if ((b and 0x80) == 0) {
                break
            } else if ((b and 0xC0) == 0x80) {
                needed++
            } else {
                val expected = when {
                    (b and 0xE0) == 0xC0 -> 1
                    (b and 0xF0) == 0xE0 -> 2
                    (b and 0xF8) == 0xF0 -> 3
                    else -> 0
                }
                if (needed < expected) {
                    return i
                } else {
                    return len
                }
            }
            i--
        }
        return len
    }

    /**
     * Add a completed line (welcome text, "[Session closed]").
     */
    private fun addOutputLine(line: String) {
        synchronized(outputBuffer) {
            val cleaned = stripAnsiCodes(line)
            outputBuffer.add(cleaned)
            trimBufferLocked()
            publishLinesLocked()
        }
    }

    private fun commitLine(line: String) {
        val cleaned = stripAnsiCodes(line)
        outputBuffer.add(cleaned)
        trimBufferLocked()
    }

    private fun trimBufferLocked() {
        while (outputBuffer.size > maxBufferLines) {
            outputBuffer.removeAt(0)
        }
    }

    private fun publishLinesLocked() {
        val currentPartial = stripAnsiCodes(partialLine.toString())
        val lines = if (currentPartial.isNotEmpty()) {
            outputBuffer + currentPartial
        } else {
            outputBuffer.toList()
        }
        _uiState.update { it.copy(outputLines = lines) }
    }

    private fun stripAnsiCodes(text: String): String {
        return text
            .replace(Regex("\u001B\\][^\u0007\u001B]*(?:\u0007|\u001B\\\\)"), "")
            .replace(Regex("\u001B\\[[0-9;?><=]*[a-zA-Z|~]"), "")
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
