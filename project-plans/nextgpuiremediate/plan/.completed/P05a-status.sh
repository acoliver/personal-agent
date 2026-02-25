#!/usr/bin/env bash
set -euo pipefail

ROOT="/Users/acoliver/projects/personal-agent/gpuui"
OUT_DIR="$ROOT/project-plans/nextgpuiremediate/plan/.completed"
CHECKLIST="$OUT_DIR/P05a-human-checklist.md"
EVIDENCE="$OUT_DIR/P05a.md"
LOG_SNAPSHOT="$OUT_DIR/P05a-runtime-log-snapshot.txt"
CHECKLIST_LINT="$OUT_DIR/P05a-checklist-lint.sh"

pass_count=0
if [[ -f "$CHECKLIST" ]]; then
  pass_count=$(awk '
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
fi

lint_state="SKIPPED (lint helper missing)"
lint_rc=0
if [[ -x "$CHECKLIST_LINT" ]]; then
  set +e
  "$CHECKLIST_LINT" --quiet
  lint_rc=$?
  set -e
  if [[ "$lint_rc" -eq 0 ]]; then
    lint_state="PASS"
  elif [[ "$lint_rc" -eq 2 ]]; then
    lint_state="FAIL"
  else
    lint_state="ERROR (rc=$lint_rc)"
  fi
fi

verdict="MISSING"
if [[ -f "$EVIDENCE" ]]; then
  verdict=$(grep -E '^## Verdict:' "$EVIDENCE" | head -n1 | sed 's/^## Verdict: //' || true)
  [[ -z "$verdict" ]] && verdict="UNKNOWN"
fi

echo "P05a Gate Status"
echo "----------------"
echo "Evidence file:   $EVIDENCE"
echo "Verdict:         $verdict"
echo "Checklist PASS sections: $pass_count / 4"
echo "Checklist lint:  $lint_state"
if [[ -s "$LOG_SNAPSHOT" ]]; then
  echo "Log snapshot:    present"
else
  echo "Log snapshot:    missing-or-empty"
fi

echo ""
if [[ "$verdict" == "PASS" ]]; then
  echo "[UNBLOCKED] P06 may proceed."
  exit 0
fi

if [[ "$lint_rc" -ne 0 ]]; then
  echo "[BLOCKED] Checklist lint failed. Fix checklist quality first:"
  echo "  $CHECKLIST_LINT"
  echo "Then run interactive completion flow:"
  echo "  $OUT_DIR/P05a-complete-manual.sh"
  exit 3
fi

if [[ "$pass_count" -ge 4 ]]; then
  echo "[READY] Checklist appears complete; run unlock helper:"
  echo "  $OUT_DIR/P05a-unlock-gate.sh"
  echo "  (or run one-shot: $OUT_DIR/P05a-complete-manual.sh)"
  exit 2
fi

echo "[BLOCKED] Human interactive verification still required."
echo "Run:"
echo "  $OUT_DIR/P05a-complete-manual.sh"
echo "(Must be run in an interactive local terminal.)"
exit 3
