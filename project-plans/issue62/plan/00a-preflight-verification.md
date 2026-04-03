# Phase 0.5: Preflight Verification

## Phase ID

`PLAN-20260402-MARKDOWN.P0.5`

## Purpose

Verify ALL assumptions before writing any code. This phase MUST complete before any implementation phase begins.

---

## Dependency Verification

### Phase A Dependencies

| Dependency | Version | Verification Command | Expected Output | Status |
|------------|---------|---------------------|-----------------|--------|
| `pulldown-cmark` | `0.13` | `cargo tree -p pulldown-cmark` (after adding to Cargo.toml) | Shows 0.13.x installed | [ ] |
| `url` | `2` | `cargo tree -p url` (after adding to Cargo.toml) | Shows 2.x installed | [ ] |

### Verification Steps

```bash
# 1. Temporarily add dependencies to Cargo.toml
# pulldown-cmark = "0.13"
# url = "2"

# 2. Fetch and verify
cargo fetch
cargo tree -p pulldown-cmark --depth 0
cargo tree -p url --depth 0

# 3. Verify compilation
cargo check

# 4. If any dependency fails to compile, STOP and update the plan.
```

### Phase B Dependencies [Phase B — Conditional] (verification deferred — record for awareness only)

| Dependency | Version | Status |
|------------|---------|--------|
| `mdstream` | `= 0.2.0` | DEFERRED — Phase B (Conditional, pending validation gate) only; do NOT add yet |

---

## GPUI API Verification Against Pinned Rev

The project pins GPUI to rev `c67328ab2e0d572718575e02ae07db37552e1cbe`. All API assumptions MUST be verified against this exact rev.

### Step 1: Locate GPUI Source

```bash
# Get GPUI manifest path
cargo metadata --format-version=1 | jq -r '.packages[] | select(.name=="gpui") | .manifest_path'

# Store for subsequent searches
GPUI_SRC=$(dirname $(cargo metadata --format-version=1 | jq -r '.packages[] | select(.name=="gpui") | .manifest_path'))/src
echo "GPUI source at: $GPUI_SRC"
```

### Step 2: Verify Each API

| API | File | Search Command | Expected | Match? |
|-----|------|----------------|----------|--------|
| `StyledText::with_runs()` | `elements/text.rs` | `grep -n 'fn with_runs' "$GPUI_SRC/elements/text.rs"` | `pub fn with_runs(mut self, runs: Vec<TextRun>) -> Self` | [ ] |
| `InteractiveText::new()` | `elements/text.rs` | `grep -n 'fn new.*ElementId.*StyledText' "$GPUI_SRC/elements/text.rs"` | Constructor taking `ElementId` + `StyledText` | [ ] |
| `InteractiveText::on_click()` | `elements/text.rs` | `grep -n 'fn on_click' "$GPUI_SRC/elements/text.rs"` | Takes `Vec<Range<usize>>` + listener | [ ] |
| `div().grid()` | `styled.rs` | `grep -n 'fn grid\b' "$GPUI_SRC/styled.rs"` | `fn grid(mut self) -> Self` | [ ] |
| `div().grid_cols()` | `styled.rs` | `grep -n 'fn grid_cols' "$GPUI_SRC/styled.rs"` | `fn grid_cols(mut self, cols: u16) -> Self` | [ ] |
| `TextRun` struct | `text_system.rs` | `grep -n 'pub struct TextRun' "$GPUI_SRC/text_system.rs"` | Fields: `len`, `font`, `color`, `background_color`, `underline`, `strikethrough` | [ ] |
| `Font` struct | `text_system.rs` | `grep -n 'pub struct Font' "$GPUI_SRC/text_system.rs"` | Fields: `family`, `features`, `fallbacks`, `weight`, `style` | [ ] |
| `StrikethroughStyle` | `style.rs` | `grep -n 'pub struct StrikethroughStyle' "$GPUI_SRC/style.rs"` | Fields: `thickness`, `color` | [ ] |
| `cx.open_url()` | `platform/mac/platform.rs` | `grep -n 'fn open_url' "$GPUI_SRC/platform/mac/platform.rs"` | Exists, uses NSWorkspace | [ ] |
| `FontWeight::BOLD` | `text_system.rs` | `grep -n 'BOLD' "$GPUI_SRC/text_system.rs"` | Constant exists | [ ] |
| `FontStyle::Italic` | `text_system.rs` | `grep -n 'Italic' "$GPUI_SRC/text_system.rs"` | Variant exists | [ ] |
| `UnderlineStyle` | `style.rs` | `grep -n 'pub struct UnderlineStyle' "$GPUI_SRC/style.rs"` | Struct exists | [ ] |

### Step 3: Verify TextRun Field Details

```bash
# Verify TextRun has all expected fields
grep -A 20 'pub struct TextRun' "$GPUI_SRC/text_system.rs"

# Verify Font has family: SharedString
grep -A 10 'pub struct Font' "$GPUI_SRC/text_system.rs"

# Verify InteractiveText on_click signature
grep -A 5 'fn on_click' "$GPUI_SRC/elements/text.rs"
```

---

## Type/Interface Verification

### Existing Types That Plan Depends On

| Type Name | Expected Location | Search Command | Match? |
|-----------|-------------------|----------------|--------|
| `Theme` struct | `src/ui_gpui/theme.rs` | `grep -n "pub struct Theme" src/ui_gpui/theme.rs` | [ ] |
| `Theme::text_primary()` | `src/ui_gpui/theme.rs` | `grep -n "fn text_primary" src/ui_gpui/theme.rs` | [ ] |
| `Theme::text_muted()` | `src/ui_gpui/theme.rs` | `grep -n "fn text_muted" src/ui_gpui/theme.rs` | [ ] |
| `Theme::bg_darker()` | `src/ui_gpui/theme.rs` | `grep -n "fn bg_darker" src/ui_gpui/theme.rs` | [ ] |
| `Theme::bg_dark()` | `src/ui_gpui/theme.rs` | `grep -n "fn bg_dark" src/ui_gpui/theme.rs` | [ ] |
| `Theme::bg_base()` | `src/ui_gpui/theme.rs` | `grep -n "fn bg_base" src/ui_gpui/theme.rs` | [ ] |
| `Theme::accent()` | `src/ui_gpui/theme.rs` | `grep -n "fn accent" src/ui_gpui/theme.rs` | [ ] |
| `Theme::border()` | `src/ui_gpui/theme.rs` | `grep -n "fn border" src/ui_gpui/theme.rs` | [ ] |
| `Theme::assistant_bubble()` | `src/ui_gpui/theme.rs` | `grep -n "fn assistant_bubble" src/ui_gpui/theme.rs` | [ ] |
| `Theme::FONT_SIZE_MD` | `src/ui_gpui/theme.rs` | `grep -n "FONT_SIZE_MD" src/ui_gpui/theme.rs` | [ ] |
| `Theme::FONT_SIZE_LG` | `src/ui_gpui/theme.rs` | `grep -n "FONT_SIZE_LG" src/ui_gpui/theme.rs` | [ ] |
| `Theme::FONT_SIZE_BASE` | `src/ui_gpui/theme.rs` | `grep -n "FONT_SIZE_BASE" src/ui_gpui/theme.rs` | [ ] |
| `Theme::SPACING_MD` | `src/ui_gpui/theme.rs` | `grep -n "SPACING_MD" src/ui_gpui/theme.rs` | [ ] |
| `Theme::SPACING_SM` | `src/ui_gpui/theme.rs` | `grep -n "SPACING_SM" src/ui_gpui/theme.rs` | [ ] |
| `Theme::RADIUS_LG` | `src/ui_gpui/theme.rs` | `grep -n "RADIUS_LG" src/ui_gpui/theme.rs` | [ ] |
| `AssistantBubble` struct | `src/ui_gpui/components/message_bubble.rs` | `grep -n "pub struct AssistantBubble" src/ui_gpui/components/message_bubble.rs` | [ ] |
| `AssistantBubble::new()` | `src/ui_gpui/components/message_bubble.rs` | `grep -n "fn new" src/ui_gpui/components/message_bubble.rs` | [ ] |
| `AssistantBubble::streaming()` | `src/ui_gpui/components/message_bubble.rs` | `grep -n "fn streaming" src/ui_gpui/components/message_bubble.rs` | [ ] |
| `ChatMessage` struct | `src/ui_gpui/views/chat_view/state.rs` | `grep -rn "struct ChatMessage" src/ui_gpui/` | [ ] |

---

## Call Path Verification

### Integration Touch Points

| Function | Expected Caller | Search Command | Evidence |
|----------|-----------------|----------------|----------|
| `render_assistant_message()` | `render_chat_area()` in `render.rs` | `grep -n "render_assistant_message" src/ui_gpui/views/chat_view/render.rs` | [ ] |
| `AssistantBubble::new()` | `render_chat_area()` streaming path | `grep -n "AssistantBubble::new" src/ui_gpui/views/chat_view/render.rs` | [ ] |
| `into_element()` on `AssistantBubble` | `render_chat_area()` via `.child(bubble)` | `grep -n "bubble\|AssistantBubble" src/ui_gpui/views/chat_view/render.rs` | [ ] |
| `cx.write_to_clipboard()` | `render_assistant_message()` on_click handler | `grep -n "write_to_clipboard" src/ui_gpui/views/chat_view/render.rs` | [ ] |
| `cx.open_url()` | Will be called from `InteractiveText` on_click | Verify GPUI has this API (see GPUI verification above) | [ ] |

---

## Test Infrastructure Verification

| Component | Test File Exists? | Verification Command | Status |
|-----------|-------------------|---------------------|--------|
| Components test location | Check where component tests live | `find src -name "*test*" -path "*component*"` | [ ] |
| GPUI test harness | `#[gpui::test]` available | `grep -rn "gpui::test" src/` | [ ] |
| Existing test pattern | Review test module structure | `grep -rn "#\[cfg(test)\]" src/ui_gpui/` | [ ] |
| Test binary | `gui_main_thread` test binary | `ls tests/` | [ ] |

```bash
# Verify GPUI test support is available
grep "test-support" Cargo.toml
# Expected: gpui features include "test-support"

# Verify existing test patterns
grep -rn "#\[cfg(test)\]" src/ui_gpui/ | head -20
grep -rn "#\[gpui::test\]" src/ | head -10
grep -rn "#\[test\]" src/ | head -20
```

---

## pulldown-cmark API Verification

After adding `pulldown-cmark = "0.13"` to Cargo.toml, verify the event types:

```bash
# Find pulldown-cmark source
PC_SRC=$(dirname $(cargo metadata --format-version=1 | jq -r '.packages[] | select(.name=="pulldown-cmark") | .manifest_path'))/src

# Verify Event enum variants
grep -n 'pub enum Event' "$PC_SRC/lib.rs"
grep -n 'Start\|End\|Text\|Code\|Html\|InlineHtml\|SoftBreak\|HardBreak\|Rule\|TaskListMarker\|FootnoteReference\|InlineMath\|DisplayMath' "$PC_SRC/lib.rs" | head -30

# Verify Tag enum has expected variants
grep -n 'pub enum Tag' "$PC_SRC/lib.rs"
grep -n 'Paragraph\|Heading\|CodeBlock\|BlockQuote\|List\|Item\|Table\|TableHead\|TableRow\|TableCell\|Strong\|Emphasis\|Strikethrough\|Link\|Image\|HtmlBlock' "$PC_SRC/lib.rs" | head -30

# Verify Options
grep -n 'ENABLE_TABLES\|ENABLE_STRIKETHROUGH\|ENABLE_TASKLISTS' "$PC_SRC/lib.rs"
```

---

## Blocking Issues Found

[To be populated during preflight execution. If any issues are found, they must be resolved before proceeding to Phase 01.]

---

## Verification Gate

- [ ] All Phase A dependencies compile (`pulldown-cmark`, `url`)
- [ ] All GPUI APIs verified against pinned rev
- [ ] All Theme methods exist as expected
- [ ] `AssistantBubble` API matches plan assumptions
- [ ] `render_assistant_message()` call path confirmed
- [ ] Test infrastructure ready
- [ ] pulldown-cmark event/tag API matches assumptions
- [ ] No blocking issues found

**IF ANY CHECKBOX IS UNCHECKED: STOP and update plan before proceeding.**
