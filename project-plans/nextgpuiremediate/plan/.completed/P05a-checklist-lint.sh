#!/usr/bin/env bash
set -euo pipefail

ROOT="/Users/acoliver/projects/personal-agent/gpuui"
OUT_DIR="$ROOT/project-plans/nextgpuiremediate/plan/.completed"
CHECKLIST="$OUT_DIR/P05a-human-checklist.md"

QUIET=0
if [[ "${1:-}" == "--quiet" ]]; then
  QUIET=1
fi

if [[ ! -f "$CHECKLIST" ]]; then
  echo "[FAIL] Missing checklist: $CHECKLIST"
  exit 1
fi

section_rows=$(awk '
  BEGIN { section=0; reading_actual=0 }
  /^### [0-9]+\)/ {
    section=$2
    sub(/\)/, "", section)
    reading_actual=0
    next
  }
  section >= 1 && section <= 4 {
    if ($0 ~ /^Actual:[[:space:]]*$/) {
      reading_actual=1
      next
    }

    if (reading_actual == 1 && $0 ~ /^- /) {
      line=$0
      if (line ~ /\[fill in\]/ || line ~ /^-[[:space:]]*$/) {
        actual_filled[section]=0
      } else {
        actual_filled[section]=1
      }
      reading_actual=0
      next
    }

    if ($0 ~ /^Result:[[:space:]]*PASS[[:space:]]*$/) result_pass[section]=1
    if ($0 ~ /^Result:[[:space:]]*FAIL[[:space:]]*$/) result_fail[section]=1
    if ($0 ~ /^-[[:space:]]*\[[xX]\][[:space:]]*PASS[[:space:]]*$/) box_pass[section]=1
    if ($0 ~ /^-[[:space:]]*\[[xX]\][[:space:]]*FAIL[[:space:]]*$/) box_fail[section]=1
  }
  END {
    for (i=1; i<=4; i++) {
      af=(i in actual_filled)?actual_filled[i]:0
      rp=(i in result_pass)?result_pass[i]:0
      rf=(i in result_fail)?result_fail[i]:0
      bp=(i in box_pass)?box_pass[i]:0
      bf=(i in box_fail)?box_fail[i]:0
      printf("SECTION|%d|%d|%d|%d|%d|%d\n", i, af, rp, rf, bp, bf)
    }
  }
' "$CHECKLIST")

issues=0
pass_sections=0

if [[ "$QUIET" -eq 0 ]]; then
  echo "P05a Checklist Lint"
  echo "-------------------"
  echo "Checklist: $CHECKLIST"
fi

while IFS='|' read -r tag idx af rp rf bp bf; do
  [[ "$tag" == "SECTION" ]] || continue

  outcome_pass=0
  outcome_fail=0
  [[ "$rp" == "1" || "$bp" == "1" ]] && outcome_pass=1
  [[ "$rf" == "1" || "$bf" == "1" ]] && outcome_fail=1

  section_issues=()

  if [[ "$af" != "1" ]]; then
    section_issues+=("Actual still placeholder/missing")
  fi

  if [[ "$outcome_pass" == "0" && "$outcome_fail" == "0" ]]; then
    section_issues+=("No outcome marker")
  fi

  if [[ "$outcome_pass" == "1" && "$outcome_fail" == "1" ]]; then
    section_issues+=("Conflicting PASS+FAIL markers")
  fi

  if [[ "$rp" == "1" && "$bf" == "1" ]]; then
    section_issues+=("Result PASS but FAIL checkbox checked")
  fi

  if [[ "$rf" == "1" && "$bp" == "1" ]]; then
    section_issues+=("Result FAIL but PASS checkbox checked")
  fi

  if [[ "$outcome_pass" == "1" && "$outcome_fail" == "0" ]]; then
    pass_sections=$((pass_sections + 1))
  fi

  if [[ "$QUIET" -eq 0 ]]; then
    if [[ ${#section_issues[@]} -eq 0 ]]; then
      echo "Section $idx: OK"
    else
      echo "Section $idx: ISSUE"
      for msg in "${section_issues[@]}"; do
        echo "  - $msg"
      done
    fi
  fi

  if [[ ${#section_issues[@]} -ne 0 ]]; then
    issues=$((issues + 1))
  fi
done <<< "$section_rows"

if [[ "$QUIET" -eq 0 ]]; then
  echo ""
  echo "PASS sections detected: $pass_sections / 4"
fi

if [[ "$issues" -ne 0 ]]; then
  if [[ "$QUIET" -eq 0 ]]; then
    echo ""
    echo "[FAIL] Checklist is not ready for gate unlock."
  fi
  exit 2
fi

if [[ "$QUIET" -eq 0 ]]; then
  echo ""
  echo "[PASS] Checklist lint passed."
fi

exit 0
