#!/usr/bin/env bash
set -euo pipefail
ROOT="/Users/acoliver/projects/personal-agent/gpuui"
LOG="/tmp/personal_agent_gpui.log"
OUT="$ROOT/.tmp_ui_automation_verify.log"
: > "$OUT"
cd "$ROOT"

pass() { echo "PASS: $1" | tee -a "$OUT"; }
fail() { echo "FAIL: $1" | tee -a "$OUT"; }

run_ui_test() {
  local test_name="$1"
  echo "== UI automation: ${test_name} ==" | tee -a "$OUT"
  if cargo test --test ui_automation_tests -- --ignored --test-threads=1 "$test_name" >> "$OUT" 2>&1; then
    pass "${test_name}"
  else
    fail "${test_name}"
    return 1
  fi
}

run_ui_test "scn_002_keyboard_profile_switch_from_chat_emits_event_and_routes_model"

# Capture deterministic startup/positioning evidence from latest runtime log.
if rg -q "PA_TEST_POPUP_ONSCREEN=1 active" "$LOG"; then
  pass "Automation startup uses explicit test popup override"
else
  fail "Missing PA_TEST_POPUP_ONSCREEN startup evidence in log"
fi

if rg -q "Popup opened" "$LOG"; then
  pass "Popup open event logged"
else
  fail "Missing popup open log line"
fi

if rg -q "SelectChatProfile" "$LOG"; then
  pass "Profile dropdown keyboard action emitted SelectChatProfile"
else
  fail "Missing SelectChatProfile emission"
fi

if rg -q 'SendMessage \{ text: ".*switch scenario test" \}' "$LOG"; then
  pass "Message send event emitted"
else
  fail "Missing SendMessage emission for SCN-002"
fi

echo "== UI automation verification complete ==" | tee -a "$OUT"
if rg -q "^FAIL:" "$OUT"; then
  echo "UI verification failed; see $OUT"
  exit 1
fi

echo "UI verification passed; see $OUT"
