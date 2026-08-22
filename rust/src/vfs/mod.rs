//! Virtual Filesystem module
//!
//! Provides a unified filesystem interface that bridges Android SAF URIs
//! to Unix-style paths for terminal operations.
//!
//! ## Module map (Phase 1d)
//!
//! - [`capabilities`] — the capability system: what operations (chmod,
//!   symlink, etc.) are actually supported on a given filesystem, and why.
//!   This used to live at the crate root as a same-level sibling of `vfs`,
//!   which was confusing (it's inherently a VFS concept). Moved here as
//!   part of the Phase 1 repo cleanup.
//! - [`provider`] — the `FsProvider` trait implemented by each storage
//!   backend, plus the always-available `InternalProvider`.
//! - [`mount`] — the virtual mount table mapping Unix-style paths to a
//!   provider + its capabilities.
//! - [`service`] — **new in Phase 1d.** The facade that ties the above
//!   together: every real filesystem operation should go through
//!   `VfsService`, which checks capabilities *before* dispatching to a
//!   provider and returns a [`service::VfsOutcome`] carrying inline,
//!   user-facing hints instead of a bare error. This is what makes the
//!   capability system in `capabilities.rs` actually load-bearing instead
//!   of being pure logic nothing calls (which is what it was before this
//!   pass — see `docs/PHASE1_STATUS.md`).
//! - [`health`] — **new in Phase 1d.** SAF permission health-check state
//!   machine: detects when a previously-granted URI permission has gone
//!   stale (reboot, OS update, user revocation) and reports it so the UI
//!   can prompt for re-grant instead of failing silently mid-operation.
//! - [`cache`] — metadata cache to avoid repeated slow SAF calls.
//! - [`android_saf`] — the real SAF-backed provider (JNI bridge to Kotlin).
//!   Only compiled with `--features android`.
//! - [`ios_provider`] — placeholder only. Not resourced for Phase 1/2. See
//!   the doc comment at the top of that file before building on it.

pub mod capabilities;
pub mod mount;
pub mod provider;
pub mod cache;
pub mod service;
pub mod health;

#[cfg(feature = "android")]
pub mod android_saf;

#[cfg(feature = "ios")]
pub mod ios_provider;

pub use capabilities::{VfsCapabilities, VfsOperation, FsType, OperationCheck, check_operation, get_capabilities_for_path};
pub use mount::*;
pub use provider::*;
pub use cache::*;
pub use service::{VfsService, VfsOutcome};
pub use health::{HealthMonitor, MountHealth, PermissionState, PermissionProbe};
