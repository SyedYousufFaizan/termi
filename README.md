<div align="center">
  <img src="https://capsule-render.vercel.app/api?type=rect&color=0D1117&height=200&section=header&text=Termi&fontSize=80&fontColor=3596F5&animation=fadeIn&fontAlignY=35&desc=The%20Next-Gen%20Terminal%20for%20Android&descAlignY=60&descAlign=50" alt="Termi Header" />
</div>

---

# Next-Gen Android Terminal

A modern terminal emulator for Android that solves real problems: external storage access, background execution, and poor UX.

![Status](https://img.shields.io/badge/status-alpha-red)
![Platform](https://img.shields.io/badge/platform-Android%2012%2B-blue)
![License](https://img.shields.io/badge/license-MIT-orange)

> **Status: Alpha.** A debug APK can spawn `/system/bin/sh` on a phone
> (session stays up; toolbar and IME work). The SAF-VFS bridge
> has a tested *policy* layer; real Android SAF I/O is still a stub. The
> package manager is unimplemented. "What This Solves" is the target
> design, not a completed feature list.

## What This Solves (target — see status doc for what's actually built)

| Problem | Existing Pain | Our Solution |
|---------|---------------|--------------|
| **Storage Access** | Can't access SD cards with traditional paths | SAF-VFS Bridge maps URIs to Unix paths |
| **Process Killing** | Long tasks (npm install) get terminated | Checkpoint/restore with foreground service |
| **Poor UX** | Text-only, no file browser | Hybrid UI with native sidebar explorer |
| **Package Failures** | Termux mirrors often down | GitHub-based package distribution |

##  Key Features

- **External Storage Access** - First terminal with proper SAF integration
- **Session Persistence** - Automatic checkpoint/restore on background
- **Hybrid UI** - Native file explorer alongside terminal
-  **Modern Stack** - Rust core + Jetpack Compose UI
- **Fast** - <16ms keystroke latency, 60 FPS rendering

## Architecture

```
┌─────────────────────────────────────────┐
│         Android App (Kotlin)            │
│    Jetpack Compose • Material 3         │
└───────────────┬─────────────────────────┘
                │ JNI (safe wrappers)
┌───────────────┴─────────────────────────┐
│         Rust Core Library               │
│  PTY • VFS • ANSI Parser • State Mgmt   │
└─────────────────────────────────────────┘
```

##  Requirements

- Android 12+ (API 31)
- ARM64 device (arm64-v8a)
- ~15MB storage for core app
- ~50MB with all packages

##  Quick Start

### For Users

1. Download APK from [Releases](https://github.com/MannanSaood/termi/releases)
2. Install and grant storage permissions
3. Mount external directories via Settings

### For Developers

```bash
# Clone
git clone https://github.com/MannanSaood/termi.git
cd termi

# Setup (Linux/macOS/WSL)
./scripts/setup_dev.sh

# Build Rust library
cd rust && cargo ndk -t arm64-v8a build --release

# Copy to Android project
cp target/aarch64-linux-android/release/libterminal_core.so \
   ../android/app/src/main/jniLibs/arm64-v8a/

# Open android/ in Android Studio and run
```

See [Development Guide](docs/DEVELOPMENT.md) for detailed setup.

##  Documentation

| Document | Description |
|----------|-------------|
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | Technical design and module structure |
| [DEVELOPMENT.md](docs/DEVELOPMENT.md) | Setup, building, and workflow |
| [API.md](docs/API.md) | Rust API reference |
| [LIMITATIONS.md](docs/LIMITATIONS.md) | Known constraints and workarounds |
| [ROADMAP.md](docs/ROADMAP.md) | Development phases and progress |
| [**PHASE1_STATUS.md**](docs/PHASE1_STATUS.md) | **Ground truth: what's actually implemented vs. scaffolded right now** |

## Developing with Cursor

This repo ships with `.cursor/rules/*.mdc` and `.cursor/skills/*.md` —
Cursor (or any agent reading those files) should pick up project
conventions, the safety/testing policy, and worked-example playbooks
(adding a VFS provider, debugging a panic, wiring the permission
health-check) automatically. Start here regardless of tooling:

```bash
cd rust
cargo test                                    # fast, no Android SDK/NDK needed
cargo check --features android --all-targets  # JNI boundary (host libc)
cargo check --target aarch64-linux-android --features android --lib  # bionic ioctl types
```

See `.cursor/rules/00-project-overview.mdc` for the full picture.

##  Project Status

| Phase | Status | Details |
|-------|--------|---------|
| Week 0: Safety Foundation | ✅ Complete | JNI safety, VFS capabilities, session state |
| Month 1: Core Terminal | ✅ Complete | Shell runs on-device; PTY streaming & prompt rendering verified — [PHASE1_STATUS.md](docs/PHASE1_STATUS.md) |
| Phase 1b: Safety cleanup | ✅ Complete | Unwrap audit, poison-safe locks, clippy enforcement — see [PHASE1_STATUS.md](docs/PHASE1_STATUS.md) |
| Phase 1d: VFS capability enforcement | 🔄 Policy layer done, SAF I/O still stubbed | `VfsService`, `HealthMonitor` — see [PHASE1_STATUS.md](docs/PHASE1_STATUS.md) |
| Month 2: SAF Integration | ⏳ Pending | Real Android-side SAF I/O |
| Month 3: Polish & Packages | ⏳ Pending | File explorer, packages |

##  Important Limitations

Read [LIMITATIONS.md](docs/LIMITATIONS.md) before reporting issues:

- **External storage**: `chmod`, symlinks don't work (Android SAF limitation). `mkdir` in the app files dir is not the same as creating a folder in Downloads.
- **Background execution**: OEMs may still kill the app (`TerminalService` is not started yet)
- **npm/yarn**: Won't work on external storage (symlink-dependent)

##  Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

**Quick rules:**
- All JNI code must use `jni_safe.rs` wrappers
- Check VFS capabilities before operations
- Test on Samsung AND Xiaomi devices

##  License

MIT License - see [LICENSE](LICENSE)

##  Acknowledgments

- [portable-pty](https://github.com/AcK77/portable-pty) - PTY implementation
- [vte](https://github.com/alacritty/vte) - ANSI parsing (from Alacritty)
- [Termux](https://termux.dev) - Inspiration and package ecosystem

---

<p align="center">
  Built with 🦀 Rust + 💜 Kotlin
</p>
