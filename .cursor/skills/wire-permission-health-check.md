# Skill: Wire up the SAF permission health-check (Rust ↔ Kotlin)

The Rust-side state machine is fully implemented and tested
(`rust/src/vfs/health.rs` — `PermissionState`, `PermissionProbe` trait,
`HealthMonitor`). What's missing is the real Android-side implementation
of `PermissionProbe` and the JNI plumbing to connect it. This skill walks
through closing that gap.

## 1. Understand what Rust needs from Kotlin

`HealthMonitor::scan(&MountTable)` calls `PermissionProbe::check(&MountPoint)
-> PermissionState` for every mount. The Android implementation of this
trait needs to, per mount:

1. Ask Kotlin (via JNI) whether the mount's URI permission is still valid.
2. Distinguish `Stale` (permission still listed but access fails — usually
   recoverable) from `Revoked` (permission no longer listed at all —
   needs the picker again). See the detailed TODO already written in
   `android/app/src/main/java/com/terminal/vfs/PermissionManager.kt` for
   the Kotlin-side logic to distinguish these.

## 2. Add a JNI export (Kotlin → Rust direction doesn't apply here — this
   is Rust calling INTO Kotlin, which is the less common direction in
   this codebase)

This needs a **callback pattern**: Rust holds a reference to a Kotlin
object (via `jni::objects::GlobalRef`, same pattern as `SafProvider` in
`vfs/android_saf.rs` already uses for `helper_ref`) and calls a method on
it through `jni_safe::safe_call_bool_method` or similar.

```rust
// New file or addition to vfs/android_saf.rs — sketch, not final:
use crate::jni_safe::{self, JniErrorCode};
use crate::vfs::health::{PermissionProbe, PermissionState};
use crate::vfs::mount::MountPoint;
use jni::objects::GlobalRef;
use jni::JNIEnv;

pub struct JniPermissionProbe {
    helper_ref: GlobalRef,
}

impl PermissionProbe for JniPermissionProbe {
    fn check(&self, mount: &MountPoint) -> PermissionState {
        // NOTE: PermissionProbe::check doesn't currently take a JNIEnv
        // parameter (it's designed to be callable from pure Rust test
        // code without one). You'll likely need to either:
        //   (a) store a way to attach to the JVM from any thread
        //       (see jni::JavaVM::attach_current_thread), or
        //   (b) change the trait signature to accept &mut JNIEnv and
        //       update FakeProbe in health.rs's tests accordingly.
        // Option (a) keeps the trait host-testable with zero JNI
        // knowledge required by test code; prefer it unless it proves
        // impractical.
        todo!("call helper_ref.checkHealth(uri) via jni_safe, map result to PermissionState")
    }
}
```

## 3. Add the Kotlin method this calls

In `PermissionManager.kt` (see the existing TODO block there for the
suggested `checkHealth(treeUri: Uri): PermissionHealthResult` shape) or
expose it via `SafHelper.kt` if that's where other JNI-facing methods
live in this codebase — check existing conventions before picking one.

## 4. Wire it into app startup

Call `HealthMonitor::scan_needs_attention` once at launch (or on a timer)
and surface results via a banner — reuse `SessionStateBanner.kt`'s pattern
rather than building a new banner component, per
`.cursor/rules/20-android-kotlin.mdc`.

## 5. Testing

- Rust side: already covered by `vfs/health.rs`'s existing tests using
  `FakeProbe` — no changes needed there unless you change the trait
  signature (see the note in step 2), in which case update `FakeProbe`
  to match and re-run `cargo test`.
- Kotlin/JNI side: cannot be verified by `cargo test`. Needs an
  instrumented test (`android/app/androidTest/`) or manual on-device
  verification — actually revoke a folder's permission in Android
  Settings and confirm the app detects it as `Revoked`, then simulate a
  "stale" state if possible (harder to trigger manually; may need to
  mock at the Kotlin layer for a unit test instead of true e2e).
