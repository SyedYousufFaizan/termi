//! PTY (Pseudo-Terminal) management module
//!
//! POSIX `posix_openpt` implementation that compiles for both host Linux
//! and Android (`target_os = "android"`). See [`unix`] for why this is
//! not `portable-pty`.

pub mod core;
pub mod process;
pub mod unix;

pub use core::*;
pub use process::*;
