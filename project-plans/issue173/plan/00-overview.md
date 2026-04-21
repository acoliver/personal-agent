# Plan Overview: Concurrent Conversations

Plan ID: `PLAN-20260416-ISSUE173`
Issue: #173
Branch: `issue173`

## Execution order (sequential, no skipping)

| Phase | Name | Kind |
|-------|------|------|
| P01   | Preflight verification | preflight |
| P02   | Service layer: TDD (failing tests first) | tests-first |
| P02a  | Service layer TDD verification | verification |
| P03   | Service layer: implementation (active_streams map + per-conv guard) | impl |
| P03a  | Service layer impl verification | verification |
| P04   | Cancel pipeline: TDD (event + presenter + service) | tests-first |
| P04a  | Cancel pipeline TDD verification | verification |
| P05   | Cancel pipeline: implementation | impl |
| P05a  | Cancel pipeline impl verification | verification |
| P06   | Approval gate scoped-resolve: TDD | tests-first |
| P06a  | Approval gate TDD verification | verification |
| P07   | Approval gate scoped-resolve: implementation | impl |
| P07a  | Approval gate impl verification | verification |
| P08   | Store `active_streaming_targets` set: TDD | tests-first |
| P08a  | Store set TDD verification | verification |
| P09   | Store `active_streaming_targets` set: implementation | impl |
| P09a  | Store set impl verification | verification |
| P10   | Sidebar indicator + history snapshot: TDD | tests-first |
| P10a  | Sidebar TDD verification | verification |
| P11   | Sidebar indicator + history snapshot: implementation | impl |
| P11a  | Sidebar impl verification | verification |
| P12   | End-to-end integration test (three concurrent streams) | integration |
| P12a  | Integration verification | verification |
| P13   | Final verification: fmt/clippy/tests/structural/lizard | verification |

Tests for each layer are written **before** the corresponding implementation.
Every implementation phase is followed by a skeptical verification phase per
`dev-docs/COORDINATING.md`.

## Traceability marker

Every new/modified item in this plan MUST carry:

```rust
/// @plan PLAN-20260416-ISSUE173.P##
/// @requirement REQ-173-###.#
```
