#!/usr/bin/env bash
# Drive personal_agent_gpui through a 50-turn conversation end-to-end via AppleScript.
#
# Preconditions:
#   - personal_agent_gpui is already launched with PA_AUTO_OPEN_POPUP=1 and its popup visible.
#   - The conversation to drive is the newest one in the SQLite DB (CONV below).
#     We track it by (id, updated_at, msg_count) deltas rather than hardcoding a UI state.
#   - LM Studio is running at 127.0.0.1:1234 with the localqwen model loaded (profile
#     a9fde715-36bb-488f-b304-e9c9e2fe46b8, auth=none).
#   - `llxprt` CLI is installed and the `fireworkskimi` profile exists.
#
# Behaviour:
#   - Seeds turn 1 with a known prompt so we have assistant text to riff on.
#   - For turns 2..N: uses llxprt+fireworkskimi to generate a short follow-up, sanitises it,
#     types it via osascript, waits for StreamCompleted in the log, verifies the assistant
#     row landed in SQLite, and records the compression phase from context_state.
#   - Halts on first failure (no silent skips). Dumps per-turn artifacts.
#
# Usage:
#   bash scripts/stabilize_50_turns.sh [TURNS]
#     TURNS defaults to 50.

set -u
set -o pipefail

TURNS=${1:-50}
DB="$HOME/Library/Application Support/PersonalAgent/personalagent.db"
LOG=/tmp/personal_agent_gpui.log
LLXPRT_PROFILE=fireworkskimi
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
ART_DIR="artifacts/stabilize/${TIMESTAMP}"
mkdir -p "$ART_DIR"
SUMMARY="$ART_DIR/summary.csv"
echo "turn,seq_user,seq_assistant,wait_seconds,compression_phase,user_chars,assistant_chars" > "$SUMMARY"

log()  { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*"; }
fail() { log "FAIL: $*"; exit 1; }

# ---------- preflight ----------

pgrep -f personal_agent_gpui >/dev/null || fail "personal_agent_gpui not running; launch it first"
[ -f "$DB" ] || fail "SQLite DB not found: $DB"
[ -f "$LOG" ] || fail "Log file not found: $LOG"
command -v llxprt >/dev/null || fail "llxprt not in PATH"
command -v osascript >/dev/null || fail "osascript not available (macOS only)"
command -v sqlite3 >/dev/null || fail "sqlite3 not in PATH"

# Force a fresh conversation so we can watch compression trigger from a clean slate.
# Cmd-N isn't bound; ctrl-n is the registered NewConversation shortcut.  We use the same
# popup-reopen pattern as send_prompt to make sure the shortcut actually reaches the app.
press_new_conversation() {
  # Cmd-N in the ChatView emits UserEvent::NewConversation AND clears the input.
  # (ctrl-n is bound as a GPUI action but only navigates; it does NOT clear state.)
  # Close+reopen the popup first so the Chat view is the key/focused responder.
  osascript \
    -e 'tell application "System Events"' \
    -e 'key up command' -e 'key up control' -e 'key up option' -e 'key up shift' \
    -e 'tell process "personal_agent_gpui"' \
    -e 'set frontmost to true' \
    -e 'delay 0.3' \
    -e 'try' \
    -e '  if (count of windows) > 0 then' \
    -e '    tell menu bar 1 to click menu bar item 1' \
    -e '    delay 0.3' \
    -e '  end if' \
    -e 'end try' \
    -e 'tell menu bar 1 to click menu bar item 1' \
    -e 'delay 0.5' \
    -e 'set frontmost to true' \
    -e 'delay 0.3' \
    -e 'keystroke "n" using command down' \
    -e 'end tell' \
    -e 'end tell' >/dev/null
  sleep 0.5
}

# We do not resolve CONV yet; the current newest in DB is the previous conversation.
# After Ctrl-N + first send, a brand-new conversation row will appear.
PREV_NEWEST=$(sqlite3 "$DB" "SELECT id FROM conversations ORDER BY updated_at DESC LIMIT 1;")
log "Previous newest conversation: $PREV_NEWEST (will switch to freshly-created one)"
log "Artifacts: $ART_DIR"

# ---------- helpers ----------

msg_count()     { sqlite3 "$DB" "SELECT COUNT(*) FROM messages WHERE conversation_id='$CONV';"; }
last_assistant(){ sqlite3 "$DB" "SELECT content FROM messages WHERE conversation_id='$CONV' AND role='assistant' ORDER BY seq DESC LIMIT 1;"; }
last_seq_of()   { sqlite3 "$DB" "SELECT COALESCE(MAX(seq),-1) FROM messages WHERE conversation_id='$CONV' AND role='$1';"; }
compression_phase() {
  sqlite3 "$DB" "SELECT json_extract(context_state,'\$.compression_phase') FROM conversations WHERE id='$CONV';"
}

# AppleScript type+enter.  Assumes app is the frontmost (we raise it first).
send_prompt() {
  local prompt="$1"
  local escaped
  escaped=$(printf '%s' "$prompt" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g')
  # Reliable send pattern:
  #   1. Raise personal_agent_gpui.
  #   2. If popup is open, close it; then open it fresh (tray click).  This focuses
  #      the input field but preserves any leftover text from prior sessions.
  #   3. Select-all (cmd-a) + delete to clear the field.
  #   4. Type + Enter.
  osascript \
    -e 'tell application "System Events"' \
    -e 'key up command' -e 'key up control' -e 'key up option' -e 'key up shift' \
    -e 'tell process "personal_agent_gpui"' \
    -e 'set frontmost to true' \
    -e 'delay 0.3' \
    -e 'try' \
    -e '  if (count of windows) > 0 then' \
    -e '    tell menu bar 1 to click menu bar item 1' \
    -e '    delay 0.3' \
    -e '  end if' \
    -e 'end try' \
    -e 'tell menu bar 1 to click menu bar item 1' \
    -e 'delay 0.5' \
    -e 'set frontmost to true' \
    -e 'delay 0.3' \
    -e 'keystroke "a" using command down' \
    -e 'delay 0.1' \
    -e 'key code 51' \
    -e 'delay 0.1' \
    -e "keystroke \"$escaped\"" \
    -e 'delay 0.2' \
    -e 'key code 36' \
    -e 'end tell' \
    -e 'end tell' >/dev/null
}

# Sanitise llxprt output.  We invoke llxprt with --set reasoning.includeInResponse=false
# so there are no <think> tags, but output still contains todo banners and [profile]
# markers interleaved with the actual answer.  Strip those, keep the last content line.
sanitise_followup() {
  # NOTE: use a here-doc as the script source (/dev/fd/3) while keeping stdin for data.
  python3 /dev/fd/3 3<<'PY'
import sys, re
raw = sys.stdin.read()
# belt-and-braces: strip any stray thinking blocks too
raw = re.sub(r'<think>.*?</think>', '', raw, flags=re.S | re.I)
lines = []
for line in raw.splitlines():
    s = line.strip()
    if not s:
        continue
    # [profile] / [llxprt] markers
    if s.startswith('[') and s.endswith(']'):
        continue
    # todo banners and list glyphs
    if s.startswith(('##', chr(0x2192), chr(0x25cb), chr(0x2022), chr(0x2591))):
        continue
    if re.match(r'^\d+\s+tasks?\b', s, re.I):
        continue
    if s.lower().startswith(('no todos found', 'use todo_write', 'todo progress')):
        continue
    # Heuristic: our real follow-up is lowercase.  Todo-title lines are typically
    # capitalised ("Generate ...", "Analyze ...").  Drop capital-imperative lines.
    if re.match(r'^[A-Z][A-Za-z]+ ', s):
        continue
    lines.append(s)
if not lines:
    sys.exit(2)
text = lines[-1]
text = text.strip('"\'' + chr(0x2018) + chr(0x2019) + chr(0x201c) + chr(0x201d))
text = re.sub(r"[^A-Za-z0-9 .,?!'-]", ' ', text)
text = re.sub(r'\s+', ' ', text).strip()
if not text:
    sys.exit(3)
text = text.lower()
m = re.search(r'[.?!]', text)
if m and m.end() < len(text):
    text = text[:m.end()]
if not re.search(r'[.?!]$', text):
    text += '.'
text = text[:140]
print(text)
PY
}

# Ask llxprt for a follow-up question grounded in the last assistant reply.
generate_followup() {
  local last_reply="$1"
  local turn="$2"
  local snippet
  snippet=$(printf '%s' "$last_reply" | tr '\n' ' ' | head -c 1200)
  local instructions
  read -r -d '' instructions <<EOF || true
You are simulating a curious end-user chatting with an AI assistant.
Produce ONE natural follow-up question that continues the conversation
based on the assistant's previous reply shown below.  The point is to have
a flowing multi-turn conversation that builds context, so ask substantive
questions that invite detailed answers.
Rules:
- Output ONLY the user question, nothing else. No preamble, no quotes, no explanation.
- All lowercase.
- One sentence ending with a period or question mark.
- No markdown, no code fences, no emoji.
- Keep it on-topic with the previous reply; you can change subject occasionally
  to let the conversation drift naturally.
- Do NOT instruct the assistant to answer briefly, keep it short, or stay under
  any word limit.  We want full natural replies so context grows.

Previous assistant reply:
${snippet}
EOF
  printf '%s' "$instructions" \
    | llxprt --profile-load "$LLXPRT_PROFILE" \
             --set reasoning.includeInResponse=false \
             -p "" 2>/dev/null \
    | sanitise_followup
}

# Wait for one additional StreamCompleted in log, plus +2 msg rows in DB.
wait_turn_complete() {
  local baseline_stream="$1"
  local baseline_msgs="$2"
  local deadline=$((SECONDS + 420))
  local stream_ok=0
  local msg_ok=0
  while [ $SECONDS -lt $deadline ]; do
    if [ "$stream_ok" -eq 0 ]; then
      local now
      now=$(grep -c 'StreamCompleted' "$LOG")
      # StreamCompleted can fire twice per turn (thinking + final).  Wait for >=2 above baseline
      # if log shows that pattern, else >=1.
      if [ "$now" -ge $((baseline_stream + 1)) ]; then stream_ok=1; fi
    fi
    if [ "$msg_ok" -eq 0 ]; then
      local mc
      mc=$(msg_count)
      if [ "$mc" -ge $((baseline_msgs + 2)) ]; then msg_ok=1; fi
    fi
    if [ $stream_ok -eq 1 ] && [ $msg_ok -eq 1 ]; then
      return 0
    fi
    sleep 1
  done
  return 1
}

# ---------- turn loop ----------

# Seed turn: use the current last assistant reply if present; otherwise send a stock prompt.
# Start a brand-new conversation, send the seed, and resolve the new conversation id
# from the DB once the app persists it on first send.
press_new_conversation
log "Pressed Ctrl-N to start fresh conversation; sending seed prompt."
SEED_PROMPT='i want to chat about bananas. tell me something interesting about them to start.'
base_stream=$(grep -c 'StreamCompleted' "$LOG")
send_prompt "$SEED_PROMPT"

# Wait for a new conversation row to appear (different id than PREV_NEWEST) with >=1 message.
log "Waiting for fresh conversation to be persisted..."
deadline=$((SECONDS + 60))
CONV=""
while [ $SECONDS -lt $deadline ]; do
  candidate=$(sqlite3 "$DB" "SELECT id FROM conversations ORDER BY updated_at DESC LIMIT 1;")
  if [ -n "$candidate" ] && [ "$candidate" != "$PREV_NEWEST" ]; then
    mc=$(sqlite3 "$DB" "SELECT COUNT(*) FROM messages WHERE conversation_id='$candidate';")
    if [ "$mc" -ge 1 ]; then
      CONV="$candidate"
      break
    fi
  fi
  sleep 1
done
[ -n "$CONV" ] || fail "fresh conversation never appeared in DB after Ctrl-N + seed send"
PROFILE_ID=$(sqlite3 "$DB" "SELECT profile_id FROM conversations WHERE id='$CONV';")
log "Driving conversation: $CONV (profile=$PROFILE_ID)"

# Now wait for the seed turn to fully complete (stream done, assistant row present).
if ! wait_turn_complete "$base_stream" 0; then
  fail "seed turn never completed (no StreamCompleted + assistant row within 420s)"
fi
SEED_REPLY=$(last_assistant)
[ -n "$SEED_REPLY" ] || fail "seed turn completed but no assistant reply persisted"
log "Seed assistant reply: $(printf '%s' "$SEED_REPLY" | tr '\n' ' ' | head -c 120)..."

for turn in $(seq 1 "$TURNS"); do
  log "--- Turn $turn/$TURNS ---"
  prompt=$(generate_followup "$SEED_REPLY" "$turn")
  if [ -z "$prompt" ]; then
    # fallback so the loop doesn't stall
    prompt="tell me one more interesting thing about that topic."
    log "llxprt returned empty; using fallback prompt"
  fi
  log "Prompt: $prompt"

  printf '%s\n' "$prompt" > "$ART_DIR/turn-$(printf '%02d' "$turn")-prompt.txt"

  before_stream=$(grep -c 'StreamCompleted' "$LOG")
  before_msgs=$(msg_count)
  before_user_seq=$(last_seq_of user)
  turn_start=$SECONDS

  send_prompt "$prompt"

  if ! wait_turn_complete "$before_stream" "$before_msgs"; then
    reply_now=$(last_assistant)
    printf '%s\n' "$reply_now" > "$ART_DIR/turn-$(printf '%02d' "$turn")-PARTIAL-reply.txt"
    fail "turn $turn timed out after $((SECONDS - turn_start))s waiting for StreamCompleted+rows"
  fi

  wait_seconds=$((SECONDS - turn_start))

  # Validate: user row present at expected new seq, assistant row immediately after
  user_seq=$(sqlite3 "$DB" "SELECT MAX(seq) FROM messages WHERE conversation_id='$CONV' AND role='user';")
  asst_seq=$(sqlite3 "$DB" "SELECT MAX(seq) FROM messages WHERE conversation_id='$CONV' AND role='assistant';")
  if [ "$user_seq" -le "$before_user_seq" ]; then
    fail "user message did not persist (seq stayed at $before_user_seq)"
  fi
  if [ "$asst_seq" -le "$user_seq" ]; then
    # Assistant must follow user in seq
    fail "assistant reply seq ($asst_seq) not after user seq ($user_seq)"
  fi

  reply=$(last_assistant)
  printf '%s\n' "$reply" > "$ART_DIR/turn-$(printf '%02d' "$turn")-reply.txt"

  phase=$(compression_phase)
  phase=${phase:-null}

  user_chars=${#prompt}
  asst_chars=${#reply}
  echo "$turn,$user_seq,$asst_seq,$wait_seconds,$phase,$user_chars,$asst_chars" >> "$SUMMARY"
  log "  done in ${wait_seconds}s; user_seq=$user_seq asst_seq=$asst_seq phase=$phase reply_chars=$asst_chars"

  SEED_REPLY=$reply
done

log "ALL $TURNS TURNS COMPLETE"
log "Summary: $SUMMARY"
tail -n 5 "$SUMMARY"

# Final compression summary
final_phase=$(compression_phase)
final_count=$(msg_count)
log "Final state: messages=$final_count compression_phase=$final_phase"
