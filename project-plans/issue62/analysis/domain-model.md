# Domain Model: Markdown Rendering

**Phase:** 01 - Domain Analysis  
**Artifact ID:** domain-model.md  
**Plan ID:** PLAN-20260402-MARKDOWN.P01

---

## 1. Overview

This document analyzes the domain model for markdown rendering in the PersonalAgent application. It covers the pulldown-cmark event stream model, how events map to intermediate representation (IR) types, and the lifecycle of markdown parsing.

---

## 2. pulldown-cmark Event Stream Model

### 2.1 Event Lifecycle

The pulldown-cmark crate produces an event stream with the following lifecycle pattern:

```
Start(Tag) → content events → End(Tag) nesting
```

Events are processed sequentially in a streaming fashion. Block-level constructs use `Start`/`End` pairs to indicate nesting boundaries.

### 2.2 Event Variants (13 total)

| Event | Type | Usage | IR Mapping |
|-------|------|-------|------------|
| `Start(Tag)` | Block/Inline | Opens a container | Starts block accumulation |
| `End(Tag)` | Block/Inline | Closes a container | Finalizes block construction |
| `Text(CowStr)` | Content | Character data | Appended to current inline buffer |
| `Code(CowStr)` | Inline | Inline code span | `MarkdownInline` with `code: true` |
| `Html(CowStr)` | Block | Raw HTML block | Stripped to text (REQ-MD-PARSE-043) |
| `InlineHtml(CowStr)` | Inline | Raw inline HTML | Stripped to text (REQ-MD-PARSE-044) |
| `FootnoteReference(CowStr)` | Inline | Footnote reference | Literal `[^{label}]` text |
| `SoftBreak` | Formatting | Single space | Space appended to buffer |
| `HardBreak` | Formatting | Newline | Newline appended to buffer |
| `Rule` | Block | Thematic break | `MarkdownBlock::ThematicBreak` |
| `TaskListMarker(bool)` | Inline | Checkbox marker | Unicode ballot box prefix |
| `InlineMath(CowStr)` | Inline | Math expression | Code-styled fallback |
| `DisplayMath(CowStr)` | Block | Display math | `MarkdownBlock::CodeBlock` fallback |

### 2.3 Tag Variants for Block-Level Constructs

| Tag | Event Pair | IR Output | Notes |
|-----|------------|-----------|-------|
| `Tag::Paragraph` | Start/End | `MarkdownBlock::Paragraph` | Contains inline spans |
| `Tag::Heading { level: u8, .. }` | Start/End | `MarkdownBlock::Heading` | level 1-6, scaled font size |
| `Tag::CodeBlock(info)` | Start/End | `MarkdownBlock::CodeBlock` | info string = language |
| `Tag::BlockQuote` | Start/End | `MarkdownBlock::BlockQuote` | Contains nested blocks |
| `Tag::List(start)` | Start/End | `MarkdownBlock::List` | None=unordered, Some(n)=ordered |
| `Tag::Item` | Start/End | List item child blocks | Nested in List.items |
| `Tag::Table(aligns)` | Start/End | `MarkdownBlock::Table` | Vec<Alignment> for columns |
| `Tag::TableHead` | Start/End | Table header cells | First row in table |
| `Tag::TableRow` | Start/End | Table body rows | After header |
| `Tag::Tag::TableCell` | Start/End | Individual cells | Contains inline spans |
| `Tag::Emphasis` | Start/End | Inline style flag | `italic: true` |
| `Tag::Strong` | Start/End | Inline style flag | `bold: true` |
| `Tag::Strikethrough` | Start/End | Inline style flag | `strikethrough: true` |
| `Tag::Link { dest_url, .. }` | Start/End | Link span + range | URL stored, range tracked |
| `Tag::Image { dest_url, .. }` | Start/End | Fallback | `MarkdownBlock::ImageFallback` |

### 2.4 Alignment Variants (for Tables)

```rust
// From pulldown-cmark
pub enum Alignment {
    None,   // Default/no alignment specified
    Left,   // :---
    Center, // :---:
    Right,  // ---:
}
```

---

## 3. Block vs Inline Distinction

### 3.1 Block-Level Constructs

Block-level constructs produce `MarkdownBlock` enum variants:

| Construct | IR Variant | Children |
|-----------|------------|----------|
| Paragraph | `MarkdownBlock::Paragraph` | Vec<MarkdownInline> + links |
| Heading | `MarkdownBlock::Heading` | Vec<MarkdownInline> + links |
| Code block | `MarkdownBlock::CodeBlock` | language: Option<String>, code: String |
| Blockquote | `MarkdownBlock::BlockQuote` | Vec<MarkdownBlock> (recursive) |
| List | `MarkdownBlock::List` | ordered: bool, start: u64, items: Vec<Vec<MarkdownBlock>> |
| Table | `MarkdownBlock::Table` | alignments, header, rows |
| Thematic break | `MarkdownBlock::ThematicBreak` | No children |
| Image | `MarkdownBlock::ImageFallback` | alt: String |

### 3.2 Inline Constructs

Inline constructs produce `MarkdownInline` spans with style flags:

| Construct | Style Flag | Notes |
|-----------|------------|-------|
| Bold (**text**) | `bold: true` | Strong tag |
| Italic (*text*) | `italic: true` | Emphasis tag |
| Bold-italic | both true | Nested tags |
| Strikethrough | `strikethrough: true` | ~~text~~ |
| Inline code | `code: true` | Backtick-delimited |
| Link | `link_url: Some(String)` | [text](url) |

---

## 4. Nesting Model

### 4.1 Supported Nesting Patterns

```markdown
> Blockquote
> > Nested blockquote
> 
> - List inside blockquote
>   - Nested list item

1. Ordered list
   Paragraph inside list item
   
   ```
   Code block inside list item
   ```

- **Bold list item**
- *Italic list item*
- [Link in list](https://example.com)
```

### 4.2 IR Representation of Nesting

```rust
// Blockquote contains Vec<MarkdownBlock> (recursive)
MarkdownBlock::BlockQuote {
    blocks: Vec<MarkdownBlock>,  // Can contain paragraphs, lists, etc.
}

// List items contain Vec<MarkdownBlock>
MarkdownBlock::List {
    ordered: bool,
    start: u64,
    items: Vec<Vec<MarkdownBlock>>,  // Each item is a Vec of blocks
}
```

---

## 5. Text Accumulation Strategy

### 5.1 Text Event Interleaving

Text events interleave with style Start/End events:

```
Start(Strong) → Text("bold") → End(Strong) → Text(" and ") → Start(Emphasis) → Text("italic") → End(Emphasis)
```

### 5.2 Inline Span Production

The parser maintains a style stack. When text arrives, it creates a `MarkdownInline` with:
- `text`: The text content
- `bold`: Current bold state from stack
- `italic`: Current italic state from stack
- `strikethrough`: Current strikethrough state
- `code`: Current code state (from `Code` event, not Start/End)
- `link_url`: Current link URL if in a link

---

## 6. Link Model

### 6.1 Link Event Structure

```rust
Tag::Link {
    link_type: LinkType::Inline,
    dest_url: CowStr,      // The URL
    title: Option<CowStr>, // Optional title
    id: CowStr,            // Reference ID (for reference-style)
}
```

### 6.2 Byte Range Tracking

For each link, the parser records:
```rust
(Range<usize>, String)  // (byte_range, url)
```

- Range is **byte offsets** into the accumulated text (not char offsets)
- Required for GPUI's `InteractiveText` API which uses byte positions
- Conversion from pulldown-cmark char positions to byte positions is needed

### 6.3 Link Collection Location

Links are collected per-block and stored in:
- `MarkdownBlock::Paragraph { links: Vec<(Range<usize>, String)> }`
- `MarkdownBlock::Heading { links: Vec<(Range<usize>, String)> }`

Links in list items and table cells are recursively discoverable.

---

## 7. HTML Handling

### 7.1 Three HTML Event Types

| Event | Context | Handling |
|-------|---------|----------|
| `Html` | Block-level | Strip tags, emit as Paragraph |
| `InlineHtml` | Inline | Strip tags, append text to current buffer |
| `HtmlBlock` start/end | Raw HTML block | Strip tags and content |

### 7.2 Tag-Stripping State Machine

For security (REQ-MD-SEC-010), HTML is stripped, not rendered:

1. **Script/Style elements**: Content stripped entirely (REQ-MD-PARSE-045)
2. **Other HTML tags**: Tags removed, inner text extracted
3. **Malformed HTML**: `<` without `>` treated as literal (REQ-MD-PARSE-050)

---

## 8. Edge Cases

### 8.1 Malformed Input

pulldown-cmark handles recovery for:
- Unclosed emphasis: `**bold without end` → renders as literal `**`
- Unclosed code: `` `code without end `` → treats trailing backtick as text
- Mismatched nesting: Attempts recovery with reasonable output

### 8.2 Empty Input

- Empty string → Empty `Vec<MarkdownBlock>`
- Whitespace-only → Empty or single empty paragraph
- No panic required (REQ-MD-RENDER-041)

### 8.3 Deeply Nested Structures

- Lists: Tested to 5+ levels (REQ-MD-TEST-044)
- Blockquotes: Nesting depth limited by stack
- Tables: 20+ columns supported (REQ-MD-TEST-045)

---

## 9. Parser Configuration

### 9.1 Required Options

```rust
let options = Options::ENABLE_TABLES
    | Options::ENABLE_STRIKETHROUGH
    | Options::ENABLE_TASKLISTS;
```

Per REQ-MD-PARSE-011, these three options must be enabled.

### 9.2 Option Effects

| Option | Effect |
|--------|--------|
| `ENABLE_TABLES` | Recognizes pipe-delimited tables |
| `ENABLE_STRIKETHROUGH` | Recognizes `~~text~~` |
| `ENABLE_TASKLISTS` | Recognizes `- [ ]` and `- [x]` |

---

## 10. Summary

The domain model centers on pulldown-cmark's event stream with:
- **13 event types** mapping to IR constructs
- **Start/End tag pairs** for nesting
- **Style stack** for inline formatting
- **Byte range tracking** for link clickability
- **Security-first HTML handling** (strip, don't render)

This domain understanding enables the IR type design and GPUI rendering implementation in subsequent phases.
