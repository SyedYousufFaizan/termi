---
name: "debug-rust-panic"
description: "Structured checklist for debugging a Rust panic or failing cargo test in the Termi core (poisoned mutexes, VFS capability blocks, JNI-only failures). Use whenever a cargo test fails or a panic message shows up."
icon: "bug"
color: "red"
---

Use this checklist whenever a `cargo test` run fails, a panic message
shows up in logs, or behavior doesn't match what a test expects.

## 1. Reproduce narrowly

```bash
cd rust
cargo test <failing_test_name> -- --nocapture
```

`--nocapture` shows `println!`/`log` output even for passing tests, which
often reveals the actual sequence of events leading to a failure that the
assertion message alone doesn't show.

## 2. Classify the failure

- **Panic with a `.unwrap()`/`.expect()` in the message**: this should
  have been caught by `clippy::unwrap_used` before it ever ran. Two
  possibilities:
  - It's in a `#[cfg(test)] mod tests` block — expected, not a bug, unless
    the *test itself* is wrong.
  - It's in production code — this is a regression in lint coverage.
    Check whether the file has `#![warn(clippy::unwrap_used)]` inherited
    from `lib.rs` (it should, crate-wide) and whether clippy was actually
    run before this code was merged (`cargo clippy --all-targets -- -D
    warnings -D clippy::unwrap_used -D clippy::expect_used`).
- **Poisoned mutex panic** (`PoisonError` or "lock" in the message): check
  whether the code uses `.lock().unwrap()` instead of
  `utils::sync_ext::LockExt::lock_safe()`. If it's a data buffer (not a
  mutex protecting a real invariant), switch to `lock_safe()` — see that
  module's doc comment for when this is/isn't appropriate.
- **VFS operation failed unexpectedly**: check whether the code called a
  `FsProvider` method directly instead of going through `VfsService`. If
  it went through `VfsService`, check whether the `VfsOutcome` was
  `Blocked` (capability check failed — expected for chmod/symlink on SAF)
  vs. an actual bug. Read `vfs/capabilities.rs`'s capability matrix for
  the relevant `FsType` before assuming it's a bug.
- **JNI-related failure**: this can only be meaningfully debugged
  on-device or with device logs (`adb logcat`). Don't try to reproduce JNI
  crashes with `cargo test` — the `android` feature only *type-checks* on
  host, it doesn't give you a JVM to actually exercise those code paths.

## 3. Check `docs/PHASE1_STATUS.md`

If the failing area is listed there as "scaffolded" or "not yet
implemented," the right fix might be "actually implement this," not
"patch around the gap." Don't paper over a known-incomplete area with a
special case.

## 4. Write the regression test first

Before fixing, write a test that fails for the same reason as the bug you
found (or extend an existing test if one is close). This repo's Phase 1d
VFS work follows this pattern throughout — every `VfsOutcome` variant
(`Ok`/`Blocked`/`Degraded`) has a dedicated test in `vfs/service.rs`.

## 5. Fix, then re-run the full suite

```bash
cd rust && cargo test
cd rust && cargo check --features android --all-targets
cd rust && cargo check --target aarch64-linux-android --features android --lib
```

Both should be clean before considering the fix complete.
