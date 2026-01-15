# Packages Directory

This directory contains pre-built packages for the terminal.

## Structure

```
packages/
├── arm64/           # ARM64 (aarch64) packages
│   ├── bash/
│   ├── coreutils/
│   ├── git/
│   ├── python/
│   └── nano/
└── README.md        # This file
```

## Package Format

Each package is a compressed archive containing:
- `bin/` - Executable binaries
- `lib/` - Shared libraries
- `share/` - Data files (man pages, etc.)
- `MANIFEST.json` - Package metadata

### MANIFEST.json Format

```json
{
  "name": "package-name",
  "version": "1.0.0",
  "description": "Package description",
  "arch": "arm64",
  "dependencies": ["dep1", "dep2"],
  "size": 1234567,
  "sha256": "checksum..."
}
```

## Core Packages (MVP)

| Package | Version | Size | Description |
|---------|---------|------|-------------|
| bash | 5.2 | ~1MB | GNU Bash shell |
| coreutils | 9.4 | ~2MB | Core Unix utilities |
| git | 2.43 | ~10MB | Version control |
| python | 3.12 | ~20MB | Python interpreter |
| nano | 7.2 | ~500KB | Text editor |

## Building Packages

Packages are built from source using cross-compilation.

### Prerequisites

```bash
# Install cross-compiler
sudo apt install gcc-aarch64-linux-android

# Or use Android NDK
export CC=$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android33-clang
```

### Build Process

1. Download source
2. Configure with `--host=aarch64-linux-android`
3. Build with cross-compiler
4. Strip debug symbols
5. Create archive
6. Generate MANIFEST.json

## Distribution

Packages are hosted on GitHub Releases:
- Primary: `https://github.com/yourusername/termi-packages/releases`
- Fallback: Mirror URLs (TBD)

## Adding New Packages

1. Create build script in `scripts/build-package-NAME.sh`
2. Test on device
3. Generate MANIFEST.json
4. Upload to releases
5. Update package index

## Security

- All packages are built from source
- SHA256 checksums verified on download
- No dynamic code loading (Play Store compliance)
- Packages run in app sandbox

## Status

| Package | Status |
|---------|--------|
| bash | ⏳ Pending |
| coreutils | ⏳ Pending |
| git | ⏳ Pending |
| python | ⏳ Pending |
| nano | ⏳ Pending |

Packages will be built during Month 3 of development.
