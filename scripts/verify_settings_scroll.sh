#!/usr/bin/env bash
set -euo pipefail

ROOT="/Users/acoliver/projects/personal-agent/gpuui"
OUT="$ROOT/.tmp_verify_settings_scroll.log"
: > "$OUT"

pass() { echo "PASS: $1" | tee -a "$OUT"; }
fail() { echo "FAIL: $1" | tee -a "$OUT"; }

cd "$ROOT"

if cargo check >> "$OUT" 2>&1; then
  pass "cargo check"
else
  fail "cargo check"
fi

# Source-level guard: profiles and MCP list containers must be scrollable.
if rg -n "\.h\(px\(100\.0\)\)[\s\S]{0,200}\.overflow_y_scroll\(\)" src/ui_gpui/views/settings_view.rs >> "$OUT" 2>&1; then
  pass "List boxes use overflow_y_scroll"
else
  fail "List boxes are not using overflow_y_scroll"
fi

if rg -n "render_profiles_section|render_mcp_section" src/ui_gpui/views/settings_view.rs >> "$OUT" 2>&1; then
  pass "Settings sections found"
else
  fail "Could not locate settings list sections"
fi

if rg -q '^FAIL:' "$OUT"; then
  echo "Settings scroll verification failed; see $OUT"
  exit 1
fi

echo "Settings scroll verification passed; see $OUT"
