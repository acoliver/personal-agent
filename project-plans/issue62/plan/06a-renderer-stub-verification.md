# Phase 06a: Renderer Stub Verification

## Phase ID

`PLAN-20260402-MARKDOWN.P06a`

## Prerequisites

- Required: Phase 06 completed
- Verification: `grep -r "@plan:PLAN-20260402-MARKDOWN.P06" src/ui_gpui/components/markdown_content.rs`
- Expected: Renderer stub functions with correct signatures, `render_markdown()` composing parse + render

## Verification Commands

### Automated Checks

```bash
# 1. P06 markers present
grep -c "@plan:PLAN-20260402-MARKDOWN.P06" src/ui_gpui/components/markdown_content.rs
# Expected: 8+

# 2. Previous phase markers intact
grep -c "@plan:PLAN-20260402-MARKDOWN.P03" src/ui_gpui/components/markdown_content.rs
grep -c "@plan:PLAN-20260402-MARKDOWN.P05" src/ui_gpui/components/markdown_content.rs
# Expected: Both >0

# 3. Compiles
cargo build || exit 1

# 3a. Clippy snapshot (non-gating in stub verification)
cargo clippy --all-targets -- -D warnings || true

# 4. Parser tests still pass
cargo test --lib -- markdown_content::tests::test_parse || exit 1
cargo test --lib -- markdown_content::tests::test_safe_url || exit 1

# 5. render_markdown composes (not todo!())
grep -A5 "fn render_markdown" src/ui_gpui/components/markdown_content.rs | grep "parse_markdown_blocks"
# Expected: Match found (render_markdown calls parse_markdown_blocks)

# 6. blocks_to_elements delegates to helpers
grep -A20 "fn blocks_to_elements" src/ui_gpui/components/markdown_content.rs | grep -c "render_paragraph\|render_heading\|render_code_block"
# Expected: 3+ (delegates to helper functions)
```

### Structural Verification Checklist

- [ ] All previous phase markers intact
- [ ] P06 markers on all new stubs
- [ ] `render_markdown()` composes parse + render (not todo!())
- [ ] `blocks_to_elements()` matches on variants and delegates
- [ ] Helper render functions have correct signatures
- [ ] `cargo build` succeeds
- [ ] All parser tests pass

## Success Criteria

- Compilable renderer stub
- Parser tests unbroken
- Correct composition structure

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P06a.md`
