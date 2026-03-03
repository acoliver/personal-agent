#!/usr/bin/env bash
set -euo pipefail

ROOT="/Users/acoliver/projects/personal-agent/gpuui"
LOG="/tmp/personal_agent_gpui.log"
OUT="$ROOT/.tmp_verify_conversation_routing.log"
: > "$OUT"

pass() { echo "PASS: $1" | tee -a "$OUT"; }
fail() { echo "FAIL: $1" | tee -a "$OUT"; }

cd "$ROOT"

# 1) Compile sanity after latest routing/dropdown fixes
if cargo check >> "$OUT" 2>&1; then
  pass "cargo check"
else
  fail "cargo check"
fi

# 2) ChatPresenter selection replay unit test
if cargo test --test presenter_selection_and_settings_tests select_conversation_emits_activation_and_replays_messages >> "$OUT" 2>&1; then
  pass "SelectConversation replay test"
else
  fail "SelectConversation replay test"
fi

# 3) Routing-level history command tests
history_ok=1
if cargo test --test gpui_wiring_command_routing_tests test_history_view_conversation_list_refreshed_is_routed >> "$OUT" 2>&1; then
  pass "History routing test: ConversationListRefreshed"
else
  fail "History routing test: ConversationListRefreshed"
  history_ok=0
fi
if cargo test --test gpui_wiring_command_routing_tests test_history_view_conversation_activated_is_routed >> "$OUT" 2>&1; then
  pass "History routing test: ConversationActivated"
else
  fail "History routing test: ConversationActivated"
  history_ok=0
fi
if [ "$history_ok" -eq 1 ]; then
  pass "History routing tests"
fi

# 4) Confirm selected IDs from manual run actually exist on disk and contain messages.
ids=$(rg -o "SelectConversation \{ id: [0-9a-f-]+ \}" /tmp/personal_agent_gpui.manual_verify.log | sed -E 's/.*id: ([0-9a-f-]+).*/\1/' | sort -u | head -5 || true)
if [ -z "$ids" ]; then
  fail "No SelectConversation IDs found in /tmp/personal_agent_gpui.manual_verify.log"
else
  pass "Found SelectConversation IDs in manual log"
  echo "IDs: $ids" | tee -a "$OUT"

  missing=0
  empties=0
  for id in $ids; do
    f="$HOME/.llxprt/conversations/${id}.json"
    if [ ! -f "$f" ]; then
      echo "MISSING: $f" | tee -a "$OUT"
      missing=$((missing + 1))
      continue
    fi
    count=$(python3 -c "import json; print(len(json.load(open('$f')).get('messages',[])))" 2>/dev/null || echo 0)
    echo "$id messages=$count" | tee -a "$OUT"
    if [ "$count" -eq 0 ]; then
      empties=$((empties + 1))
    fi
  done

  if [ "$missing" -eq 0 ]; then
    pass "All logged SelectConversation IDs resolve to existing conversation files"
  else
    fail "$missing selected conversation files missing"
  fi

  if [ "$empties" -eq 0 ]; then
    pass "Selected conversations contain persisted messages"
  else
    fail "$empties selected conversations had zero persisted messages"
  fi
fi

if rg -q '^FAIL:' "$OUT"; then
  echo "Conversation routing verification failed; see $OUT"
  exit 1
fi

echo "Conversation routing verification passed; see $OUT"
