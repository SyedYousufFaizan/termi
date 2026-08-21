<div align="center">
  <img src="https://capsule-render.vercel.app/api?type=rect&color=0D1117&height=200&section=header&text=Termi&fontSize=80&fontColor=3596F5&animation=fadeIn&fontAlignY=35&desc=The%20Next-Gen%20Terminal%20for%20Android&descAlignY=60&descAlign=50" alt="Termi Header" />
</div>

---

# Next-Gen Android Terminal

A modern terminal emulator for Android that solves real problems: external storage access, background execution, and poor UX.

![Status](https://img.shields.io/badge/status-Week%200%20Complete-green)
![Platform](https://img.shields.io/badge/platform-Android%2012%2B-blue)
![License](https://img.shields.io/badge/license-MIT-orange)

## What This Solves

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

##  Project Status

| Phase | Status | Details |
|-------|--------|---------|
| Week 0: Safety Foundation | ✅ Complete | JNI safety, VFS capabilities, session state |
| Month 1: Core Terminal | 🔄 In Progress | PTY, UI, basic commands |
| Month 2: SAF Integration | ⏳ Pending | External storage access |
| Month 3: Polish & Packages | ⏳ Pending | File explorer, packages |

##  Important Limitations

Read [LIMITATIONS.md](docs/LIMITATIONS.md) before reporting issues:

- **External storage**: `chmod`, symlinks don't work (Android SAF limitation)
- **Background execution**: OEMs may still kill the app (use internal storage for critical work)
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
