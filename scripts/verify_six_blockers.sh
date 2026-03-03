#!/usr/bin/env bash
set -euo pipefail
ROOT="/Users/acoliver/projects/personal-agent/gpuui"
OUT="$ROOT/.tmp_verify_six_blockers.log"
: > "$OUT"

pass() { echo "PASS: $1" | tee -a "$OUT"; }
fail() { echo "FAIL: $1" | tee -a "$OUT"; }
run_step() {
  local name="$1"; shift
  echo "\n=== $name ===" | tee -a "$OUT"
  if "$@" >> "$OUT" 2>&1; then
    pass "$name"
  else
    fail "$name"
  fi
}

cd "$ROOT"

run_step "Startup contract (no auto-open outside test mode)" ./scripts/prove_broken_now.sh
run_step "Regression bug suite" ./scripts/verify_bug_suite.sh
run_step "UI automation and tray startup evidence" ./scripts/verify_ui_current.sh
run_step "Rename visual proof (Cmd+R does rename, not new conversation)" ./scripts/verify_rename_visual.sh
run_step "Legacy profile compatibility loading" ./scripts/verify_profile_load.sh
run_step "Simple memory validation" ./scripts/verify_memory_simple.sh

# Additional direct check: key forwarding for ProfileEditor in MainPanel source
if rg -q "current == ViewId::ProfileEditor" src/ui_gpui/views/main_panel.rs \
  && rg -q "view.handle_key_input\(key, modifiers, cx\)" src/ui_gpui/views/main_panel.rs; then
  pass "ProfileEditor key forwarding is wired in MainPanel"
else
  fail "ProfileEditor key forwarding missing in MainPanel"
fi

if rg -q "^FAIL:" "$OUT"; then
  echo "\nOne or more blocker verifications failed. See $OUT"
  exit 1
fi

echo "\nAll six blocker verifications passed. See $OUT"
