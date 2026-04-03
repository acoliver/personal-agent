# IR Type Design Analysis

**Phase:** 01 - Domain Analysis  
**Artifact ID:** ir-type-design.md  
**Plan ID:** PLAN-20260402-MARKDOWN.P01

---

## 1. Overview

This document specifies the Intermediate Representation (IR) type design for markdown rendering. The IR provides a pure data layer between pulldown-cmark parsing and GPUI rendering, with no GPUI dependencies.

---

## 2. Design Goals

1. **GPUI-agnostic**: IR types must not depend on GPUI
2. **Testable**: Types must support equality assertions for testing
3. **Cloneable**: Types must be cheaply cloneable for GPUI's reactive model
4. **Complete**: Must represent all supported markdown constructs
5. **Link-aware**: Must track clickable link ranges for InteractiveText

---

## 3. MarkdownInline

### 3.1 Purpose

Represents a single span of inline text with uniform styling. Multiple `MarkdownInline` instances form the content of paragraphs, headings, table cells, etc.

### 3.2 Structure

```rust
/// A single inline text span with style flags.
/// 
/// Represents a segment of text with uniform styling. Multiple spans
/// compose the content of block-level elements.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MarkdownInline {
    /// The text content of this span.
    pub text: String,
    
    /// Bold style flag (from `**text**` or `__text__`).
    pub bold: bool,
    
    /// Italic style flag (from `*text*` or `_text_`).
    pub italic: bool,
    
    /// Strikethrough flag (from `~~text~~`).
    pub strikethrough: bool,
    
    /// Inline code flag (from `` `text` ``).
    /// When true, text should render with monospace font.
    pub code: bool,
    
    /// Link URL for clickable text (from `[text](url)`).
    /// When Some, this span is part of a link.
    pub link_url: Option<String>,
}
```

### 3.3 Style Flag Combinations

| Markdown | `bold` | `italic` | `strikethrough` | `code` | `link_url` |
|----------|--------|----------|-----------------|--------|------------|
| `**bold**` | true | false | false | false | None |
| `*italic*` | false | true | false | false | None |
| `***both***` | true | true | false | false | None |
| `~~strike~~` | false | false | true | false | None |
| `` `code` `` | false | false | false | true | None |
| `[link](url)` | false | false | false | false | Some("url") |
| `[**bold link**](url)` | true | false | false | false | Some("url") |

### 3.4 Default Constructor

```rust
impl MarkdownInline {
    /// Create a plain text span with all flags false.
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bold: false,
            italic: false,
            strikethrough: false,
            code: false,
            link_url: None,
        }
    }
}
```

---

## 4. MarkdownBlock

### 4.1 Purpose

Represents a block-level markdown construct. The top-level parse result is `Vec<MarkdownBlock>`.

### 4.2 Enum Definition

```rust
/// A block-level markdown element.
///
/// This enum represents all supported block-level constructs from the
/// markdown input. The parser produces a `Vec<MarkdownBlock>` from input text.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MarkdownBlock {
    /// A paragraph containing inline spans.
    Paragraph {
        /// The inline content of the paragraph.
        spans: Vec<MarkdownInline>,
        /// Clickable link ranges with their URLs.
        /// Each tuple is (byte_range, url) for InteractiveText.
        links: Vec<(Range<usize>, String)>,
    },
    
    /// A heading with level 1-6.
    Heading {
        /// Heading level (1 = H1, 6 = H6).
        level: u8,
        /// The inline content of the heading.
        spans: Vec<MarkdownInline>,
        /// Clickable link ranges with their URLs.
        links: Vec<(Range<usize>, String)>,
    },
    
    /// A code block (fenced or indented).
    CodeBlock {
        /// The language identifier from the fence (e.g., "rust").
        /// None for indented code blocks.
        language: Option<String>,
        /// The code content (raw text, not parsed as markdown).
        code: String,
    },
    
    /// A blockquote containing nested blocks.
    BlockQuote {
        /// The nested block content.
        blocks: Vec<MarkdownBlock>,
    },
    
    /// A list (ordered or unordered).
    List {
        /// true for numbered lists (1., 2., ...).
        /// false for bullet lists (-, *, +).
        ordered: bool,
        /// Starting number for ordered lists (default: 0 for unordered).
        start: u64,
        /// List items, where each item contains its block content.
        items: Vec<Vec<MarkdownBlock>>,
    },
    
    /// A table with header and body rows.
    Table {
        /// Column alignments (left, center, right, or none).
        /// Length equals column count.
        alignments: Vec<Alignment>,
        /// Header row cells, each containing inline content.
        header: Vec<TableCell>,
        /// Body rows, each containing cells with inline content.
        rows: Vec<Vec<TableCell>>,
    },
    
    /// A horizontal rule (thematic break).
    ThematicBreak,
    
    /// An image rendered as fallback text.
    /// Per REQ-MD-PARSE-040, images are not rendered.
    ImageFallback {
        /// The alt text extracted from the image markdown.
        alt: String,
    },
}
```

### 4.3 TableCell Helper Type

```rust
/// A single table cell containing inline content.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TableCell {
    /// The inline content of the cell.
    pub spans: Vec<MarkdownInline>,
    /// Clickable link ranges with their URLs.
    pub links: Vec<(Range<usize>, String)>,
}
```

### 4.4 Alignment Enum

```rust
/// Text alignment for table columns.
///
/// Maps to pulldown-cmark's Alignment type.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Alignment {
    /// Default/no alignment specified.
    None,
    /// Left-aligned (|:---).
    Left,
    /// Center-aligned (:|---:).
    Center,
    /// Right-aligned (---:|).
    Right,
}

impl From<pulldown_cmark::Alignment> for Alignment {
    fn from(a: pulldown_cmark::Alignment) -> Self {
        match a {
            pulldown_cmark::Alignment::None => Alignment::None,
            pulldown_cmark::Alignment::Left => Alignment::Left,
            pulldown_cmark::Alignment::Center => Alignment::Center,
            pulldown_cmark::Alignment::Right => Alignment::Right,
        }
    }
}
```

---

## 5. Type Derive Requirements

Per REQ-MD-PARSE-060:

| Type | `Debug` | `Clone` | `PartialEq` | Reason |
|------|---------|---------|-------------|--------|
| `MarkdownBlock` | [OK] | [OK] | [OK] | Test assertions, debug logging |
| `MarkdownInline` | [OK] | [OK] | [OK] | Test assertions, span comparison |
| `TableCell` | [OK] | [OK] | [OK] | Test assertions, table comparison |
| `Alignment` | [OK] | [OK] | [OK] | Test assertions, alignment comparison |

---

## 6. Visibility

Per REQ-MD-PARSE-064:

All IR types have `pub(crate)` visibility — they are internal implementation details, not part of the public API.

```rust
pub(crate) struct MarkdownInline { ... }
pub(crate) enum MarkdownBlock { ... }
pub(crate) struct TableCell { ... }
pub(crate) enum Alignment { ... }
```

The public API is a single function:

```rust
pub fn render_markdown(content: &str) -> Vec<gpui::AnyElement> {
    let blocks = parse_markdown_blocks(content);
    blocks_to_elements(&blocks)
}
```

---

## 7. Block Variant Coverage

### 7.1 Supported Block Constructs

| Markdown | Variant | Requirements |
|----------|---------|--------------|
| Plain text paragraph | `Paragraph` | REQ-MD-PARSE-002 |
| `## Heading` | `Heading { level: 2, ... }` | REQ-MD-PARSE-003 |
| ` ```code``` ` | `CodeBlock` | REQ-MD-PARSE-004, REQ-MD-PARSE-005 |
| `> quote` | `BlockQuote` | REQ-MD-PARSE-006 |
| `- item` / `1. item` | `List` | REQ-MD-PARSE-007, REQ-MD-PARSE-008 |
| `\| table \|` | `Table` | REQ-MD-PARSE-009 |
| `---` | `ThematicBreak` | REQ-MD-PARSE-010 |
| `![alt](url)` | `ImageFallback` | REQ-MD-PARSE-040 |

### 7.2 Supported Inline Constructs

| Markdown | Inline Properties |
|----------|-------------------|
| `**bold**` | `bold: true` |
| `*italic*` | `italic: true` |
| `~~strike~~` | `strikethrough: true` |
| `` `code` `` | `code: true` |
| `[link](url)` | `link_url: Some("url")` |

---

## 8. Link Collection Design

### 8.1 Per-Block Link Storage

Links are stored as `(Range<usize>, String)` tuples:

- `Range<usize>`: Byte offsets into the accumulated text
- `String`: The URL from the link destination

### 8.2 Blocks with Links

| Block | Link Storage Location |
|-------|----------------------|
| `Paragraph` | `links` field |
| `Heading` | `links` field |
| `TableCell` | `links` field |
| `BlockQuote` | In nested blocks (recursively) |
| `List` | In item blocks (recursively) |

### 8.3 Byte Range Calculation

Critical: pulldown-cmark provides character offsets, but GPUI requires **byte offsets**.

```rust
// Conversion required
let char_pos = event_position; // from pulldown-cmark
let byte_pos = text[..char_pos].len(); // UTF-8 byte count
```

### 8.4 InteractiveText Construction

```rust
// From a Paragraph or Heading
let text = concatenate_span_texts(&spans);
let runs = build_text_runs(&spans);
let click_ranges: Vec<Range<usize>> = links.iter().map(|(r, _)| r.clone()).collect();

InteractiveText::new(
    ElementId::Name("paragraph-1".into()),
    text.into(),
    runs,
    click_ranges,
).on_click(cx, |this, idx, window, cx| {
    let url = &links[idx].1;
    // Open URL...
})
```

---

## 9. Usage Examples

### 9.1 Simple Paragraph

```rust
// Input: "Hello **world**"
MarkdownBlock::Paragraph {
    spans: vec![
        MarkdownInline { text: "Hello ".into(), bold: false, italic: false, strikethrough: false, code: false, link_url: None },
        MarkdownInline { text: "world".into(), bold: true, italic: false, strikethrough: false, code: false, link_url: None },
    ],
    links: vec![],
}
```

### 9.2 Paragraph with Link

```rust
// Input: "Click [here](https://example.com) now"
MarkdownBlock::Paragraph {
    spans: vec![
        MarkdownInline { text: "Click ".into(), ..MarkdownInline::plain("") },
        MarkdownInline { text: "here".into(), link_url: Some("https://example.com".into()), .. },
        MarkdownInline { text: " now".into(), ..MarkdownInline::plain("") },
    ],
    links: vec![(5..9, "https://example.com".into())], // Byte range of "here"
}
```

### 9.3 Code Block

```rust
// Input: ```rust\nfn main() {}\n```
MarkdownBlock::CodeBlock {
    language: Some("rust".into()),
    code: "fn main() {}".into(),
}
```

### 9.4 List

```rust
// Input: "- Item 1\n- Item 2"
MarkdownBlock::List {
    ordered: false,
    start: 0,
    items: vec![
        vec![MarkdownBlock::Paragraph { 
            spans: vec![MarkdownInline::plain("Item 1")], 
            links: vec![] 
        }],
        vec![MarkdownBlock::Paragraph { 
            spans: vec![MarkdownInline::plain("Item 2")], 
            links: vec![] 
        }],
    ],
}
```

---

## 10. Summary

The IR type design provides:

1. **`MarkdownInline`**: Styled text spans with boolean flags
2. **`MarkdownBlock`**: Block-level enum with variants for all supported constructs
3. **Link tracking**: `(Range<usize>, String)` tuples at appropriate nesting levels
4. **Table support**: `TableCell` and `Alignment` helper types
5. **Derives**: `Debug`, `Clone`, `PartialEq` for testability
6. **Visibility**: `pub(crate)` as internal implementation detail

This design satisfies all parsing requirements (REQ-MD-PARSE-060 through REQ-MD-PARSE-065) and enables straightforward GPUI rendering.
