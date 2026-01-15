//! Terminal emulation module
//!
//! Handles ANSI parsing, screen buffer management, and rendering.

pub mod cell;
pub mod parser;
pub mod screen;
pub mod renderer;

pub use cell::*;
pub use parser::*;
pub use screen::*;
pub use renderer::*;
