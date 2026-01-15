//! Session State Management - MANDATORY for handling Android process lifecycle
//!
//! Android WILL kill your process. This is not a bug, it's a feature.
//! This module tracks session state and provides checkpointing/restore capabilities.
//!
//! CRITICAL: Treat checkpoint/restore as NORMAL operation, not edge case.
//! Design for "expect to die" not "hope to survive".

use bincode;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Session lifecycle states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum SessionState {
    /// Session is running normally
    Active = 0,
    /// Session has been checkpointed to disk (app backgrounded)
    Checkpointed = 1,
    /// Session was restored from checkpoint
    Restored = 2,
    /// Session failed to restore or encountered fatal error
    Failed = 3,
}

impl SessionState {
    /// Human-readable description for UI
    pub fn display_message(&self) -> &'static str {
        match self {
            SessionState::Active => "Session active",
            SessionState::Checkpointed => "Session saved (backgrounded)",
            SessionState::Restored => "Session restored from checkpoint",
            SessionState::Failed => "Session restoration failed",
        }
    }

    /// UI indicator color (for Android Color class)
    pub fn indicator_color(&self) -> u32 {
        match self {
            SessionState::Active => 0xFF4CAF50,       // Green
            SessionState::Checkpointed => 0xFFFFC107, // Amber
            SessionState::Restored => 0xFF2196F3,     // Blue
            SessionState::Failed => 0xFFF44336,       // Red
        }
    }

    /// Whether the state requires user attention
    pub fn needs_attention(&self) -> bool {
        matches!(self, SessionState::Failed | SessionState::Restored)
    }
}

/// Complete terminal session state for checkpointing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalState {
    /// Unique session identifier
    pub session_id: String,
    /// Current session state
    pub state: SessionState,
    /// Terminal screen buffer lines
    pub screen_buffer: Vec<ScreenLine>,
    /// Cursor position (row, column)
    pub cursor_position: (u32, u32),
    /// Current working directory
    pub cwd: String,
    /// Environment variables
    pub env_vars: Vec<(String, String)>,
    /// Scrollback buffer size
    pub scrollback_size: usize,
    /// Terminal dimensions (cols, rows)
    pub dimensions: (u16, u16),
    /// Timestamp when checkpointed
    pub checkpoint_time: u64,
    /// Version for forward compatibility
    pub version: u32,
}

/// Single line in the screen buffer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenLine {
    /// Text content of the line
    pub text: String,
    /// Style information (start, end, style_code)
    pub styles: Vec<StyleSpan>,
}

/// Style span for text formatting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleSpan {
    pub start: usize,
    pub end: usize,
    pub foreground: u32,
    pub background: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Default for TerminalState {
    fn default() -> Self {
        Self {
            session_id: generate_session_id(),
            state: SessionState::Active,
            screen_buffer: Vec::new(),
            cursor_position: (0, 0),
            cwd: String::from("/"),
            env_vars: Vec::new(),
            scrollback_size: 10000,
            dimensions: (80, 24),
            checkpoint_time: 0,
            version: 1,
        }
    }
}

impl TerminalState {
    /// Create a new terminal state
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            ..Default::default()
        }
    }

    /// Transition to a new state
    pub fn transition_to(&mut self, new_state: SessionState) {
        let old_state = self.state;
        self.state = new_state;
        info!(
            "Session {} state transition: {:?} -> {:?}",
            self.session_id, old_state, new_state
        );
    }
}

/// Generate a unique session ID
fn generate_session_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_micros();
    format!("session_{:016x}", timestamp)
}

/// Session checkpoint manager
pub struct CheckpointManager {
    /// Directory to store checkpoints
    checkpoint_dir: PathBuf,
    /// Last successful checkpoint time
    last_checkpoint: Option<Instant>,
    /// Minimum interval between checkpoints
    min_interval: Duration,
}

impl CheckpointManager {
    /// Create a new checkpoint manager
    pub fn new(checkpoint_dir: impl Into<PathBuf>) -> Self {
        let dir = checkpoint_dir.into();
        
        // Ensure directory exists
        if let Err(e) = fs::create_dir_all(&dir) {
            error!("Failed to create checkpoint directory {:?}: {}", dir, e);
        }

        Self {
            checkpoint_dir: dir,
            last_checkpoint: None,
            min_interval: Duration::from_secs(30),
        }
    }

    /// Set minimum checkpoint interval
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.min_interval = interval;
        self
    }

    /// Get checkpoint file path for a session
    fn checkpoint_path(&self, session_id: &str) -> PathBuf {
        self.checkpoint_dir.join(format!("{}.checkpoint", session_id))
    }

    /// Check if enough time has passed since last checkpoint
    pub fn should_checkpoint(&self) -> bool {
        match self.last_checkpoint {
            Some(last) => last.elapsed() >= self.min_interval,
            None => true,
        }
    }

    /// Force checkpoint regardless of interval (e.g., on app background)
    pub fn force_checkpoint(&mut self, state: &TerminalState) -> Result<(), CheckpointError> {
        self.checkpoint_internal(state)
    }

    /// Checkpoint if interval has elapsed
    pub fn maybe_checkpoint(&mut self, state: &TerminalState) -> Result<bool, CheckpointError> {
        if self.should_checkpoint() {
            self.checkpoint_internal(state)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Internal checkpoint implementation
    fn checkpoint_internal(&mut self, state: &TerminalState) -> Result<(), CheckpointError> {
        let path = self.checkpoint_path(&state.session_id);
        debug!("Checkpointing session {} to {:?}", state.session_id, path);

        // Create checkpoint state with updated timestamp
        let mut checkpoint_state = state.clone();
        checkpoint_state.state = SessionState::Checkpointed;
        checkpoint_state.checkpoint_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();

        // Serialize to binary
        let data = bincode::serialize(&checkpoint_state)
            .map_err(|e| CheckpointError::Serialization(e.to_string()))?;

        // Write to temporary file first (atomic write)
        let temp_path = path.with_extension("checkpoint.tmp");
        let mut file = File::create(&temp_path)
            .map_err(|e| CheckpointError::Io(e.to_string()))?;
        
        file.write_all(&data)
            .map_err(|e| CheckpointError::Io(e.to_string()))?;
        
        file.sync_all()
            .map_err(|e| CheckpointError::Io(e.to_string()))?;

        // Atomic rename
        fs::rename(&temp_path, &path)
            .map_err(|e| CheckpointError::Io(e.to_string()))?;

        self.last_checkpoint = Some(Instant::now());
        info!(
            "Checkpoint saved: {} ({} bytes)",
            state.session_id,
            data.len()
        );

        Ok(())
    }

    /// Restore session from checkpoint
    pub fn restore(&self, session_id: &str) -> Result<TerminalState, CheckpointError> {
        let path = self.checkpoint_path(session_id);
        info!("Restoring session {} from {:?}", session_id, path);

        if !path.exists() {
            return Err(CheckpointError::NotFound(session_id.into()));
        }

        let mut file = File::open(&path)
            .map_err(|e| CheckpointError::Io(e.to_string()))?;
        
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| CheckpointError::Io(e.to_string()))?;

        let mut state: TerminalState = bincode::deserialize(&data)
            .map_err(|e| CheckpointError::Deserialization(e.to_string()))?;

        // Update state to indicate restoration
        state.state = SessionState::Restored;

        info!(
            "Session restored: {} (checkpointed at {})",
            session_id, state.checkpoint_time
        );

        Ok(state)
    }

    /// List all available checkpoints
    pub fn list_checkpoints(&self) -> Vec<CheckpointInfo> {
        let mut checkpoints = Vec::new();

        if let Ok(entries) = fs::read_dir(&self.checkpoint_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "checkpoint") {
                    if let Some(stem) = path.file_stem() {
                        let session_id = stem.to_string_lossy().into_owned();
                        let metadata = entry.metadata().ok();
                        
                        checkpoints.push(CheckpointInfo {
                            session_id,
                            path,
                            size: metadata.as_ref().map(|m| m.len()).unwrap_or(0),
                            modified: metadata.and_then(|m| m.modified().ok()),
                        });
                    }
                }
            }
        }

        checkpoints.sort_by(|a, b| b.modified.cmp(&a.modified));
        checkpoints
    }

    /// Delete a checkpoint
    pub fn delete_checkpoint(&self, session_id: &str) -> Result<(), CheckpointError> {
        let path = self.checkpoint_path(session_id);
        
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| CheckpointError::Io(e.to_string()))?;
            info!("Deleted checkpoint: {}", session_id);
        }

        Ok(())
    }

    /// Clean up old checkpoints (keep most recent N)
    pub fn cleanup(&self, keep_count: usize) -> Result<usize, CheckpointError> {
        let checkpoints = self.list_checkpoints();
        let mut deleted = 0;

        for checkpoint in checkpoints.iter().skip(keep_count) {
            if let Err(e) = self.delete_checkpoint(&checkpoint.session_id) {
                warn!("Failed to delete old checkpoint {}: {:?}", checkpoint.session_id, e);
            } else {
                deleted += 1;
            }
        }

        if deleted > 0 {
            info!("Cleaned up {} old checkpoints", deleted);
        }

        Ok(deleted)
    }
}

/// Information about a checkpoint file
#[derive(Debug, Clone)]
pub struct CheckpointInfo {
    pub session_id: String,
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

/// Errors that can occur during checkpointing
#[derive(Debug, Clone)]
pub enum CheckpointError {
    /// Checkpoint file not found
    NotFound(String),
    /// I/O error
    Io(String),
    /// Serialization error
    Serialization(String),
    /// Deserialization error
    Deserialization(String),
    /// Version mismatch
    VersionMismatch { expected: u32, found: u32 },
}

impl std::fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckpointError::NotFound(id) => write!(f, "Checkpoint not found: {}", id),
            CheckpointError::Io(msg) => write!(f, "I/O error: {}", msg),
            CheckpointError::Serialization(msg) => write!(f, "Serialization error: {}", msg),
            CheckpointError::Deserialization(msg) => write!(f, "Deserialization error: {}", msg),
            CheckpointError::VersionMismatch { expected, found } => {
                write!(f, "Version mismatch: expected {}, found {}", expected, found)
            }
        }
    }
}

impl std::error::Error for CheckpointError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_session_state_transitions() {
        let mut state = TerminalState::new("test");
        assert_eq!(state.state, SessionState::Active);

        state.transition_to(SessionState::Checkpointed);
        assert_eq!(state.state, SessionState::Checkpointed);

        state.transition_to(SessionState::Restored);
        assert_eq!(state.state, SessionState::Restored);
    }

    #[test]
    fn test_checkpoint_roundtrip() {
        let temp_dir = env::temp_dir().join("terminal_test_checkpoints");
        let _ = fs::remove_dir_all(&temp_dir);

        let mut manager = CheckpointManager::new(&temp_dir);
        
        let mut state = TerminalState::new("test_session");
        state.cwd = "/home/test".into();
        state.screen_buffer.push(ScreenLine {
            text: "Hello, world!".into(),
            styles: vec![],
        });

        // Save checkpoint
        manager.force_checkpoint(&state).expect("Checkpoint failed");

        // Restore checkpoint
        let restored = manager.restore("test_session").expect("Restore failed");
        
        assert_eq!(restored.session_id, "test_session");
        assert_eq!(restored.cwd, "/home/test");
        assert_eq!(restored.state, SessionState::Restored);
        assert_eq!(restored.screen_buffer.len(), 1);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_checkpoint_not_found() {
        let temp_dir = env::temp_dir().join("terminal_test_notfound");
        let manager = CheckpointManager::new(&temp_dir);
        
        let result = manager.restore("nonexistent");
        assert!(matches!(result, Err(CheckpointError::NotFound(_))));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_session_id_generation() {
        let id1 = generate_session_id();
        std::thread::sleep(Duration::from_micros(10));
        let id2 = generate_session_id();
        
        assert_ne!(id1, id2);
        assert!(id1.starts_with("session_"));
    }
}
