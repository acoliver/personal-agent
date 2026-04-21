# Phase 06: Approval Gate Scoped-Resolve TDD

Plan ID: `PLAN-20260416-ISSUE173.P06`

## Prerequisites

- P05 verified PASS.

## Requirements implemented (tests only)

- REQ-173-003.1, REQ-173-003.2

## Tasks

Add to the existing `src/llm/client_agent/tests.rs`:

```rust
/// @plan PLAN-20260416-ISSUE173.P06
/// @requirement REQ-173-003.2
#[tokio::test]
async fn resolve_all_for_conversation_resolves_only_target() {
    let gate = ApprovalGate::new();
    let conv_a = Uuid::new_v4();
    let conv_b = Uuid::new_v4();

    let waiter_a = gate.wait_for_approvals(
        "req-a".into(),
        vec!["tool".into()],
        conv_a,
    );
    let waiter_b = gate.wait_for_approvals(
        "req-b".into(),
        vec!["tool".into()],
        conv_b,
    );

    let resolved = gate.resolve_all_for_conversation(conv_a, false);
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].0, conv_a);

    // Waiter A was resolved with false
    assert_eq!(waiter_a.wait().await.unwrap(), false);

    // Waiter B is still pending — resolve it explicitly and check it was untouched
    // by the scoped resolve above.
    gate.resolve(&format!("req-b"), true);
    assert_eq!(waiter_b.wait().await.unwrap(), true);
}

/// @plan PLAN-20260416-ISSUE173.P06
/// @requirement REQ-173-003.1
#[tokio::test]
async fn resolving_one_conversation_does_not_wake_another() {
    let gate = ApprovalGate::new();
    let conv_a = Uuid::new_v4();
    let conv_b = Uuid::new_v4();

    let _waiter_a = gate.wait_for_approvals("req-a".into(), vec!["tool".into()], conv_a);
    let waiter_b   = gate.wait_for_approvals("req-b".into(), vec!["tool".into()], conv_b);

    gate.resolve_all_for_conversation(conv_a, false);

    // B's waiter should still be pending: use a timeout.
    let result = tokio::time::timeout(std::time::Duration::from_millis(50), waiter_b.wait()).await;
    assert!(result.is_err(), "waiter B should still be pending");
}
```

## Verification

- `cargo build --all-targets 2>&1 | tail -10` — expected failure: "no method
  named `resolve_all_for_conversation`".
- `grep -c "@plan PLAN-20260416-ISSUE173.P06" src/llm/`

Deliverable: `project-plans/issue173/plan/.completed/P06.md`.
