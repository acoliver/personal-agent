# Phase 02a: Pseudocode Verification

## Phase ID

`PLAN-20260402-MARKDOWN.P02a`

## Prerequisites

- Required: Phase 02 completed
- Verification: Pseudocode files exist in `project-plans/issue62/analysis/pseudocode/`
- Expected files from previous phase:
  - `project-plans/issue62/analysis/pseudocode/parse-markdown-blocks.md`
  - `project-plans/issue62/analysis/pseudocode/blocks-to-elements.md`
  - `project-plans/issue62/analysis/pseudocode/render-markdown.md`

## Verification Commands

### Structural Verification

```bash
# All pseudocode files exist
test -f project-plans/issue62/analysis/pseudocode/parse-markdown-blocks.md && echo "OK" || echo "FAIL"
test -f project-plans/issue62/analysis/pseudocode/blocks-to-elements.md && echo "OK" || echo "FAIL"
test -f project-plans/issue62/analysis/pseudocode/render-markdown.md && echo "OK" || echo "FAIL"
```

### Line Number Coverage

```bash
# Parser pseudocode: 150+ numbered lines
wc -l project-plans/issue62/analysis/pseudocode/parse-markdown-blocks.md
# Expected: substantial file

# Renderer pseudocode: 150+ numbered lines
wc -l project-plans/issue62/analysis/pseudocode/blocks-to-elements.md
# Expected: substantial file

# Public API pseudocode: 25+ numbered lines
wc -l project-plans/issue62/analysis/pseudocode/render-markdown.md
# Expected: smaller but complete
```

### Requirement Coverage

```bash
# Parser covers all PARSE requirements
for req in PARSE-001 PARSE-002 PARSE-003 PARSE-004 PARSE-005 PARSE-006 PARSE-007 PARSE-008 PARSE-009 PARSE-010 PARSE-011 PARSE-020 PARSE-021 PARSE-022 PARSE-023 PARSE-024 PARSE-025 PARSE-026 PARSE-027 PARSE-028 PARSE-029 PARSE-040 PARSE-041 PARSE-042 PARSE-043 PARSE-044 PARSE-045 PARSE-046 PARSE-047 PARSE-048 PARSE-049 PARSE-050 PARSE-051 PARSE-060 PARSE-061 PARSE-062 PARSE-063 PARSE-064 PARSE-065; do
    grep -q "$req" project-plans/issue62/analysis/pseudocode/parse-markdown-blocks.md || echo "MISSING: $req"
done

# Renderer covers all RENDER requirements
for req in RENDER-001 RENDER-002 RENDER-003 RENDER-004 RENDER-005 RENDER-006 RENDER-007 RENDER-008 RENDER-009 RENDER-010 RENDER-011 RENDER-020 RENDER-021 RENDER-022 RENDER-023 RENDER-024 RENDER-025 RENDER-026 RENDER-030 RENDER-031 RENDER-032 RENDER-033 RENDER-040 RENDER-041 RENDER-042 RENDER-043 RENDER-050 RENDER-051 RENDER-052 RENDER-053; do
    grep -q "$req" project-plans/issue62/analysis/pseudocode/blocks-to-elements.md || echo "MISSING: $req"
done

# Public API covers SEC requirements
for req in SEC-001 SEC-002 SEC-003 SEC-004 SEC-005 SEC-006; do
    grep -q "$req" project-plans/issue62/analysis/pseudocode/render-markdown.md || echo "MISSING: $req"
done
```

### Semantic Verification Checklist

- [ ] Parser pseudocode handles ALL pulldown-cmark events from the spec event table (§5.1)
- [ ] Parser pseudocode includes the HTML tag-stripping state machine
- [ ] Parser pseudocode includes link byte-range tracking
- [ ] Parser pseudocode handles nested inline styles (style stack)
- [ ] Parser pseudocode handles nested block structures (block stack)
- [ ] Renderer pseudocode handles ALL MarkdownBlock variants
- [ ] Renderer pseudocode uses correct Theme methods for each element
- [ ] Renderer pseudocode includes spans_to_text_runs() helper with UTF-8 byte len calculation
- [ ] Renderer pseudocode includes InteractiveText wrapping for link-containing blocks
- [ ] Public API pseudocode includes is_safe_url() with scheme allowlist
- [ ] Line numbers are sequential and unambiguous
- [ ] No pseudocode references external state (no store, no presenter — pure functions)

## Success Criteria

- All pseudocode files exist and have numbered lines
- Every Phase A requirement ID appears in at least one pseudocode file
- Pseudocode is specific enough that implementation can cite line numbers
- No gaps or TBD placeholders

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P02a.md`
