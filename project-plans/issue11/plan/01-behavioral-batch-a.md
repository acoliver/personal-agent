# Phase 01: Behavioral Coverage Batch A

## Phase ID

`PLAN-20260325-ISSUE11.P01`

## Prerequisites

- Required: Phase `P00a` completed with PASS
- Verification: `grep "^## Verdict: PASS" project-plans/issue11/plan/.completed/P00A.md`
- Expected files from previous phase: `project-plans/issue11/plan/.completed/P00.md`, `project-plans/issue11/plan/.completed/P00A.md`

## Requirements Implemented (Expanded)

### REQ-COV-001: Coverage gate reaches at least 80% line coverage

**Full Text**: The project must reach the enforced 80% workspace line coverage gate and do so using meaningful behavioral tests.
**Behavior**:
- GIVEN the baseline is far below the gate
- WHEN the first implementation batch is executed
- THEN it adds high-yield behavioral tests around major product behavior paths

**Why This Matters**: The first batch should materially move coverage rather than nibbling at low-value lines.

### REQ-COV-002: New tests are behavioral per goodtests.md

**Full Text**: Added tests must be behavioral, externally meaningful, and consistent with `dev-docs/goodtests.md`.
**Behavior**:
- GIVEN existing mixed-quality tests
- WHEN Batch A adds or updates tests
- THEN each test proves a meaningful external behavior, persistence result, or state transition

**Why This Matters**: The issue is about confidence, not vanity metrics.

### REQ-COV-003: Avoid structural/mock-theater coverage padding

**Full Text**: Coverage work must avoid trivial structural tests and mock-driven theater that does not materially validate behavior.
**Behavior**:
- GIVEN tempting low-effort coverage-chasing options
- WHEN Batch A is implemented
- THEN weak patterns are avoided or removed, and strong patterns are favored

**Why This Matters**: Weak coverage would satisfy the letter of the gate without improving release readiness.

## Implementation Tasks

### Batch A target areas

Prioritize the strongest behavioral additions in these areas first:
- `src/services/chat_impl.rs`
- `src/presentation/chat_presenter.rs`
- `src/mcp/runtime.rs`
- `src/services/conversation_impl.rs`
- `src/services/profile_impl.rs`

### Expected test styles

Prefer tests such as:
- real temp-dir persistence and reload flows
- meaningful presenter command flows and error-state outcomes
- chat streaming / failure / fallback behavior at service boundaries
- MCP runtime lifecycle and failure cleanup behavior
- profile default-resolution, persistence, and recovery behavior

### Disallowed test styles

Do NOT add tests that are mainly:
- enum/display tours
- constructor/getter/setter identity checks
- pure source-text greps as evidence of correctness
- call-count assertions without meaningful external outcomes
- fake-value-forwarding through mocked layers as the main proof

### Files to Modify

- existing `tests/*.rs` files where strong behavioral suites already exist
- new `tests/*.rs` files only if they cover a coherent user-visible behavior cluster
- production files only if a small enabling refactor is required for a strong test seam

### Required Code Markers

Any new tests or enabling production seams created in this phase MUST include:

```rust
/// @plan PLAN-20260325-ISSUE11.P01
/// @requirement REQ-COV-001
```

Add `REQ-COV-002` and `REQ-COV-003` markers where appropriate.

## Verification Commands

```bash
cargo test --lib --tests
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

## Behavioral Review Requirement

Before this phase can be considered complete, review every added/changed test against `dev-docs/goodtests.md` and record for each suite:
- what user-visible or persistence-visible behavior it proves
- why it is not structural padding
- what real regression it would catch

## Success Criteria

- A meaningful first batch of behavioral tests is added
- Tests pass locally
- No obvious low-value/mock-theater additions remain in the changed set
- Evidence file explains why the added tests are good tests

## Failure Recovery

If this phase fails:
1. remove or rewrite weak tests
2. prefer strengthening existing behavioral suites over adding more small tests
3. do not proceed until P01a verifies the tests are strong enough
