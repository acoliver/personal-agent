# Phase 09: Integration TDD Tests

## Phase ID

`PLAN-20260402-MARKDOWN.P09`

## Prerequisites

- Required: Phase 08a completed (renderer implementation verified)
- Verification: `grep -r "@plan:PLAN-20260402-MARKDOWN.P08" src/ui_gpui/components/markdown_content.rs`
- Expected files from previous phase: `markdown_content.rs` fully implemented with all tests passing
- Preflight verification: Phase 0.5 completed

## Integration Analysis (MANDATORY per PLAN.md)

### Why integration tests come FIRST

Per PLAN.md and PLAN-TEMPLATE.md, integration tests MUST be written BEFORE production integration code. This ensures behavioral guardrails exist before any wiring changes touch `message_bubble.rs` or `render.rs`. The integration stub (P10) and implementation (P11) are only allowed to modify production code after these tests exist.

### What existing code will USE this feature?

1. **`src/ui_gpui/components/message_bubble.rs`** — `AssistantBubble::into_element()` will call `parse_markdown_blocks()` + `blocks_to_elements()` to render markdown instead of raw text. It will inspect the IR for link presence to determine click-to-copy behavior.

2. **`src/ui_gpui/views/chat_view/render.rs`** — `render_assistant_message()` will delegate to `AssistantBubble` instead of building its own raw-text div.

### What existing code needs to be REPLACED?

1. **In `AssistantBubble::into_element()`**: The `.child(content_text)` raw string rendering will be replaced by `.children(blocks_to_elements(&blocks))`
2. **In `render_assistant_message()`**: The entire function body will be replaced with `AssistantBubble` delegation

### How will users ACCESS this feature?

- Any assistant message in the chat view — completed or streaming — will render markdown automatically through the unified `AssistantBubble` rendering path.

### What needs to be MIGRATED?

- No data migration needed. Messages are stored as raw markdown strings; rendering is purely a view concern.

## Requirements Implemented (Expanded)

This phase writes behavioral integration tests that verify the markdown rendering feature works IN CONTEXT — not just in isolation.

### REQ-MD-INTEGRATE-002: AssistantBubble Renders Markdown

**Full Text**: WHEN the assistant bubble renders, the system shall parse markdown from the content text and produce GPUI elements, replacing the current raw string rendering pattern.
**Behavior**:
- GIVEN: `AssistantBubble::new("**bold** text")`
- WHEN: `.into_any_element()` is called
- THEN: Produces GPUI elements containing styled text with BOLD weight for "bold" (not literal `**bold**`)
**Why This Matters**: End-to-end verification that the feature works through the actual component.

### REQ-MD-INTEGRATE-020: Click-to-Copy (No Links)

**Full Text**: WHEN rendered markdown contains no links, the assistant bubble's outermost container div shall have a click handler that copies the raw markdown source to the clipboard, and pointer cursor style.
**Behavior**:
- GIVEN: `AssistantBubble::new("plain text")` (no links)
- WHEN: Rendered (non-streaming) and clicked
- THEN: The bubble's container has cursor_pointer style AND click handler copies raw markdown source to clipboard (verified via ClipboardItem)
**Why This Matters**: Preserves existing click-to-copy UX.

### REQ-MD-INTEGRATE-021: No Click-to-Copy (Has Links)

**Full Text**: WHEN rendered markdown contains one or more links, the assistant bubble shall NOT have a bubble-level click handler.
**Behavior**:
- GIVEN: `AssistantBubble::new("[link](https://example.com)")`
- WHEN: Rendered (non-streaming)
- THEN: No bubble-level click handler, no cursor_pointer style; links produce InteractiveText elements that handle their own click events
**Why This Matters**: Prevents click conflict between link navigation and copy.

### REQ-MD-INTEGRATE-022: No Click During Streaming

**Full Text**: WHILE streaming is active, the assistant bubble shall NOT attach a bubble-level click handler regardless of link content.
**Behavior**:
- GIVEN: `AssistantBubble::new("text").streaming(true)`
- WHEN: Rendered
- THEN: No bubble-level click handler

### REQ-MD-INTEGRATE-030: User Messages Stay Raw

**Full Text**: The system shall render user messages as raw text with no markdown processing.
**Behavior**:
- GIVEN: A `UserBubble::new("**not bold**")`
- WHEN: Rendered
- THEN: Shows literal `**not bold**` as plain text — no BOLD font weight, no parsed markdown structure, no call to `parse_markdown_blocks()`
**Why This Matters**: Ensures user messages don't accidentally get markdown treatment.

### REQ-MD-INTEGRATE-040: Streaming Cursor

**Full Text**: WHILE streaming is active, the assistant bubble shall append the streaming cursor character to the content string before passing it to markdown parsing.
**Behavior**:
- GIVEN: `AssistantBubble::new("Hello").streaming(true)`
- WHEN: Rendered
- THEN: Content includes cursor `▋` at end

### REQ-MD-INTEGRATE-041: Cursor Never in Persisted Content

**Full Text**: The streaming cursor character never appears in committed or persisted content.
**Behavior**:
- GIVEN: Any non-streaming AssistantBubble
- WHEN: Rendered
- THEN: No cursor character in content

### REQ-MD-INTEGRATE-050 / -051: Store/Presenter Independence

**Full Text**: The markdown rendering implementation shall not modify the store or presenter layers.
**Behavior**:
- GIVEN: The store and presenter source files
- WHEN: Inspected
- THEN: No markdown-related imports or changes

### REQ-MD-INTEGRATE-070: Phase Isolation Guard

**Full Text**: WHILE only Phase A is deployed, no production code path shall reference the `mdstream` crate.
**Behavior**:
- GIVEN: The project codebase
- WHEN: Searched for mdstream references
- THEN: No references found in production code

## Implementation Tasks

### Files to Modify

#### Create integration test section in `src/ui_gpui/components/message_bubble.rs` (test module)

OR add tests to a separate test file depending on project convention. Since the project uses `#[cfg(test)] mod tests` within source files, add integration tests there.

**NOTE**: At this stage, production code in `message_bubble.rs` and `render.rs` has NOT been modified yet. Tests that exercise the integration wiring (e.g., asserting that AssistantBubble calls parse_markdown_blocks) will fail — that's expected and correct per TDD. Tests that assert isolation guarantees (store/presenter/mdstream) and user bubble behavior will pass immediately since they verify things that should NOT change.

### Test Categories (15+ tests)

#### AssistantBubble Integration Tests (6+ tests, `#[gpui::test]`)

```rust
/// @plan:PLAN-20260402-MARKDOWN.P09
/// @requirement:REQ-MD-INTEGRATE-002
/// @scenario AssistantBubble renders markdown content with correct styling
/// @given AssistantBubble with markdown text "**bold** and *italic*"
/// @when into_any_element() is called
/// @then Bold text produces elements with BOLD font weight; italic text produces
///       elements with ITALIC style (not literal asterisks)
#[gpui::test]
fn test_assistant_bubble_renders_markdown(cx: &mut gpui::TestAppContext) { ... }

/// @requirement:REQ-MD-INTEGRATE-020
/// @scenario Click-to-copy attached for link-free messages and copies raw content
/// @given AssistantBubble with "plain text", no links, not streaming
/// @when Rendered and click handler is invoked
/// @then Bubble has cursor_pointer style AND clicking copies raw markdown source
///       to clipboard (verified by ClipboardItem content)
#[gpui::test]
fn test_bubble_click_to_copy_no_links(cx: &mut gpui::TestAppContext) { ... }

/// @requirement:REQ-MD-INTEGRATE-021
/// @scenario Click-to-copy NOT attached for messages with links; links produce InteractiveText
/// @given AssistantBubble with link "[click](https://example.com)"
/// @when Rendered
/// @then Bubble does NOT have cursor_pointer style, no bubble-level on_click handler,
///       AND the rendered output contains InteractiveText elements for the link
#[gpui::test]
fn test_bubble_no_click_when_has_links(cx: &mut gpui::TestAppContext) { ... }

/// @requirement:REQ-MD-INTEGRATE-022
/// @scenario No click during streaming
/// @given AssistantBubble with streaming(true)
/// @when Rendered
/// @then No bubble-level click handler
#[gpui::test]
fn test_bubble_no_click_during_streaming(cx: &mut gpui::TestAppContext) { ... }

/// @requirement:REQ-MD-INTEGRATE-040
/// @scenario Streaming cursor appended
/// @given AssistantBubble with "Hello" and streaming(true)
/// @when Rendered
/// @then Content string passed to parser includes cursor
#[gpui::test]
fn test_streaming_cursor_appended(cx: &mut gpui::TestAppContext) { ... }

/// @requirement:REQ-MD-INTEGRATE-003
/// @scenario AssistantBubble API unchanged
/// @given Standard builder chain: new().model_id().thinking().show_thinking().streaming()
/// @when Compiled
/// @then All builder methods still work, no new required fields
#[gpui::test]
fn test_assistant_bubble_api_unchanged(cx: &mut gpui::TestAppContext) { ... }
```

#### render_assistant_message Tests (3+ tests)

```rust
/// @requirement:REQ-MD-INTEGRATE-010
/// @scenario render_assistant_message delegates to AssistantBubble
/// @given A ChatMessage with content
/// @when render_assistant_message() is called
/// @then Produces a valid element (not a raw string child)
#[gpui::test]
fn test_render_assistant_message_delegates(cx: &mut gpui::TestAppContext) { ... }

/// @requirement:REQ-MD-INTEGRATE-015
/// @scenario Model label fallback
/// @given ChatMessage with model_id = None
/// @when render_assistant_message() called
/// @then Model label shows "Assistant"
#[gpui::test]
fn test_render_assistant_message_model_fallback(cx: &mut gpui::TestAppContext) { ... }

/// @requirement:REQ-MD-INTEGRATE-011
/// @scenario Thinking content passed through
/// @given ChatMessage with thinking content and show_thinking=true
/// @when render_assistant_message() called
/// @then AssistantBubble receives thinking content
#[gpui::test]
fn test_render_assistant_message_with_thinking(cx: &mut gpui::TestAppContext) { ... }
```

#### User Message Non-Regression (1 test)

```rust
/// @requirement:REQ-MD-INTEGRATE-030
/// @scenario User messages stay raw — no markdown processing applied
/// @given UserBubble with markdown-looking content "**not bold**"
/// @when Rendered
/// @then Output contains literal "**not bold**" as plain text (no BOLD styling,
///       no parsed markdown elements — verified by checking the element tree does
///       NOT call parse_markdown_blocks)
#[gpui::test]
fn test_user_bubble_stays_raw(cx: &mut gpui::TestAppContext) { ... }
```

#### Boundary / Isolation Tests (3+ tests, `#[test]`)

```rust
/// @requirement:REQ-MD-INTEGRATE-050
/// @scenario Store layer not modified
/// @given The store source files
/// @when Searched for markdown-related changes
/// @then No markdown imports or references
#[test]
fn test_store_layer_not_modified() {
    // Read store source and verify no markdown_content imports
    let app_store = include_str!("../../app_store.rs");
    assert!(!app_store.contains("markdown_content"), "Store should not reference markdown");
    assert!(!app_store.contains("parse_markdown_blocks"), "Store should not reference parser");
}

/// @requirement:REQ-MD-INTEGRATE-051
/// @scenario Presenter layer not modified
#[test]
fn test_presenter_layer_not_modified() { ... }

/// @requirement:REQ-MD-INTEGRATE-070
/// @scenario No mdstream references in production code
#[test]
fn test_no_mdstream_references() {
    // Verify Cargo.toml does not include mdstream
    let cargo_toml = include_str!("../../../Cargo.toml");
    assert!(!cargo_toml.contains("mdstream"), "Phase A must not depend on mdstream");
}
```

#### Edge Case Integration Tests (2+ tests)

```rust
/// @requirement:REQ-MD-INTEGRATE-002, REQ-MD-RENDER-041
/// @scenario Empty content renders without panic
#[gpui::test]
fn test_bubble_empty_content(cx: &mut gpui::TestAppContext) { ... }

/// @requirement:REQ-MD-INTEGRATE-041
/// @scenario Cursor character never in non-streaming content
#[test]
fn test_cursor_not_in_completed_messages() {
    let content = "Hello world";
    assert!(!content.contains('▋'), "Completed content should not have cursor");
}
```

### Required Code Markers

Every test MUST include:

```rust
/// @plan:PLAN-20260402-MARKDOWN.P09
/// @requirement:REQ-MD-INTEGRATE-XXX
```

## Verification Commands

### Automated Checks (Structural)

```bash
# Check P09 markers
grep -r "@plan:PLAN-20260402-MARKDOWN.P09" src/ | wc -l
# Expected: 15+

# Check integration requirement markers
grep -r "@requirement:REQ-MD-INTEGRATE" src/ | wc -l
# Expected: 15+

# Compile with test targets
cargo build --all-targets || exit 1

# Previous tests still pass
cargo test --lib -- markdown_content::tests || exit 1

# Integration tests — some will fail (integration not yet wired), some will pass (isolation tests)
cargo test --lib -- message_bubble::tests 2>&1 | tail -10
# Expected: Mixed pass/fail — isolation tests pass, wiring tests fail
```

### Structural Verification Checklist

- [ ] Previous phase markers intact (P03–P08)
- [ ] P09 markers on all new tests
- [ ] 15+ integration tests created
- [ ] Tests use `#[gpui::test]` where GPUI context needed
- [ ] Tests compile
- [ ] No `#[should_panic]` annotations
- [ ] Store/presenter isolation tests present
- [ ] mdstream isolation test present
- [ ] Click-to-copy behavior tests present

### Semantic Verification Checklist

1. **Do tests verify integration, not just unit behavior?**
   - [ ] Tests exercise `AssistantBubble::into_element()` end-to-end
   - [ ] Tests verify click-to-copy conditional behavior
   - [ ] Tests verify `render_assistant_message()` delegation
   - [ ] Tests verify user messages are NOT affected

2. **Do tests cover the click event precedence gate (§2.5)?**
   - [ ] No-link message: click-to-copy attached
   - [ ] Link message: click-to-copy NOT attached
   - [ ] Streaming: no click-to-copy regardless

3. **Do tests verify isolation guarantees?**
   - [ ] Store layer clean
   - [ ] Presenter layer clean
   - [ ] No mdstream dependency

## Success Criteria

- 15+ integration tests created
- Tests compile
- Previous tests unbroken
- Click event precedence gate tests present
- Isolation tests present and PASSING (they verify things that should NOT change)
- **Integration wiring tests MUST FAIL at this point** — production code has not been modified yet, so tests exercising `AssistantBubble` markdown rendering, click-to-copy behavior, `render_assistant_message` delegation, and streaming cursor will fail. This is correct TDD: the tests define the target behavior before any production code changes.
- Specifically expected to PASS: isolation tests (store/presenter clean, no mdstream), user bubble raw text, API compatibility
- Specifically expected to FAIL: all tests that assert markdown rendering output, click handler presence/absence, InteractiveText for links, clipboard copy behavior, streaming cursor content

## Failure Recovery

If this phase fails:
1. Rollback new integration tests only
2. Re-attempt with corrected test approach
3. Cannot proceed to Phase 10 until tests are correct

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P09.md`
