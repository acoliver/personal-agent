# Issue #151 execution tracker

Plan: `PLAN-20260713-ISSUE151`
Branch: `issue151-selectable-markdown`

| Phase | Status | Started | Completed | Evidence |
|---|---|---|---|---|
| 0. Preflight and first-frame proof | Complete | 2026-07-13 | 2026-07-13 | `cargo test --lib selectable_leaf`: 3 first-frame live-layout GPUI tests passed |
| 1. Visible document and selection model | Complete | 2026-07-13 | 2026-07-13 | `cargo test --lib visible_document`: 87 UTF-8, range, link, block, and freshness tests passed |
| 2. Rich renderer metadata | Complete | 2026-07-13 | 2026-07-13 | Shared rich-tree builder retains styles, links, and live leaf layouts; selectable suite passed |
| 3. Pointer interaction and selection painting | Complete | 2026-07-13 | 2026-07-13 | `cargo test --lib selectable_markdown`: 20 pointer, selection, wrapping, and painting tests passed |
| 4. Links, keyboard, invalidation, context menu | Complete | 2026-07-13 | 2026-07-13 | `cargo test --lib chat_view`: 95 tests passed, including Linux Ctrl+C, stale-copy rejection, and cross-message clearing |
| 5. Deterministic app automation | Complete | 2026-07-13 | 2026-07-13 | Production X11 E2E selected/copied `ISSUE151_ALPHA heading`; keyboard/context copies matched; selection/menu screenshots changed by 14,489/5,530 pixels |
| 6. Full verification and PR | In progress | 2026-07-13 | - | fmt, strict all-target Clippy, guard, lizard, file-size gate, 1,008 lib tests, all 118 integration targets, selection E2E, and tray regression verification passed; coverage build timed out compiling dependencies; PR/CI pending |

## Acceptance evidence

| Requirement | In-process interaction test | Linux desktop E2E | macOS CI | Status |
|---|---|---|---|---|
| REQ-151-001 drag selection | First-frame forward/reverse and cross-leaf GPUI tests pass | Production first-attempt drag copied expected partial text | Same GPUI suite runs in CI | Complete locally |
| REQ-151-002 preserved rendering | Shared-tree style/layout invariants pass | Selection screenshot changed 14,489 pixels without renderer swap | Same GPUI suite runs in CI | Complete locally |
| REQ-151-003 copy | Selection-priority Cmd/Ctrl+C tests pass | Keyboard Copy returned `ISSUE151_ALPHA heading` | Same keyboard routing tests run in CI | Complete locally |
| REQ-151-004 multi-click | Unicode word and semantic-block click-count tests pass | Covered deterministically in-process | Same GPUI suite runs in CI | Complete locally |
| REQ-151-005 context menu | Right-click snapshot/menu action tests pass | Real menu changed 5,530 pixels and Copy matched | Same GPUI suite runs in CI | Complete locally |
| REQ-151-006 links | Safe-link click and link-drag arbitration tests pass | Renderer retains production link behavior | Same GPUI suite runs in CI | Complete locally |
| REQ-151-007 Unicode/freshness | UTF-8, emoji, revision, and conversation invalidation tests pass | Fixture and clipboard path verified | Same pure/GPUI suites run in CI | Complete locally |
| REQ-151-008 cross-platform automation | Platform shortcut routing and GPUI interaction tests pass | X11 production E2E passes | macOS workflow pending | Awaiting CI |

## Guardrails

- [x] Fresh branch from current `main`; stale remote `issue151` untouched.
- [x] Failed PR #159 reviewed; its architecture will not be reused.
- [x] GPL Zed Markdown implementation treated as API/concept reference only.
- [x] First-frame real-tree hit-testing proof passes.
- [x] No flat rendering branch or layout sink introduced.
- [x] All modified behavior is reachable through the shipped chat UI.
- [x] Local format, strict Clippy, guard, library, and integration verification passes.
- [x] Linux desktop E2E passes without human steps.
- [ ] macOS CI passes.
- [ ] PR checks and CodeRabbit are green.
