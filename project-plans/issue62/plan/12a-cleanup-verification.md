# Phase 12a: Cleanup Verification (Final Gate)

## Phase ID

`PLAN-20260402-MARKDOWN.P12a`

## Prerequisites

- Required: Phase 12 completed
- Verification: `grep -r "@plan:PLAN-20260402-MARKDOWN.P12" src/`
- Expected: All cleanup done, full verification suite passing

## This is the FINAL GATE

Phase 12a is the last verification before the feature is considered complete for Phase A. If this gate passes, the feature is ready for PR creation.

## Verification Commands

### Complete Verification Suite

```bash
# 1. Format
cargo fmt --all -- --check || exit 1

# 2. Lint
cargo clippy --all-targets -- -D warnings || exit 1

# 3. Test (full project)
cargo test --lib --tests || exit 1

# 4. Build (full project)
cargo build || exit 1

echo "=== FULL VERIFICATION SUITE PASSED ==="
```

### Plan Marker Completeness

```bash
# Every phase must have markers in the codebase
for phase in P03 P04 P05 P06 P07 P08 P09 P10 P11 P12; do
    count=$(grep -r "@plan:PLAN-20260402-MARKDOWN.$phase" src/ 2>/dev/null | wc -l | tr -d ' ')
    if [ "$count" -eq "0" ]; then
        echo "FAIL: No markers for $phase"
    else
        echo "PASS: $phase has $count markers"
    fi
done
```

### Requirement Completeness

```bash
# Parse requirements
grep -r "@requirement:REQ-MD-PARSE" src/ | wc -l
# Expected: 30+

# Render requirements
grep -r "@requirement:REQ-MD-RENDER" src/ | wc -l
# Expected: 12+

# Integration requirements
grep -r "@requirement:REQ-MD-INTEGRATE" src/ | wc -l
# Expected: 15+

# Security requirements
grep -r "@requirement:REQ-MD-SEC" src/ | wc -l
# Expected: 4+
```

### Zero Tolerance Checks

```bash
# No todo!/unimplemented! in production code
grep -rn "todo!\|unimplemented!" src/ui_gpui/components/markdown_content.rs src/ui_gpui/components/message_bubble.rs src/ui_gpui/views/chat_view/render.rs | grep -v "mod tests" | grep -v "#\[cfg(test)\]"
# MUST be: 0 matches

# No debug code
grep -rn "println!\|dbg!" src/ui_gpui/components/markdown_content.rs src/ui_gpui/components/message_bubble.rs src/ui_gpui/views/chat_view/render.rs
# MUST be: 0 matches (tracing:: is OK)

# No hardcoded colors
grep -rn "rgb(0x" src/ui_gpui/components/markdown_content.rs src/ui_gpui/components/message_bubble.rs | grep -v "mod tests"
# MUST be: 0 matches

# No mdstream
grep "mdstream" Cargo.toml src/**/*.rs 2>/dev/null
# MUST be: 0 matches

# No TODO comments
grep -rn "// TODO\|// FIXME\|// HACK" src/ui_gpui/components/markdown_content.rs src/ui_gpui/components/message_bubble.rs src/ui_gpui/views/chat_view/render.rs
# MUST be: 0 matches
```

### Mandatory Behavioral Evidence (Critical Gates)

These are runtime-behavior tests that MUST produce concrete evidence, not just structural grep checks. Each gate requires a specific test that exercises the behavior and asserts a concrete outcome. If any of these tests do not exist or do not pass, the gate FAILS.

#### Gate 1: Click Precedence — Clicking a Link Does NOT Trigger Clipboard Copy

```
Test: test_click_link_does_not_copy_to_clipboard
Evidence required:
  - AssistantBubble with content containing a link ("[click](https://example.com)")
  - Simulate click on the rendered element
  - Assert: clipboard does NOT receive the raw markdown content
  - Assert: bubble has NO bubble-level on_click handler (link messages suppress it)
  - This verifies REQ-MD-INTEGRATE-021 at the behavioral level, not just
    by checking cursor_pointer absence
```

#### Gate 2: Streaming Cursor — Cursor Appears Only During Streaming

```
Test: test_streaming_cursor_only_during_streaming
Evidence required:
  - AssistantBubble with streaming(true) and content "Hello"
  - Assert: rendered content string includes '▋' cursor character
  - AssistantBubble with streaming(false) and content "Hello"
  - Assert: rendered content string does NOT include '▋' cursor character
  - This verifies both REQ-MD-INTEGRATE-040 and REQ-MD-INTEGRATE-041 together
    as a paired behavioral contract: cursor present IFF streaming
```

#### Gate 3: Table-Cell Link Suppression — Bubble Copy Disabled When Table Cells Contain Links

```
Test: test_table_cell_links_suppress_bubble_copy
Evidence required:
  - AssistantBubble with content containing a table where a cell has a link:
    "| Col |\n|---|\n| [link](https://example.com) |"
  - Assert: has_any_links() returns true for the parsed blocks
  - Assert: bubble does NOT have cursor_pointer style
  - Assert: bubble does NOT have a bubble-level on_click handler
  - This verifies REQ-MD-INTEGRATE-024 end-to-end: table cell link detection
    correctly propagates to click-to-copy suppression
```

#### Behavioral Evidence Verification

```bash
# All three critical-gate tests must exist and pass
cargo test --lib -- test_click_link_does_not_copy_to_clipboard || exit 1
cargo test --lib -- test_streaming_cursor_only_during_streaming || exit 1
cargo test --lib -- test_table_cell_links_suppress_bubble_copy || exit 1
echo "=== ALL CRITICAL BEHAVIORAL GATES PASSED ==="
```

### Final Checklist

#### Feature Completeness

- [ ] `parse_markdown_blocks()` handles all block-level constructs (paragraph, heading, code block, blockquote, list, table, thematic break)
- [ ] `parse_markdown_blocks()` handles all inline styles (bold, italic, strikethrough, code, link)
- [ ] `parse_markdown_blocks()` handles all fallbacks (image, HTML, math, metadata, footnotes)
- [ ] `blocks_to_elements()` renders all MarkdownBlock variants to GPUI elements
- [ ] `render_markdown()` composes parse + render
- [ ] `is_safe_url()` validates URL schemes (http/https allowlist)
- [ ] `AssistantBubble` renders markdown via two-phase pipeline
- [ ] `render_assistant_message()` delegates to AssistantBubble
- [ ] Click-to-copy conditional on link presence
- [ ] Streaming cursor appended correctly
- [ ] User messages stay raw text

#### Code Quality

- [ ] All colors from Theme::*
- [ ] No debug code
- [ ] No placeholder code
- [ ] Clippy clean
- [ ] Properly formatted
- [ ] All functions have doc comments with plan/requirement markers

#### Test Coverage

- [ ] 51+ tests total
- [ ] Parser tests (#[test]) cover all markdown constructs
- [ ] Renderer tests (#[gpui::test]) cover element construction
- [ ] Integration tests (#[gpui::test]) cover end-to-end flow
- [ ] URL security tests cover allowlist/rejection
- [ ] Isolation tests verify store/presenter/mdstream boundaries
- [ ] Edge case tests cover empty, whitespace, malformed input

#### Architecture

- [ ] Two-phase IR model (parse → render)
- [ ] Single canonical rendering owner (AssistantBubble)
- [ ] Store/presenter untouched
- [ ] Module properly exported
- [ ] No Phase B dependencies

## Success Criteria

ALL of the following must be true:
1. `cargo fmt --all -- --check` passes
2. `cargo clippy --all-targets -- -D warnings` passes
3. `cargo test --lib --tests` passes (all tests)
4. `cargo build` passes
5. Zero todo!/unimplemented! in production code
6. Zero hardcoded colors
7. Zero debug code
8. Zero mdstream references
9. 51+ tests with plan and requirement markers
10. All plan phases (P03–P12) have markers in codebase
11. Critical behavioral gate: click-link-does-not-copy test passes
12. Critical behavioral gate: streaming-cursor-only-during-streaming test passes
13. Critical behavioral gate: table-cell-links-suppress-bubble-copy test passes

## Gate Decision

IF all success criteria pass:
→ **GATE PASSED** — Feature ready for PR and Phase A merge

IF any criteria fail:
→ **GATE FAILED** — Return to appropriate phase for remediation

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P12a.md`
Mark execution tracker: All phases COMPLETE
