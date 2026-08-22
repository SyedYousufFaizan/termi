# Skill: Audit `.unwrap()`/`.expect()` calls

Use this if `clippy::unwrap_used` flags something and it's not obvious how
to fix it, or when doing a periodic safety sweep.

## Step 1: find them, excluding test code

```bash
cd rust/src
grep -rn "\.unwrap()\|\.expect(" --include="*.rs" . | grep -v "^\./.*test"
```

This is a rough filter — always manually check whether a hit is actually
inside a `#[cfg(test)] mod tests { ... }` block before treating it as a
production issue. `.unwrap()` in test assertions is normal and expected;
the project's `mod tests` blocks are explicitly annotated with
`#[allow(clippy::unwrap_used, clippy::expect_used)]` for exactly this
reason — don't "fix" those.

## Step 2: classify each production hit

For each real (non-test) occurrence, ask:

1. **Is this a `Mutex::lock()`?** → use `utils::sync_ext::LockExt::lock_safe()`
   instead. See that module's doc comment for the one case where this is
   NOT appropriate (mutexes protecting real invariants where a torn write
   could leave data in a genuinely broken state — rare in this codebase,
   but check).

2. **Is this parsing/converting something that could legitimately fail
   (user input, file content, JNI-provided data)?** → propagate via `?`
   using the appropriate error type from `utils::error` (`VfsError`,
   `PtyError`, `TerminalError`, `JniErrorCode`). If the calling function
   doesn't return a `Result` yet, that's the actual fix — change its
   signature, don't swallow the error with a fallback default silently
   (silent fallbacks hide real problems just as much as a panic does,
   they just do it later and more confusingly).

3. **Is this "genuinely can't happen" (e.g. unwrapping a regex you just
   compiled from a string literal)?** → still avoid `.unwrap()`. Either:
   - Use `.expect("clear message about why this can't fail")` — this is
     marginally better since it documents intent, but `clippy::expect_used`
     is enforced too, so this still needs a `#[allow(clippy::expect_used)]`
     with a comment justifying it, used sparingly and only for genuine
     invariants, not convenience.
   - Better: restructure so the "can't happen" case is checked once at
     startup/construction time and stored as an already-valid value,
     rather than re-asserted on every call.

## Step 3: verify the fix

```bash
cd rust && cargo test
cd rust && cargo clippy --all-targets -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
```

Both must be clean.

## Historical context

As of the Phase 1b safety pass, a full audit found ~47 `.unwrap()` sites,
of which ~42 were in test code (fine) and 5 were genuine production risks
— all `Mutex::lock().unwrap()` calls in `pty/core.rs`'s read-timeout path,
fixed via `LockExt::lock_safe()`. If you're doing a fresh audit and find a
very different ratio, that's worth flagging as a trend (regression or
improvement) rather than just fixing the individual sites silently.
