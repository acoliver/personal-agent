# Phase 07: Approval Gate Impl

Plan ID: `PLAN-20260416-ISSUE173.P07`

## Prerequisites

- P06 verified PASS.

## Requirements implemented

- REQ-173-003.1, REQ-173-003.2, REQ-173-003.3

## Tasks

### `src/llm/client_agent.rs`

Add:

```rust
/// Resolve all pending approvals owned by `conversation_id`.
///
/// @plan PLAN-20260416-ISSUE173.P07
/// @requirement REQ-173-003.1
/// @requirement REQ-173-003.2
#[must_use]
pub fn resolve_all_for_conversation(
    &self,
    conversation_id: Uuid,
    approved: bool,
) -> Vec<(Uuid, String)> {
    let matching = {
        let mut pending = self.pending.lock().unwrap();
        let keys: Vec<String> = pending
            .iter()
            .filter(|(_, p)| p.conversation_id == conversation_id)
            .map(|(k, _)| k.clone())
            .collect();
        keys.into_iter()
            .filter_map(|k| pending.remove(&k).map(|p| (k, p)))
            .collect::<Vec<_>>()
    };

    let mut resolved = Vec::with_capacity(matching.len());
    for (request_id, pending_approval) in matching {
        let _ = pending_approval.tx.send(approved);
        resolved.push((pending_approval.conversation_id, request_id));
    }
    resolved
}
```

### `src/services/chat_impl.rs` — `cancel_active_stream`

Replace:
```rust
let resolved_requests = self.approval_gate.resolve_all(false);
```
with:
```rust
let resolved_requests = self
    .approval_gate
    .resolve_all_for_conversation(conversation_id, false);
```

Everything else in that loop stays the same.

### Mark `resolve_all` for non-cancel use only

`resolve_all` is still legitimate for app shutdown paths. Do not remove it.
Just stop calling it from `cancel_active_stream`.

## Verification

```bash
cargo build --all-targets 2>&1 | tail -10
cargo test --lib client_agent 2>&1 | tail -30
cargo test --lib --tests 2>&1 | grep -E "test result|FAILED" | tail -10
grep -rn "resolve_all(false)" src/services/chat_impl.rs       # MUST be empty
grep -rn "unimplemented!\\|todo!\\|// TODO\\|placeholder" src/llm/ src/services/
grep -c "@plan PLAN-20260416-ISSUE173.P07" src/llm/client_agent.rs src/services/chat_impl.rs
```

Deliverable: `project-plans/issue173/plan/.completed/P07.md` with full command outputs.
