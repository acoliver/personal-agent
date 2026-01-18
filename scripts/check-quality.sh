#!/bin/bash
set -e

WARN_EXIT=0
ERROR_EXIT=0

echo "=== Running quality checks ==="

# Format check
echo "Checking formatting..."
cargo fmt -- --check || { echo "ERROR: Format check failed"; exit 1; }

# Clippy
echo "Running clippy..."
cargo clippy -- -D warnings || { echo "ERROR: Clippy failed"; exit 1; }

# Complexity check (CCN 50, function length error at 100, warn at 80)
echo "Checking complexity..."
if ! command -v lizard &> /dev/null; then
    echo "WARNING: lizard not installed, skipping complexity check"
    echo "Install with: pip3 install lizard"
else
    lizard -C 50 -L 100 -w src/ || { echo "ERROR: Complexity/function length exceeded"; ERROR_EXIT=1; }

    # Function length warnings (80 lines)
    long_funcs=$(lizard -L 80 src/ 2>/dev/null | grep -c "warning" || true)
    if [ "$long_funcs" -gt 0 ]; then
        echo "WARNING: $long_funcs functions exceed 80 lines"
        WARN_EXIT=1
    fi
fi

# File length check
echo "Checking file lengths..."
for file in $(find src -name "*.rs"); do
    lines=$(wc -l < "$file")
    if [ "$lines" -gt 1000 ]; then
        echo "ERROR: $file has $lines lines (max 1000)"
        ERROR_EXIT=1
    elif [ "$lines" -gt 750 ]; then
        echo "WARNING: $file has $lines lines (recommended max 750)"
        WARN_EXIT=1
    fi
done

# Tests with coverage
echo "Running tests with coverage..."
if ! command -v cargo-llvm-cov &> /dev/null; then
    echo "WARNING: cargo-llvm-cov not installed, skipping coverage check"
    echo "Install with: cargo install cargo-llvm-cov"
else
    # Set LLVM_COV and LLVM_PROFDATA paths if not already set
    if [ -z "$LLVM_COV" ]; then
        LLVM_COV="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin/llvm-cov"
    fi
    if [ -z "$LLVM_PROFDATA" ]; then
        LLVM_PROFDATA="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin/llvm-profdata"
    fi
    
    export LLVM_COV
    export LLVM_PROFDATA
    
    IGNORE_REGEX="research/serdesAI/|src/ui/|src/main_menubar.rs|src/popover.rs|src/llm/client_agent.rs"
    cargo llvm-cov --summary-only --ignore-filename-regex "$IGNORE_REGEX" > /tmp/cov_summary.txt 2>&1 || true
    coverage=$(grep -oE '[0-9]+\.[0-9]+%' /tmp/cov_summary.txt | head -1 | grep -oE '[0-9.]+' || echo "0")

    if (( $(echo "$coverage < 80" | bc -l 2>/dev/null || echo "0") )); then
        echo "ERROR: Coverage ${coverage}% is below 80%"
        ERROR_EXIT=1
    elif (( $(echo "$coverage < 90" | bc -l 2>/dev/null || echo "0") )); then
        echo "WARNING: Coverage ${coverage}% is below 90%"
        WARN_EXIT=1
    else
        echo "Coverage: ${coverage}%"
    fi
fi

# Summary
if [ "$ERROR_EXIT" -eq 1 ]; then
    echo "=== FAILED: Quality errors found ==="
    exit 1
elif [ "$WARN_EXIT" -eq 1 ]; then
    echo "=== PASSED with warnings ==="
    exit 0
else
    echo "=== PASSED: All checks clean ==="
    exit 0
fi
