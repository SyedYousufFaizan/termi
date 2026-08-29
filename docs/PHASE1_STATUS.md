# Phase 1 Status — Ground Truth

This document exists because the repo previously had a gap between what
the README claimed and what was actually built. This file is the
single source of truth for "is X actually done" — keep it updated as
Phase 1 progresses, and check it before assuming a feature exists.

**Last updated:** 2026-08-29 (on-device handoff). Shell commands run on a
real phone. **Compose output streaming is still wrong** — that is the
bug to hand to the next person. Rebuild Kotlin + `libterminal_core.so`
together (`nativeSpawnShell` takes a cwd argument).

---

## Handoff — read this first

### What is verified on a real device

These were checked on a phone with a debug APK (session stays up, UI
responds):

| Check | Result |
|-------|--------|
| App opens, session **Active**, `/system/bin/sh` starts | Yes |
| Idle 30s+ without ANR / 20s crash | Yes |
| ✕ closes the session, **New** starts another | Yes |
| Gboard + rotate + Home/resume | Yes |
| Toolbar Ctrl+C / Ctrl+D / Tab | Yes |
| `ps` (and similar multi-line output) | Visible in the UI |
| Commands **execute in the child** | Yes (`ps` showed `sh`; later `mkdir`/`echo` run in the PTY) |

The 20s crash and dead ✕/IME were an ANR: `nativeRead` blocked on the
**main thread**. That is fixed (`Dispatchers.IO` + `poll(0)` on the PTY).

Session-create `"Error code: -1905618432"` was Kotlin treating a tagged
heap pointer as a failure (`handle > 0`). Handles are now opaque positive
IDs.

### Open bug: output streaming (this is the handoff)

**The PTY is not the problem. The Compose transcript is.**

The UI is a `LazyColumn` of strings in
`android/.../viewmodels/TerminalViewModel.kt` (`processOutput`). It is
**not** the Rust `Screen` / ANSI renderer. Bytes are UTF-8-decoded,
control characters dropped, ANSI regex-stripped, then lines are committed.

That pipeline still loses or mangles output even when the shell did the
right thing:

1. **One-line results vanish.** Prompts have no trailing newline. Older
   code always *replaced the last row* with the next prompt, so `echo hello`
   and a one-name `ls` disappeared while `ps` (many lines) looked fine.
2. **CRLF from the PTY.** Kernel `ONLCR` turns NL into CR+LF. Treating CR
   as “wipe this line” deleted the line just received, then LF committed
   an empty row. Partial fix is in `processOutput` (`pendingCr` / CRLF);
   **still not trusted on-device** — treat as the first place to debug.
3. **mksh CSI / `1|` garbage.** `/system/etc/mkshrc` paints a color prompt.
   Spawn now sets `ENV=/dev/null` and a simple `PS1`. If `1|` is still
   visible, the stripper in `stripAnsiCodes` is incomplete.
4. **Do not debug `mkdir` from the transcript alone.** We used to export
   `PWD=/data/.../files` without a real `chdir`. Logical `pwd` and the
   prompt showed the app dir; **`mkdir testdir` ran in `/` and failed.**
   Child now `chdir`s in `pre_exec` (host test
   `test_spawn_chdir_is_real_not_just_pwd_env`). After rebuild, `pwd -P`
   must show `…/files/home`. Folders are **app-private**; they will not
   appear in Downloads / system Files. `mkdir /sdcard/…` needs SAF (not
   wired).

**Suggested debug order for the next person**

1. Confirm the APK includes the latest `.so` (chdir + opaque handles +
   `poll`) *and* the latest `TerminalViewModel`.
2. `adb logcat` while running `echo hello` / `ls` / `mkdir testdir`.
   Compare JNI read sizes to lines that land in `outputLines`.
3. `adb shell run-as com.terminal ls files/home` (package id may differ)
   to see whether `mkdir` created a dir when the UI did not show it.
4. Long-term fix: stop building a parallel string buffer. Drive the UI
   from the native `Screen` (already updated on every PTY read in Rust)
   instead of stripping ANSI in Kotlin.

**Code to start in**

| Area | File |
|------|------|
| Line buffer / prompt / CR | `android/app/src/main/java/com/terminal/ui/viewmodels/TerminalViewModel.kt` |
| Read loop (must stay off Main) | `android/.../core/SessionManager.kt` (`startReadLoop`) |
| JNI read | `rust/src/android_jni.rs` (`nativeRead`) |
| Non-blocking PTY read | `rust/src/pty/core.rs`, `rust/src/pty/unix.rs` (`poll_in`, `chdir` in `pre_exec`) |
| Native grid (unused by Compose today) | `rust/src/terminal/screen.rs` |

The input box is a **command line** (type, IME Send/Enter). It is not a
full tty: keys are not sent one-by-one except via the toolbar.

### Rebuild reminder

```bash
cd rust
cargo ndk -t arm64-v8a build --release --features android
# copy libterminal_core.so into android/.../jniLibs/arm64-v8a/
```

Then rebuild/install the Android app so Kotlin and the `.so` match.

---

## 1a. Core terminal — parser/screen attached; **UI transcript is not**

- PTY spawn, read/write, ANSI parsing, screen buffer: implemented in Rust.
- **`PtySession` owns `TerminalParser` + `Screen`.** Every PTY `read` /
  `read_timeout` (and `feed_output` for tests) feeds the parser.
  `checkpoint()` snapshots the live grid. `restore_from_disk()` rebuilds
  the screen. Restored session is **not running**; caller must
  `spawn_shell`.
- **Spawn:** slave via `TIOCGPTPEER` then `/dev/pts/N`. `grantpt` /
  `TIOCSWINSZ` failures are non-fatal. Login-tty first; fallback slave
  stdio (no controlling tty). **Android `chdir`:** `pre_exec` +
  `libc::chdir` because bionic `posix_spawn` often ignores
  `Command::current_dir`. Kotlin cwd is `filesDir/home` (created from Java).
- **Ioctl portability:** do not use `libc::TIOCGPTN` on Android (missing).
  Request type is `c_int` on bionic, `c_ulong` on glibc. CI must
  `cargo check --target aarch64-linux-android --features android --lib`
  — host `--features android` is not enough.
- CSI: erase-to-cursor, save/restore cursor. Mutexes use `lock_safe()`.
- `CHECKPOINT_VERSION` mismatch is a hard error.
- **JNI:** `nativeCheckpoint` uses the real snapshot. `nativeRestore`
  exists; **`SessionManager` still does not call `restore()`.**
- Opaque session handles (positive IDs), not raw pointers.

## 1b. Safety cleanup — DONE (Rust core)

- `clippy::unwrap_used` / `expect_used` are CI hard errors.
- `jni_safe.rs` split so `cargo test` needs no NDK; JNI half is
  `#[cfg(any(feature = "android", target_os = "android"))]`.

## 1d. VFS/SAF — policy layer done, **not** on Android

**Host-tested:** `vfs/capabilities.rs`, `VfsService` (`Ok` / `Blocked` /
`Degraded`), `health.rs` (`PermissionProbe` / `HealthMonitor`),
`InternalProvider` chmod/symlink/readlink.

**Not done:** `SafProvider` is still a stub. No Kotlin → `VfsService` JNI.
No real `ContentResolver` I/O. Permission probe not wired (see
`.cursor/skills/wire-permission-health-check.md`). The shell talks to the
**real Linux FS**, not `VfsService` — that is why `mkdir /sdcard/...`
cannot work yet and why capability warnings never appear in the terminal.

## 1c. Keyboard UX — toolbar on screen; not a real tty

- `CommandToolbar` is on `TerminalScreen` (Ctrl+C/D/Z, Tab, arrows, Esc,
  Home, End). Toolbar `|` `/` `-` buttons are still no-ops.
- Sticky-Ctrl and swipe history: not implemented (see
  `.cursor/skills/add-keyboard-toolbar-gesture.md`).
- Input is IME Send of a whole line, not per-key PTY.

## 1e. Packages — still scaffolding

`package/manager.rs` / `package/repository.rs` unchanged. See ROADMAP.

## 1f. Polish — disconnected

- `TerminalService` is declared, **not started**.
- Checkpoint/restore JNI exists; UI / `SessionManager` do not restore.
- Settings / explorer / copy-paste not a usable flow.
- **Compose does not render native `Screen` or colors** — that is the
  same gap as the streaming handoff above.

## What is left in Phase 1 (priority)

1. **Fix Compose PTY transcript** (or switch UI to native `Screen`) —
   current handoff.
2. **VFS/SAF on device** — real `SafProvider`, JNI to `VfsService`,
   inline capability warnings.
3. Keyboard: sticky Ctrl, per-key input, remaining toolbar keys.
4. Start `TerminalService`; checkpoint on background; restore on relaunch.
5. Packages (scaffolding only).

Do not start Phase 2 (SSH) or Phase 3 (GUI Linux) unless explicitly asked.

## Verification commands (host)

```bash
cd rust
cargo test
# last counted: 84 passed, 1 ignored (`test_pty_spawn_and_command`)
cargo check --features android --all-targets
cargo check --target aarch64-linux-android --features android --lib
cargo clippy --all-targets -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
cargo clippy --features android --all-targets -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
```

**Not a substitute for a phone:** Gradle/Kotlin was not compiled in the
agent environment. After a display-loop change, re-test `echo`, `ls`,
`mkdir testdir`, and `pwd -P` on device — and confirm with `adb` if the
UI still lies.
