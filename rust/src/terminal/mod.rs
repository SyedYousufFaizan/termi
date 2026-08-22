//! Terminal emulation module
//!
//! Handles ANSI parsing, screen buffer management, and rendering.

pub mod cell;
pub mod parser;
pub mod renderer;
pub mod screen;

pub use cell::*;
pub use parser::*;
pub use renderer::*;
pub use screen::*;
