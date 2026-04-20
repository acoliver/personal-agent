# Phase 13: Final Verification

Plan ID: `PLAN-20260416-ISSUE173.P13`

## Prerequisites

- P12 verified PASS.

## Commands (paste exact output for each)

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --lib --tests
grep -rn "unimplemented!\\|todo!\\|// TODO\\|// FIXME\\|// HACK\\|placeholder\\|not yet implemented" src/ | grep -v "^$"   # MUST be empty
grep -rn "is_streaming: Arc<AtomicBool>" src/    # empty
grep -rn "active_streaming_target\b" src/ | grep -v "active_streaming_targets"   # empty
grep -rn "resolve_all(false)" src/services/chat_impl.rs   # empty
grep -rn "UserEvent::StopStreaming\b" src/ | grep -v "conversation_id"   # empty

# Plan marker audit
grep -rn "@plan PLAN-20260416-ISSUE173" src/ | wc -l
# Expected: ≥ 20 markers across service, event, presenter, llm, ui_gpui modules.

# Lizard (existing workflow)
./scripts/structural-check.sh 2>&1 | tail -20   # if present
./scripts/lizard.sh 2>&1 | tail -20             # if present
```

If the project has other standard checks (coverage, e2e), run them as
specified in `.github/workflows/*.yml` and record results.

## Verdict

PASS only when every command above passes cleanly. Otherwise FAIL + remediate.

Deliverable: `project-plans/issue173/plan/.completed/P13.md`.
