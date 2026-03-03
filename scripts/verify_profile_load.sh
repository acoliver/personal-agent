#!/usr/bin/env bash
set -euo pipefail
ROOT="/Users/acoliver/projects/personal-agent/gpuui"
LOG="/tmp/personal_agent_gpui.log"
OUT="$ROOT/.tmp_verify_profile_load.log"
: > "$OUT"

pass() { echo "PASS: $1" | tee -a "$OUT"; }
fail() { echo "FAIL: $1" | tee -a "$OUT"; }

cd "$ROOT"

# Seed compatibility fixtures (idempotent)
./scripts/seed_legacy_profiles.py >> "$OUT" 2>&1

pkill -f "target/debug/personal_agent_gpui" 2>/dev/null || true
pkill -f "target/debug/personal_agent" 2>/dev/null || true

nohup env RUST_LOG=info ./target/debug/personal_agent_gpui > "$LOG" 2>&1 &
APP_PID=$!
sleep 3

echo "PID=$APP_PID" >> "$OUT"

if rg -q "Loaded legacy profile .*legacy_test_profile_1.json" "$LOG"; then
  pass "Legacy profile fixture #1 loaded"
else
  fail "Legacy profile fixture #1 was not loaded"
fi

if rg -q "Loaded legacy profile .*legacy_test_profile_2.json" "$LOG"; then
  pass "Legacy profile fixture #2 loaded"
else
  fail "Legacy profile fixture #2 was not loaded"
fi

if rg -q "ProfileService: loaded [0-9]+ profiles from disk" "$LOG"; then
  pass "ProfileService emitted total loaded count"
else
  fail "Missing ProfileService load count log"
fi

if rg -q "Skipping invalid profile" "$LOG"; then
  echo "NOTE: some profiles still skipped (expected for irrecoverable shapes)" | tee -a "$OUT"
fi

pkill -f "target/debug/personal_agent_gpui" 2>/dev/null || true

if rg -q "^FAIL:" "$OUT"; then
  echo "Profile load verification failed; see $OUT"
  exit 1
fi

echo "Profile load verification passed; see $OUT"
