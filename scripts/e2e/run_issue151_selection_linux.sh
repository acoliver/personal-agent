#!/usr/bin/env bash
# Real Linux/X11 production-window verification for issue #151.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN="${1:-$ROOT/target/debug/personal_agent_gpui}"
ARTIFACTS="${ISSUE151_ARTIFACT_DIR:-$ROOT/artifacts/issue151}"
TMP_ROOT="$(mktemp -d /tmp/personal-agent-issue151.XXXXXX)"
DATA_HOME="$TMP_ROOT/data"
CONFIG_HOME="$TMP_ROOT/config"
LOG="$ARTIFACTS/app.log"
APP_PID=""

cleanup() {
  if [[ -n "$APP_PID" ]]; then
    kill "$APP_PID" 2>/dev/null || true
    wait "$APP_PID" 2>/dev/null || true
  fi
  rm -rf "$TMP_ROOT"
}
trap cleanup EXIT

for tool in xdotool xclip import compare python3; do
  command -v "$tool" >/dev/null || { echo "Missing required tool: $tool" >&2; exit 2; }
done
[[ "$(uname -s)" == "Linux" ]] || { echo "This runner is Linux-only" >&2; exit 2; }
[[ -n "${DISPLAY:-}" ]] || { echo "DISPLAY is not set" >&2; exit 2; }

mkdir -p "$ARTIFACTS" "$DATA_HOME/PersonalAgent" "$CONFIG_HOME/PersonalAgent/profiles"
rm -f "$ARTIFACTS/before-selection.png" "$ARTIFACTS/selected.png" \
  "$ARTIFACTS/context-menu.png" "$ARTIFACTS/evidence.txt" "$LOG" \
  "$ARTIFACTS/build.log"

# Ensure the production binary reflects the working tree and preserve build
# diagnostics alongside the interaction evidence.
cargo build --bin personal_agent_gpui >"$ARTIFACTS/build.log" 2>&1

# Preserve the configured provider/profile shape in the isolated test config.
# Profile files contain keychain labels, not secret values; secrets stay in the
# OS keyring and are never printed or copied by this script.
if [[ -d "$HOME/.config/PersonalAgent/profiles" ]]; then
  cp -a "$HOME/.config/PersonalAgent/profiles/." "$CONFIG_HOME/PersonalAgent/profiles/"
fi

DB="$DATA_HOME/PersonalAgent/personalagent.db"
DB="$DB" python3 - <<'PY'
import datetime
import os
import sqlite3
import uuid

path = os.environ["DB"]
conn = sqlite3.connect(path)
conn.executescript("""
CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    title TEXT,
    profile_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    context_state TEXT
);
CREATE TABLE messages (
    id INTEGER PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system')),
    content TEXT NOT NULL,
    thinking_content TEXT,
    model_id TEXT,
    tool_calls TEXT,
    tool_results TEXT,
    created_at TEXT NOT NULL,
    seq INTEGER NOT NULL
);
""")
conversation_id = str(uuid.uuid4())
now = datetime.datetime.now(datetime.timezone.utc).isoformat()
conn.execute(
    "INSERT INTO conversations VALUES (?, ?, NULL, ?, ?, NULL)",
    (conversation_id, "Issue 151 selection E2E", now, now),
)
messages = [
    (
        "user",
        "ISSUE151_ALPHA user bubble café 😀 — drag and copy this exact marker.",
        None,
    ),
    (
        "assistant",
        "# ISSUE151_ALPHA heading\n\n"
        "Rich **BRAVO** and *italic* text with [a safe link](https://example.com).\n\n"
        "`CODE_TOKEN café 😀`\n\n"
        "- first selectable list item\n- second selectable list item",
        "zai-glm-e2e",
    ),
]
for seq, (role, content, model_id) in enumerate(messages):
    conn.execute(
        """INSERT INTO messages
           (conversation_id, role, content, thinking_content, model_id,
            tool_calls, tool_results, created_at, seq)
           VALUES (?, ?, ?, NULL, ?, NULL, NULL, ?, ?)""",
        (conversation_id, role, content, model_id, now, seq),
    )
conn.commit()
PY

# Isolate app data/config so this never mutates the user's conversations or
# settings while still exercising the real production binary and X11 backend.
XDG_DATA_HOME="$DATA_HOME" \
XDG_CONFIG_HOME="$CONFIG_HOME" \
PA_AUTO_OPEN_POPUP=1 \
PA_TEST_POPUP_ONSCREEN=1 \
"$BIN" >"$LOG" 2>&1 &
APP_PID=$!

WINDOW_ID=""
for _ in $(seq 1 120); do
  if ! kill -0 "$APP_PID" 2>/dev/null; then
    echo "App exited before opening a window" >&2
    tail -100 "$LOG" >&2 || true
    exit 1
  fi
  WINDOW_ID="$(
    { xdotool search --onlyvisible --pid "$APP_PID" 2>/dev/null || true; } |
      while read -r candidate; do
        eval "$(xdotool getwindowgeometry --shell "$candidate" 2>/dev/null)" || continue
        printf '%s %s\n' "$((WIDTH * HEIGHT))" "$candidate"
      done |
      sort -nr |
      awk 'NR == 1 { print $2 }'
  )"
  [[ -n "$WINDOW_ID" ]] && break
  sleep 0.1
done

if [[ -z "$WINDOW_ID" ]]; then
  echo "Could not find the production popup window for PID $APP_PID" >&2
  tail -100 "$LOG" >&2 || true
  exit 1
fi


sleep 1
python3 "$ROOT/scripts/e2e/issue151_selection_linux.py" "$WINDOW_ID" "$ARTIFACTS" "$LOG"
