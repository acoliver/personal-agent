# Pre-Implementation Reconciliation Checklist

**Issue:** [#62 — Markdown rendering for assistant messages](https://github.com/acoliver/personal-agent/issues/62)
**Purpose:** Verify cross-document consistency and codebase readiness before Phase 0.5 begins.
**Last Updated:** 2026-04-02

---

## Checklist Completion Note

> **This checklist is designed to be completed at the START of implementation (before Phase 0.5).** Checkboxes are intentionally unchecked until that point — they represent gates to be verified, not gaps to be filled in planning.

---

## Instructions

**Run this reconciliation before Phase 0.5 (preflight verification).** This checklist ensures:
1. Every requirement has an implementing phase and a verifying test phase.
2. The canonical IR schema is consistent across all documents.
3. The codebase state matches planning assumptions.

If any check fails, resolve the discrepancy before proceeding.

---

## 1. Requirement → Plan Phase → Test Phase Traceability

### REQ-MD-PARSE: Markdown Parsing (Phase A)

| Req ID | Type | Impl Phase | Test Phase | Verification Method |
|--------|------|------------|------------|---------------------|
| REQ-MD-PARSE-001 | Behavioral | P05 (Parser Impl) | P04 (Parser TDD) | `#[test]` — IR output structure |
| REQ-MD-PARSE-002 | Behavioral | P05 | P04 | `#[test]` — paragraph block output |
| REQ-MD-PARSE-003 | Behavioral | P05 | P04 | `#[test]` — heading level + spans |
| REQ-MD-PARSE-004 | Behavioral | P05 | P04 | `#[test]` — fenced code block + language |
| REQ-MD-PARSE-005 | Behavioral | P05 | P04 | `#[test]` — indented code block |
| REQ-MD-PARSE-006 | Behavioral | P05 | P04 | `#[test]` — blockquote children |
| REQ-MD-PARSE-007 | Behavioral | P05 | P04 | `#[test]` — unordered list, start: 0 |
| REQ-MD-PARSE-008 | Behavioral | P05 | P04 | `#[test]` — ordered list, start: u64 |
| REQ-MD-PARSE-009 | Behavioral | P05 | P04 | `#[test]` — table structure |
| REQ-MD-PARSE-010 | Behavioral | P05 | P04 | `#[test]` — thematic break |
| REQ-MD-PARSE-011 | Constraint | P05 | P04 | `#[test]` — parser options enabled |
| REQ-MD-PARSE-020 | Behavioral | P05 | P04 | `#[test]` — bold span |
| REQ-MD-PARSE-021 | Behavioral | P05 | P04 | `#[test]` — italic span |
| REQ-MD-PARSE-022 | Behavioral | P05 | P04 | `#[test]` — bold-italic span |
| REQ-MD-PARSE-023 | Behavioral | P05 | P04 | `#[test]` — strikethrough span |
| REQ-MD-PARSE-024 | Behavioral | P05 | P04 | `#[test]` — inline code span |
| REQ-MD-PARSE-025 | Behavioral | P05 | P04 | `#[test]` — link span + links field |
| REQ-MD-PARSE-026 | Behavioral | P05 | P04 | `#[test]` — task list markers |
| REQ-MD-PARSE-027 | Behavioral | P05 | P04 | `#[test]` — nested inline styles |
| REQ-MD-PARSE-028 | Behavioral | P05 | P04 | `#[test]` — soft break |
| REQ-MD-PARSE-029 | Behavioral | P05 | P04 | `#[test]` — hard break |
| REQ-MD-PARSE-040 | Behavioral | P05 | P04 | `#[test]` — image fallback |
| REQ-MD-PARSE-041 | Behavioral | P05 | P04 | `#[test]` — footnote definition |
| REQ-MD-PARSE-042 | Behavioral | P05 | P04 | `#[test]` — footnote reference |
| REQ-MD-PARSE-043 | Behavioral | P05 | P04 | `#[test]` — HTML block strip |
| REQ-MD-PARSE-044 | Behavioral | P05 | P04 | `#[test]` — inline HTML strip |
| REQ-MD-PARSE-045 | Behavioral | P05 | P04 | `#[test]` — script/style strip |
| REQ-MD-PARSE-046 | Behavioral | P05 | P04 | `#[test]` — inline math → code |
| REQ-MD-PARSE-047 | Behavioral | P05 | P04 | `#[test]` — display math → code block |
| REQ-MD-PARSE-048 | Behavioral | P05 | P04 | `#[test]` — superscript/subscript plaintext |
| REQ-MD-PARSE-049 | Behavioral | P05 | P04 | `#[test]` — metadata block skip |
| REQ-MD-PARSE-050 | Behavioral | P05 | P04 | `#[test]` — malformed HTML no panic |
| REQ-MD-PARSE-051 | Behavioral | P05 | P04 | `#[test]` — definition list fallback |
| REQ-MD-PARSE-060 | Constraint | P03 (Parser Stub) | P04 | `#[test]` — derive traits present |
| REQ-MD-PARSE-061 | Constraint | P03 | P04 | `#[test]` — struct fields |
| REQ-MD-PARSE-062 | Constraint | P03 | P04 | `#[test]` — enum variants |
| REQ-MD-PARSE-063 | Constraint | P03 | P04 | `#[test]` — links field on variants |
| REQ-MD-PARSE-064 | Constraint | P03 | Static check | Code review — visibility modifiers |
| REQ-MD-PARSE-065 | Behavioral | P05 | P04 | `#[test]` — unknown event fallback |

### REQ-MD-RENDER: GPUI Rendering (Phase A)

| Req ID | Type | Impl Phase | Test Phase | Verification Method |
|--------|------|------------|------------|---------------------|
| REQ-MD-RENDER-001 | Behavioral | P08 (Renderer Impl) | P07 (Renderer TDD) | `#[gpui::test]` — paragraph element |
| REQ-MD-RENDER-002 | Behavioral | P08 | P07 | `#[gpui::test]` — interactive text for links |
| REQ-MD-RENDER-003 | Behavioral | P08 | P07 | `#[gpui::test]` — heading element |
| REQ-MD-RENDER-004 | Constraint | P08 | P07 | `#[gpui::test]` — heading size values |
| REQ-MD-RENDER-005 | Behavioral | P08 | P07 | `#[gpui::test]` — code block container |
| REQ-MD-RENDER-006 | Behavioral | P08 | P07 | `#[gpui::test]` — language label |
| REQ-MD-RENDER-007 | Behavioral | P08 | P07 | `#[gpui::test]` — blockquote element |
| REQ-MD-RENDER-008 | Behavioral | P08 | P07 | `#[gpui::test]` — list element |
| REQ-MD-RENDER-009 | Behavioral | P08 | P07 | `#[gpui::test]` — table grid |
| REQ-MD-RENDER-010 | Behavioral | P08 | P07 | `#[gpui::test]` — thematic break element |
| REQ-MD-RENDER-011 | Behavioral | P08 | P07 | `#[gpui::test]` — image fallback text |
| REQ-MD-RENDER-020 | Behavioral | P08 | P07 | `#[gpui::test]` — bold text run |
| REQ-MD-RENDER-021 | Behavioral | P08 | P07 | `#[gpui::test]` — italic text run |
| REQ-MD-RENDER-022 | Behavioral | P08 | P07 | `#[gpui::test]` — strikethrough text run |
| REQ-MD-RENDER-023 | Behavioral | P08 | P07 | `#[gpui::test]` — code text run |
| REQ-MD-RENDER-024 | Behavioral | P08 | P07 | `#[gpui::test]` — link text run |
| REQ-MD-RENDER-025 | Behavioral | P08 | P07 | `#[gpui::test]` — bullet/number prefix style |
| REQ-MD-RENDER-026 | Constraint | P08 | P07 | `#[gpui::test]` — font fallback behavior |
| REQ-MD-RENDER-030 | Constraint | P08 | P12 (Cleanup) | Static check — grep for hardcoded colors |
| REQ-MD-RENDER-031 | Constraint | P08 | P07 | `#[gpui::test]` — color token mapping |
| REQ-MD-RENDER-032 | Constraint | P08 | P12 | Static check — no new Theme methods |
| REQ-MD-RENDER-033 | Constraint | P08 | P07 | `#[gpui::test]` — default text style |
| REQ-MD-RENDER-040 | Behavioral | P06 (Renderer Stub) | P07 | `#[gpui::test]` — render_markdown API |
| REQ-MD-RENDER-041 | Behavioral | P08 | P07 | `#[gpui::test]` — empty content |
| REQ-MD-RENDER-042 | Constraint | P06 | P09 (Integration TDD) | `#[test]` — module export |
| REQ-MD-RENDER-043 | Constraint | P08 | P09 | `#[test]` — store/presenter isolation |
| REQ-MD-RENDER-050 | Behavioral | P08 | P07 | `#[gpui::test]` — table grid column count |
| REQ-MD-RENDER-051 | Behavioral | P08 | P07 | `#[gpui::test]` — table header background |
| REQ-MD-RENDER-052 | Behavioral | P08 | P07 | `#[gpui::test]` — alternating row striping |
| REQ-MD-RENDER-053 | Behavioral | P08 | P07 | `#[gpui::test]` — table cell borders |

### REQ-MD-INTEGRATE: Integration (Phase A)

| Req ID | Type | Impl Phase | Test Phase | Verification Method |
|--------|------|------------|------------|---------------------|
| REQ-MD-INTEGRATE-001 | Constraint | P11 (Integration Impl) | P09 (Integration TDD) | `#[gpui::test]` — single rendering owner |
| REQ-MD-INTEGRATE-002 | Behavioral | P11 | P09 | `#[gpui::test]` — markdown parsing in bubble |
| REQ-MD-INTEGRATE-003 | Constraint | P11 | P09 | `#[test]` — no new public fields |
| REQ-MD-INTEGRATE-010 | Constraint | P11 | P09 | `#[gpui::test]` — delegation to bubble |
| REQ-MD-INTEGRATE-011 | Behavioral | P11 | P09 | `#[gpui::test]` — model/thinking pass-through |
| REQ-MD-INTEGRATE-012 | Behavioral | P11 | P09 | `#[gpui::test]` — visual baseline match |
| REQ-MD-INTEGRATE-015 | Behavioral | P11 | P09 | `#[gpui::test]` — "Assistant" label fallback |
| REQ-MD-INTEGRATE-020 | Behavioral | P11 | P09 | `#[gpui::test]` — click-to-copy no links |
| REQ-MD-INTEGRATE-021 | Behavioral | P11 | P09 | `#[gpui::test]` — no click handler with links |
| REQ-MD-INTEGRATE-022 | Behavioral | P11 | P09 | `#[gpui::test]` — no click during streaming |
| REQ-MD-INTEGRATE-023 | Behavioral | P11 | P09 | `#[gpui::test]` — copies raw markdown |
| REQ-MD-INTEGRATE-024 | Behavioral | P11 | P09 | `#[gpui::test]` — recursive link detection |
| REQ-MD-INTEGRATE-030 | Behavioral | P11 | P09 | `#[gpui::test]` — user messages raw text |
| REQ-MD-INTEGRATE-040 | Behavioral | P11 | P09 | `#[gpui::test]` — streaming cursor appended |
| REQ-MD-INTEGRATE-041 | Behavioral | P11 | P09 | `#[gpui::test]` — cursor not in committed/persisted |
| REQ-MD-INTEGRATE-050 | Constraint | P11 | P09 | `#[test]` — store layer unchanged |
| REQ-MD-INTEGRATE-051 | Constraint | P11 | P09 | `#[test]` — presenter layer unchanged |
| REQ-MD-INTEGRATE-061 | Constraint | Phase B | Phase B | [Phase B — Conditional] |
| REQ-MD-INTEGRATE-070 | Constraint | P11 | P09 | `#[test]` — no mdstream references |

### REQ-MD-STREAM: Streaming (Phase B — Conditional, pending validation gate)

| Req ID | Type | Impl Phase | Test Phase | Verification Method |
|--------|------|------------|------------|---------------------|
| REQ-MD-STREAM-001 | Constraint | Phase B | Phase B | [Phase B — Conditional] view-local placement |
| REQ-MD-STREAM-002 | Constraint | Phase B | Phase B | [Phase B — Conditional] field existence |
| REQ-MD-STREAM-003 | Constraint | Phase B | Phase B | [Phase B — Conditional] dependency gate |
| REQ-MD-STREAM-004 | Behavioral | Phase B | Phase B | [Phase B — Conditional] adapter module |
| REQ-MD-STREAM-005 | Behavioral | Phase B | Phase B | [Phase B — Conditional] custom splitter |
| REQ-MD-STREAM-010 | Behavioral | Phase B | Phase B | [Phase B — Conditional] delta feeding |
| REQ-MD-STREAM-011 | Constraint | Phase B | Phase B | [Phase B — Conditional] assert char boundary |
| REQ-MD-STREAM-012 | Constraint | Phase B | Phase B | [Phase B — Conditional] assert message |
| REQ-MD-STREAM-020 | Behavioral | Phase B | Phase B | [Phase B — Conditional] committed block render |
| REQ-MD-STREAM-021 | Behavioral | Phase B | Phase B | [Phase B — Conditional] pending block render |
| REQ-MD-STREAM-022 | Behavioral | Phase B | Phase B | [Phase B — Conditional] element concatenation |
| REQ-MD-STREAM-030 | Behavioral | Phase B | Phase B | [Phase B — Conditional] finalize on completion |
| REQ-MD-STREAM-031 | Behavioral | Phase B | Phase B | [Phase B — Conditional] reset on Escape |
| REQ-MD-STREAM-032 | Behavioral | Phase B | Phase B | [Phase B — Conditional] reset on convo switch |
| REQ-MD-STREAM-033 | Behavioral | Phase B | Phase B | [Phase B — Conditional] reset on new convo |
| REQ-MD-STREAM-034 | Behavioral | Phase B | Phase B | [Phase B — Conditional] reset on error |
| REQ-MD-STREAM-035 | Behavioral | Phase B | Phase B | [Phase B — Conditional] reset on convo cleared |
| REQ-MD-STREAM-036 | Behavioral | Phase B | Phase B | [Phase B — Conditional] idempotent reset |
| REQ-MD-STREAM-037 | Behavioral | Phase B | Phase B | [Phase B — Conditional] finalize-once guard |
| REQ-MD-STREAM-038 | Behavioral | Phase B | Phase B | [Phase B — Conditional] stale state warning |
| REQ-MD-STREAM-040 | Constraint | Phase B | Phase B | [Phase B — Conditional] store-driven handler |
| REQ-MD-STREAM-041 | Constraint | Phase B | Phase B | [Phase B — Conditional] user-action handler |
| REQ-MD-STREAM-042 | Behavioral | Phase B | Phase B | [Phase B — Conditional] streaming state precedence |
| REQ-MD-STREAM-043 | Behavioral | Phase B | Phase B | [Phase B — Conditional] parser reset on disagreement |
| REQ-MD-STREAM-050 | Behavioral | P11 (Integration Impl) | P09 (Integration TDD) | `#[gpui::test]` — full re-parse each frame |
| REQ-MD-STREAM-051 | Constraint | P11 | P12 | Static check — tracing::debug! present |
| REQ-MD-STREAM-060 | Behavioral | Phase B | Phase B | [Phase B — Conditional] incomplete table |
| REQ-MD-STREAM-061 | Behavioral | Phase B | Phase B | [Phase B — Conditional] partial table grid |

### REQ-MD-PERF: Performance

| Req ID | Type | Impl Phase | Test Phase | Verification Method |
|--------|------|------------|------------|---------------------|
| REQ-MD-PERF-001 | Behavioral | P12 (Cleanup) | P12 | Manual perf test — release build |
| REQ-MD-PERF-002 | Behavioral | P12 | P12 | Manual perf test — release build |
| REQ-MD-PERF-003 | Behavioral | P12 | P12 | Manual perf test — release build |
| REQ-MD-PERF-004 | Behavioral | P12 | P12 | Manual perf test — release build |
| REQ-MD-PERF-005 | Behavioral | P12 | P12 | Manual perf test — document only |
| REQ-MD-PERF-006 | Behavioral | P12 | P12 | Manual perf test — memory check |
| REQ-MD-PERF-007 | Constraint | P12 | P12 | Verify fixtures committed |
| REQ-MD-PERF-008 | Constraint | P12 | P12 | Verify release build used |
| REQ-MD-PERF-009 | Constraint | P12 | P12 | Verify measurement protocol |
| REQ-MD-PERF-009a | Constraint | P12 | P12 | Verify delta calculation |
| REQ-MD-PERF-009b | Constraint | P12 | P12 | Verify Instant::now() timing |
| REQ-MD-PERF-010 | Behavioral | Phase B | Phase B | [Phase B — Conditional] streaming perf |
| REQ-MD-PERF-011 | Behavioral | Phase B | Phase B | [Phase B — Conditional] committed block stability |
| REQ-MD-PERF-012 | Behavioral | Phase B | Phase B | [Phase B — Conditional] UTF-8 no panic |
| REQ-MD-PERF-020 | Constraint | P12 | P12 | Conditional — profile if threshold exceeded |

### REQ-MD-SEC: Security

| Req ID | Type | Impl Phase | Test Phase | Verification Method |
|--------|------|------------|------------|---------------------|
| REQ-MD-SEC-001 | Behavioral | P05 (Parser Impl) | P04 (Parser TDD) | `#[test]` — URL validation |
| REQ-MD-SEC-002 | Behavioral | P05 | P04 | `#[test]` — dangerous scheme rejection |
| REQ-MD-SEC-003 | Behavioral | P05 | P04 | `#[test]` — malformed URL rendering |
| REQ-MD-SEC-004 | Constraint | P05 | P04 | `#[test]` — url crate used |
| REQ-MD-SEC-005 | Constraint | P03 (Parser Stub) | P12 | Static check — Cargo.toml |
| REQ-MD-SEC-006 | Behavioral | P05 | P04 | `#[test]` — relative URL no-op |
| REQ-MD-SEC-010 | Behavioral | P05 | P04 | `#[test]` — HTML tag stripping |
| REQ-MD-SEC-011 | Behavioral | P05 | P04 | `#[test]` — script/style content stripping |
| REQ-MD-SEC-020 | — (cross-ref) | Phase B | Phase B | [Phase B — Conditional] see REQ-MD-STREAM-011 |
| REQ-MD-SEC-021 | Behavioral | Phase B | Phase B | [Phase B — Conditional] panic with diagnostics |

### REQ-MD-TEST: Testing

| Req ID | Type | Impl Phase | Test Phase | Verification Method |
|--------|------|------------|------------|---------------------|
| REQ-MD-TEST-001 | Constraint | P04 (Parser TDD) | P04 | Self-verifying — tests exist |
| REQ-MD-TEST-002 | Constraint | P04 | P04 | Self-verifying — tests exist |
| REQ-MD-TEST-003 | Constraint | P04 | P04 | Self-verifying — tests exist |
| REQ-MD-TEST-004 | Constraint | P04 | P04 | Self-verifying — tests exist |
| REQ-MD-TEST-005 | Constraint | P04 | P04 | Self-verifying — tests exist |
| REQ-MD-TEST-010 | Constraint | P07 (Renderer TDD) | P07 | Self-verifying — tests exist |
| REQ-MD-TEST-011 | Constraint | P07 | P07 | Self-verifying — tests exist |
| REQ-MD-TEST-012 | Constraint | P07 | P07 | Self-verifying — tests exist |
| REQ-MD-TEST-013 | Constraint | P09 (Integration TDD) | P09 | Self-verifying — tests exist |
| REQ-MD-TEST-014 | Constraint | P07 | P07 | Self-verifying — tests exist |
| REQ-MD-TEST-020 | Constraint | Phase B | Phase B | [Phase B — Conditional] |
| REQ-MD-TEST-021 | Constraint | Phase B | Phase B | [Phase B — Conditional] |
| REQ-MD-TEST-022a | Constraint | Phase B | Phase B | [Phase B — Conditional] |
| REQ-MD-TEST-022b | Constraint | Phase B | Phase B | [Phase B — Conditional] |
| REQ-MD-TEST-022c | Constraint | Phase B | Phase B | [Phase B — Conditional] |
| REQ-MD-TEST-022d | Constraint | Phase B | Phase B | [Phase B — Conditional] |
| REQ-MD-TEST-022e | Constraint | Phase B | Phase B | [Phase B — Conditional] |
| REQ-MD-TEST-022f | Constraint | Phase B | Phase B | [Phase B — Conditional] |
| REQ-MD-TEST-022g | Constraint | Phase B | Phase B | [Phase B — Conditional] |
| REQ-MD-TEST-022h | Constraint | Phase B | Phase B | [Phase B — Conditional] |
| REQ-MD-TEST-022i | Constraint | Phase B | Phase B | [Phase B — Conditional] |
| REQ-MD-TEST-023 | Constraint | Phase B | Phase B | [Phase B — Conditional] |
| REQ-MD-TEST-024 | Constraint | Phase B | Phase B | [Phase B — Conditional] |
| REQ-MD-TEST-030 | Constraint | P09 | P09 | Self-verifying — tests exist |
| REQ-MD-TEST-031 | Constraint | P09 | P09 | Self-verifying — tests exist |
| REQ-MD-TEST-032 | Constraint | P09 | P09 | Self-verifying — tests exist |
| REQ-MD-TEST-033 | Constraint | P09 | P09 | Self-verifying — tests exist |
| REQ-MD-TEST-034 | Constraint | P09 | P09 | Self-verifying — tests exist |
| REQ-MD-TEST-035 | Constraint | P09 | P09 | Self-verifying — tests exist |
| REQ-MD-TEST-036 | Constraint | P09 | P09 | Self-verifying — tests exist |
| REQ-MD-TEST-040 | Constraint | P04 | P04 | Self-verifying — tests exist |
| REQ-MD-TEST-041 | Constraint | P04 | P04 | Self-verifying — tests exist |
| REQ-MD-TEST-042 | Constraint | P04 | P04 | Self-verifying — tests exist |
| REQ-MD-TEST-043 | Constraint | Phase B | Phase B | [Phase B — Conditional] |
| REQ-MD-TEST-044 | Constraint | P04 | P04 | Self-verifying — tests exist |
| REQ-MD-TEST-045 | Constraint | P07 | P07 | Self-verifying — tests exist |
| REQ-MD-TEST-046 | Constraint | P07 | P07 | Self-verifying — tests exist |
| REQ-MD-TEST-047 | Constraint | P04 | P04 | Self-verifying — tests exist |
| REQ-MD-TEST-050 | Constraint | All test phases | P12 | Code review — behavioral testing philosophy |
| REQ-MD-TEST-051 | Constraint | All test phases | P12 | Code review — test naming convention |
| REQ-MD-TEST-052 | Constraint | All test phases | P12 | Code review — no mocks |
| REQ-MD-TEST-053 | Constraint | All test phases | P12 | Code review — test pyramid balance |

---

## 2. Traceability Summary

| Group | Total Reqs | Phase A Reqs | Phase B Reqs | Behavioral | Constraint |
|-------|-----------|-------------|-------------|------------|------------|
| REQ-MD-PARSE | 29 | 29 | 0 | 23 | 6 |
| REQ-MD-RENDER | 25 | 25 | 0 | 19 | 6 |
| REQ-MD-INTEGRATE | 15 | 13 | 2 | 10 | 5 |
| REQ-MD-STREAM | 26 | 2 | 24 | 19 | 7 |
| REQ-MD-PERF | 13 | 11 | 2 | 9 | 4 |
| REQ-MD-SEC | 8 | 6 | 2 | 6 | 2 |
| REQ-MD-TEST | 31 | 22 | 9 | 0 | 31 |
| **Total** | **147** | **108** | **39** | **86** | **61** |

### Coverage Check

- [ ] All 108 Phase A requirements have an assigned Impl Phase (P03–P12)
- [ ] All 108 Phase A requirements have an assigned Test Phase (P04/P07/P09/P12)
- [ ] All 39 Phase B requirements are marked "[Phase B — Conditional]" and deferred
- [ ] No Phase A requirement depends on a Phase B requirement
- [ ] No Phase A implementation phase references mdstream

---

## 3. Fresh Preflight — Codebase State Snapshot

**Run these commands immediately before Phase 0.5 and record the output.**

### 3.1 Dependency State

```bash
# Record current Cargo.toml dependencies
grep -E '^\[dependencies\]' -A 50 Cargo.toml | head -60

# Verify pulldown-cmark is NOT yet present (will be added in P03)
grep "pulldown.cmark" Cargo.toml && echo "UNEXPECTED: pulldown-cmark already present" || echo "OK: pulldown-cmark not yet present"

# Verify url crate is NOT yet present (will be added in P03)
grep "^url " Cargo.toml && echo "UNEXPECTED: url already present" || echo "OK: url not yet present"

# Verify mdstream is NOT present (Phase B only)
grep "mdstream" Cargo.toml && echo "FAIL: mdstream present in Phase A" || echo "OK: mdstream not present"
```

### 3.2 File State

```bash
# Verify markdown_content.rs does NOT exist yet (will be created in P03)
ls src/ui_gpui/components/markdown_content.rs 2>/dev/null && echo "UNEXPECTED: file exists" || echo "OK: file does not exist"

# Verify current files that will be modified exist
ls src/ui_gpui/components/message_bubble.rs && echo "OK" || echo "MISSING"
ls src/ui_gpui/components/mod.rs && echo "OK" || echo "MISSING"
ls src/ui_gpui/views/chat_view/render.rs && echo "OK" || echo "MISSING"
ls src/ui_gpui/views/chat_view/mod.rs && echo "OK" || echo "MISSING"
```

### 3.3 GPUI API Verification

```bash
# Confirm GPUI pinned rev
grep -A2 'gpui' Cargo.toml | head -5

# Fetch dependencies
cargo fetch

# Find GPUI source path
GPUI_MANIFEST=$(cargo metadata --format-version=1 | jq -r '.packages[] | select(.name=="gpui") | .manifest_path')
echo "GPUI manifest: $GPUI_MANIFEST"
GPUI_SRC=$(dirname "$GPUI_MANIFEST")/src

# Verify critical APIs exist
echo "--- StyledText::with_runs ---"
grep -n 'fn with_runs' "$GPUI_SRC/elements/text.rs" | head -3

echo "--- InteractiveText::new ---"
grep -n 'fn new' "$GPUI_SRC/elements/text.rs" | grep -i interactive | head -3

echo "--- InteractiveText::on_click ---"
grep -n 'fn on_click' "$GPUI_SRC/elements/text.rs" | head -3

echo "--- grid/grid_cols ---"
grep -n 'fn grid\b\|fn grid_cols' "$GPUI_SRC/styled.rs" | head -5

echo "--- TextRun ---"
grep -n 'pub struct TextRun' "$GPUI_SRC/text_system.rs" | head -3

echo "--- Font ---"
grep -n 'pub struct Font' "$GPUI_SRC/text_system.rs" | head -3

echo "--- open_url ---"
grep -rn 'fn open_url' "$GPUI_SRC/platform/" | head -3
```

### 3.4 Build Health

```bash
# Current build state
cargo check 2>&1 | tail -5

# Current test state
cargo test --lib --tests 2>&1 | tail -10
```

### 3.5 Theme API Verification

```bash
# Verify expected Theme methods exist
grep -n 'pub fn bg_base\|pub fn bg_dark\|pub fn bg_darker\|pub fn text_primary\|pub fn text_muted\|pub fn accent\|pub fn border' src/ui_gpui/theme.rs
```

---

## 4. Canonical IR Schema Verification

The canonical `MarkdownBlock::List` definition is:

```rust
List { ordered: bool, start: u64, items: Vec<Vec<MarkdownBlock>> }
```

- `start` is `u64` (NOT `Option<u64>`)
- For unordered lists: `start: 0`
- For ordered lists: `start` equals the first item number (e.g., `1` for a standard `1. 2. 3.` list)
- Forbidden pattern: `Option<u64>` for list start in any doc/code/test.

### Consistency Check

Run these to verify no schema drift has been reintroduced:

```bash
# Check overview.md
grep -n "start.*Option\|start.*None\|start.*Some" project-plans/issue62/overview.md && echo "DRIFT FOUND" || echo "OK: overview.md consistent"

# Check requirements.md
grep -n "start.*Option\|start.*None\|start.*Some" project-plans/issue62/requirements.md && echo "DRIFT FOUND" || echo "OK: requirements.md consistent"

# Check all plan files
grep -rn "start.*Option\|start.*None\|start.*Some" project-plans/issue62/plan/ && echo "DRIFT FOUND" || echo "OK: plan files consistent"

# Confirm canonical definition present
grep -n "start: u64" project-plans/issue62/overview.md && echo "OK: canonical definition found" || echo "MISSING: canonical definition"
```

---

## 5. Phase B Conditional Language Verification

Verify no Phase B content reads as settled fact:

```bash
# Check that spec-phase-b.md has conditional banner
head -12 project-plans/issue62/spec-phase-b.md | grep -i "conditional\|pending\|tentative" && echo "OK" || echo "MISSING BANNER"

# Check that all Phase B requirements have conditional prefix
grep "Phase B" project-plans/issue62/requirements.md | grep -v "Conditional\|conditional\|pending\|Inactivity\|deferred\|CONDITIONAL" && echo "UNQUALIFIED Phase B REFERENCES FOUND" || echo "OK: all Phase B references qualified"

# Check plan files
grep "Phase B" project-plans/issue62/plan/*.md | grep -v "Conditional\|conditional\|pending\|Inactivity\|deferred\|CONDITIONAL\|do NOT\|NOT.*mdstream\|not.*mdstream\|shall not\|No.*Phase B\|No mdstream\|FAIL\|isolation" && echo "REVIEW NEEDED" || echo "OK: plan files qualified"
```

---

## 6. Reconciliation Sign-Off

| Check | Status | Verified By | Date |
|-------|--------|-------------|------|
| All Phase A reqs have impl+test phases | [ ] | | |
| All Phase B reqs marked conditional | [ ] | | |
| IR schema consistent across all docs | [ ] | | |
| Codebase snapshot recorded (§3) | [ ] | | |
| GPUI APIs verified at pinned rev (§3.3) | [ ] | | |
| Theme methods verified (§3.5) | [ ] | | |
| Build health confirmed (§3.4) | [ ] | | |
| Phase B language qualified (§5) | [ ] | | |
| No Phase A → Phase B dependency leaks | [ ] | | |

**This reconciliation must pass before Phase 0.5 begins.**

---

## 7. Expected Baseline Gap: `message_bubble.rs`

| Item | Status | Notes |
|------|--------|-------|
| `message_bubble.rs` has no parser hooks | [OK] PASS | This is the **expected starting state**, not an error. The file is pre-markdown baseline — `AssistantBubble::into_element()` currently renders raw text via `.child(content_text)`. The plan explicitly accounts for this: Phase 9 (Integration TDD) writes tests against the integration API, Phase 10 (Integration Stub) adds the module export, and Phase 11 (Integration Impl) modifies `message_bubble.rs` to use `parse_markdown_blocks()` and `blocks_to_elements()`. No parser import or call should exist in this file until Phase 11. |

**Pass criteria:** This item passes because the plan's phase sequence (P09 → P10 → P11) accounts for the gap. If `message_bubble.rs` already had parser hooks before Phase 11, that would be an *unexpected* state requiring investigation.
