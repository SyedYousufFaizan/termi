# Development Roadmap

This document tracks development progress and upcoming milestones.

> **Note:** For what is actually implemented vs scaffolded, including the
> on-device **output-streaming handoff**, see
> [PHASE1_STATUS.md](PHASE1_STATUS.md). This file is the phased plan;
> PHASE1_STATUS.md is ground truth.

## Project Status

| Phase | Status | Progress |
|-------|--------|----------|
| Week 0: Safety Foundation | ✅ **COMPLETE** | 100% |
| Month 1: Core Terminal | 🔄 In Progress | 75% |
| Month 2: SAF Integration | ⏳ Pending | 10% |
| Month 3: Polish & Packages | ⏳ Pending | 0% |

---

## ✅ Week 0: Safety Foundation (COMPLETE)

**Duration:** 1 weekend (8-12 hours)  
**Status:** ✅ Complete  
**Completed:** January 2026

### Deliverables

| Task | Status | File |
|------|--------|------|
| Safe JNI Wrapper Module | ✅ | `rust/src/jni_safe.rs` |
| VFS Capability System | ✅ | `rust/src/vfs/capabilities.rs` (moved from crate root in Phase 1 cleanup) |
| Session State Management | ✅ | `rust/src/session_state.rs` |
| Error Types | ✅ | `rust/src/utils/error.rs` |
| User-Facing Limitations Doc | ✅ | `docs/LIMITATIONS.md` |
| Module Structure | ✅ | All modules compile |

### Validation Results

- ✅ 46 unit tests passing
- ✅ Zero `unwrap()` in JNI boundary code
- ✅ All modules compile cleanly
- ✅ Documentation complete

---

## 🔄 Month 1: Core Terminal (IN PROGRESS)

**Duration:** Weeks 1-4  
**Goal:** Terminal that can run basic shell commands with safety guarantees

### Week 1-2: Rust PTY Bridge ✅ COMPLETE

| Task | Status | Notes |
|------|--------|-------|
| Integrate `portable-pty` crate | ✅ | Full integration in PtySession |
| PTY spawning for Android | ✅ | Uses configurable shell path |
| Read/write to PTY | ✅ | Non-blocking with timeout |
| Session state integration | ✅ | Tracks lifecycle via SessionState |
| Checkpoint infrastructure | ✅ | CheckpointManager with atomic writes |
| ANSI parser (vte crate) | ✅ | Full parser with Screen integration |
| Terminal screen buffer | ✅ | 80x24 default, scrollback support |

**Validation:**
- ✅ 46 Rust tests passing
- ✅ Parser handles colors, cursor movement, clear
- ✅ Screen buffer with scrollback

### Week 3-4: Kotlin UI + JNI Bindings 🔄 IN PROGRESS (on-device)

Scaffolding is in place and a debug APK **does** open a live `/system/bin/sh`.
That is **not** the same as "output display is done." Ground truth:
[PHASE1_STATUS.md](PHASE1_STATUS.md) (handoff section).

| Task | Status | Notes |
|------|--------|-------|
| JNI exports in Rust | ✅ | `android_jni.rs` |
| TerminalEngine.kt JNI bindings | ✅ | Error codes + `nativeLastError` |
| TerminalApplication.kt | ✅ | Library loading |
| MainActivity.kt | ✅ | Compose entry point |
| SessionManager.kt | 🔄 | Lifecycle + I/O loop; read loop must stay on `Dispatchers.IO` |
| TerminalService.kt | ⚠️ | Declared in the manifest, **never started** |
| TerminalScreen.kt | 🔄 | Toolbar + command box; not a full tty |
| TerminalViewModel.kt | 🔄 | **Transcript/streaming bug lives here** |
| Theme & Colors | ✅ | Dracula-inspired dark theme |
| SafBridge provider | ⚠️ | Scaffold only — no real SAF I/O |
| AndroidManifest.xml | ✅ | Permissions, service declaration |

**Success Criteria:**
- [x] App opens and shows terminal (on-device)
- [x] Session can stay Active past ~20s (ANR from blocking `nativeRead` on Main is fixed)
- [x] ✕ / New, IME, rotate, Home/resume (spot-checked)
- [x] Toolbar Ctrl+C / Ctrl+D (spot-checked)
- [ ] Transcript shows one-line commands (`echo`, `ls`, `mkdir` errors) reliably
- [ ] `pwd -P` matches a writable app dir and `mkdir` is visible via `ls` *and* `adb`
- [ ] Output displays with colors (native `Screen`, not Kotlin strip)
- [ ] SessionStateBanner / restore after process death
- [ ] APK size <10MB

### Remaining Tasks (Month 1)

| Task | Status | Notes |
|------|--------|-------|
| Compose PTY transcript | 🔄 | **Current handoff** — see PHASE1_STATUS |
| `cargo check` Android target | ✅ | CI `android-feature-check` includes `aarch64-linux-android` |
| Device testing | 🔄 | Shell works; streaming still wrong |
| ANSI color rendering in UI | ⏳ | Use native `Screen` / renderer, not strip-in-ViewModel |

### Testing Checklist (Month 1)

- [x] App does not die at ~20s idle (was ANR)
- [ ] Run app for 1 hour without crashes
- [ ] Send 1000 rapid commands
- [ ] Try invalid UTF-8 input
- [ ] Background app → verify checkpoint
- [ ] Restore app → verify restoration
- [ ] Check Logcat for JNI errors vs UI lines on `echo hello`

---

## ⏳ Month 2: SAF Integration

**Duration:** Weeks 5-8  
**Goal:** Access external storage properly

### Week 5-6: SAF Document Picker

| Task | Status | Notes |
|------|--------|-------|
| StorageManager.kt | ⏳ | Manage permissions, tree URIs |
| SAF picker integration | ⏳ | Document tree selection |
| Persist URI permissions | ⏳ | `takePersistableUriPermission()` |
| VFS mount integration | ✅ | Mount points ready in Rust |

### Week 7-8: VFS Operations via JNI

| Task | Status | Notes |
|------|--------|-------|
| SafHelper.kt | ⏳ | JNI callbacks for SAF |
| Implement SAF operations | ⏳ | Read, write, list, create, delete |
| Metadata caching | ✅ | VfsCache in Rust ready |
| Capability warnings in UI | ⏳ | Show SAF limitations |

**Success Criteria:**
- [ ] Can navigate to SD card via picker
- [ ] `ls /sdcard` shows external files
- [ ] Limitation warnings show for chmod/symlink
- [ ] Tool compatibility hints work

---

## ⏳ Month 3: Polish & Packages

**Duration:** Weeks 9-12  
**Goal:** Usable terminal with common tools

### Week 9-10: Package System

| Task | Status | Notes |
|------|--------|-------|
| Package format design | ✅ | Documented in packages/README.md |
| PackageManager.rs | ⏳ | Module structure ready |
| PackageRepository.rs | ⏳ | Module structure ready |
| Initial packages | ⏳ | vim-static, busybox |

### Week 11-12: Final Polish

| Task | Status | Notes |
|------|--------|-------|
| Settings screen | ⏳ | Font size, theme, shell path |
| Keyboard shortcuts | ⏳ | Ctrl+C, Ctrl+D, etc. |
| Copy/paste | ⏳ | Selection support |
| Session tabs | ⏳ | Multiple sessions |
| Performance optimization | ⏳ | Profile and optimize |

---

## Success Criteria (Overall)

### Performance Targets
- [ ] App startup: <500ms
- [ ] PTY latency: <10ms
- [ ] APK size: <15MB
- [ ] Memory usage: <100MB idle

### Stability Targets
- [ ] 8 hours continuous use without crash
- [ ] Survive 1000 rapid command submissions
- [ ] Proper checkpoint/restore after process death
- [ ] No JNI-related crashes in crashlytics

### Feature Targets
- [ ] Execute common shell commands
- [ ] Navigate filesystem (internal + SAF)
- [ ] Run vim/nano from packages
- [ ] Foreground service keeps sessions alive

---

## Risk Mitigation

| Risk | Mitigation | Status |
|------|------------|--------|
| JNI crashes | Safe wrappers in jni_safe.rs | ✅ Implemented |
| SAF limitations | VFS capability system | ✅ Implemented |
| Process death | Checkpoint system | ✅ Implemented |
| Memory leaks | Handle tracking, explicit drop | ✅ Implemented |
| Build complexity | cargo-ndk, CI scripts | ✅ Scripts ready |

---

## Changelog

### January 16, 2026
- Completed all Kotlin layer implementation
- TerminalEngine.kt with 15+ JNI bindings
- Full UI with TerminalScreen, ViewModel
- Foreground service for background operation
- SAF bridge provider
- Updated progress tracking

### January 15, 2026
- Completed Week 0 safety foundation
- All PTY integration done
- ANSI parser integration complete
- 46 tests passing

---

## Next Steps

1. **Install Android targets**: `rustup target add aarch64-linux-android`
2. **Build native library**: `cargo ndk -t arm64-v8a build --release`
3. **Test on device**: Run app on Android emulator/device
4. **Verify JNI bindings**: Check all native methods work correctly
5. **Add ANSI color rendering**: Use ColoredTerminalLine in UI
