---
name: "jni-safety-review"
description: "Safety review checklist for any change touching android_jni.rs, jni_safe.rs, or vfs/android_saf.rs. Use before merging JNI-boundary changes, since mistakes there crash the whole app instead of returning an error."
icon: "shield"
color: "orange"
---

Use this before merging any change that touches `android_jni.rs`,
`jni_safe.rs`, or `vfs/android_saf.rs` — this is the highest-consequence
code in the repo, since a mistake here crashes the whole app instead of
returning an error.

## Checklist

- [ ] No raw `JNIEnv` method calls outside `jni_safe.rs`'s wrappers. If you
      wrote `env.call_method(...)` directly anywhere else, stop and use
      `jni_safe::safe_call_*_method` instead.
- [ ] Every JNI call is followed by exception checking
      (`jni_safe::check_and_clear_exception`), not left to propagate as an
      unchecked pending exception.
- [ ] No `.unwrap()`/`.expect()` anywhere in the call chain from a JNI
      entry point (`extern "C" fn Java_...`) down through whatever it
      calls. A panic that unwinds across the FFI boundary is undefined
      behavior in Rust — it must never happen. Use `catch_unwind` at the
      JNI entry point if there's any doubt (check `android_jni.rs` for the
      existing pattern before adding a new export).
- [ ] Handles crossing the boundary go through `jni_safe::handle_box`/
      `handle_to_ptr`/`handle_to_ref`/`handle_to_mut`/`handle_drop` — never
      a raw pointer cast.
- [ ] New JNI-crate-dependent code (anything using `JNIEnv`, `JObject`,
      `JString`, etc.) is behind `#[cfg(feature = "android")]`. Verify with:
      ```bash
      cd rust && cargo check --features android --all-targets
      cd rust && cargo check --target aarch64-linux-android --features android --lib
      cd rust && cargo build --no-default-features   # must NOT need the jni crate
      ```
      If `cargo build --no-default-features` fails because it's pulling in `jni`,
      something new isn't properly feature-gated.
- [ ] If this touches `vfs/android_saf.rs`: does the change respect the
      capability system? `SafProvider` should NOT implement
      `chmod`/`symlink` — if you're adding code there that makes those
      "work" via some workaround, that's very likely wrong; SAF genuinely
      doesn't support these, and pretending otherwise will produce
      confusing partial failures for users. Talk to whoever owns this repo
      before doing this.

## What "tested" means here (and what it doesn't)

`cargo test` cannot exercise real JNI calls — there's no JVM in that
environment. Passing `cargo check --features android` means the code
*compiles* against the real `jni` crate types, which catches signature
drift, but says nothing about runtime correctness. Real verification for
JNI-boundary changes requires either:

- An instrumented test run on a device/emulator
  (`android/app/androidTest/...`), or
- Manual on-device testing by a human before merge.

Don't describe JNI-boundary changes as "tested" based on `cargo check`
alone — say specifically what was and wasn't verified.
