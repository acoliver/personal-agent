#!/usr/bin/env bash
set -euo pipefail

ROOT="/Users/acoliver/projects/personal-agent/gpuui"
PRE_WT="/tmp/gpuui-pre-ExOh7J"

echo "== CODE-LEVEL VERIFICATION: PRE-FIX (expected failures) =="
if [ ! -e "$PRE_WT/.git" ]; then
  echo "Pre-fix worktree missing: $PRE_WT"
  exit 1
fi
if [ ! -f "$PRE_WT/tests/seven_bugs_regression_tests.rs" ]; then
  cp "$ROOT/tests/seven_bugs_regression_tests.rs" "$PRE_WT/tests/seven_bugs_regression_tests.rs"
fi
if [ ! -d "$PRE_WT/research/serdesAI/serdes-ai" ]; then
  rm -rf "$PRE_WT/research"
  cp -R "$ROOT/research" "$PRE_WT/research"
fi

PRE_OUT="$PRE_WT/.tmp_pre_fix_regression.log"
(
  cd "$PRE_WT"
  cargo test --test seven_bugs_regression_tests > "$PRE_OUT" 2>&1 || true
)

grep -E "test result:|FAILED|bug[0-9]" "$PRE_OUT" | tail -40 || true

if ! grep -q "test result: FAILED" "$PRE_OUT"; then
  echo "ERROR: Expected failing pre-fix regression suite"
  exit 2
fi


echo ""
echo "== CODE-LEVEL VERIFICATION: CURRENT (expected pass) =="
CUR_OUT="$ROOT/.tmp_current_regression.log"
(
  cd "$ROOT"
  cargo test --test seven_bugs_regression_tests > "$CUR_OUT" 2>&1
)

grep -E "running [0-9]+ tests|test result:" "$CUR_OUT" | tail -10 || true
if ! grep -q "test result: ok" "$CUR_OUT"; then
  echo "ERROR: Current regression suite did not pass"
  exit 3
fi


echo ""
echo "== CODE SIGNATURE CHECKS (current source should show fixes) =="
(
  cd "$ROOT"
  echo "[chat_view] StartRenameConversation emit removed?"
  rg -n "StartRenameConversation" src/ui_gpui/views/chat_view.rs || true

  echo "[chat_impl] duplicate LlmMessage push removed?"
  rg -n "messages.push\(LlmMessage::user\(" src/services/chat_impl.rs || true

  echo "[history_view] list+created handlers present?"
  rg -n "ConversationListRefreshed|ConversationCreated" src/ui_gpui/views/history_view.rs || true

  echo "[profile_editor_view] editable model field present?"
  rg -n "ActiveField::Model|field-model-id" src/ui_gpui/views/profile_editor_view.rs || true
)


echo ""
echo "Verification complete: pre-fix fails reproduced; current build passes code-level regression checks."

# UI automation is validated separately by scripts/verify_ui_current.sh,
# scripts/verify_rename_visual.sh, and scripts/verify_memory_simple.sh.
exit 0
