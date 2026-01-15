//! Virtual Filesystem module
//!
//! Provides a unified filesystem interface that bridges Android SAF URIs
//! to Unix-style paths for terminal operations.

pub mod mount;
pub mod provider;
pub mod cache;

#[cfg(feature = "android")]
pub mod android_saf;

#[cfg(feature = "ios")]
pub mod ios_provider;

pub use mount::*;
pub use provider::*;
pub use cache::*;
