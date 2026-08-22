# Phase 1 Status — Ground Truth

This document exists because the repo previously had a gap between what
the README claimed and what was actually built. This file is the
single source of truth for "is X actually done" — keep it updated as
Phase 1 progresses, and check it before assuming a feature exists.

Last updated: as of the Phase 1b/1d safety + VFS pass (Rust core only —
see "What's NOT done" below for the honest boundary of this pass).

## 1a. Core terminal — mostly pre-existing, unchanged this pass

- PTY spawn, read/write, ANSI parsing, screen buffer: implemented,
  pre-existing. Not touched in this pass beyond the safety fix below.
- **Fixed this pass**: 5 `Mutex::lock().unwrap()` calls in
  `pty/core.rs`'s `read_timeout` path replaced with
  `utils::sync_ext::LockExt::lock_safe()`, which recovers from lock
  poisoning instead of cascading a panic across threads. Covered by new
  tests in `utils/sync_ext.rs`.

## 1b. Safety cleanup — DONE (Rust core)

- Audited all 47 originally-flagged `.unwrap()`/`.expect()` sites. ~42
  were in test code (fine, idiomatic — no change needed). 5 were genuine
  production risk (see above) and are fixed.
- `#![warn(clippy::unwrap_used, clippy::expect_used)]` enforced crate-wide
  in `lib.rs`; CI (`rust_tests.yml`) runs this as a hard error
  (`-D clippy::unwrap_used -D clippy::expect_used`), so this can't
  silently regress.
- **Architecture fix**: `jni_safe.rs` previously pulled in the `jni` crate
  unconditionally, meaning the entire crate couldn't compile or test
  without a full Android NDK toolchain. Split into a pure,
  always-compiled half (handle management, `JniErrorCode`) and a
  `#[cfg(feature = "android")]`-gated half (actual `JNIEnv` calls).
  `Cargo.toml` default features changed from `["android"]` to `[]`
  accordingly. This is what makes `cargo test` work with zero Android
  setup — verified: 64/64 tests pass with `--no-default-features`, and
  the `android`-gated code separately verified to type-check cleanly with
  `cargo check --features android` (confirmed against a pinned
  toolchain during this pass; CI runs it against current stable Rust).

## 1d. VFS/SAF capability system — the actual product — SUBSTANTIALLY ADVANCED, still not wired to real Android

**What's new and tested (host-side, 100% real, no gaps):**

- `vfs/capabilities.rs` — moved from the crate root (`vfs_capabilities.rs`)
  to its correct location under `vfs/`. No logic changes, pure
  reorganization.
- `FsProvider` trait extended with `chmod`/`symlink`/`readlink`, defaulting
  to `OperationNotSupported`. `InternalProvider` now has real
  implementations of all three (tested: chmod actually changes file mode,
  symlink+readlink round-trip correctly).
- **`vfs/service.rs` (new)** — `VfsService`, the facade that actually
  enforces the capability system before dispatching to a provider.
  Before this pass, `check_operation()` existed but nothing called it —
  capability checking and actual file operations were two unconnected
  systems. Now every operation goes through `VfsService`, which returns a
  `VfsOutcome<T>` (`Ok` / `Blocked{reason,hint}` / `Degraded{value,caveat}`)
  carrying a user-facing hint string — this is the concrete mechanism for
  the roadmap item "surface inline warnings in the terminal instead of
  failing silently." Tested with a `MockSafProvider` standing in for real
  SAF (proves the trait-default blocking behavior works without needing a
  device).
- **`vfs/health.rs` (new)** — permission health-check state machine
  (`PermissionState::{Valid,Stale,Revoked,NotApplicable}`,
  `PermissionProbe` trait, `HealthMonitor`). Fully tested on host with a
  `FakeProbe`. This answers "has this mount's access grant gone stale,"
  which is a different question from "does this filesystem type support
  chmod" (capabilities) — see
  `.cursor/rules/40-vfs-saf-architecture.mdc` for why they're deliberately
  separate.

**What's explicitly NOT done (be honest about this):**

- `vfs/android_saf.rs`'s `SafProvider` is still a stub — every method
  returns "not implemented." The actual JNI calls to Kotlin's
  `ContentResolver`/`DocumentFile` APIs are not written. This pass built
  and tested the *policy layer* (what should happen given a working
  provider); the real SAF I/O implementation is separate work requiring
  actual JNI wiring and on-device testing.
- `PermissionProbe`'s real Android implementation doesn't exist yet — see
  the TODO in `android/.../PermissionManager.kt` and
  `.cursor/skills/wire-permission-health-check.md` for the concrete next
  steps.
- No Kotlin code calls into `VfsService` yet — the JNI exports
  (`android_jni.rs`) haven't been extended to expose it. Kotlin currently
  has no way to trigger the new capability-checked operations.
- Nothing in this pass was run on an actual Android device or emulator —
  no Android SDK/NDK was available in the environment this work was done
  in. Everything above is verified via `cargo test`/`cargo check` on a
  host Linux toolchain only.

## 1c. Keyboard UX — scaffolded, not implemented

- `CommandToolbar.kt`: added Esc/Home/End buttons (wired to real escape
  sequences, should work as-is). Sticky-Ctrl modifier and swipe-based
  history cycling are documented as TODOs with a full worked example in
  `.cursor/skills/add-keyboard-toolbar-gesture.md`, but not implemented —
  this needs actual Compose UI work and on-device gesture testing that
  wasn't possible in this pass.

## 1e. Package system — unchanged, still 0% built

No changes this pass. `package/manager.rs` and `package/repository.rs`
remain scaffolding. See `docs/ROADMAP.md`.

## 1f. Polish — unchanged

Session tabs, copy/paste, settings screen: no changes this pass.

## Verification commands (reproduce this yourself)

```bash
cd rust
cargo test                                  # 64 passed, 0 failed, 1 ignored
cargo check --features android --all-targets  # type-checks clean
cargo build --no-default-features           # builds clean, zero warnings
```

The 1 ignored test (`test_pty_spawn_and_command`) requires spawning a real
shell binary and is ignored in this sandboxed environment — not a
regression, pre-existing.
