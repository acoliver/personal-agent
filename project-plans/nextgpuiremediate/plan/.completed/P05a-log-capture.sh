#!/usr/bin/env bash
set -euo pipefail

ROOT="/Users/acoliver/projects/personal-agent/gpuui"
OUT_DIR="$ROOT/project-plans/nextgpuiremediate/plan/.completed"
LOG_FILE="/tmp/personal_agent_gpui.log"
SNAPSHOT_FILE="$OUT_DIR/P05a-runtime-log-snapshot.txt"

ALLOW_NON_INTERACTIVE=0
WAIT_SECONDS=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --allow-noninteractive)
      ALLOW_NON_INTERACTIVE=1
      shift
      ;;
    --wait-seconds)
      WAIT_SECONDS="${2:-}"
      if [[ -z "$WAIT_SECONDS" || ! "$WAIT_SECONDS" =~ ^[0-9]+$ ]]; then
        echo "[FAIL] --wait-seconds requires a non-negative integer"
        exit 1
      fi
      shift 2
      ;;
    *)
      echo "[FAIL] Unknown argument: $1"
      echo "Usage: $0 [--allow-noninteractive] [--wait-seconds N]"
      exit 1
      ;;
  esac
done

if [[ ! -t 0 && "$ALLOW_NON_INTERACTIVE" -ne 1 ]]; then
  echo "[BLOCKED] Non-interactive execution cannot complete manual checks."
  echo "Run this script from an interactive local terminal, or rerun with:"
  echo "  --allow-noninteractive --wait-seconds N"
  exit 4
fi

cleanup() {
  pkill -f personal_agent_gpui || true
}
trap cleanup EXIT

cd "$ROOT"
mkdir -p "$OUT_DIR"

echo "[P05a] Starting manual verification harness"
pkill -f personal_agent_gpui || true

cargo build --bin personal_agent_gpui
nohup cargo run --bin personal_agent_gpui >"$LOG_FILE" 2>&1 &
APP_PID=$!

sleep 2

echo ""
echo "App started (pid: $APP_PID)."
echo "Run the four manual checks now:"
echo "  1) Settings -> Model selector -> results load"
echo "  2) Profile editor save path"
echo "  3) MCP add/configure path"
echo "  4) Chat send + stream updates"
echo ""

if [[ -t 0 ]]; then
  read -r -p "Press ENTER when manual checks are complete... " _
else
  if [[ "$WAIT_SECONDS" -gt 0 ]]; then
    echo "Non-interactive mode enabled; waiting ${WAIT_SECONDS}s before snapshot."
    sleep "$WAIT_SECONDS"
  else
    echo "Non-interactive mode enabled; capturing snapshot immediately."
  fi
fi

if [[ -f "$LOG_FILE" ]]; then
  tail -n 400 "$LOG_FILE" > "$SNAPSHOT_FILE"
else
  echo "(missing runtime log: $LOG_FILE)" > "$SNAPSHOT_FILE"
fi

echo "Saved log snapshot: $SNAPSHOT_FILE"
echo "Now fill: $OUT_DIR/P05a-human-checklist.md"
echo "Then copy final outcomes into: $OUT_DIR/P05a.md (or start from P05a-pass-template.md)"
echo "Done."
