# Review of `project-plans/issue62/overview.md` (Issue #62)

## Critical Issues (MUST fix before implementation)

1. **Spec claims direct contradiction about intermediate representation vs direct builder path**
   - In §3.1 and §1/issue framing, the architecture is presented as a direct builder over pulldown-cmark events (three stacks, direct GPUI emission).
   - In §3.5 + §5.1, the spec explicitly switches to a two-phase model (`parse_markdown_blocks` IR then `blocks_to_elements`).
   - This is not just stylistic; it changes implementation/testing architecture and complexity assumptions.
   - **Why critical:** implementers can follow two incompatible designs.
   - **Fix:** make one approach normative and demote the other to rejected alternative (with rationale), then update all sections accordingly.

2. **Click handling plan is under-specified for actual current code hierarchy and likely conflict path**
   - Current code has bubble-level `.on_click()` for assistant message copy in `render_assistant_message()` (`src/ui_gpui/views/chat_view/render.rs`), and spec moves copy handler into `AssistantBubble` (`src/ui_gpui/components/message_bubble.rs`) where markdown links would live.
   - `InteractiveText` click behavior vs parent div bubbling is identified as a gate (§2.5), but no concrete implementation is selected.
   - **Why critical:** feature can ship with broken UX (link click also copies or vice versa).
   - **Fix:** pick one concrete strategy as normative now (not “ordered options”), with fallback only if verified test fails.

3. **Phase B is both “non-authoritative” and deeply normative throughout the document**
   - Top preconditions call Phase B tentative pending mdstream validation.
   - Yet §6 defines extensive normative state transition acceptance criteria and required reset behavior tied to assumed mdstream fields.
   - `Cargo.toml` currently has **no `mdstream` dependency**, matching the spec warning.
   - **Why critical:** document has conflicting authority; implementers cannot tell what is stable.
   - **Fix:** split into “authoritative now” (Phase A only) and “conditional draft” (Phase B), with clear go/no-go gate output and a short delta-update section to be filled after validation.

4. **Streaming completion semantics are partially accurate but operationally incomplete**
   - Verified: current `streaming_state_from_snapshot()` always sets `done: false`; completion is inferred by transition to `Idle` (`src/ui_gpui/views/chat_view/mod.rs`).
   - But current UI also sets `StreamingState::Idle` in multiple user actions (Escape, Stop button, Cmd+N, conversation switch, ConversationCleared path in command module).
   - Spec enumerates these, but does not resolve ordering with snapshot updates in a way that maps directly to existing `apply_store_snapshot()` control flow.
   - **Why critical:** risk of stale mdstream state or duplicate finalize/reset when moving between local UI state mutation and store-driven state.
   - **Fix:** define a single authoritative transition handler location (preferably in `apply_store_snapshot` + explicit local action hooks) and idempotency rules with exact pseudocode.

5. **GPUI verification section overstates confidence**
   - Spec marks GPUI APIs as “[VERIFIED — procedure provided]” but also says outputs are from prior checkout and must be re-run.
   - This is not true verification in the current repo state.
   - **Why critical:** false certainty around APIs like `InteractiveText`, `StyledText::with_runs`, and grid methods at pinned rev.
   - **Fix:** relabel as “verification procedure + expected signatures” until command outputs are captured in this repo context.

## Important Suggestions (SHOULD improve)

1. **Align spec with current code ownership boundaries**
   - Current assistant completed path is in `render.rs::render_assistant_message`, streaming path already uses `AssistantBubble` in `render_chat_area`, and `AssistantBubble` currently renders raw text.
   - Spec should include a migration sequence that avoids style regressions (width mismatch today: 300px in render.rs vs 400px in AssistantBubble).

2. **Add explicit perf guardrails for Phase A (no mdstream yet)**
   - Without mdstream, streaming will parse full content repeatedly.
   - Add explicit acceptance threshold for conversation sizes and a forced cutoff condition to trigger Phase B or minimal cache earlier.

3. **Security section should explicitly include URL scheme allowlist and clipboard behavior expectations**
   - Issue comments and spec discuss URL sanitization; ensure strict allowlist (`http`, `https`) is normative.
   - Clarify behavior for malformed URLs and very long URLs.

4. **HTML stripping approach needs caveats in requirements language**
   - State-machine stripping is pragmatic but not a full parser; edge cases around malformed tags/attributes should be documented as acceptable degradation.
   - This is especially relevant for correctness claims in fallback matrix.

5. **CodeRabbit coverage mapping is directionally good but needs explicit per-item closure evidence**
   - The issue has the 10-item CodeRabbit review comment.
   - Spec should include an explicit checklist table with each item, status (resolved/accepted/deferred/rejected), and section reference + rationale.

6. **Testing plan should better follow repo’s Rust testing conventions**
   - `dev-docs/RUST-RULES.md` strongly favors behavioral evidence.
   - The spec currently includes many structural assertions (“verify produced type/shape”).
   - Rebalance toward behavioral GPUI and lifecycle tests, using unit structural tests as support.

## Minor Notes (nice-to-have)

1. **Update wording on “user messages remain raw text”**
   - Good product decision; add one sentence clarifying this is independent of assistant markdown rollout.

2. **Tighten terminology around “committed blocks stabilize and don’t re-render”**
   - In GPUI, elements can still be rebuilt each frame by code path; what stabilizes is parsing workload/content semantics unless explicit cache exists.

3. **Clarify if thinking blocks are markdown-rendered in this phase**
   - Current code shows thinking as plain text; spec mostly scopes it out but references mixed rendering contexts.

4. **Keep examples synchronized with actual function signatures**
   - Sample snippets are close, but should be validated against exact imports/types used in this project to reduce copy/paste drift.

## Accuracy checks against current repo (summary)

- **Current rendering paths:** Accurate. Completed assistant path is raw text in `render.rs::render_assistant_message`; streaming path uses `AssistantBubble` and also renders raw text currently.
- **`AssistantBubble` current behavior:** Accurate that it appends `▋` when `is_streaming` and renders raw `.child(content_text)`.
- **`StreamingState` semantics:** Accurate that `done` is effectively unused and `streaming_state_from_snapshot` infers active streaming via `active_target` or non-empty buffer.
- **Dependencies:** Accurate that `mdstream` is not currently in `Cargo.toml`.
- **GPUI pin:** Accurate (`gpui` pinned to rev `c67328ab2e0d572718575e02ae07db37552e1cbe`).

## CodeRabbit Coverage Assessment (10 items)

Based on issue #62 comments, the spec addresses most major CodeRabbit concerns (mdstream uncertainty, URL sanitization mention, GPUI rev verification process, table streaming behavior, clippy complexity awareness, caching/perf discussion), but closure is incomplete in two areas:

- **Not fully closed:** concrete decision for click propagation conflict (link click vs bubble copy).
- **Not fully closed:** authoritative status split between Phase A and tentative Phase B.

## Verdict

**Needs revision before implementation.**

The spec is strong in breadth and codebase awareness, but it still contains high-impact contradictions and unresolved implementation gates. It is close to implementable after a focused revision pass that:

1) makes architecture choice unambiguous (direct builder vs IR),
2) resolves click precedence with one normative approach,
3) cleanly separates authoritative Phase A from conditional Phase B,
4) tightens transition ownership for streaming lifecycle integration.
