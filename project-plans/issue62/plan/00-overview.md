# Plan: Markdown Rendering for Assistant Messages (Phase A)

Plan ID: PLAN-20260402-MARKDOWN
Generated: 2026-04-02
Total Phases: 12 (plus preflight 0.5)
Requirements: REQ-MD-PARSE-001 through REQ-MD-PARSE-065, REQ-MD-RENDER-001 through REQ-MD-RENDER-053, REQ-MD-INTEGRATE-001 through REQ-MD-INTEGRATE-070, REQ-MD-STREAM-001 through REQ-MD-STREAM-061, REQ-MD-PERF-001 through REQ-MD-PERF-020, REQ-MD-SEC-001 through REQ-MD-SEC-021, REQ-MD-TEST-001 through REQ-MD-TEST-053
## Current State vs Target State

> **The current codebase has NO markdown rendering.** All architecture described
> in this plan is **TARGET STATE to be built**. The two-phase IR pipeline
> (`parse_markdown_blocks()` → `blocks_to_elements()`), the `markdown_content.rs`
> module, and the `AssistantBubble` delegation do not exist yet. Dependencies
> (`pulldown-cmark`, `url`) will be added in Phase 0.5/P03. Do not assume any
> plan-described API, type, or module is available until the phase that creates it
> has been completed and verified.


## Critical Reminders

Before implementing ANY phase, ensure you have:

1. Completed preflight verification (Phase 0.5)
2. Read and understood the two-phase IR architecture: `parse_markdown_blocks()` → `blocks_to_elements()`
3. Verified all GPUI APIs against the pinned rev (`c67328ab`)
4. Written integration tests BEFORE integration stubs/implementation (P09=TDD tests, P10=stub, P11=impl)
5. Verified all dependencies (`pulldown-cmark`, `url`) compile

## Lint Gate Sequencing Policy (Escalation A+C)

- Keep Clippy configuration unchanged (`cargo clippy --all-targets -- -D warnings`).
- Do **not** enforce strict Clippy as a pass/fail gate in **stub phases** (P03/P03a, P06/P06a, P10/P10a).
- For phases that intentionally coexist with later-phase stubs in the same module, defer strict Clippy to the first downstream phase where those stubs are replaced.
- For parser work specifically, strict Clippy enforcement is deferred from P05/P05a to P08a.

## Scope

This plan covers **Phase A only** — completed and streaming messages rendered through the two-phase IR model without mdstream. Phase B (streaming optimization with mdstream — **Conditional**, pending validation gate) will be planned separately after Phase A passes its validation gate and the mdstream dependency validation gate is executed.

Phase A is fully self-contained:
- Adds `pulldown-cmark` and `url` dependencies (NOT `mdstream`)
- Creates `markdown_content.rs` with `parse_markdown_blocks()` and `blocks_to_elements()`
- Modifies `AssistantBubble` to use the two-phase pipeline
- Refactors `render_assistant_message()` to delegate to `AssistantBubble`
- Streaming messages get markdown rendering via full re-parse each frame (O(n²) acceptable for Phase A)

## Architecture Summary

```
markdown text (&str)
  │
  ▼
parse_markdown_blocks(content) → Vec<MarkdownBlock>   [Phase 1: Pure Rust, no GPUI]
  │
  ▼
blocks_to_elements(&blocks) → Vec<AnyElement>          [Phase 2: GPUI element construction]
  │
  ▼
render_markdown(content) → Vec<AnyElement>             [Public API: composes both phases]
```

## Files Touched (Phase A)

| File | Change | Phase(s) |
|------|--------|----------|
| `Cargo.toml` | Add `pulldown-cmark = "0.13"` and `url = "2"` | P03 |
| `src/ui_gpui/components/markdown_content.rs` | **NEW** — IR model, parser, renderer, public API | P03–P08 |
| `src/ui_gpui/components/mod.rs` | Export `markdown_content` module | P09 |
| `src/ui_gpui/components/message_bubble.rs` | `AssistantBubble::into_element()` uses pipeline | P09–P11 |
| `src/ui_gpui/views/chat_view/render.rs` | `render_assistant_message()` delegates to `AssistantBubble` | P09–P11 |

## Phase Sequence

| Phase | ID | Title | Type |
|-------|----|-------|------|
| 0.5 | P0.5 | Preflight Verification | Verification |
| 01 | P01 | Domain Analysis | Analysis |
| 01a | P01a | Analysis Verification | Verification |
| 02 | P02 | Pseudocode | Design |
| 02a | P02a | Pseudocode Verification | Verification |
| 03 | P03 | Parser Stub | Stub (TDD cycle A) |
| 03a | P03a | Parser Stub Verification | Verification |
| 04 | P04 | Parser TDD | TDD (TDD cycle B) |
| 04a | P04a | Parser TDD Verification | Verification |
| 05 | P05 | Parser Implementation | Implementation (TDD cycle C) |
| 05a | P05a | Parser Implementation Verification | Verification |
| 06 | P06 | Renderer Stub | Stub (TDD cycle A) |
| 06a | P06a | Renderer Stub Verification | Verification |
| 07 | P07 | Renderer TDD | TDD (TDD cycle B) |
| 07a | P07a | Renderer TDD Verification | Verification |
| 08 | P08 | Renderer Implementation | Implementation (TDD cycle C) |
| 08a | P08a | Renderer Implementation Verification | Verification |
| 09 | P09 | Integration TDD | Integration TDD |
| 09a | P09a | Integration TDD Verification | Verification |
| 10 | P10 | Integration Stub | Integration Stub |
| 10a | P10a | Integration Stub Verification | Verification |
| 11 | P11 | Integration Implementation | Integration Implementation |
| 11a | P11a | Integration Implementation Verification | Verification |
| 12 | P12 | Cleanup | Cleanup |
| 12a | P12a | Cleanup Verification | Verification |

## Execution Tracker

| Phase | ID | Status | Started | Completed | Verified | Semantic? | Notes |
|-------|----|--------|---------|-----------|----------|-----------|-------|
| 0.5 | P0.5 | [ ] | - | - | - | N/A | Preflight verification |
| 01 | P01 | [ ] | - | - | - | N/A | Domain analysis |
| 01a | P01a | [ ] | - | - | - | N/A | Analysis verification |
| 02 | P02 | [ ] | - | - | - | N/A | Pseudocode |
| 02a | P02a | [ ] | - | - | - | N/A | Pseudocode verification |
| 03 | P03 | [ ] | - | - | - | [ ] | Parser stub |
| 03a | P03a | [ ] | - | - | - | [ ] | Parser stub verification |
| 04 | P04 | [ ] | - | - | - | [ ] | Parser TDD tests |
| 04a | P04a | [ ] | - | - | - | [ ] | Parser TDD verification |
| 05 | P05 | [ ] | - | - | - | [ ] | Parser implementation |
| 05a | P05a | [ ] | - | - | - | [ ] | Parser impl verification |
| 06 | P06 | [ ] | - | - | - | [ ] | Renderer stub |
| 06a | P06a | [ ] | - | - | - | [ ] | Renderer stub verification |
| 07 | P07 | [ ] | - | - | - | [ ] | Renderer TDD tests |
| 07a | P07a | [ ] | - | - | - | [ ] | Renderer TDD verification |
| 08 | P08 | [ ] | - | - | - | [ ] | Renderer implementation |
| 08a | P08a | [ ] | - | - | - | [ ] | Renderer impl verification |
| 09 | P09 | [ ] | - | - | - | [ ] | Integration TDD tests (BEFORE stub per PLAN.md) |
| 09a | P09a | [ ] | - | - | - | [ ] | Integration TDD verification |
| 10 | P10 | [ ] | - | - | - | [ ] | Integration stub |
| 10a | P10a | [ ] | - | - | - | [ ] | Integration stub verification |
| 11 | P11 | [ ] | - | - | - | [ ] | Integration implementation |
| 11a | P11a | [ ] | - | - | - | [ ] | Integration impl verification |
| 12 | P12 | [ ] | - | - | - | [ ] | Cleanup |
| 12a | P12a | [ ] | - | - | - | [ ] | Cleanup verification |

## Completion Markers

- [ ] All phases have `@plan:PLAN-20260402-MARKDOWN.P##` markers in code
- [ ] All requirements have `@requirement:REQ-MD-*` markers in code
- [ ] Verification script passes for each phase
- [ ] No phases skipped in sequence
- [ ] Phase A validation gate passed (§2.5 click event precedence verified)
