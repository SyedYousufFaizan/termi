#!/bin/bash
# Format all code in the project
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "🎨 Formatting code..."
echo ""

# Rust formatting
echo "→ Formatting Rust code..."
cd "$PROJECT_ROOT/rust"
cargo fmt
echo "  ✓ Rust formatted"

# Kotlin formatting (if ktlint available)
echo ""
echo "→ Checking for ktlint..."
cd "$PROJECT_ROOT/android"

if command -v ktlint &> /dev/null; then
    echo "  Running ktlint..."
    ktlint -F "**/*.kt" || true
    echo "  ✓ Kotlin formatted"
elif [ -f "gradlew" ]; then
    # Try gradle ktlint plugin if available
    ./gradlew ktlintFormat 2>/dev/null || {
        echo "  ⚠️  ktlint not configured in Gradle. Skipping Kotlin formatting."
        echo "     To add: https://github.com/JLLeitschuh/ktlint-gradle"
    }
else
    echo "  ⚠️  ktlint not found. Install with: brew install ktlint"
fi

echo ""
echo "✅ Formatting complete!"
