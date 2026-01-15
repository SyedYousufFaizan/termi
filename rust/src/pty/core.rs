//! Core PTY functionality
//!
//! Provides the main interface for creating and managing PTY sessions.
//! Uses portable-pty for cross-platform PTY support.

use crate::session_state::{SessionState, TerminalState};
use crate::utils::error::{PtyError, PtyResult};
use log::{debug, error, info, warn};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// PTY session wrapper - manages a pseudo-terminal and child process
pub struct PtySession {
    /// Session state for checkpointing
    state: Arc<Mutex<TerminalState>>,
    /// The PTY master (for read/write)
    master: Option<Box<dyn MasterPty + Send>>,
    /// Writer handle for the PTY
    writer: Option<Box<dyn Write + Send>>,
    /// Reader handle for the PTY  
    reader: Option<Box<dyn Read + Send>>,
    /// Child process handle
    child: Option<Box<dyn Child + Send + Sync>>,
    /// Whether the PTY is running
    running: bool,
    /// Exit code if process has exited
    exit_code: Option<i32>,
    /// Current PTY size
    size: PtySize,
}

impl PtySession {
    /// Create a new PTY session
    pub fn new(session_id: impl Into<String>) -> PtyResult<Self> {
        let state = TerminalState::new(session_id);
        info!("Creating new PTY session: {}", state.session_id);

        Ok(Self {
            state: Arc::new(Mutex::new(state)),
            master: None,
            writer: None,
            reader: None,
            child: None,
            running: false,
            exit_code: None,
            size: PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            },
        })
    }

    /// Spawn a shell process in the PTY
    pub fn spawn_shell(&mut self, shell_path: &str) -> PtyResult<()> {
        if self.running {
            warn!("PTY already running, closing first");
            self.close()?;
        }

        info!("Spawning shell: {}", shell_path);

        // Get the native PTY system
        let pty_system = native_pty_system();

        // Open a PTY pair
        let pair = pty_system.openpty(self.size).map_err(|e| {
            error!("Failed to open PTY: {}", e);
            PtyError::SpawnFailed(format!("Failed to open PTY: {}", e))
        })?;

        // Build the command
        let mut cmd = CommandBuilder::new(shell_path);
        
        // Set up environment
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("LANG", "en_US.UTF-8");
        
        // Get current working directory from state
        if let Ok(state) = self.state.lock() {
            if !state.cwd.is_empty() && state.cwd != "/" {
                cmd.cwd(&state.cwd);
            }
        }

        // Spawn the child process
        let child = pair.slave.spawn_command(cmd).map_err(|e| {
            error!("Failed to spawn shell: {}", e);
            PtyError::SpawnFailed(format!("Failed to spawn {}: {}", shell_path, e))
        })?;

        // Get reader and writer handles
        let reader = pair.master.try_clone_reader().map_err(|e| {
            error!("Failed to get PTY reader: {}", e);
            PtyError::SpawnFailed(format!("Failed to get reader: {}", e))
        })?;

        let writer = pair.master.take_writer().map_err(|e| {
            error!("Failed to get PTY writer: {}", e);
            PtyError::SpawnFailed(format!("Failed to get writer: {}", e))
        })?;

        // Store handles
        self.master = Some(pair.master);
        self.reader = Some(reader);
        self.writer = Some(writer);
        self.child = Some(child);
        self.running = true;
        self.exit_code = None;

        // Update state
        if let Ok(mut state) = self.state.lock() {
            state.transition_to(SessionState::Active);
        }

        info!("Shell spawned successfully");
        Ok(())
    }

    /// Write data to the PTY (user input)
    pub fn write(&mut self, data: &[u8]) -> PtyResult<usize> {
        if !self.running {
            return Err(PtyError::NotInitialized);
        }

        let writer = self.writer.as_mut().ok_or(PtyError::NotInitialized)?;

        let written = writer.write(data).map_err(|e| {
            error!("PTY write failed: {}", e);
            PtyError::WriteFailed(e.to_string())
        })?;

        // Flush to ensure data is sent immediately
        writer.flush().map_err(|e| {
            warn!("PTY flush failed: {}", e);
            PtyError::WriteFailed(e.to_string())
        })?;

        debug!("PTY write: {} bytes", written);
        Ok(written)
    }

    /// Read data from the PTY (terminal output)
    /// Returns 0 if no data available (non-blocking behavior)
    pub fn read(&mut self, buf: &mut [u8]) -> PtyResult<usize> {
        if !self.running {
            return Err(PtyError::NotInitialized);
        }

        let reader = self.reader.as_mut().ok_or(PtyError::NotInitialized)?;

        // Note: portable-pty reader is blocking by default
        // For non-blocking, we'd need to use poll/select or async
        // For now, this will block until data is available
        match reader.read(buf) {
            Ok(0) => {
                // EOF - process likely exited
                self.check_child_status();
                Ok(0)
            }
            Ok(n) => {
                debug!("PTY read: {} bytes", n);
                Ok(n)
            }
            Err(e) => {
                // Check if it's just a temporary error
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    Ok(0)
                } else {
                    error!("PTY read failed: {}", e);
                    self.check_child_status();
                    Err(PtyError::ReadFailed(e.to_string()))
                }
            }
        }
    }

    /// Read with timeout (for non-blocking behavior)
    pub fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> PtyResult<usize> {
        if !self.running {
            return Err(PtyError::NotInitialized);
        }

        // Simple timeout implementation using a thread
        // In production, use proper async I/O
        let reader = self.reader.take().ok_or(PtyError::NotInitialized)?;
        
        let buf_len = buf.len();
        let shared_buf = Arc::new(Mutex::new(vec![0u8; buf_len]));
        let shared_buf_clone = shared_buf.clone();
        let shared_result = Arc::new(Mutex::new(None::<Result<usize, String>>));
        let shared_result_clone = shared_result.clone();

        let handle = thread::spawn(move || {
            let mut reader = reader;
            let mut local_buf = shared_buf_clone.lock().unwrap();
            match reader.read(&mut local_buf) {
                Ok(n) => *shared_result_clone.lock().unwrap() = Some(Ok(n)),
                Err(e) => *shared_result_clone.lock().unwrap() = Some(Err(e.to_string())),
            }
            reader
        });

        // Wait for thread with timeout
        thread::sleep(timeout);
        
        // Check if we got a result
        let result = shared_result.lock().unwrap().take();
        
        // Try to get reader back
        match handle.join() {
            Ok(reader) => {
                self.reader = Some(reader);
            }
            Err(_) => {
                warn!("Read thread panicked");
                return Err(PtyError::ReadFailed("Read thread panicked".into()));
            }
        }

        match result {
            Some(Ok(n)) => {
                let shared = shared_buf.lock().unwrap();
                buf[..n].copy_from_slice(&shared[..n]);
                Ok(n)
            }
            Some(Err(e)) => Err(PtyError::ReadFailed(e)),
            None => Ok(0), // Timeout, no data
        }
    }

    /// Resize the PTY
    pub fn resize(&mut self, cols: u16, rows: u16) -> PtyResult<()> {
        info!("Resizing PTY to {}x{}", cols, rows);

        self.size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        // Resize the actual PTY if running
        if let Some(master) = &self.master {
            master.resize(self.size).map_err(|e| {
                error!("PTY resize failed: {}", e);
                PtyError::ResizeFailed(e.to_string())
            })?;
        }

        // Update state
        if let Ok(mut state) = self.state.lock() {
            state.dimensions = (cols, rows);
        }

        Ok(())
    }

    /// Check if child process has exited
    fn check_child_status(&mut self) {
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let code = status.exit_code() as i32;
                    info!("Child process exited with code: {}", code);
                    self.exit_code = Some(code);
                    self.running = false;

                    if let Ok(mut state) = self.state.lock() {
                        state.transition_to(SessionState::Checkpointed);
                    }
                }
                Ok(None) => {
                    // Still running
                }
                Err(e) => {
                    warn!("Failed to check child status: {}", e);
                }
            }
        }
    }

    /// Wait for child process to exit
    pub fn wait(&mut self) -> PtyResult<i32> {
        let child = self.child.as_mut().ok_or(PtyError::NotInitialized)?;

        let status = child.wait().map_err(|e| {
            error!("Failed to wait for child: {}", e);
            PtyError::SpawnFailed(format!("Wait failed: {}", e))
        })?;

        let code = status.exit_code() as i32;
        self.exit_code = Some(code);
        self.running = false;

        if let Ok(mut state) = self.state.lock() {
            state.transition_to(SessionState::Checkpointed);
        }

        Ok(code)
    }

    /// Get current session state
    pub fn session_state(&self) -> SessionState {
        self.state
            .lock()
            .map(|s| s.state)
            .unwrap_or(SessionState::Failed)
    }

    /// Get terminal state for checkpointing
    pub fn terminal_state(&self) -> Option<TerminalState> {
        self.state.lock().ok().map(|s| s.clone())
    }

    /// Update terminal state (for restoration)
    pub fn set_terminal_state(&mut self, new_state: TerminalState) {
        if let Ok(mut state) = self.state.lock() {
            *state = new_state;
        }
    }

    /// Check if PTY is still running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Get exit code if process has exited
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    /// Get session ID
    pub fn session_id(&self) -> String {
        self.state
            .lock()
            .map(|s| s.session_id.clone())
            .unwrap_or_default()
    }

    /// Send a signal to the PTY process (Unix-like behavior)
    pub fn signal(&mut self, signal: i32) -> PtyResult<()> {
        if !self.running {
            return Err(PtyError::NotInitialized);
        }

        info!("Sending signal {} to PTY", signal);

        // portable-pty doesn't have direct signal support
        // For SIGINT (Ctrl+C), we can send the character
        match signal {
            2 => {
                // SIGINT - send Ctrl+C
                self.write(&[0x03])?;
            }
            3 => {
                // SIGQUIT - send Ctrl+\
                self.write(&[0x1c])?;
            }
            // For other signals, we'd need platform-specific code
            _ => {
                warn!("Signal {} not directly supported, attempting via child", signal);
                // On Unix, we could use kill() on the child PID
                // For now, log a warning
            }
        }

        Ok(())
    }

    /// Gracefully close the PTY
    pub fn close(&mut self) -> PtyResult<()> {
        info!("Closing PTY session");

        // Drop child first to trigger cleanup
        if let Some(mut child) = self.child.take() {
            // Try to kill if still running
            let _ = child.kill();
            let _ = child.wait();
        }

        // Drop PTY handles
        self.writer = None;
        self.reader = None;
        self.master = None;
        self.running = false;

        if let Ok(mut state) = self.state.lock() {
            state.transition_to(SessionState::Checkpointed);
        }

        Ok(())
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        if self.running {
            let _ = self.close();
        }
    }
}

// Ensure PtySession is Send (required for JNI)
// Note: We manage thread safety through the Arc<Mutex<>> on state
unsafe impl Send for PtySession {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_session_creation() {
        let session = PtySession::new("test").unwrap();
        assert!(!session.is_running());
        assert_eq!(session.session_id(), "test");
    }

    #[test]
    fn test_pty_size() {
        let mut session = PtySession::new("test").unwrap();
        session.resize(120, 40).unwrap();
        
        // Verify dimensions via terminal_state()
        let state = session.terminal_state().unwrap();
        assert_eq!(state.dimensions, (120, 40));
    }

    // Note: Full PTY tests require an actual shell
    // These are run manually or in integration tests
    #[test]
    #[ignore] // Run with: cargo test -- --ignored
    fn test_pty_spawn_and_command() {
        let mut session = PtySession::new("integration_test").unwrap();
        
        // Try to spawn a shell (this test requires /bin/sh)
        #[cfg(unix)]
        {
            session.spawn_shell("/bin/sh").unwrap();
            assert!(session.is_running());
            
            // Write a command
            session.write(b"echo hello\n").unwrap();
            
            // Read output (blocking)
            let mut buf = [0u8; 1024];
            let n = session.read(&mut buf).unwrap();
            assert!(n > 0);
            
            session.close().unwrap();
            assert!(!session.is_running());
        }
    }
}
