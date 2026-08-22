//! Process management for PTY sessions
//!
//! Handles process spawning, environment setup, and signal handling.

use crate::utils::error::{PtyError, PtyResult};
use std::collections::HashMap;
use std::path::PathBuf;

/// Configuration for spawning a PTY process
#[derive(Debug, Clone)]
pub struct ProcessConfig {
    /// Path to the shell or program to run
    pub program: PathBuf,
    /// Arguments to pass to the program
    pub args: Vec<String>,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Working directory
    pub cwd: PathBuf,
    /// Terminal dimensions (cols, rows)
    pub size: (u16, u16),
}

impl Default for ProcessConfig {
    fn default() -> Self {
        let mut env = HashMap::new();
        env.insert("TERM".into(), "xterm-256color".into());
        env.insert("COLORTERM".into(), "truecolor".into());
        
        Self {
            program: PathBuf::from("/system/bin/sh"),
            args: vec![],
            env,
            cwd: PathBuf::from("/"),
            size: (80, 24),
        }
    }
}

impl ProcessConfig {
    /// Create a new process config with default shell
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the program to run
    pub fn program(mut self, program: impl Into<PathBuf>) -> Self {
        self.program = program.into();
        self
    }

    /// Add an argument
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Add multiple arguments
    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(|s| s.into()));
        self
    }

    /// Set an environment variable
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set the working directory
    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = cwd.into();
        self
    }

    /// Set terminal size
    pub fn size(mut self, cols: u16, rows: u16) -> Self {
        self.size = (cols, rows);
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> PtyResult<()> {
        if !self.program.exists() {
            return Err(PtyError::SpawnFailed(format!(
                "Program not found: {}",
                self.program.display()
            )));
        }
        Ok(())
    }
}

/// Unix signal constants
pub mod signals {
    pub const SIGHUP: i32 = 1;
    pub const SIGINT: i32 = 2;
    pub const SIGQUIT: i32 = 3;
    pub const SIGKILL: i32 = 9;
    pub const SIGTERM: i32 = 15;
    pub const SIGCONT: i32 = 18;
    pub const SIGSTOP: i32 = 19;
    pub const SIGWINCH: i32 = 28;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_process_config_builder() {
        let config = ProcessConfig::new()
            .program("/bin/bash")
            .arg("-l")
            .env("MY_VAR", "my_value")
            .cwd("/home")
            .size(120, 40);

        assert_eq!(config.program, PathBuf::from("/bin/bash"));
        assert_eq!(config.args, vec!["-l"]);
        assert_eq!(config.env.get("MY_VAR"), Some(&"my_value".to_string()));
        assert_eq!(config.cwd, PathBuf::from("/home"));
        assert_eq!(config.size, (120, 40));
    }
}
