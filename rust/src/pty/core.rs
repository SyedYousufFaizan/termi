//! Core PTY functionality
//!
//! Provides the main interface for creating and managing PTY sessions.
//! Uses POSIX `posix_openpt` (Linux and Android). We do **not** use
//! `portable-pty` here: that crate depends on `termios` 0.2, which does
//! not compile for `target_os = "android"` and is what made the NDK CI
//! job fail.

use crate::pty::unix::{child_exit_code, spawn_on_pty, PtyMaster, PtySize};
use crate::session_state::{SessionState, TerminalState};
use crate::utils::error::{PtyError, PtyResult};
use crate::utils::sync_ext::LockExt;
use log::{debug, error, info, warn};
use std::fs::File;
use std::io::{Read, Write};
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// PTY session wrapper - manages a pseudo-terminal and child process
pub struct PtySession {
    /// Session state for checkpointing
    state: Arc<Mutex<TerminalState>>,
    /// The PTY master (for resize ioctl)
    master: Option<PtyMaster>,
    /// Writer handle for the PTY
    writer: Option<File>,
    /// Reader handle for the PTY
    reader: Option<File>,
    /// Child process handle
    child: Option<Child>,
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
            size: PtySize::default(),
        })
    }

    /// Spawn a shell process in the PTY
    pub fn spawn_shell(&mut self, shell_path: &str) -> PtyResult<()> {
        if self.running {
            warn!("PTY already running, closing first");
            self.close()?;
        }

        info!("Spawning shell: {}", shell_path);

        let cwd = self.state.lock().ok().and_then(|state| {
            if !state.cwd.is_empty() && state.cwd != "/" {
                Some(state.cwd.clone())
            } else {
                None
            }
        });

        let (master, child) = spawn_on_pty(shell_path, self.size, cwd.as_deref())?;

        let reader = master.try_clone_reader()?;
        let writer = master.try_clone_writer()?;

        self.master = Some(master);
        self.reader = Some(reader);
        self.writer = Some(writer);
        self.child = Some(child);
        self.running = true;
        self.exit_code = None;

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

        match reader.read(buf) {
            Ok(0) => {
                self.check_child_status();
                Ok(0)
            }
            Ok(n) => {
                debug!("PTY read: {} bytes", n);
                Ok(n)
            }
            Err(e) => {
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

        let reader = self.reader.take().ok_or(PtyError::NotInitialized)?;

        let buf_len = buf.len();
        let shared_buf = Arc::new(Mutex::new(vec![0u8; buf_len]));
        let shared_buf_clone = shared_buf.clone();
        let shared_result = Arc::new(Mutex::new(None::<Result<usize, String>>));
        let shared_result_clone = shared_result.clone();

        let handle = thread::spawn(move || {
            let mut reader = reader;
            // Safety cleanup (Phase 1b): these were `.lock().unwrap()`. A
            // poisoned mutex here (from a panic on some other reader thread
            // in a prior call) would previously panic *this* thread too,
            // which — because this closure runs on a detached
            // `thread::spawn` — could silently wedge the session instead of
            // surfacing a clean PtyError. `lock_safe()` recovers instead;
            // see utils::sync_ext for why that's safe for plain data buffers.
            let mut local_buf = shared_buf_clone.lock_safe();
            match reader.read(&mut local_buf) {
                Ok(n) => *shared_result_clone.lock_safe() = Some(Ok(n)),
                Err(e) => *shared_result_clone.lock_safe() = Some(Err(e.to_string())),
            }
            reader
        });

        thread::sleep(timeout);

        let result = shared_result.lock_safe().take();

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
                let shared = shared_buf.lock_safe();
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

        if let Some(master) = &self.master {
            master.resize(self.size)?;
        }

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
                    let code = child_exit_code(status);
                    info!("Child process exited with code: {}", code);
                    self.exit_code = Some(code);
                    self.running = false;

                    if let Ok(mut state) = self.state.lock() {
                        state.transition_to(SessionState::Checkpointed);
                    }
                }
                Ok(None) => {}
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

        let code = child_exit_code(status);
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

        match signal {
            2 => {
                self.write(&[0x03])?;
            }
            3 => {
                self.write(&[0x1c])?;
            }
            _ => {
                warn!(
                    "Signal {} not directly supported, attempting via child",
                    signal
                );
            }
        }

        Ok(())
    }

    /// Gracefully close the PTY
    pub fn close(&mut self) -> PtyResult<()> {
        info!("Closing PTY session");

        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
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

        let state = session.terminal_state().unwrap();
        assert_eq!(state.dimensions, (120, 40));
    }

    #[test]
    #[ignore]
    fn test_pty_spawn_and_command() {
        let mut session = PtySession::new("integration_test").unwrap();

        session.spawn_shell("/bin/sh").unwrap();
        assert!(session.is_running());

        session.write(b"echo hello\n").unwrap();

        let mut buf = [0u8; 1024];
        let n = session.read(&mut buf).unwrap();
        assert!(n > 0);

        session.close().unwrap();
        assert!(!session.is_running());
    }
}
