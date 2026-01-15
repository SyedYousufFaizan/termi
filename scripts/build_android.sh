#!/bin/bash
# Build complete Android project (Rust + Kotlin)
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "🔨 Building Next-Gen Terminal for Android..."
echo ""

# Check prerequisites
if [ -z "$ANDROID_NDK_HOME" ]; then
    echo "❌ ANDROID_NDK_HOME not set"
    echo "   Set it to your NDK path, e.g.:"
    echo "   export ANDROID_NDK_HOME=\$HOME/Android/Sdk/ndk/25.2.9519653"
    exit 1
fi

# Build Rust library
echo "📦 Step 1/3: Building Rust library..."
cd "$PROJECT_ROOT/rust"

# ARM64 (modern devices)
echo "   Building for arm64-v8a..."
cargo ndk -t arm64-v8a build --release

# Optional: ARMv7 (older devices)
# echo "   Building for armeabi-v7a..."
# cargo ndk -t armeabi-v7a build --release

# Copy to Android project
echo ""
echo "📋 Step 2/3: Copying native libraries..."
mkdir -p "$PROJECT_ROOT/android/app/src/main/jniLibs/arm64-v8a"
cp "$PROJECT_ROOT/rust/target/aarch64-linux-android/release/libterminal_core.so" \
   "$PROJECT_ROOT/android/app/src/main/jniLibs/arm64-v8a/"

# Show library size
LIB_SIZE=$(du -h "$PROJECT_ROOT/android/app/src/main/jniLibs/arm64-v8a/libterminal_core.so" | cut -f1)
echo "   Library size: $LIB_SIZE"

# Build Android APK
echo ""
echo "🤖 Step 3/3: Building Android APK..."
cd "$PROJECT_ROOT/android"

if [ "$1" == "--release" ]; then
    ./gradlew assembleRelease
    APK_PATH="app/build/outputs/apk/release/app-release.apk"
else
    ./gradlew assembleDebug
    APK_PATH="app/build/outputs/apk/debug/app-debug.apk"
fi

if [ -f "$APK_PATH" ]; then
    APK_SIZE=$(du -h "$APK_PATH" | cut -f1)
    echo ""
    echo "✅ Build complete!"
    echo "   APK: $APK_PATH"
    echo "   Size: $APK_SIZE"
else
    echo ""
    echo "⚠️  APK not found. Check Gradle output for errors."
    exit 1
fi
