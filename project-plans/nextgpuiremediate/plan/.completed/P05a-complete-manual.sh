#!/usr/bin/env bash
set -euo pipefail

ROOT="/Users/acoliver/projects/personal-agent/gpuui"
OUT_DIR="$ROOT/project-plans/nextgpuiremediate/plan/.completed"
CHECKLIST="$OUT_DIR/P05a-human-checklist.md"
LOG_CAP="$OUT_DIR/P05a-log-capture.sh"
UNLOCK="$OUT_DIR/P05a-unlock-gate.sh"
CHECKLIST_LINT="$OUT_DIR/P05a-checklist-lint.sh"
CANDIDATE="$OUT_DIR/P05a-pass-candidate.md"
FINAL="$OUT_DIR/P05a.md"

cd "$ROOT"

if [[ ! -x "$LOG_CAP" ]]; then
  echo "[FAIL] Missing executable log capture harness: $LOG_CAP"
  exit 1
fi
if [[ ! -x "$UNLOCK" ]]; then
  echo "[FAIL] Missing executable unlock script: $UNLOCK"
  exit 1
fi
if [[ ! -f "$CHECKLIST" ]]; then
  echo "[FAIL] Missing checklist: $CHECKLIST"
  exit 1
fi

if [[ ! -t 0 ]]; then
  echo "[BLOCKED] P05a manual completion requires an interactive terminal/session."
  echo "Run this script locally so you can perform the 4 manual checks, then rerun."
  exit 4
fi

cat <<'EOF'
[P05a] Manual completion flow starting.

You will now:
  1) run the GPUI manual verification harness
  2) fill checklist outcomes
  3) validate gate (requires 4x PASS sections)
  4) confirm replacement of P05a.md
EOF

"$LOG_CAP"

echo ""
echo "Open checklist and fill outcomes now:"
echo "  $CHECKLIST"
read -r -p "Press ENTER after checklist is filled... " _

if [[ -x "$CHECKLIST_LINT" ]]; then
  set +e
  "$CHECKLIST_LINT"
  LINT_RC=$?
  set -e
  if [[ "$LINT_RC" -ne 0 ]]; then
    echo "[LOCKED] Checklist lint indicates unresolved issues (rc=$LINT_RC)."
    exit "$LINT_RC"
  fi
fi

set +e
"$UNLOCK"
UNLOCK_RC=$?
set -e

if [[ "$UNLOCK_RC" -ne 0 ]]; then
  echo "[LOCKED] Unlock gate did not pass (rc=$UNLOCK_RC)."
  echo "         Ensure all four checks are marked PASS in checklist"
  echo "         (either 'Result: PASS' or '- [x] PASS')."
  exit "$UNLOCK_RC"
fi

if [[ ! -f "$CANDIDATE" ]]; then
  echo "[FAIL] Expected pass candidate missing: $CANDIDATE"
  exit 1
fi

echo ""
echo "Pass candidate generated: $CANDIDATE"
echo "Review and finalize evidence."
read -r -p "Copy candidate over final P05a.md now? [y/N]: " ANSWER

if [[ "$ANSWER" =~ ^[Yy]$ ]]; then
  cp "$CANDIDATE" "$FINAL"
  echo "[DONE] Updated: $FINAL"
else
  echo "[SKIP] Final file not modified. Candidate remains at: $CANDIDATE"
fi

echo "Complete."
