#!/usr/bin/env bash
set -euo pipefail

ROOT="/Users/acoliver/projects/personal-agent/gpuui"
OUT_DIR="$ROOT/project-plans/nextgpuiremediate/plan/.completed"
CHECKLIST="$OUT_DIR/P05a-human-checklist.md"
LOG_SNAPSHOT="$OUT_DIR/P05a-runtime-log-snapshot.txt"
PASS_TEMPLATE="$OUT_DIR/P05a-pass-template.md"
CHECKLIST_LINT="$OUT_DIR/P05a-checklist-lint.sh"
CANDIDATE="$OUT_DIR/P05a-pass-candidate.md"

if [[ ! -f "$CHECKLIST" ]]; then
  echo "[FAIL] Missing checklist: $CHECKLIST"
  exit 1
fi

if [[ ! -s "$LOG_SNAPSHOT" ]]; then
  echo "[FAIL] Missing or empty log snapshot: $LOG_SNAPSHOT"
  exit 1
fi

if [[ -x "$CHECKLIST_LINT" ]]; then
  set +e
  "$CHECKLIST_LINT" --quiet
  LINT_RC=$?
  set -e
  if [[ "$LINT_RC" -ne 0 ]]; then
    echo "[LOCKED] Checklist lint failed. Run: $CHECKLIST_LINT"
    exit 2
  fi
fi

# Count PASS per manual-check section (supports either explicit "Result: PASS"
# or checked PASS checkbox "- [x] PASS").
PASS_COUNT=$(awk '
  BEGIN { in_section=0; section_pass=0; pass_sections=0 }
  /^### [0-9]+\)/ {
    if (in_section && section_pass) {
      pass_sections++
    }
    in_section=1
    section_pass=0
    next
  }
  /^[[:space:]]*Result:[[:space:]]*PASS[[:space:]]*$/ {
    section_pass=1
    next
  }
  /^[[:space:]]*-[[:space:]]*\[[xX]\][[:space:]]*PASS[[:space:]]*$/ {
    section_pass=1
    next
  }
  END {
    if (in_section && section_pass) {
      pass_sections++
    }
    print pass_sections
  }
' "$CHECKLIST")

if [[ "$PASS_COUNT" -lt 4 ]]; then
  echo "[LOCKED] Manual gate not satisfied. Found $PASS_COUNT of 4 PASSed manual-check sections in checklist."
  echo "         Mark each section with either 'Result: PASS' or '- [x] PASS'."
  exit 2
fi

if [[ ! -f "$PASS_TEMPLATE" ]]; then
  echo "[FAIL] Missing pass template: $PASS_TEMPLATE"
  exit 1
fi

STAMP=$(date "+%Y-%m-%d %H:%M")
sed "s/YYYY-MM-DD HH:MM/$STAMP/" "$PASS_TEMPLATE" > "$CANDIDATE"

cat <<EOF
[PASS-CANDIDATE] Generated: $CANDIDATE
Next steps:
  1) Copy Expected/Actual outcomes from $CHECKLIST into $CANDIDATE
  2) Add relevant log lines from $LOG_SNAPSHOT
  3) Replace .completed/P05a.md only after final human review confirms all 4 checks are PASS
EOF
