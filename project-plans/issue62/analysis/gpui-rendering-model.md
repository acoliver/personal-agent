# GPUI Rendering Model Analysis

**Phase:** 01 - Domain Analysis  
**Artifact ID:** gpui-rendering-model.md  
**Plan ID:** PLAN-20260402-MARKDOWN.P01

---

## 1. Overview

This document analyzes the GPUI rendering APIs available for markdown rendering. It covers `StyledText`, `InteractiveText`, `TextRun`, grid layout, and the element composition model.

---

## 2. Text Rendering APIs

### 2.1 StyledText

**Location:** GPUI `elements/text.rs`  
**Usage:** Static styled text with per-run formatting

```rust
// Construction via builder
StyledText::with_runs(text: SharedString, runs: Vec<TextRun>, window, cx)
    -> StyledText
```

**Key characteristics:**
- `text`: Complete text content as `SharedString`
- `runs`: Vec of `TextRun` defining style per segment
- Runs must cover the entire text (no gaps)
- Run lengths are **UTF-8 byte counts**, not char counts

### 2.2 InteractiveText

**Location:** GPUI `elements/text.rs:665`  
**Usage:** Clickable regions within text (for links)

```rust
// Construction
InteractiveText::new(
    element_id: ElementId,
    text: SharedString,
    runs: Vec<TextRun>,
    click_ranges: Vec<Range<usize>>,  // Byte ranges
)
```

**Interaction handling:**
```rust
interactive_text.on_click(cx, |this, range_index, window, cx| {
    // range_index indicates which click_ranges entry was clicked
})
```

**Key constraints:**
- `click_ranges` are byte positions (not char positions)
- Ranges must not overlap
- Click handler receives the index of the clicked range

---

## 3. TextRun Structure

### 3.1 Definition

**Location:** `text_system.rs:752-765`

```rust
pub struct TextRun {
    pub len: usize,                    // UTF-8 byte count
    pub font: Font,                    // Font specification
    pub color: Hsla,                   // Text color
    pub background_color: Option<Hsla>, // Background highlight
    pub underline: Option<UnderlineStyle>, // Link underline
    pub strikethrough: Option<StrikethroughStyle>, // Strikethrough
}
```

### 3.2 Length Calculation

Critical: `len` is **UTF-8 bytes**, not characters.

```rust
// Correct: byte length
let text = "Hello";
let len = text.len(); // 5 (bytes = chars for ASCII)

// Correct: for multi-byte UTF-8
let text = "Héllo";  // é is 2 bytes
let len = text.len(); // 6 bytes, but 5 chars

// Incorrect: char count
let char_count = text.chars().count(); // 5 - NOT what GPUI wants
```

### 3.3 Run Construction Example

```rust
let runs = vec![
    TextRun {
        len: 5,  // "Hello"
        font: Font::system_font(),
        color: Theme::text_primary(),
        background_color: None,
        underline: None,
        strikethrough: None,
    },
    TextRun {
        len: 5,  // "World"
        font: Font::system_font().with_weight(FontWeight::BOLD),
        color: Theme::text_primary(),
        background_color: None,
        underline: None,
        strikethrough: None,
    },
];
```

---

## 4. Font System

### 4.1 Font Struct

**Location:** `text_system.rs:808+`

```rust
pub struct Font {
    pub family: SharedString,          // e.g., "Menlo", ".SystemUIFont"
    pub features: FontFeatures,        // OpenType features
    pub fallbacks: Option<FontFallbacks>,
    pub weight: FontWeight,           // Weight constant
    pub style: FontStyle,             // Normal, Italic, Oblique
}
```

### 4.2 Font Weight Constants

**Location:** `text_system.rs:695`

```rust
pub struct FontWeight(pub u16);

impl FontWeight {
    pub const THIN: FontWeight = FontWeight(100);
    pub const EXTRA_LIGHT: FontWeight = FontWeight(200);
    pub const LIGHT: FontWeight = FontWeight(300);
    pub const NORMAL: FontWeight = FontWeight(400);
    pub const MEDIUM: FontWeight = FontWeight(500);
    pub const SEMIBOLD: FontWeight = FontWeight(600);
    pub const BOLD: FontWeight = FontWeight(700);
    pub const EXTRA_BOLD: FontWeight = FontWeight(800);
    pub const BLACK: FontWeight = FontWeight(900);
}
```

### 4.3 Font Style

**Location:** `text_system.rs:739`

```rust
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}
```

### 4.4 Font Families for Markdown

| Usage | Family | Fallback |
|-------|--------|----------|
| Body text | `.SystemUIFont` | System default |
| Code blocks | `"Menlo"` | GPUI built-in fallback |
| Monospace inline | `"Menlo"` | GPUI built-in fallback |

**Note:** Per REQ-MD-RENDER-026, rely on GPUI's built-in font fallback chain.

---

## 5. Grid Layout (for Tables)

### 5.1 CSS Grid API

**Location:** `styled.rs:52`, `styled.rs:650`

```rust
// Grid container
div()
    .grid()                      // Enable grid layout
    .grid_cols(n)               // Set column count
    .grid_rows(n)               // Optional: set row count
    .gap(px(4.0))               // Gap between cells
    .children(cells)           // Cell elements
```

### 5.2 Table Cell Construction

```rust
// Header cell
let header_cell = div()
    .bg(Theme::bg_dark())
    .px(px(8.0))
    .py(px(4.0))
    .border_1()
    .border_color(Theme::border())
    .child(styled_header_text);

// Body cell
let body_cell = div()
    .px(px(8.0))
    .py(px(4.0))
    .border_1()
    .border_color(Theme::border())
    .child(styled_body_text);

// Alternating row background
let body_cell_even = div()
    .bg(Theme::bg_base())  // Alternating stripe
    // ... rest same as odd
```

### 5.3 Column Alignment

GPUI grid doesn't directly support cell-level alignment. Alignment is applied via text styling within cells:

```rust
// Left aligned (default)
.text_left()

// Center aligned
.text_center()

// Right aligned
.text_right()
```

---

## 6. Element Composition

### 6.1 Div Builder Pattern

GPUI uses a fluent builder API for element construction:

```rust
div()
    .flex()                    // Enable flex layout
    .flex_col()               // Column direction
    .gap(px(8.0))             // Gap between children
    .p(px(12.0))              // Padding
    .px(px(12.0))             // Horizontal padding only
    .py(px(8.0))              // Vertical padding only
    .w(px(400.0))             // Fixed width
    .max_w(px(300.0))         // Max width
    .h(px(100.0))             // Fixed height
    .rounded(px(8.0))         // Border radius color
    .
    .bg(border_1()               // Theme::bg1px border_darker())  
    .border_color(Theme:: // Backgroundborder())
    .child(element)           // Single child
    .children(elements)      // Multiple children
```

### 6.2 IntoElement Trait

**Pattern for component conversion:**

```rust
impl IntoElement for MyComponent {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        div()
            // ... configuration
            .child(self.content)
    }
}
```

### 6.3 Element Wrapping

For markdown blocks that need special handling (links), choose between:

```rust
// Option 1: Plain styled text (no links)
div().child(StyledText::with_runs(text, runs, window, cx))

// Option 2: Interactive text (with links)
div().child(
    InteractiveText::new(id, text, runs, click_ranges)
        .on_click(cx, |this, idx, window, cx| { ... })
)

// Option 3: Container with multiple elements (for block-level)
div()
    .flex()
    .flex_col()
    .gap(px(8.0))
    .children(block_elements)
```

---

## 7. Color System

### 7.1 Hsla Type

GPUI uses `Hsla` for colors:

```rust
pub struct Hsla {
    pub h: f32,  // Hue (0.0 to 1.0)
    pub s: f32,  // Saturation (0.0 to 1.0)
    pub l: f32,  // Lightness (0.0 to 1.0)
    pub a: f32,  // Alpha (0.0 to 1.0)
}
```

### 7.2 Available Theme Methods

| Method | Location | Usage |
|--------|----------|-------|
| `Theme::text_primary()` | theme.rs | Body text, headings |
| `Theme::text_muted()` | theme.rs | Muted elements, bullets, code lang |
| `Theme::text_secondary()` | theme.rs | Alias for muted |
| `Theme::bg_base()` | theme.rs | Table alternating rows |
| `Theme::bg_dark()` | theme.rs | Table header, user bubbles |
| `Theme::bg_darker()` | theme.rs | Code blocks, blockquotes, assistant bubbles |
| `Theme::accent()` | theme.rs | Links, blockquote border |
| `Theme::border()` | theme.rs | Table borders, thematic break |

### 7.3 Color Mapping for Markdown

| Element | Theme Method | Requirement |
|---------|--------------|-------------|
| Body text | `text_primary()` | REQ-MD-RENDER-031 |
| Link text | `accent()` | REQ-MD-RENDER-024 |
| Code block bg | `bg_darker()` | REQ-MD-RENDER-005 |
| Inline code bg | `bg_darker()` | REQ-MD-RENDER-023 |
| Blockquote border | `accent()` | REQ-MD-RENDER-007 |
| Blockquote bg | `bg_base()` | REQ-MD-RENDER-007 |
| Table header | `bg_dark()` | REQ-MD-RENDER-051 |
| Table border | `border()` | REQ-MD-RENDER-053 |
| Table stripe | `bg_base()` | REQ-MD-RENDER-052 |
| Muted elements | `text_muted()` | REQ-MD-RENDER-025 |

---

## 8. Link Click Handling

### 8.1 URL Opening

```rust
// From window context
cx.open_url("https://example.com");
```

**Location:** `platform/mac/platform.rs:626`

### 8.2 Clipboard Operations

```rust
// Write to clipboard
cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));

// Read from clipboard
if let Some(item) = cx.read_from_clipboard() {
    if let Some(text) = item.text() {
        // use text
    }
}
```

**Used in:** `render.rs:178` (cmd+c handling), `render.rs:298` (click-to-copy)

---

## 9. Key Constraints Summary

### 9.1 TextRun Constraints

| Constraint | Impact |
|------------|--------|
| `len` is UTF-8 bytes | Must convert char positions from pulldown-cmark |
| Runs must cover all text | Need run for every text segment |
| No overlapping runs | Adjacent spans only |

### 9.2 InteractiveText Constraints

| Constraint | Impact |
|------------|--------|
| Byte ranges | Same UTF-8 requirement as TextRun |
| No overlap | Links cannot overlap in markdown |
| Index-based callback | Must map index to URL |

### 9.3 Grid Constraints

| Constraint | Impact |
|------------|--------|
| Explicit column count | Must count header cells |
| No automatic alignment | Apply per-cell text alignment |
| Flat structure | All cells at same level |

---

## 10. Rendering Strategy

### 10.1 Block-to-Element Mapping

| Block Type | GPUI Element |
|------------|--------------|
| Paragraph | `div().child(styled_text)` or `div().child(interactive_text)` |
| Heading | `div().child(styled_text_with_size)` |
| Code block | `div().bg(bg_darker()).rounded().child(styled_code)` |
| Blockquote | `div().border_l_2().border_color(accent()).child(children)` |
| List | `div().flex_col().gap().children(items)` |
| Table | `div().grid().grid_cols(n).children(cells)` |
| Thematic break | `div().h(px(1.0)).bg(border()).w_full()` |
| Image fallback | `div().text_muted().child("[image: alt]")` |

### 10.2 Style Stacking

For nested inline styles (bold within italic):

```rust
// Build TextRun with combined styles
TextRun {
    font: Font::system_font()
        .with_weight(FontWeight::BOLD)
        .with_style(FontStyle::Italic),
    color: Theme::text_primary(),
    // ...
}
```

### 10.3 Font Per Run

Inline code requires monospace font per-run (cannot use HighlightStyle):

```rust
let code_font = Font::system_font().with_family("Menlo".into());
```

---

## 11. Summary

The GPUI rendering model provides:

1. **StyledText**: For static, multi-styled text blocks
2. **InteractiveText**: For clickable link regions
3. **TextRun**: Granular style control with UTF-8 byte lengths
4. **Grid layout**: For table rendering
5. **Div builder**: For container composition

Key implementation notes:
- **Byte offsets everywhere** for text ranges
- **Menlo for monospace**, system font for body
- **Theme methods** for all color values
- **Builder pattern** for element construction
