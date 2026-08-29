# Known Limitations

This document explains important limitations of the terminal app that affect how you can use it. Please read this before reporting issues.

For **what is actually implemented vs scaffolded right now**, including the
on-device output-streaming handoff, see
[PHASE1_STATUS.md](PHASE1_STATUS.md). This file is about platform
constraints (SAF, OEMs). Do not use it to claim the Compose transcript
is finished.

## Current alpha (on-device)

These are product/UI gaps, not SAF theory:

- **PTY output in the UI can lie.** The shell may run `echo` / `ls` /
  `mkdir` correctly while Compose drops or overwrites lines. Debug the
  transcript (`TerminalViewModel.processOutput`) before assuming the
  command failed. Confirm files with `adb` when unsure.
- **Input is a command box** (IME Send), not a full tty. `vi` / Ctrl+R
  will not feel like Termux until per-key PTY is wired.
- **Writable cwd is app-private** (`files/home` after a current rebuild).
  Folders created there do **not** show up in Downloads or the system
  Files app. `mkdir /sdcard/...` is expected to fail until SAF is
  implemented. `pwd` without `-P` used to show a fake `PWD` env while
  the process was still in `/` — use `pwd -P`.
- **Foreground service is not started.** Home/resume may keep the
  session for a while; process death will not restore it.

## Storage Limitations

### External Storage (SD Cards, Downloads, etc.)

Android's modern storage system (Scoped Storage) **does not support all Unix filesystem operations**. When working with files outside the app's private storage, the following limitations apply:

#### What Doesn't Work on External Storage

| Operation | Status | What Happens |
|-----------|--------|--------------|
| `chmod` | ❌ **Silently Ignored** | Command appears to succeed but permissions don't change |
| `chown` | ❌ **Not Supported** | Operation will fail |
| Symbolic links | ❌ **Not Supported** | `ln -s` will fail |
| Hard links | ❌ **Not Supported** | `ln` will fail |
| File watching | ❌ **Not Supported** | `inotify` doesn't work |
| Atomic renames | ⚠️ **Not Guaranteed** | Files may be corrupted if interrupted |
| File locking | ❌ **Not Supported** | `flock` won't work reliably |

#### What Works on External Storage

| Operation | Status | Notes |
|-----------|--------|-------|
| Read files | ✅ Works | First access may be slow (~200ms), then cached |
| Write files | ✅ Works | |
| Create files | ✅ Works | |
| Delete files | ✅ Works | |
| Rename files | ⚠️ Partially | Not atomic - avoid concurrent modifications |
| List directories | ✅ Works | First listing may be slow |
| Create directories | ✅ Works | |

### Tool Compatibility on External Storage

Some development tools **will not work correctly** on external storage:

#### ❌ npm / yarn / pnpm - NOT COMPATIBLE
- These tools rely heavily on symlinks for `node_modules`
- **Use internal storage for Node.js projects**

#### ⚠️ git - WORKS WITH LIMITATIONS
- Clone and basic operations work
- Rename detection may be unreliable
- Performance is slower than internal storage
- File permissions are not preserved
- **For best results, clone to internal storage**

#### ⚠️ Python / pip - PARTIALLY COMPATIBLE
- Virtual environments (`venv`) may fail due to symlink requirements
- Some packages expect `chmod` to work
- **Create virtual environments in internal storage**

#### ⚠️ make / cmake - PARTIALLY COMPATIBLE
- Timestamp-based rebuilds may be unreliable
- Build artifacts may have incorrect permissions
- **Build in internal storage, copy results to external**

#### ⚠️ tar / zip - PARTIALLY COMPATIBLE
- Cannot preserve file permissions
- Cannot create symbolic links
- Archives will extract but lose permission metadata

### Our Recommendations

1. **Development work** (git repos, npm projects, Python venvs): Use **internal storage**
2. **Data files** (documents, media, backups): Use **external storage**
3. **Large files**: External storage is fine if you don't need Unix semantics

---

## Background Execution Limitations

### Android Will Kill Your Process

This is not a bug - it's how Android works. Even with a foreground service running:

| Device | Behavior |
|--------|----------|
| Stock Android | May kill after ~10 minutes of heavy background work |
| Samsung (One UI) | Aggressive battery optimization kills processes |
| Xiaomi (MIUI) | **Very aggressive** - kills even foreground services |
| OPPO (ColorOS) | Kills processes with minimal warning |
| OnePlus (OxygenOS) | Similar to OPPO |

### What We Do About It

1. **Automatic Checkpointing**: We save your terminal state every 30 seconds
2. **Foreground Service**: We run as a visible service to reduce kill likelihood
3. **Session Restoration**: When restarted, we restore your terminal output and working directory

### What You Can Expect

- ✅ Short commands will complete normally
- ✅ Terminal output is preserved across restarts
- ⚠️ Long-running processes (npm install, compilation) may be interrupted
- ⚠️ If killed, you'll see "Session restored from checkpoint" when reopening
- ❌ We **cannot guarantee** uninterrupted background execution

### Tips for Long-Running Tasks

1. **Keep the app in foreground** during critical operations
2. **Disable battery optimization** for this app in Android settings
3. **Use `nohup` or `screen`** for very long tasks (if available)
4. **Split large operations** into smaller chunks

---

## Performance Notes

### First Access is Slow

When you first access external storage after mounting:
- Directory listings: ~50-200ms
- File reads: ~50-200ms

After first access, metadata is cached and subsequent operations are fast (<1ms).

### Batch Operations

Operations that touch many files (like `npm install` or `git status` on large repos) will be slower on external storage because:
1. Each file access requires a system call through Android's Storage Access Framework
2. We cache aggressively, but cache misses are expensive

---

## Reporting Issues

Before reporting a bug:

1. **Check if it's a known limitation** (listed above)
2. **Try internal storage** - if it works there, the issue is SAF limitations
3. **Note your device model** - some manufacturers have more aggressive process killing
4. **Check Logcat** for errors (if you're technical)

When reporting:
- Device model and Android version
- Storage location (internal vs external)
- Exact command that failed
- Session state shown in the app (Active/Restored/Failed)

---

## Why These Limitations Exist

### Storage Access Framework (SAF)

Google introduced SAF in Android 10+ to improve privacy and security. Apps no longer have direct filesystem access to external storage - they must go through SAF, which:
- Doesn't support Unix permissions
- Doesn't support symlinks
- Has higher latency than direct access

**We can't change this** - it's an Android platform limitation that affects all apps.

### Background Process Limits

Android's Phantom Process Killer (Android 12+) actively terminates background processes to:
- Save battery
- Free memory
- Reduce CPU usage

OEMs often make this even more aggressive. **We can't prevent this** - the OS has full control.

### What We Can Do

We've implemented every mitigation possible:
- Foreground Service with notification
- START_STICKY for auto-restart
- Aggressive state checkpointing
- Metadata caching for SAF
- Batch operations where possible

But some limitations are fundamental to the Android platform.
