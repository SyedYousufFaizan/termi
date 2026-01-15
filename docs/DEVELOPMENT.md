# Development Guide

This guide covers setting up your development environment and building the project.

## Prerequisites

### Required Tools

| Tool | Version | Purpose |
|------|---------|---------|
| **Rust** | 1.75+ | Core library development |
| **Android Studio** | Hedgehog+ | Android app development |
| **Android NDK** | r25+ | Cross-compilation |
| **cargo-ndk** | 3.0+ | Rust-Android toolchain |
| **Git** | 2.0+ | Version control |

### Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| Windows (WSL) | ✅ Supported | Primary dev environment |
| macOS | ✅ Supported | Full Android + iOS |
| Linux | ✅ Supported | Full Android support |

## Quick Start

### 1. Clone Repository

```bash
git clone https://github.com/yourusername/termi.git
cd termi
```

### 2. Run Setup Script

**Linux/macOS/WSL:**
```bash
chmod +x scripts/setup_dev.sh
./scripts/setup_dev.sh
```

**Windows (PowerShell):**
```powershell
# Install Rust targets manually
rustup target add aarch64-linux-android armv7-linux-androideabi

# Install cargo-ndk
cargo install cargo-ndk

# Create directories
New-Item -ItemType Directory -Force -Path android/app/src/main/jniLibs/arm64-v8a
```

### 3. Set Environment Variables

**Linux/macOS/WSL:**
```bash
export ANDROID_NDK_HOME=$HOME/Android/Sdk/ndk/25.2.9519653
export PATH=$PATH:$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin
```

**Windows:**
```powershell
$env:ANDROID_NDK_HOME = "C:\Users\YourName\AppData\Local\Android\Sdk\ndk\25.2.9519653"
```

### 4. Build Rust Library

```bash
cd rust
cargo build  # Debug build (faster, for testing)

# Or for Android:
cargo ndk -t arm64-v8a build --release
```

### 5. Copy to Android Project

```bash
# Linux/macOS/WSL
cp target/aarch64-linux-android/release/libterminal_core.so \
   android/app/src/main/jniLibs/arm64-v8a/

# Windows PowerShell
Copy-Item target\aarch64-linux-android\release\libterminal_core.so `
   android\app\src\main\jniLibs\arm64-v8a\
```

### 6. Open in Android Studio

1. Open Android Studio
2. File → Open → Select `android/` directory
3. Wait for Gradle sync
4. Run on device/emulator

## Project Structure

```
termi/
├── rust/                   # Rust core library
│   ├── Cargo.toml         # Dependencies
│   ├── src/               # Source code
│   └── tests/             # Integration tests
├── android/               # Android app
│   ├── app/
│   │   ├── src/main/
│   │   │   ├── java/     # Kotlin source
│   │   │   ├── jniLibs/  # Native libraries
│   │   │   └── res/      # Resources
│   │   └── build.gradle.kts
│   └── build.gradle.kts
├── docs/                  # Documentation
├── scripts/               # Build scripts
└── packages/              # Pre-built packages
```

## Development Workflow

### Rust Development

```bash
cd rust

# Check compilation
cargo check

# Run tests
cargo test

# Run specific test
cargo test test_name

# Format code
cargo fmt

# Check lints
cargo clippy
```

### Android Development

1. Make Rust changes
2. Build with `cargo ndk -t arm64-v8a build --release`
3. Copy `.so` file to `android/app/src/main/jniLibs/arm64-v8a/`
4. Rebuild Android app in Android Studio

### Hot Reload (UI Only)

Jetpack Compose supports hot reload for UI changes:
- Modify Kotlin/Compose code
- Click "Apply Changes" in Android Studio
- No rebuild needed for UI-only changes

Native code changes always require full rebuild.

## Build Configurations

### Rust Build Profiles

| Profile | Command | Binary Size | Speed | Use Case |
|---------|---------|-------------|-------|----------|
| Debug | `cargo build` | ~20MB | Slow | Development |
| Release | `cargo build --release` | ~2MB | Fast | Production |

### Release Optimizations

Configured in `rust/Cargo.toml`:

```toml
[profile.release]
opt-level = "z"      # Optimize for size
lto = true           # Link-time optimization
codegen-units = 1    # Better optimization
strip = true         # Remove debug symbols
panic = "abort"      # Smaller binary
```

## Testing

### Rust Unit Tests

```bash
cd rust
cargo test
```

Current test coverage:
- ✅ JNI safety wrappers
- ✅ VFS capabilities
- ✅ Session state management
- ✅ Terminal screen buffer
- ✅ Cell rendering
- ✅ Metadata cache
- ✅ Mount table

### Android Instrumented Tests

```bash
cd android
./gradlew connectedAndroidTest
```

### Manual Testing Checklist

Before release, test on:
- [ ] Android emulator (API 33+)
- [ ] Samsung device (One UI)
- [ ] Xiaomi device (MIUI)
- [ ] Pixel device (Stock Android)

Test scenarios:
- [ ] Basic shell commands (ls, cd, pwd)
- [ ] Long-running commands (sleep 60)
- [ ] Background app and restore
- [ ] Mount external storage
- [ ] File operations on external storage

## Debugging

### Rust Panics

If app crashes in native code:

1. Check Android Logcat for panic message
2. Filter by tag: `TerminalCore`
3. Look for "PANIC in native code" message

### JNI Errors

Common issues:
- **SIGABRT**: Unchecked Java exception
- **SIGSEGV**: Invalid handle or null pointer
- **Memory leak**: Missing `handle_drop()` call

Debug with:
```bash
adb logcat -s TerminalCore:V
```

### SAF Issues

If file operations fail:
1. Check if path is SAF-backed: `/mnt/*`
2. Verify URI permission is persisted
3. Check `VfsCapabilities` for operation support

## Code Style

### Rust

- Follow `rustfmt` defaults
- Use `cargo clippy` warnings
- Document all public APIs
- No `unwrap()` in JNI code

### Kotlin

- Follow `ktlint` defaults
- Use Kotlin idioms (scope functions, null safety)
- Compose best practices

## Common Issues

### "cargo-ndk not found"

```bash
cargo install cargo-ndk
```

### "NDK not found"

Set `ANDROID_NDK_HOME`:
```bash
export ANDROID_NDK_HOME=/path/to/ndk
```

### "Cannot find libterminal_core.so"

Build and copy the library:
```bash
cd rust
cargo ndk -t arm64-v8a build --release
cp target/aarch64-linux-android/release/libterminal_core.so \
   ../android/app/src/main/jniLibs/arm64-v8a/
```

### "UnsatisfiedLinkError"

- Check library is in correct jniLibs folder
- Verify ABI matches device (arm64-v8a for most modern devices)
- Check library was built for Android, not host

### Tests Failing on CI

- Ensure same Rust version as CI
- Run `cargo fmt --check`
- Run `cargo clippy -- -D warnings`

## Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md) for guidelines.

## Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [JNI Documentation](https://docs.rs/jni/latest/jni/)
- [Jetpack Compose](https://developer.android.com/jetpack/compose)
- [Android NDK Guide](https://developer.android.com/ndk/guides)
