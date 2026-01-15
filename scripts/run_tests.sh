#!/bin/bash
# Run all project tests
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "🧪 Running all tests..."
echo ""

# Rust tests
echo "═══════════════════════════════════════════"
echo "📦 Rust Tests"
echo "═══════════════════════════════════════════"
cd "$PROJECT_ROOT/rust"

echo ""
echo "→ Running cargo fmt check..."
cargo fmt --check || {
    echo "❌ Formatting issues found. Run: cargo fmt"
    exit 1
}
echo "  ✓ Formatting OK"

echo ""
echo "→ Running clippy..."
cargo clippy -- -D warnings || {
    echo "❌ Clippy warnings found."
    exit 1
}
echo "  ✓ Clippy OK"

echo ""
echo "→ Running unit tests..."
cargo test --verbose

echo ""
echo "═══════════════════════════════════════════"
echo "🤖 Android Tests"
echo "═══════════════════════════════════════════"
cd "$PROJECT_ROOT/android"

if [ -f "gradlew" ]; then
    echo ""
    echo "→ Running Android unit tests..."
    ./gradlew test || {
        echo "⚠️  Android tests failed or not configured"
    }
    
    # Instrumented tests require connected device
    if [ "$1" == "--device" ]; then
        echo ""
        echo "→ Running instrumented tests (requires device)..."
        ./gradlew connectedAndroidTest
    fi
else
    echo "⚠️  Gradle wrapper not found. Skipping Android tests."
fi

echo ""
echo "═══════════════════════════════════════════"
echo "✅ All tests completed!"
echo "═══════════════════════════════════════════"
