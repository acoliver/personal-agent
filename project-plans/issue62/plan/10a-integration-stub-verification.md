# Phase 10a: Integration Stub Verification

## Phase ID

`PLAN-20260402-MARKDOWN.P10a`

## Prerequisites

- Required: Phase 10 completed
- Verification: `grep -r "@plan:PLAN-20260402-MARKDOWN.P10" src/`
- Expected files modified: `message_bubble.rs`, `render.rs`

## Verification Commands

### Automated Checks

```bash
# 1. P10 markers present across both files
grep -r "@plan:PLAN-20260402-MARKDOWN.P10" src/ | wc -l
# Expected: 3+

# 2. Integration requirement markers
grep -r "@requirement:REQ-MD-INTEGRATE" src/ | wc -l
# Expected: 18+ (15 from P09 tests + 3 from P10 stub)

# 3. Compiles
cargo build || exit 1

# 3a. Clippy snapshot (non-gating in stub verification)
cargo clippy --all-targets -- -D warnings || true

# 4. All tests pass
cargo test --lib || exit 1

# 5. render_assistant_message delegates to AssistantBubble
grep "AssistantBubble" src/ui_gpui/views/chat_view/render.rs | head -3
# Expected: AssistantBubble::new found

# 6. message_bubble.rs imports markdown_content
grep "markdown_content\|parse_markdown_blocks\|blocks_to_elements" src/ui_gpui/components/message_bubble.rs | head -3
# Expected: imports found

# 7. Old raw string rendering removed
grep '\.child(content_text)' src/ui_gpui/components/message_bubble.rs && echo "FAIL: Old raw rendering still present"
# Expected: No match (old pattern replaced)

# 8. No new public fields on AssistantBubble
grep "pub struct AssistantBubble" -A 10 src/ui_gpui/components/message_bubble.rs | grep "pub " | grep -v "pub struct"
# Expected: No public fields (all fields are private)
```

### Structural Verification Checklist

- [ ] All previous phase markers intact (including P09 integration tests)
- [ ] P10 markers in both modified files
- [ ] `cargo build` succeeds
- [ ] All tests pass
- [ ] AssistantBubble calls markdown pipeline
- [ ] render_assistant_message delegates to AssistantBubble
- [ ] Old `.child(content_text)` removed from message_bubble.rs
- [ ] No new public fields on AssistantBubble

### Semantic Verification

- [ ] Full call chain: render.rs → AssistantBubble → parse_markdown_blocks → blocks_to_elements → GPUI elements
- [ ] Streaming path also gets markdown rendering
- [ ] Model label fallback to "Assistant" when model_id is None

## Success Criteria

- Integration wiring compiles and all tests pass
- Old rendering paths replaced
- No API changes to AssistantBubble

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P10a.md`
