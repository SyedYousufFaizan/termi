#!/bin/bash
set -e

echo "Setting up development environment..."

# Check prerequisites
command -v rustc >/dev/null 2>&1 || { echo "Rust not installed. Install from https://rustup.rs"; exit 1; }
command -v cargo >/dev/null 2>&1 || { echo "Cargo not found."; exit 1; }

# Install Android targets
echo "Installing Android targets..."
rustup target add aarch64-linux-android armv7-linux-androideabi

# Install cargo-ndk
echo "Installing cargo-ndk..."
cargo install cargo-ndk

# Create necessary directories
echo "Creating directory structure..."
mkdir -p android/app/src/main/jniLibs/arm64-v8a
mkdir -p docs
mkdir -p packages/arm64

# Initialize git if not already
if [ ! -d .git ]; then
    git init
    echo "Git repository initialized"
fi

echo "✓ Development environment setup complete!"
echo ""
echo "Next steps:"
echo "1. Set ANDROID_NDK_HOME environment variable"
echo "2. Run ./scripts/build_rust.sh to build native library"
echo "3. Open android/ in Android Studio"