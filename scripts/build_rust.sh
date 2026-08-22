#!/bin/bash
set -e

echo "Building Rust library for Android..."

cd rust

# Build for Android ARM64 (modern devices)
cargo ndk -t arm64-v8a build --release --features android

# Optional: Build for ARM32 (older devices)
# cargo ndk -t armeabi-v7a build --release --features android

echo "Copying library to Android project..."
mkdir -p ../android/app/src/main/jniLibs/arm64-v8a
cp target/aarch64-linux-android/release/libterminal_core.so \
   ../android/app/src/main/jniLibs/arm64-v8a/

echo "✓ Rust library built successfully!"