# Phase 10: Integration Stub

## Phase ID

`PLAN-20260402-MARKDOWN.P10`

## Prerequisites

- Required: Phase 09a completed (integration TDD tests verified)
- Verification: `grep -r "@plan:PLAN-20260402-MARKDOWN.P09" src/`
- Expected files from previous phase: Integration tests in `message_bubble.rs` test module, all compiling
- Preflight verification: Phase 0.5 completed

## Why This Phase Comes After Integration TDD (P09)

Per PLAN.md rule "integration tests written BEFORE unit tests" and RUST-RULES.md requiring "failing behavioral tests first for production changes," the integration TDD tests in P09 establish behavioral guardrails. This phase now modifies production code in `message_bubble.rs` and `render.rs` with the integration wiring, knowing that P09 tests already define the expected behavior.

## Requirements Implemented (Expanded)

### REQ-MD-INTEGRATE-001: Single Canonical Rendering Owner

**Full Text**: The system shall route all assistant content — both completed messages and streaming messages — through a single canonical markdown rendering owner.
**Behavior**:
- GIVEN: An assistant message (completed or streaming)
- WHEN: Rendered in the chat view
- THEN: All rendering goes through `AssistantBubble::into_element()` → `parse_markdown_blocks()` → `blocks_to_elements()`
**Why This Matters**: Eliminates dual rendering paths, reduces bugs.

### REQ-MD-INTEGRATE-002: AssistantBubble Renders Markdown

**Full Text**: WHEN the assistant bubble renders, the system shall parse markdown from the content text and produce GPUI elements, replacing the current raw string rendering pattern.
**Behavior**:
- GIVEN: `AssistantBubble::new("**bold** text")`
- WHEN: `.into_element()` is called
- THEN: Renders bold text, not literal `**bold** text`
**Why This Matters**: Core feature delivery.

### REQ-MD-INTEGRATE-003: No New Public Fields

**Full Text**: The `AssistantBubble` struct shall gain no new public fields — its external API shall remain unchanged.
**Behavior**:
- GIVEN: Any existing caller of `AssistantBubble::new(content)`
- WHEN: Code is compiled after changes
- THEN: Compiles without modification
**Why This Matters**: Backward compatibility.

### REQ-MD-INTEGRATE-010: render_assistant_message Delegation

**Full Text**: The completed-message rendering path shall delegate to the canonical assistant bubble rather than building its own raw-text div.
**Behavior**:
- GIVEN: A completed ChatMessage
- WHEN: `render_assistant_message()` is called
- THEN: Creates an `AssistantBubble` and returns `bubble.into_any_element()`
**Why This Matters**: Eliminates the second rendering path.

### REQ-MD-INTEGRATE-020: Click-to-Copy (No Links)

**Full Text**: WHEN rendered markdown contains no links, the assistant bubble's outermost container div shall have a click handler that copies the raw markdown source to the clipboard, and pointer cursor style.
**Behavior**:
- GIVEN: Markdown with no links (e.g., `"**bold** text"`)
- WHEN: Bubble is rendered
- THEN: Container div has `.on_click()` → clipboard copy, `.cursor_pointer()`

### REQ-MD-INTEGRATE-021: No Click-to-Copy (Has Links)

**Full Text**: WHEN rendered markdown contains one or more links, the assistant bubble's outermost container div shall NOT have a click handler.
**Behavior**:
- GIVEN: Markdown with links (e.g., `"[click](https://example.com)"`)
- WHEN: Bubble is rendered
- THEN: Container div does NOT have `.on_click()`, no `.cursor_pointer()`

### REQ-MD-INTEGRATE-022: No Click During Streaming

**Full Text**: WHILE streaming is active, the assistant bubble shall NOT attach a bubble-level click handler.
**Behavior**:
- GIVEN: `AssistantBubble::new(content).streaming(true)`
- WHEN: Rendered
- THEN: No bubble-level `.on_click()`

### REQ-MD-INTEGRATE-040: Streaming Cursor

**Full Text**: WHILE streaming is active, the assistant bubble shall append the streaming cursor character (`▋`) to the content string before passing it to markdown parsing.
**Behavior**:
- GIVEN: Streaming with content `"Hello"`
- WHEN: Rendered
- THEN: Parses `"Hello▋"` and renders with cursor at end

### REQ-MD-INTEGRATE-015: Model Label Fallback

**Full Text**: WHEN the model identifier for an assistant message is absent, the system shall render the model label as `"Assistant"`.
**Behavior**:
- GIVEN: `msg.model_id == None`
- WHEN: `render_assistant_message()` delegates to AssistantBubble
- THEN: Model label shows "Assistant"

## Implementation Tasks

### Files to Modify

#### `src/ui_gpui/components/message_bubble.rs`

- MODIFY the `AssistantBubble` `IntoElement` implementation, specifically the `into_element()` method:
  - Add import: `use super::markdown_content::{parse_markdown_blocks, blocks_to_elements};`
  - In `into_element()`, replace the `.child(content_text)` raw string rendering with the markdown pipeline
  - Add link detection for click-to-copy behavior
  - Add conditional `.on_click()` handler
  - Preserve streaming cursor behavior
  - Add `/// @plan:PLAN-20260402-MARKDOWN.P10`
  - Add `/// @requirement:REQ-MD-INTEGRATE-002`

  **Stub approach for this phase**: Wire in the calls but keep changes minimal. The actual behavior is tested by P09 tests and refined in P11.

  ```rust
  // In AssistantBubble::into_element() — replaces .child(content_text) with markdown
  let blocks = parse_markdown_blocks(&content_text);
  let has_links = has_any_links(&blocks);
  let elements = blocks_to_elements(&blocks);
  // ... use elements as children, conditionally attach on_click
  ```

#### `src/ui_gpui/views/chat_view/render.rs`

- MODIFY the `render_assistant_message()` function:
  - Replace the entire function body with `AssistantBubble` delegation
  - Add `/// @plan:PLAN-20260402-MARKDOWN.P10`
  - Add `/// @requirement:REQ-MD-INTEGRATE-010`

  **Stub**: Change the function to construct an `AssistantBubble` and return its element.

  ```rust
  pub(super) fn render_assistant_message(msg: &ChatMessage, show_thinking: bool) -> gpui::AnyElement {
      let mut bubble = AssistantBubble::new(msg.content.clone());
      if let Some(ref model_id) = msg.model_id {
          bubble = bubble.model_id(model_id.clone());
      } else {
          bubble = bubble.model_id("Assistant");
      }
      if show_thinking {
          if let Some(ref thinking) = msg.thinking {
              bubble = bubble.thinking(thinking.clone()).show_thinking(true);
          }
      }
      bubble.into_any_element()
  }
  ```

#### `src/ui_gpui/components/mod.rs`

- ADD export for `markdown_content` module (if not already added in P03)

### Required Code Markers

```rust
/// @plan:PLAN-20260402-MARKDOWN.P10
/// @requirement:REQ-MD-INTEGRATE-002
impl IntoElement for AssistantBubble {
    // ...
}
```

## Verification Commands

### Automated Checks (Structural)

```bash
# 1. P10 markers present
grep -c "@plan:PLAN-20260402-MARKDOWN.P10" src/ui_gpui/components/message_bubble.rs
# Expected: 2+

grep -c "@plan:PLAN-20260402-MARKDOWN.P10" src/ui_gpui/views/chat_view/render.rs
# Expected: 1+

# 2. Compiles
cargo build || exit 1

# 2a. Clippy snapshot (non-gating in stub phases)
cargo clippy --all-targets -- -D warnings || true

# 3. All existing tests pass (parser + renderer + project)
cargo test --lib || exit 1

# 4. No hardcoded colors in new code
grep -rn "rgb(0x" src/ui_gpui/components/message_bubble.rs | grep -v "// " && echo "FAIL: hardcoded colors"

# 5. AssistantBubble API unchanged (no new public fields)
grep -c "pub " src/ui_gpui/components/message_bubble.rs
# Should match pre-change count (verify manually)

# 6. render_assistant_message delegates to AssistantBubble
grep "AssistantBubble::new\|AssistantBubble" src/ui_gpui/views/chat_view/render.rs
# Expected: Match found
```

### Structural Verification Checklist

- [ ] P09 markers (integration tests) still intact
- [ ] P10 markers in message_bubble.rs and render.rs
- [ ] `cargo build` succeeds
- [ ] Legacy suites (pre-P09) pass; P09 behavioral integration tests are expected to remain mostly failing until P11
- [ ] `AssistantBubble::into_element()` calls `parse_markdown_blocks()` + `blocks_to_elements()`
- [ ] `render_assistant_message()` creates and returns `AssistantBubble`
- [ ] No new public fields on `AssistantBubble`
- [ ] Streaming cursor still appended when `is_streaming`
- [ ] Link detection code present in `into_element()`

### Semantic Verification Checklist

1. **Is the feature reachable?**
   - [ ] `render_assistant_message()` → `AssistantBubble::into_element()` → `parse_markdown_blocks()` → `blocks_to_elements()` — full call chain verified
   - [ ] Streaming path (already used AssistantBubble) now gets markdown rendering too

2. **Is the old code replaced?**
   - [ ] `render_assistant_message()` no longer builds its own raw-text div
   - [ ] `AssistantBubble::into_element()` no longer uses `.child(content_text)` with raw string

3. **Is backward compatibility preserved?**
   - [ ] `AssistantBubble::new("content")` still works
   - [ ] `.model_id()`, `.thinking()`, `.show_thinking()`, `.streaming()` builders unchanged
   - [ ] Streaming path in `render_chat_area()` unchanged (already uses AssistantBubble)

4. **What is the expected P09 test state after P10?**
   - [ ] Some P09 tests may start passing now that basic wiring is in place (e.g., markdown rendering produces elements)
   - [ ] **Most P09 behavioral tests are still expected to FAIL** — the stub wires in the call chain but does not yet implement full click-to-copy logic, link detection, or streaming cursor behavior. P11 makes these pass.

## Success Criteria

- Project compiles
- All pre-P09 tests pass (parser, renderer, isolation)
- Integration wiring stub is in place (call chain connected)
- Old rendering paths replaced with AssistantBubble delegation
- AssistantBubble public API unchanged
- **P09 integration tests: most still FAIL** — stubs connect the pipeline but do not yet implement conditional click-to-copy, recursive link detection, or clipboard behavior. This is expected. P11 completes the implementation and makes all P09 tests pass.

## Failure Recovery

If this phase fails:
1. Rollback: `git checkout -- src/ui_gpui/components/message_bubble.rs src/ui_gpui/views/chat_view/render.rs`
2. Re-attempt with corrected integration approach
3. Cannot proceed to Phase 11 until fixed

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P10.md`
