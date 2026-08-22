//! SAF permission health-check — Phase 1d
//!
//! ## The problem this solves
//!
//! Android's persistable URI permissions (the mechanism that lets Termi
//! remember "yes, you can access this folder" across app restarts) are
//! notoriously fragile in practice: they can silently go stale after a
//! device reboot, an OS update, the user manually revoking access in
//! Settings, or the underlying storage provider being uninstalled. When
//! that happens, every operation against that mount starts failing with an
//! opaque permission error — and because it looks identical to "SAF just
//! doesn't support this operation" (the capability-system case handled by
//! `vfs::service`), users can't tell which problem they're looking at.
//!
//! This module gives that failure mode a name (`PermissionState::Stale` /
//! `Revoked`) and a place to be checked proactively — on app launch, or on
//! a timer — rather than being discovered reactively when a read fails
//! mid-operation.
//!
//! ## Design: the platform boundary is a trait
//!
//! Actually asking Android "is this URI permission still valid?" requires
//! a JNI call to Kotlin's `ContentResolver.persistedUriPermissions`. That
//! can't be exercised outside a real Android runtime. So the *check itself*
//! is behind the [`PermissionProbe`] trait — on-device, this is implemented
//! by a JNI-backed prober (see `TODO` below, wired up when the Kotlin side
//! lands); on host, tests use `FakeProbe` to simulate every state
//! transition. Everything downstream of the trait — aggregating results
//! across all mounts, deciding what the UI should say — is pure logic and
//! is what's actually tested here.

use crate::vfs::mount::{MountPoint, MountTable};
use std::path::PathBuf;

/// The health of a single mount's underlying permission grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    /// Permission is valid; no action needed.
    Valid,
    /// Permission appears to have lapsed (e.g. survived a reboot but the
    /// grant wasn't renewed) — usually recoverable by re-requesting via
    /// the same SAF picker without the user re-navigating to the folder.
    Stale,
    /// Permission was actively revoked (user turned it off in Settings, or
    /// the storage provider/app was uninstalled) — requires the user to
    /// re-grant via the document picker from scratch.
    Revoked,
    /// This mount doesn't have a permission concept (internal storage) or
    /// the check couldn't be completed for a reason unrelated to the
    /// permission itself (e.g. transient JNI error).
    NotApplicable,
}

impl PermissionState {
    /// Whether this state should prompt the user for action.
    pub fn needs_attention(&self) -> bool {
        matches!(self, PermissionState::Stale | PermissionState::Revoked)
    }

    /// User-facing suggested action, or `None` if nothing needs doing.
    pub fn suggested_action(&self) -> Option<&'static str> {
        match self {
            PermissionState::Stale => Some(
                "Access to this storage may have lapsed. Tap to refresh — this usually \
                 doesn't require re-selecting the folder.",
            ),
            PermissionState::Revoked => Some(
                "Access to this storage was revoked. Tap to choose the folder again.",
            ),
            PermissionState::Valid | PermissionState::NotApplicable => None,
        }
    }
}

/// Platform boundary: given a mount, determine its current permission
/// state. Implemented on-device via JNI; implemented by `FakeProbe` in
/// tests.
pub trait PermissionProbe: Send + Sync {
    fn check(&self, mount: &MountPoint) -> PermissionState;
}

/// Probe that treats every mount as valid. Used as the default for
/// platforms/mounts with no permission concept (internal storage always
/// uses this in practice, since `MountSource::Internal` never needs a SAF
/// grant).
pub struct AlwaysValidProbe;

impl PermissionProbe for AlwaysValidProbe {
    fn check(&self, _mount: &MountPoint) -> PermissionState {
        PermissionState::Valid
    }
}

/// Health report for a single mount, suitable for direct display in a
/// settings screen or a startup health-check banner.
#[derive(Debug, Clone)]
pub struct MountHealth {
    pub virtual_path: PathBuf,
    pub display_name: String,
    pub state: PermissionState,
    pub suggested_action: Option<&'static str>,
}

/// Scans a [`MountTable`] and reports the permission health of every mount.
///
/// ## TODO (Kotlin side, not yet wired up)
/// The real Android implementation of `PermissionProbe` should call
/// `SafHelper.hasPersistedPermission(uri): Boolean` via the existing
/// `jni_safe::safe_call_bool_method` wrapper (same pattern already used in
/// `vfs::android_saf::SafProvider::call_bool_method`) and map:
/// `true` → `Valid`, `false` → distinguish `Stale` vs `Revoked` by also
/// checking whether the URI still resolves at all via
/// `ContentResolver.getPersistedUriPermissions()` — if the URI isn't in
/// that list anymore, it's `Revoked`; if it's in the list but access
/// fails, it's `Stale`. This distinction matters because the recovery
/// flows are different (silent refresh vs. re-picker).
pub struct HealthMonitor {
    probe: Box<dyn PermissionProbe>,
}

impl HealthMonitor {
    pub fn new(probe: Box<dyn PermissionProbe>) -> Self {
        Self { probe }
    }

    /// Convenience constructor for platforms/contexts with no real
    /// permission model to check (e.g. running the core on a non-Android
    /// host, or before any SAF mounts have been added).
    pub fn always_valid() -> Self {
        Self::new(Box::new(AlwaysValidProbe))
    }

    /// Check every currently-mounted path and report its health.
    pub fn scan(&self, table: &MountTable) -> Vec<MountHealth> {
        table
            .list_mounts()
            .into_iter()
            .map(|mount| {
                let state = self.probe.check(mount);
                MountHealth {
                    virtual_path: mount.virtual_path.clone(),
                    display_name: mount.display_name.clone(),
                    suggested_action: state.suggested_action(),
                    state,
                }
            })
            .collect()
    }

    /// Convenience: just the mounts that need the user's attention, for a
    /// launch-time banner that shouldn't mention mounts that are fine.
    pub fn scan_needs_attention(&self, table: &MountTable) -> Vec<MountHealth> {
        self.scan(table)
            .into_iter()
            .filter(|h| h.state.needs_attention())
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::vfs::mount::MountPoint;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Test double that returns a pre-programmed state per virtual path,
    /// defaulting to `Valid` for anything not explicitly configured. This
    /// is what lets every permission-loss scenario be exercised without an
    /// Android runtime.
    struct FakeProbe {
        states: Mutex<HashMap<PathBuf, PermissionState>>,
    }

    impl FakeProbe {
        fn new() -> Self {
            Self { states: Mutex::new(HashMap::new()) }
        }

        fn set(&self, path: &str, state: PermissionState) {
            self.states.lock().unwrap().insert(PathBuf::from(path), state);
        }
    }

    impl PermissionProbe for FakeProbe {
        fn check(&self, mount: &MountPoint) -> PermissionState {
            self.states
                .lock()
                .unwrap()
                .get(&mount.virtual_path)
                .copied()
                .unwrap_or(PermissionState::Valid)
        }
    }

    fn table_with_two_mounts() -> MountTable {
        let mut table = MountTable::new("/data/data/com.termi/files");
        table
            .mount(MountPoint::saf("/mnt/sdcard", "content://fake/sdcard", "SD Card"))
            .unwrap();
        table
            .mount(MountPoint::saf("/mnt/usb", "content://fake/usb", "USB Drive"))
            .unwrap();
        table
    }

    #[test]
    fn test_always_valid_probe_reports_all_healthy() {
        let table = table_with_two_mounts();
        let monitor = HealthMonitor::always_valid();
        let report = monitor.scan(&table);

        // root + 2 SAF mounts
        assert_eq!(report.len(), 3);
        assert!(report.iter().all(|h| h.state == PermissionState::Valid));
        assert!(monitor.scan_needs_attention(&table).is_empty());
    }

    #[test]
    fn test_stale_mount_is_flagged_with_refresh_hint() {
        let table = table_with_two_mounts();
        let probe = FakeProbe::new();
        probe.set("/mnt/sdcard", PermissionState::Stale);
        let monitor = HealthMonitor::new(Box::new(probe));

        let needs_attention = monitor.scan_needs_attention(&table);
        assert_eq!(needs_attention.len(), 1);
        assert_eq!(needs_attention[0].display_name, "SD Card");
        assert!(needs_attention[0].suggested_action.unwrap().contains("refresh"));
    }

    #[test]
    fn test_revoked_mount_suggests_re_picking() {
        let table = table_with_two_mounts();
        let probe = FakeProbe::new();
        probe.set("/mnt/usb", PermissionState::Revoked);
        let monitor = HealthMonitor::new(Box::new(probe));

        let needs_attention = monitor.scan_needs_attention(&table);
        assert_eq!(needs_attention.len(), 1);
        assert_eq!(needs_attention[0].display_name, "USB Drive");
        assert!(needs_attention[0]
            .suggested_action
            .unwrap()
            .to_lowercase()
            .contains("choose the folder again"));
    }

    #[test]
    fn test_multiple_stale_mounts_all_reported() {
        let table = table_with_two_mounts();
        let probe = FakeProbe::new();
        probe.set("/mnt/sdcard", PermissionState::Stale);
        probe.set("/mnt/usb", PermissionState::Revoked);
        let monitor = HealthMonitor::new(Box::new(probe));

        assert_eq!(monitor.scan_needs_attention(&table).len(), 2);
        // Root (internal storage) is never affected by SAF permission loss.
        assert_eq!(monitor.scan(&table).len(), 3);
    }
}
