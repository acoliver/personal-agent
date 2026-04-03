# Phase 05: Parser Implementation

## Phase ID

`PLAN-20260402-MARKDOWN.P05`

## Prerequisites

- Required: Phase 04a completed (parser TDD verified)
- Verification: `grep -r "@plan:PLAN-20260402-MARKDOWN.P04" src/ui_gpui/components/markdown_content.rs`
- Expected files from previous phase: `markdown_content.rs` with stub functions and 30+ failing tests
- Preflight verification: Phase 0.5 completed

## Requirements Implemented (Expanded)

This phase implements all REQ-MD-PARSE requirements to make all P04 tests pass.

### REQ-MD-PARSE-001: Parser Entry Point

**Full Text**: WHEN markdown text is passed to the parser entry point, the system shall produce a `Vec<MarkdownBlock>` intermediate representation with no GPUI dependency.
**Behavior**:
- GIVEN: Any markdown string
- WHEN: `parse_markdown_blocks()` is called
- THEN: Returns a `Vec<MarkdownBlock>` representing the parsed structure
**Why This Matters**: Core of the two-phase architecture.

### REQ-MD-PARSE-011: pulldown-cmark Options

**Full Text**: The parser shall enable pulldown-cmark options `ENABLE_TABLES`, `ENABLE_STRIKETHROUGH`, and `ENABLE_TASKLISTS`.
**Behavior**:
- GIVEN: Parser initialization
- WHEN: pulldown-cmark Parser is constructed
- THEN: Options include TABLES, STRIKETHROUGH, TASKLISTS
**Why This Matters**: Without these options, tables/strikethrough/task lists are not parsed.

### REQ-MD-PARSE-002 through REQ-MD-PARSE-010: Block-Level Constructs

(See Phase 04 for full expansions. Implementation must make all block-level tests pass.)

### REQ-MD-PARSE-020 through REQ-MD-PARSE-029: Inline Constructs

(See Phase 04 for full expansions. Implementation must make all inline tests pass.)

### REQ-MD-PARSE-040 through REQ-MD-PARSE-051: Fallbacks

(See Phase 04 for full expansions. Implementation must make all fallback tests pass.)

This explicitly includes the following SHOULD/COULD requirements that must be implemented:
- **REQ-MD-PARSE-041**: Footnote definition fallback — render footnote content as inline paragraph
- **REQ-MD-PARSE-042**: Footnote reference literal text — emit `[^label]` as plain text
- **REQ-MD-PARSE-047**: Display math to code block — `$$...$$` → `MarkdownBlock::CodeBlock { language: None }`
- **REQ-MD-PARSE-049**: Metadata block skip — skip YAML/TOML front matter entirely
- **REQ-MD-PARSE-051**: Definition lists fallback — render as plain paragraphs

### REQ-MD-PARSE-026: Task List Marker

**Full Text**: WHEN the parser encounters a task list marker event, it shall insert a Unicode checkbox character into the current text accumulation.
**Behavior**:
- GIVEN: `"- [x] Done"` → list item text starts with U+2611
- GIVEN: `"- [ ] Open"` → list item text starts with U+2610
**Why This Matters**: Task lists appear frequently in AI assistant step-by-step responses.

### REQ-MD-PARSE-065: Unknown Event Fallback

**Full Text**: IF an unknown or unhandled pulldown-cmark event type is encountered, THEN the parser shall extract any text content and append it as plain text rather than panicking.

### REQ-MD-SEC-001 through REQ-MD-SEC-006: URL Validation

(See Phase 04 for full expansions. Implementation must make all URL tests pass.)

## Implementation Tasks

### Files to Modify

#### `src/ui_gpui/components/markdown_content.rs`

- UPDATE `parse_markdown_blocks()`: Replace `todo!()` with full implementation
  - MUST follow pseudocode from `project-plans/issue62/analysis/pseudocode/parse-markdown-blocks.md`
  - Add `/// @plan:PLAN-20260402-MARKDOWN.P05` marker
  - Add `/// @pseudocode lines X-Y` references for each section
  - Reference specific pseudocode line numbers in implementation comments

- UPDATE `is_safe_url()`: Replace `todo!()` with URL validation
  - MUST follow pseudocode from `project-plans/issue62/analysis/pseudocode/render-markdown.md`
  - Add plan and pseudocode markers

- ADD helper functions (following pseudocode decomposition for Clippy compliance):
  - `handle_block_start()` — processes `Start(Tag)` events
  - `handle_block_end()` — processes `End(Tag)` events  
  - `handle_inline_event()` — processes inline style Start/End events
  - `handle_text_event()` — processes Text, Code, SoftBreak, HardBreak events
  - `strip_html_tags()` — the tag-stripping state machine

### Required Code Markers

```rust
/// @plan:PLAN-20260402-MARKDOWN.P05
/// @requirement:REQ-MD-PARSE-001
/// @pseudocode parse-markdown-blocks.md lines 1-10
pub(crate) fn parse_markdown_blocks(content: &str) -> Vec<MarkdownBlock> {
    // Implementation following pseudocode
}
```

### Implementation Approach

Follow the pseudocode line-by-line from `parse-markdown-blocks.md`:

1. **Lines 1-10**: Set up Options with ENABLE_TABLES | ENABLE_STRIKETHROUGH | ENABLE_TASKLISTS, create Parser
2. **Lines 11-20**: Initialize block stack, inline style stack, list stack, result accumulator
3. **Lines 21-60**: Block event handling — each Start/End pair pushes/pops block builders
4. **Lines 61-80**: List handling — ListContext tracking, item builders
5. **Lines 81-100**: Table handling — alignment recording, cell accumulation
6. **Lines 101-120**: Inline style stack management
7. **Lines 121-140**: Text accumulation with current style state
8. **Lines 141-160**: Link byte-range tracking
9. **Lines 161-180**: Fallback handlers (image, footnote, HTML, math, metadata)
10. **Lines 181-190**: HTML tag-stripping state machine
11. **Lines 191-200**: Return accumulated blocks

For `is_safe_url()`, follow `render-markdown.md` lines 11-25:
1. Trim whitespace
2. Parse with `url::Url::parse()`
3. Check scheme is "http" or "https"
4. Return false for parse errors

### FORBIDDEN

- Creating new test files or modifying existing tests
- Creating `markdown_content_v2.rs` or any duplicate
- `println!()` or `dbg!()` in production code
- `// TODO` comments
- `todo!()` or `unimplemented!()` remaining after implementation

## Verification Commands

### Automated Checks (Structural)

```bash
# 1. Plan markers for P05 exist
grep -c "@plan:PLAN-20260402-MARKDOWN.P05" src/ui_gpui/components/markdown_content.rs
# Expected: 5+ (one per implemented function)

# 2. Pseudocode references exist
grep -c "@pseudocode" src/ui_gpui/components/markdown_content.rs
# Expected: 5+ references

# 3. All tests pass
cargo test --lib -- markdown_content || exit 1

# 4. No test modifications
git diff src/ui_gpui/components/markdown_content.rs | grep -E "^[-+].*#\[test\]" | head -5
# Expected: No test additions or removals (only implementation code changed)

# 5. Format check
cargo fmt --all -- --check || exit 1

# 6. Clippy snapshot (non-gating in P05 due to deferred strict lint gate)
# Strict clippy gate for parser+renderer is enforced in P08a.
cargo clippy --all-targets -- -D warnings || true
```

### Deferred Implementation Detection (MANDATORY)

```bash
# Check for todo!/unimplemented! left in implementation
grep -rn "todo!\|unimplemented!" src/ui_gpui/components/markdown_content.rs | grep -v "#\[cfg(test)\]" | grep -v "mod tests"
# Expected: No matches in production code

# Check for "cop-out" comments
grep -rn -E "(// TODO|// FIXME|// HACK|placeholder|not yet)" src/ui_gpui/components/markdown_content.rs
# Expected: No matches

# Check for empty implementations
grep -rn "fn .* \{\s*\}" src/ui_gpui/components/markdown_content.rs | grep -v test
# Expected: No matches in production code

# Check for debug code
grep -rn "println!\|dbg!" src/ui_gpui/components/markdown_content.rs
# Expected: No matches
```

### Semantic Verification Checklist

1. **Does the code DO what the requirements say?**
   - [ ] I read REQ-MD-PARSE-001 and confirmed `parse_markdown_blocks()` returns `Vec<MarkdownBlock>`
   - [ ] I read the implementation and can trace how each Event type maps to a MarkdownBlock variant
   - [ ] I confirmed pulldown-cmark Options include TABLES, STRIKETHROUGH, TASKLISTS (REQ-MD-PARSE-011)
   - [ ] I confirmed `is_safe_url()` uses `url::Url::parse` with http/https allowlist (REQ-MD-SEC-001)

2. **Is this REAL implementation, not placeholder?**
   - [ ] Deferred implementation detection passed
   - [ ] No empty function bodies
   - [ ] No "will be implemented" comments
   - [ ] Every function does actual work

3. **Would the test FAIL if implementation was removed?**
   - [ ] Tests verify specific parsed output, not just that code ran
   - [ ] Tests would catch wrong block types, wrong text content, wrong style flags

4. **Does pseudocode match implementation?**
   - [ ] Implementation follows pseudocode line-by-line
   - [ ] `@pseudocode` markers reference correct line numbers
   - [ ] No significant logic added that isn't in pseudocode
   - [ ] No pseudocode logic omitted

5. **What's MISSING?**
   - [ ] All SHOULD/COULD requirements now explicitly listed (REQ-MD-PARSE-026/041/042/047/049/051)
   - [ ] [Verify no other gaps remain]

## Success Criteria

- All P04 tests pass
- No tests were modified
- No `todo!()` or `unimplemented!()` in production code
- No debug code
- fmt passes
- Strict clippy gate is deferred to P08a (do not fail P05 solely on stub-phase lint noise)
- Pseudocode references present
- Implementation follows pseudocode line-by-line

## Failure Recovery

If this phase fails:
1. Rollback: `git checkout -- src/ui_gpui/components/markdown_content.rs`
2. Then re-apply P03 stub + P04 tests from git history
3. Re-attempt implementation
4. Cannot proceed to Phase 06 until all tests pass

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P05.md`
Contents:
```markdown
Phase: P05
Completed: [timestamp]
Files Modified: markdown_content.rs [diff stats]
Tests Added: 0 (tests from P04, not modified)
Tests Passing: [count] / [total]
Verification: [paste of cargo test, clippy, fmt outputs]

## Holistic Functionality Assessment

### What was implemented?
[Describe parse_markdown_blocks() and is_safe_url() in own words]

### Does it satisfy the requirements?
[For each REQ-MD-PARSE requirement, explain how]

### What is the data flow?
[Trace: markdown string → pulldown-cmark events → block stack → Vec<MarkdownBlock>]

### What could go wrong?
[Edge cases, error conditions]

### Verdict
[PASS/FAIL with explanation]
```
