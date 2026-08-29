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
     */
    private fun processOutput(data: ByteArray) {
        // Decode UTF-8 (handle partial sequences)
        val text = try {
            String(data, Charsets.UTF_8)
        } catch (e: Exception) {
            // Fall back to ASCII if UTF-8 fails
            String(data, Charsets.US_ASCII)
        }
        
        // Process each character
        for (char in text) {
            when (char) {
                '\n' -> {
                    // End of line - flush partial buffer
                    addOutputLine(partialLine.toString())
                    partialLine.clear()
                }
                '\r' -> {
                    // Carriage return - ignore or handle for overwrite
                }
                '\t' -> {
                    // Tab - expand to spaces
                    partialLine.append("    ")
                }
                else -> {
                    // Regular character
                    if (char.code >= 32 || char == '\u001b') {
                        partialLine.append(char)
                    }
                }
            }
        }
        
        // If partial line has content, show it (for prompts without newline)
        if (partialLine.isNotEmpty()) {
            updateLastLine(partialLine.toString())
        }
        
        // Update session state
        currentSession?.let { session ->
            _uiState.update { it.copy(sessionState = session.stateString) }
        }
    }
    
    /**
     * Add a line to the output buffer.
     */
    private fun addOutputLine(line: String) {
        synchronized(outputBuffer) {
            outputBuffer.add(stripAnsiCodes(line))
            
            // Trim buffer if too large
            while (outputBuffer.size > maxBufferLines) {
                outputBuffer.removeAt(0)
            }
            
            _uiState.update { it.copy(outputLines = outputBuffer.toList()) }
        }
    }
    
    /**
     * Update the last line (for prompts).
     */
    private fun updateLastLine(line: String) {
        synchronized(outputBuffer) {
            if (outputBuffer.isNotEmpty()) {
                outputBuffer[outputBuffer.size - 1] = stripAnsiCodes(line)
            } else {
                outputBuffer.add(stripAnsiCodes(line))
            }
            
            _uiState.update { it.copy(outputLines = outputBuffer.toList()) }
        }
    }
    
    /**
     * Strip ANSI escape codes for display.
     * TODO: Parse and render colors properly in future
     */
    private fun stripAnsiCodes(text: String): String {
        return text.replace(Regex("\u001b\\[[0-9;]*[A-Za-z]"), "")
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
