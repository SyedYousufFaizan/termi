---
name: "run-tests-and-fix"
description: "Full test/lint workflow matching CI exactly (cargo fmt, test, clippy with unwrap_used enforced, android feature check). Use when asked to test and fix issues, or after any non-trivial change."
icon: "terminal"
color: "green"
---

This is the "make sure everything is actually green" playbook — run this
after any non-trivial change, or when asked to "test and fix issues."

## Step-by-step

```bash
cd rust

# 1. Format check (fast, catches nothing functional but keeps diffs clean)
cargo fmt --check
# if this fails: cargo fmt   (then review the diff, don't blindly trust it)

# 2. Core test suite — no Android SDK/NDK needed
cargo test --verbose

# 3. Lint with the same flags CI uses — unwrap/expect are hard errors here
cargo clippy --all-targets -- -D warnings -D clippy::unwrap_used -D clippy::expect_used

# 4. Verify the JNI boundary still type-checks (doesn't run it, just compiles it)
cargo check --features android --all-targets
cargo clippy --features android -- -D warnings -D clippy::unwrap_used -D clippy::expect_used

# 5. Release build sanity check
cargo build --release
```

If `cargo-clippy` isn't installed in your environment (e.g. a minimal
`apt`-installed Rust toolchain without the clippy component), you won't be
able to run steps 3/5 locally — say so explicitly rather than skipping
them silently, and rely on CI (`.github/workflows/rust_tests.yml`) to
catch what you couldn't check locally. `cargo test` and `cargo check` work
with any standard Rust install and should always be run regardless.

## If something fails

- Test failure → see `.cursor/skills/debug-rust-panic/SKILL.md`.
- Clippy failure on `unwrap_used`/`expect_used` → replace with proper
  error propagation (`?` + the relevant `*Error` enum in `utils/error.rs`)
  or `LockExt::lock_safe()` for mutex locks. Don't `#[allow(...)]` your way
  around it outside a test module — that defeats the point of the lint.
- `cargo check --features android` failure → likely a `jni`-crate-typed
  value leaked outside a `#[cfg(feature = "android")]` block, or a genuine
  signature mismatch after a Kotlin-side JNI method change. Check
  `android_jni.rs` and the corresponding Kotlin `external fun` declaration
  agree on the signature.
- `cargo fmt --check` failure → just run `cargo fmt`, this one's safe to
  auto-fix.

## Don't do this

Don't mark a task "done" if any of the above steps couldn't actually be
run in your environment (e.g. no clippy available) — report exactly which
steps passed, which failed, and which couldn't be attempted, rather than
rounding up to "all tests pass."
