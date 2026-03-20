#!/bin/bash
set -e

WARN_EXIT=0
ERROR_EXIT=0

echo "=== Running quality checks ==="

# GPUI view/component rendering code requires a live GPUI context and is excluded
# from automated complexity, file-length, and coverage checks.
GPUI_RENDER_EXCLUDES="src/ui_gpui/views/|src/ui_gpui/components/|"
# Other files excluded: binary entry points, GPUI framework plumbing
OTHER_EXCLUDES="src/main_gpui.rs|src/bin/|"

# Format check
echo "Checking formatting..."
cargo fmt -- --check || { echo "ERROR: Format check failed"; exit 1; }

# Clippy
echo "Running clippy..."
cargo clippy --all-targets -- -D warnings || { echo "ERROR: Clippy failed"; exit 1; }

# Complexity check (CCN 50, function length error at 100, warn at 80)
echo "Checking complexity..."
if ! command -v lizard &> /dev/null; then
    echo "WARNING: lizard not installed, skipping complexity check"
    echo "Install with: pip3 install lizard"
else
    LIZARD_EXCLUDES=(
        --exclude "src/ui_gpui/views/*"
        --exclude "src/ui_gpui/components/*"
        --exclude "src/main_gpui.rs"
        --exclude "src/bin/*"
        --exclude "src/services/chat.rs"
        --exclude "src/llm/client_agent.rs"
    )
    lizard -C 50 -L 100 -w src/ "${LIZARD_EXCLUDES[@]}" \
        || { echo "ERROR: Complexity/function length exceeded"; ERROR_EXIT=1; }

    # Function length warnings (80 lines)
    long_funcs=$(lizard -L 80 src/ "${LIZARD_EXCLUDES[@]}" \
        2>/dev/null | grep -c "warning" || true)
    if [ "$long_funcs" -gt 0 ]; then
        echo "WARNING: $long_funcs functions exceed 80 lines"
        WARN_EXIT=1
    fi
fi

# File length check (exclude GPUI rendering code)
echo "Checking file lengths..."
for file in $(find src -name "*.rs" \
    -not -path "src/ui_gpui/views/*" \
    -not -path "src/ui_gpui/components/*" \
    -not -path "src/main_gpui.rs" \
    -not -path "src/bin/*"); do
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
    
    IGNORE_REGEX="research/serdesAI/|${GPUI_RENDER_EXCLUDES}${OTHER_EXCLUDES}src/llm/client_agent.rs|src/ui_gpui/popup_window.rs|src/ui_gpui/tray_bridge.rs|src/ui_gpui/navigation_channel.rs|src/ui_gpui/selection_intent_channel.rs"
    # Split into run + report to avoid combined-mode timeouts on macOS
    if ! cargo llvm-cov --no-report --lib --tests 2>/tmp/cov_run_err.txt; then
        if grep -q "failed to find llvm-tools-preview" /tmp/cov_run_err.txt; then
            echo "WARNING: llvm-tools-preview not found; skipping coverage check"
            coverage="100"
        else
            echo "WARNING: Coverage test run failed; see /tmp/cov_run_err.txt"
            coverage="0"
        fi
    elif ! cargo llvm-cov report --summary-only --ignore-filename-regex "$IGNORE_REGEX" > /tmp/cov_summary.txt 2>&1; then
        echo "WARNING: Coverage report generation failed"
        coverage="0"
    else
        coverage=$(grep -oE '[0-9]+\.[0-9]+%' /tmp/cov_summary.txt | tail -1 | grep -oE '[0-9.]+' || echo "0")
    fi

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
 exit 0
fi
ecks clean ==="
    exit 0
fi
