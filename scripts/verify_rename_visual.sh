#!/usr/bin/env bash
set -euo pipefail
ROOT="/Users/acoliver/projects/personal-agent/gpuui"
LOG="/tmp/personal_agent_gpui.log"
OUT="$ROOT/.tmp_verify_rename_visual.log"
: > "$OUT"

pass() { echo "PASS: $1" | tee -a "$OUT"; }
fail() { echo "FAIL: $1" | tee -a "$OUT"; }

cd "$ROOT"

pkill -f "target/debug/personal_agent_gpui" 2>/dev/null || true

# Launch deterministic popup for automation
nohup env RUST_LOG=info PA_AUTO_OPEN_POPUP=1 PA_TEST_POPUP_ONSCREEN=1 ./target/debug/personal_agent_gpui > "$LOG" 2>&1 &
APP_PID=$!
sleep 3

echo "PID=$APP_PID" >> "$OUT"

# Send initial message so we have a conversation
osascript -e 'tell application "System Events"' \
  -e 'tell process "personal_agent_gpui"' \
  -e 'set frontmost to true' \
  -e 'delay 0.1' \
  -e 'keystroke "rename proof seed"' \
  -e 'key code 36' \
  -e 'end tell' \
  -e 'end tell' >/dev/null 2>&1 || true
sleep 1

# Trigger rename via Cmd+R, type title, press Enter
osascript -e 'tell application "System Events"' \
  -e 'tell process "personal_agent_gpui"' \
  -e 'set frontmost to true' \
  -e 'delay 0.1' \
  -e 'key down command' \
  -e 'keystroke "r"' \
  -e 'key up command' \
  -e 'delay 0.2' \
  -e 'keystroke "renamed via visual test"' \
  -e 'key code 36' \
  -e 'end tell' \
  -e 'end tell' >/dev/null 2>&1 || true
sleep 1

# Verify rename event emitted, and no NewConversation emitted during rename path.
if rg -q 'ConfirmRenameConversation \{.*title: ".*renamed via visual test"' "$LOG"; then
  pass "Rename confirm event emitted with expected title suffix"
else
  fail "Missing ConfirmRenameConversation event for visual rename"
fi

if rg -q 'ChatView::emit called with event: NewConversation' "$LOG"; then
  # Allow a NewConversation only if it happened before rename seed/send in this run.
  # We require no NewConversation after the first ConfirmRenameConversation occurrence.
  RENAME_LINE=$(rg -n 'ConfirmRenameConversation \{.*title: ".*renamed via visual test"' "$LOG" | head -1 | cut -d: -f1)
  NEW_AFTER=$(awk -v ln="$RENAME_LINE" 'NR > ln && /ChatView::emit called with event: NewConversation/' "$LOG" | wc -l | tr -d ' ')
  if [ "${NEW_AFTER}" = "0" ]; then
    pass "No NewConversation emitted after rename confirm"
  else
    fail "NewConversation emitted after rename confirm"
  fi
else
  pass "No NewConversation emission observed in rename scenario"
fi

# Verify persisted latest conversation title changed.
LATEST_CONV=$(ls -1t ~/.llxprt/conversations/*.json 2>/dev/null | head -1 || true)
if [ -n "$LATEST_CONV" ]; then
  if python3 - "$LATEST_CONV" <<'PY'
import json, sys
p = sys.argv[1]
obj = json.load(open(p))
title = obj.get('title','')
print(title)
if title.endswith('renamed via visual test'):
    raise SystemExit(0)
raise SystemExit(1)
PY
  then
    pass "Latest conversation title persisted with renamed suffix"
  else
    fail "Latest conversation title did not persist expected rename suffix"
  fi
else
  fail "No conversation artifact found to validate renamed title"
fi

pkill -f "target/debug/personal_agent_gpui" 2>/dev/null || true

if rg -q "^FAIL:" "$OUT"; then
  echo "Rename visual verification failed; see $OUT"
  exit 1
fi

echo "Rename visual verification passed; see $OUT"
