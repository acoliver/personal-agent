#!/usr/bin/env bash
set -euo pipefail

ROOT="/Users/acoliver/projects/personal-agent/gpuui"
LOG="/tmp/personal_agent_gpui.log"
OUT="$ROOT/.tmp_verify_profile_dropdown_population.log"
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

extract_last_int() {
  local pattern="$1"
  python3 - "$LOG" "$pattern" <<'PY'
import re, sys
from pathlib import Path
log_path = Path(sys.argv[1])
pattern = sys.argv[2]
if not log_path.exists():
    print("0")
    raise SystemExit(0)
text = log_path.read_text(errors='ignore')
text = re.sub(r'\x1b\[[0-9;]*[A-Za-z]', '', text)
matches = re.findall(pattern, text, flags=re.MULTILINE)
if not matches:
    print("0")
else:
    value = matches[-1]
    if isinstance(value, tuple):
        value = value[-1]
    print(value)
PY
}

cd "$ROOT"

if [ ! -x "./target/debug/personal_agent_gpui" ]; then
  fail "Missing binary: ./target/debug/personal_agent_gpui"
  echo "Build first with cargo build" | tee -a "$OUT"
  exit 1
fi

pkill -f "target/debug/personal_agent_gpui" 2>/dev/null || true
: > "$LOG"
nohup env RUST_LOG=info PA_AUTO_OPEN_POPUP=1 PA_TEST_POPUP_ONSCREEN=1 ./target/debug/personal_agent_gpui > "$LOG" 2>&1 &
APP_PID=$!
echo "PID=$APP_PID" >> "$OUT"

if wait_for_regex 'ChatView: received profile snapshot count=[0-9]+' 30; then
  count=$(extract_last_int 'ChatView: received profile snapshot count=([0-9]+)')
  if [[ "$count" =~ ^[0-9]+$ ]] && [ "$count" -gt 0 ]; then
    pass "ChatView received non-empty profile snapshot (count=$count)"
  else
    fail "Profile snapshot count not > 0 (count=$count)"
  fi
else
  fail "Profile snapshot did not reach ChatView"
fi

# Open profile dropdown via keyboard shortcut and select next profile.
osascript \
  -e 'tell application "System Events"' \
  -e 'tell process "personal_agent_gpui"' \
  -e 'set frontmost to true' \
  -e 'delay 0.2' \
  -e 'key down command' \
  -e 'keystroke "p"' \
  -e 'key up command' \
  -e 'delay 0.2' \
  -e 'key code 125' \
  -e 'delay 0.1' \
  -e 'key code 36' \
  -e 'end tell' \
  -e 'end tell' >/dev/null 2>&1 || true

if wait_for_regex 'ChatView: toggled profile dropdown .*open=true' 30; then
  pass "Profile dropdown opened"
else
  fail "Profile dropdown did not open"
fi

if wait_for_regex 'ChatView::emit called with event: SelectChatProfile' 30; then
  pass "Profile dropdown emitted SelectChatProfile event"
else
  fail "SelectChatProfile event was not emitted"
fi

if rg -q '^FAIL:' "$OUT"; then
  echo "Profile dropdown population verification failed; see $OUT"
  exit 1
fi

echo "Profile dropdown population verification passed; see $OUT"
