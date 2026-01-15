//! PTY (Pseudo-Terminal) management module
//!
//! Handles spawning and managing terminal processes using the portable-pty crate.

pub mod core;
pub mod process;

pub use core::*;
pub use process::*;
