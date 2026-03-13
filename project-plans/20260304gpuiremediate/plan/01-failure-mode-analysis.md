# Phase 01: Prove Current Failure Mode

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P01`

## Prerequisites

- Required: Phase 00a completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P00a.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P00a.md`
- Expected files from previous phase:
  - `plan/00a-preflight-verification.md`
- Preflight verification: Phase 00a MUST be PASS before starting

## Requirements Implemented (Expanded)

### REQ-INT-001: Test-First Recovery

**Full Text**: The implementation plan MUST prove the current failure mode before code changes.

**Behavior**:
- GIVEN: the current GPUI architecture and diagnosis
- WHEN: this phase is executed
- THEN: evidence shows startup correctness is coming from bootstrap replay while manual selection depends on popup-coupled runtime delivery

**Why This Matters**: The implementation must target the real architecture seam, not a guessed data-loading bug.

### REQ-ARCH-005: MainPanel Responsibility Reduction

**Full Text**: `MainPanel` responsibilities must be explicitly separated in the implementation plan.

**Behavior**:
- GIVEN: current `MainPanel` responsibilities
- WHEN: this phase documents them
- THEN: the plan can reduce them deliberately instead of moving bugs around

**Why This Matters**: `MainPanel` is currently overloaded and obscures the true state owner.

## Implementation Tasks

### Files to Create or Validate

- `analysis/01-state-path-analysis.md`
  - Record or validate startup path, runtime path, failure seam, and cited source evidence
  - If the file already exists from pre-plan drafting, verify it still matches preflight evidence instead of rewriting it gratuitously

### Required Evidence

- Cite the current startup path from `src/main_gpui.rs` and `src/ui_gpui/views/main_panel.rs`
- Cite the current runtime selection path from `src/ui_gpui/views/chat_view.rs`, `src/ui_gpui/views/history_view.rs`, and `src/presentation/chat_presenter.rs`
- Cite `ChatView` handling that ignores `ConversationMessagesLoaded` for inactive state
- Explicitly list `MainPanel` overload responsibilities

## Verification Commands

```bash
grep -n "build_startup_view_commands\|startup_commands" src/main_gpui.rs src/ui_gpui/views/main_panel.rs
grep -n "apply_startup_commands\|ensure_bridge_polling" src/ui_gpui/views/main_panel.rs
grep -n "SelectConversation" src/ui_gpui/views/chat_view.rs src/ui_gpui/views/history_view.rs
grep -n "handle_select_conversation\|ConversationMessagesLoaded" src/presentation/chat_presenter.rs
grep -n "ConversationMessagesLoaded\|active_conversation_id" src/ui_gpui/views/chat_view.rs
```

## Semantic Verification Checklist

- [ ] Startup path and runtime path are both documented distinctly
- [ ] Failure explanation is architectural/integration-focused, not data-focused
- [ ] `MainPanel` overload is documented concretely
- [ ] The resulting analysis is sufficient to justify an authoritative store plan

## Success Criteria

- Analysis file proves the current failure mode with direct file citations
- No ambiguity remains about why startup works but manual selection fails

## Failure Recovery

- Re-read the cited files and tighten the analysis until the two delivery paths and failure seam are explicit
