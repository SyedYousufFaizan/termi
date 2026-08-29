# Phase 1 Status — Ground Truth

This document exists because the repo previously had a gap between what
the README claimed and what was actually built. This file is the
single source of truth for "is X actually done" — keep it updated as
Phase 1 progresses, and check it before assuming a feature exists.

Last updated: as of the main-thread PTY-read ANR fix (opaque handles +
non-blocking `poll` + Kotlin `Dispatchers.IO` read loop). On-device still
needs a rebuilt APK.

## 1a. Core terminal — parser/screen now attached to the PTY session

- PTY spawn, read/write, ANSI parsing, screen buffer: implemented.
- **This pass (host-tested):** `PtySession` owns a `TerminalParser` +
  `Screen`. Every PTY `read` / `read_timeout` (and a public `feed_output`
  for tests) runs bytes through the parser. `checkpoint()` snapshots the
  live grid (including scrollback and style spans) into `TerminalState`.
  `restore_from_disk()` rebuilds the screen from that file. The restored
  session is **not running** — Android already killed the old shell; the
  caller must `spawn_shell` if a live PTY is needed. Display state is
  what "Restored" means.
- **Session create (host-tested spawn path):** opening the PTY slave tries
  `TIOCGPTPEER` then `/dev/pts/N`. `grantpt` / `TIOCSWINSZ` failures no
  longer abort. If login-tty `fork`+`setsid` fails, spawn retries with the
  slave attached as stdio. JNI now returns the real error string (not just
  "PTY error"). Kotlin must be rebuilt together with the `.so` (`nativeSpawnShell`
  gained a `cwd` argument). **On-device "Error code: -1905618432"** was
  Kotlin treating a tagged heap pointer as a failed `handle > 0` check —
  `PtySession::new` does not open a PTY and usually succeeds. Handles are
  now opaque positive IDs. **Main-thread ANR (~20s crash, dead X/IME):**
  `SessionManager` was constructed with `viewModelScope` (Main) and
  `nativeRead` blocked on a blocking PTY fd, so the first idle read froze
  the UI until the OEM ANR watchdog killed the app. Reads now `poll(0)`
  and the Kotlin loop runs on `Dispatchers.IO`. **Not verified on a device
  in this environment.**
- CSI gaps filled on the same path: erase-to-cursor (CSI J/K mode 1) and
  save/restore cursor (CSI `s`/`u`). Parser mutexes use `lock_safe()`.
- Checkpoint format version is enforced (`CHECKPOINT_VERSION`); mismatch
  is a hard error, not a silent load.
- **JNI:** `nativeCheckpoint` now goes through `PtySession::checkpoint`
  (so the screen is included). New `nativeRestore(sessionId, dir) -> handle`
  is exported and has a matching Kotlin `external fun`. **Not run on a
  device** — `cargo check --features android` only. Kotlin `SessionManager`
  still does not call `restore()`.
- **Fixed previously:** 5 `Mutex::lock().unwrap()` calls in
  `pty/core.rs`'s `read_timeout` path replaced with
  `utils::sync_ext::LockExt::lock_safe()`.

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
  setup.

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
- Nothing VFS-related in this pass was run on an actual Android device or
  emulator.

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

## 1f. Polish — checkpoint/restore core done; UI still disconnected

Rust can now checkpoint and restore the parsed screen. Kotlin
`TerminalEngine.restore()` exists as a JNI wrapper only. `SessionManager`,
`TerminalService`, and `SessionStateBanner` are still not wired to that
path. Settings/tabs/copy-paste unchanged. The current APK still displays
raw PTY bytes (ANSI stripped in the ViewModel), so the restored *native*
screen is not what the Compose UI shows until that is connected.

## Verification commands (reproduce this yourself)

```bash
cd rust
cargo test                                  # 80 passed (77 lib + 3 integration placeholders), 0 failed, 1 ignored
cargo check --features android --all-targets  # type-checks JNI on host libc
cargo check --target aarch64-linux-android --features android --lib  # bionic ioctl types
cargo clippy --all-targets -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
cargo clippy --features android -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
cargo build --release
```

The 1 ignored test (`test_pty_spawn_and_command`) requires spawning a real
shell binary and is ignored in this sandboxed environment — not a
regression, pre-existing.

**Not verified:** JNI `nativeRestore` runtime, Kotlin compile/Gradle, on-device
checkpoint after process death. Those need an Android SDK/device.
