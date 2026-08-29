//! Utility modules for the terminal core

pub mod error;
pub mod last_error;
pub mod logger;
pub mod sync_ext;

pub use error::*;
pub use last_error::{set_last_error, take_last_error};
pub use sync_ext::LockExt;
