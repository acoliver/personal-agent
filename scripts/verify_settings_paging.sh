#!/usr/bin/env bash
set -euo pipefail

ROOT="/Users/acoliver/projects/personal-agent/gpuui"
OUT="$ROOT/.tmp_verify_settings_paging.log"
: > "$OUT"

pass() { echo "PASS: $1" | tee -a "$OUT"; }
fail() { echo "FAIL: $1" | tee -a "$OUT"; }

cd "$ROOT"

if cargo check >> "$OUT" 2>&1; then
  pass "cargo check"
else
  fail "cargo check"
fi

if rg -n "profiles_page_start|mcps_page_start|btn-profiles-page-up|btn-profiles-page-down|btn-mcps-page-up|btn-mcps-page-down" src/ui_gpui/views/settings_view.rs >> "$OUT" 2>&1; then
  pass "settings paging state and controls present"
else
  fail "settings paging controls/state missing"
fi

if rg -n "visible_profiles|visible_mcps|Rows \{\}-\{\} of \{\}" src/ui_gpui/views/settings_view.rs >> "$OUT" 2>&1; then
  pass "settings sections render paged subsets with row indicator"
else
  fail "settings paged subset rendering missing"
fi

if rg -n "\.ml\(px\(2\.0\)\).*child\(\"\|\"\)" src/ui_gpui/views/profile_editor_view.rs >> "$OUT" 2>&1; then
  pass "profile editor active-field caret glyph present"
else
  fail "profile editor caret glyph missing"
fi

if rg -n "emit\(UserEvent::EditProfile \{ id \}\)" src/ui_gpui/views/settings_view.rs >> "$OUT" 2>&1; then
  pass "settings edit action emits EditProfile event"
else
  fail "settings edit action event emission missing"
fi

if rg -n "request_navigate\([[:space:]]*crate::presentation::view_command::ViewId::ProfileEditor" src/ui_gpui/views/settings_view.rs >> "$OUT" 2>&1; then
  fail "settings view still directly navigates to ProfileEditor (can race prefill)"
else
  pass "settings view no longer directly navigates to ProfileEditor"
fi

if rg -q '^FAIL:' "$OUT"; then
  echo "Settings paging verification failed; see $OUT"
  exit 1
fi

echo "Settings paging verification passed; see $OUT"
