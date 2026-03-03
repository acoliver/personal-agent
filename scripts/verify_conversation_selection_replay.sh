#!/usr/bin/env bash
set -euo pipefail

ROOT="/Users/acoliver/projects/personal-agent/gpuui"
LOG="/tmp/personal_agent_gpui.log"
OUT="$ROOT/.tmp_verify_conversation_selection_replay.log"
CONV_DIR="$HOME/Library/Application Support/PersonalAgent/conversations"
: > "$OUT"

APP_PID=""

pass() { echo "PASS: $1" | tee -a "$OUT"; }
fail() { echo "FAIL: $1" | tee -a "$OUT"; }

cleanup() {
  if [ -n "$APP_PID" ] && kill -0 "$APP_PID" 2>/dev/null; then
    kill "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
  fi
  pkill -f "target/debug/personal_agent_gpui" 2>/dev/null || true
}
trap cleanup EXIT

log_has_regex() {
  local pattern="$1"
  python3 - "$LOG" "$pattern" <<'PY'
import re, sys
from pathlib import Path
log_path = Path(sys.argv[1])
pattern = sys.argv[2]
if not log_path.exists():
    raise SystemExit(1)
text = log_path.read_text(errors='ignore')
text = re.sub(r'\x1b\[[0-9;]*[A-Za-z]', '', text)
raise SystemExit(0 if re.search(pattern, text, flags=re.MULTILINE) else 1)
PY
}

wait_for_regex() {
  local pattern="$1"
  local timeout_secs="${2:-20}"
  local i=0
  while [ "$i" -lt "$timeout_secs" ]; do
    if log_has_regex "$pattern"; then
      return 0
    fi
    sleep 1
    i=$((i + 1))
  done
  return 1
}

cd "$ROOT"

if [ ! -x "./target/debug/personal_agent_gpui" ]; then
  fail "Missing binary: ./target/debug/personal_agent_gpui"
  echo "Build first with cargo build" | tee -a "$OUT"
  exit 1
fi

if [ ! -d "$CONV_DIR" ]; then
  fail "Missing conversations dir: $CONV_DIR"
  exit 1
fi

TARGET_CONV=""

pkill -f "target/debug/personal_agent_gpui" 2>/dev/null || true
: > "$LOG"
nohup env RUST_LOG=info PA_AUTO_OPEN_POPUP=1 PA_TEST_POPUP_ONSCREEN=1 ./target/debug/personal_agent_gpui > "$LOG" 2>&1 &
APP_PID=$!
echo "PID=$APP_PID" >> "$OUT"

if wait_for_regex 'ChatView: received ConversationListRefreshed count=[0-9]+' 30; then
  pass "Conversation list loaded into chat view before selection"
else
  fail "Conversation list did not load before selection"
fi

# Open conversation dropdown via keyboard shortcut (Cmd+K).
osascript \
  -e 'tell application "System Events"' \
  -e 'tell process "personal_agent_gpui"' \
  -e 'set frontmost to true' \
  -e 'delay 0.2' \
  -e 'key down command' \
  -e 'keystroke "k"' \
  -e 'key up command' \
  -e 'delay 0.2' \
  -e 'end tell' \
  -e 'end tell' >/dev/null 2>&1 || true

if wait_for_regex 'ChatView: toggled conversation dropdown .*open=true' 30; then
  pass "Conversation dropdown opened"
else
  fail "Conversation dropdown did not open"
fi

# Move down once and select that (known old conversation path), then verify replay happened.
osascript \
  -e 'tell application "System Events"' \
  -e 'tell process "personal_agent_gpui"' \
  -e 'set frontmost to true' \
  -e 'key code 125' \
  -e 'delay 0.05' \
  -e 'key code 36' \
  -e 'end tell' \
  -e 'end tell' >/dev/null 2>&1 || true

if wait_for_regex 'ChatView::emit called with event: SelectConversation \{ id: [0-9a-f-]+ \}' 30; then
  TARGET_CONV=$(python3 - "$LOG" <<'PY'
import re, sys
from pathlib import Path
text = Path(sys.argv[1]).read_text(errors='ignore')
text = re.sub(r'\x1b\[[0-9;]*[A-Za-z]', '', text)
matches = re.findall(r'ChatView::emit called with event: SelectConversation \{ id: ([0-9a-f-]+) \}', text)
print(matches[-1] if matches else '')
PY
)
  if [ -n "$TARGET_CONV" ]; then
    pass "Selected old conversation from dropdown (id=$TARGET_CONV)"
  else
    fail "Could not parse selected conversation id from log"
  fi
else
  fail "No SelectConversation event observed"
fi

if [ -n "$TARGET_CONV" ] && wait_for_regex "ChatPresenter: replaying selected conversation messages.*conversation_id=${TARGET_CONV}" 30; then
  pass "Presenter replayed messages for selected old conversation"
else
  fail "No replay log for selected old conversation"
fi

if wait_for_regex 'MainPanel: routing ViewCommand Discriminant\(1\)' 30; then
  pass "MainPanel routed MessageAppended commands after selection"
else
  fail "No MessageAppended routing observed after selection"
fi

if rg -q '^FAIL:' "$OUT"; then
  echo "Conversation selection replay verification failed; see $OUT"
  exit 1
fi

echo "Conversation selection replay verification passed; see $OUT"
