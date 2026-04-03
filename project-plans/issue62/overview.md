# Functional/Technical Specification: Markdown Rendering for Assistant Messages

**Issue:** [#62 — Markdown rendering for assistant messages (pulldown-cmark + mdstream + GPUI builder)](https://github.com/acoliver/personal-agent/issues/62)
**Status:** Draft (Revised — post-review round 4)
**Last Updated:** 2026-04-02

---

## Top-Level Preconditions

### Normative Architecture: Two-Phase IR Model

**This specification uses the two-phase intermediate representation (IR) architecture as its normative design.** The pipeline is:

1. **Phase 1 — Parse:** `parse_markdown_blocks(content: &str) -> Vec<MarkdownBlock>` converts pulldown-cmark events into a plain Rust data structure (no GPUI dependency).
2. **Phase 2 — Render:** `blocks_to_elements(blocks: &[MarkdownBlock]) -> Vec<AnyElement>` converts the IR into GPUI elements.
3. **Public API:** `render_markdown(content: &str) -> Vec<AnyElement>` composes both phases.

**Rejected alternative — direct builder (three stacks, direct GPUI emission):** The original issue (#62) recommended a direct builder pattern that walks pulldown-cmark events and emits GPUI elements in a single pass using three stacks (div stack, style stack, list stack). This approach was rejected because GPUI's `AnyElement` type is opaque — it has no public inspection API, so testing a direct builder requires `#[gpui::test]` for every case, which is slower, harder to write, and less deterministic. The IR model enables ~80% of test coverage through fast, pure-Rust `#[test]` assertions on `MarkdownBlock` values. See [§3.5](#35-intermediate-model-architecture-justification) for the full rationale. The three-stack description in §5.1's "Builder Internals" subsection describes the *internal implementation* of `parse_markdown_blocks()` — it is how the IR is *produced*, not an alternative architecture.

### Normative Click Handling Strategy

**`InteractiveText` handles link clicks; bubble-level copy is disabled when markdown content contains links, replaced with a future copy affordance (Phase 2).** Specifically:

- When rendered markdown contains **no links**: The bubble-level `.on_click()` handler copies the raw markdown source to the clipboard (preserving existing behavior).
- When rendered markdown contains **one or more links**: The bubble-level `.on_click()` handler is **not attached**. `InteractiveText::on_click()` handles link clicks exclusively. Bubble-level copy is not available for these messages.
- **Phase 2 follow-up:** Add an explicit "copy" affordance button (small icon in the bubble header) that provides copy functionality for all messages regardless of link content. This replaces the whole-bubble click-to-copy for link-containing messages.

This is a **normative commitment**, not a conditional plan. The implementation gate test in [§2.5](#25-click-event-precedence-bubble-copy-vs-link-click--implementation-gate) verifies the behavior but does not change the strategy.

### Phase B Non-Authoritative Status

**Phase B of this specification (§6) is a Conditional Draft — pending mdstream validation gate.** The `mdstream` crate (v0.2.0) is not currently present in `Cargo.toml` and has not been compiled against this project. All Phase B sections — including streaming state transitions (§6), reset matrix (§6.9), block model assumptions (§6.7), and mdstream API usage (§6.2) — are based on [ASSUMED] documentation from docs.rs and are **tentative until the Dependency Validation Gate (§4.9) passes**.

Specifically:
- Hard requirements in §6 that depend on mdstream internals (`Update` struct fields, `Block` fields, `reset`/`invalidated` semantics, `display_or_raw()` behavior) are **tentative until validation gate passes**.
- If the mdstream API diverges from assumptions, those sections must be revised before Phase B implementation begins.
- **Phase A is fully self-contained and does not depend on mdstream in any way.** Phase A can be implemented and shipped by reading only §1–§5, §7–§13, §15–§22. Phase B (§6) need not be read at all for Phase A work.

### Normative Fallback Architecture (if mdstream validation fails)

If the Dependency Validation Gate fails — meaning `mdstream` 0.2.0 does not compile, its API diverges structurally from the assumptions in §4.9, or runtime behavior differs materially — the following fallback architecture is **normative** (not optional):

1. **Cosmetic API divergence** (field names, method signatures differ but the block model is the same): Write a thin adapter module (`md_stream_adapter.rs`, ~50-100 lines) that normalizes the actual API to match the interfaces assumed in this spec. Proceed with Phase B.

2. **Structural divergence** (different block model, no committed/pending split, or incompatible update semantics): Implement a **minimal custom block splitter** (~100-200 lines) using pulldown-cmark's block-level event boundaries directly. This splitter:
   - Tracks open/close events to identify block boundaries
   - Maintains a `committed: Vec<String>` of finalized block raw text
   - Maintains a `pending: String` for the trailing incomplete block
   - Exposes `append(delta) -> BlockUpdate` and `finalize() -> BlockUpdate` matching the interface the rest of the spec assumes
   - Does NOT require `reset`/`invalidated` semantics — simpler model where committed blocks are append-only

3. **Crate does not compile or is yanked**: Use the custom block splitter from option 2. Remove `mdstream` from the dependency plan entirely.

In all fallback cases, Phase A is unaffected. The `render_markdown()` builder, intermediate model, and all Phase A components work identically regardless of which streaming splitter is used.

---

## Non-Goals

This specification does **not** cover:

- Syntax highlighting of code blocks (requires `syntect` or `tree-sitter` — Phase 2)
- Interactive task-list checkboxes (AI responses are read-only)
- Markdown rendering for user messages (convention: user messages stay raw text)
- Code block copy buttons (Phase 2 fast-follow)
- Image rendering (images fallback to styled alt-text placeholders)
- Math/LaTeX rendering (rendered as code-styled monospace text)

---

## Table of Contents

> **Reading guide:** Phase A is self-contained in this document (sections 1-5, 7-22). Phase B (streaming optimization) has been extracted to [`spec-phase-b.md`](spec-phase-b.md) to reduce document size and truncation risk. Section 6 below is a stub pointing to that file.

| # | Section | Scope |
|---|---------|-------|
| 1 | [Purpose & Problem Statement](#1-purpose--problem-statement) | Phase A |
| 2 | [Functional Requirements](#2-functional-requirements) | Phase A |
| 3 | [Technical Architecture](#3-technical-architecture) | Phase A |
| 4 | [GPUI API Usage & Verification](#4-gpui-api-usage--verification) | Phase A (4.9 = Phase B) |
| 5 | [Component Design](#5-component-design) | Phase A |
| 6 | [Streaming Design -- Phase B Reference](#6-streaming-design-phase-b--conditional-draft) | **Phase B** -- see [`spec-phase-b.md`](spec-phase-b.md). `spec-phase-b.md` is the sole normative location for Phase B design details while conditional. |
| 7 | [Theme Integration](#7-theme-integration) | Phase A |
| 8 | [Security](#8-security) | Phase A |
| 9 | [Error Handling](#9-error-handling) | Phase A |
| 10 | [Integration Points](#10-integration-points) | Phase A |
| 11 | [Testing Strategy](#11-testing-strategy) | Phase A (11.4 = Phase B) |
| 12 | [Incremental Rollout Strategy](#12-incremental-rollout-strategy) | Phase A |
| 13 | [Out of Scope (Phase 2)](#13-out-of-scope-phase-2) | Phase A |
| 14 | [CodeRabbit Review Findings](#14-coderabbit-review-findings) | Phase A |
| 15 | [CodeRabbit Item -> Spec Section Traceability](#15-coderabbit-item--spec-section-traceability) | Phase A |
| 16 | [Dependencies](#16-dependencies) | Phase A |
| 17 | [Files Touched](#17-files-touched) | Phase A |
| 18 | [Risk Assessment](#18-risk-assessment) | Phase A |
| 19 | [Performance Acceptance Criteria](#19-performance-acceptance-criteria) | Phase A (19.2 = Phase B) |
| 20 | [Implementation Checklist](#20-implementation-checklist) | Phase A |
| 21 | [Implementation Minimum Slices](#21-implementation-minimum-slices) | Phase A |
| 22 | [References](#22-references) | Phase A |


---

## 1. Purpose & Problem Statement

### The Problem

Assistant messages currently render raw markdown text literally. Two separate code paths produce assistant message content:

**Path 1 — Completed messages** (`render.rs`, `render_assistant_message()`):
The method builds a div directly and passes `msg.content` as a raw string child:

```rust
// Current code (render.rs, render_assistant_message):
.child({
    let content = msg.content.clone();
    // ...
    div()
        // ... styling ...
        .child(content)   // ← Raw string, no parsing
})
```

**Path 2 — Streaming messages** (`render.rs`, `render_chat_area()`):
The streaming path constructs an `AssistantBubble` which also renders raw content:

```rust
// Current code (render.rs, render_chat_area streaming block):
let mut bubble = AssistantBubble::new(content)
    .model_id("streaming")
    .show_thinking(show_thinking)
    .streaming(true);
// ...
d.child(div().id("streaming-msg").child(bubble))
```

And in `message_bubble.rs`, `AssistantBubble::into_element()` passes the content as a raw string:

```rust
// Current code (message_bubble.rs, AssistantBubble::into_element):
let content_text = if self.is_streaming {
    format!("{}▋", self.content)
} else {
    self.content.clone()
};
// ...
.child(content_text)   // ← Raw string, no parsing
```

This means:
- `**bold**` renders as the literal characters `**bold**`
- `` `code` `` renders as the literal characters with backticks visible
- Code fences render as triple-backtick strings with the content inline
- Headings show with literal `#` characters
- Lists show as lines starting with `-` or `1.` rather than indented list items
- Links show as `[text](url)` rather than clickable links

### Why This Matters

The initial requirements document (section 3.3) calls for basic markdown support: code blocks, bold, italic, lists. Every major AI chat interface (ChatGPT, Claude.ai, etc.) renders markdown. Without it, assistant responses are substantially harder to read, especially for code-heavy responses where inline code and fenced code blocks are used constantly.

### Performance Dimension

There is also a performance concern: during streaming, re-parsing the entire growing message on every arriving token would degrade to O(n²). For a 10,000-token response, naively re-parsing the full buffer ~10,000 times is unacceptable. The solution must address both rendering quality and streaming performance.

### Width/Styling Divergence Note

The current `render_assistant_message()` and `AssistantBubble` code paths use different width constraints, padding, and visual structure. When refactoring to unify through `AssistantBubble` as canonical rendering owner (§3.4), the implementation must normalize these visual properties to avoid regressions. Specifically, the implementer must compare the effective styling (max-width, padding, gap, text size, cursor style) from both paths and ensure the unified `AssistantBubble` output matches the completed-message visual baseline. Any intentional divergences (e.g., streaming cursor) must be explicitly gated on `self.is_streaming`.

---

## 2. Functional Requirements

### 2.1 What the User Sees

When an assistant message arrives (streamed or completed), markdown constructs are rendered visually:

#### Block-Level Constructs

| Construct | Visual Rendering |
|---|---|
| Paragraph | Vertically-spaced text block |
| Heading 1–6 | Larger/bolder text with scaled sizes |
| Fenced code block | Monospace font, tinted background, rounded corners, language label |
| Indented code block | Same as fenced but no language label |
| Block quote | Left border bar + tinted background |
| Unordered list | Bullet characters with indentation per nesting level |
| Ordered list | Numbered items with indentation per nesting level |
| Table | Grid layout with header row, cell borders, alternating row striping |
| Thematic break (`---`) | Thin horizontal line |

#### Inline Constructs

| Construct | Visual Rendering |
|---|---|
| Bold (`**text**`) | Bold font weight |
| Italic (`*text*`) | Italic font style |
| Bold italic (`***text***`) | Both bold + italic |
| Strikethrough (`~~text~~`) | Horizontal line through text |
| Inline code (`` `code` ``) | Monospace font with background highlight |
| Links (`[text](url)`) | Underlined, accent-colored, clickable (opens browser) |
| Task list markers (`- [x]`, `- [ ]`) | Visual checkbox characters: `[x]` (checked), `[ ]` (unchecked) — rendered as Unicode ballot box characters, not interactive |

#### Graceful Fallbacks

| Construct | Fallback |
|---|---|
| Images (`![alt](url)`) | Styled text: `[image: alt_text]` |
| Footnotes | Inline text with label |
| Raw HTML (see [§5.1.1](#511-pulldown-cmark-html-event-handling)) | Strip tags, render text content only (no sanitization-render pipeline; text-only extraction by design — HTML is never interpreted or rendered as markup) |
| Math (inline/display) | Show in code-styled run (monospace + background) |
| Superscript/Subscript | Render as plain text |
| Metadata blocks | Skip entirely |

#### Table Rendering During Incomplete Streaming

During streaming, a table may be partially received. The fallback behavior is:

- **Header row incomplete:** pulldown-cmark does not emit `Start(Table)` events until it has enough context to identify the table structure (header row + delimiter row). Until then, the partial text is treated as a plain paragraph in the pending block. No grid is rendered.
- **Header row complete, body rows arriving:** The table renders as a grid with the known column count. Body rows appear progressively as they complete. Incomplete trailing cells are rendered as empty.
- **Transition:** When a pending block containing partial table text becomes committed (after the full table is received), it re-renders as a proper grid. There is no visual "flash" because the pending block was already rendering the best-effort view.

### 2.2 User Messages

User messages remain raw text (no markdown rendering). This is an **intentional product decision**, not a temporary limitation. The rationale:

1. **Convention alignment:** ChatGPT, Claude.ai, and other major chat interfaces render user messages as plain text. Users type natural language, not markdown — rendering their input as markdown would produce surprising formatting from accidental markup (e.g., asterisks around words, backtick characters).
2. **Input fidelity:** Users should see exactly what they typed, not a rendered interpretation. This avoids confusion where the displayed message differs from the input.
3. **Scope control:** Rendering user messages as markdown would require handling the `UserBubble` component, which is a separate concern.

This decision is revisitable in future phases if user research indicates demand, but it is not a Phase 1 or Phase 2 item.

### 2.3 Streaming Behavior

During streaming:
- Text appears progressively as tokens arrive
- Committed blocks (complete paragraphs, headings, code blocks, etc.) stabilize and don't re-render
- Only the pending (in-progress) block at the tail re-parses per token
- The streaming cursor (`▋`) appears at the end of the pending block
- When the stream ends, all content finalizes into committed blocks

### 2.4 Click-to-Copy Behavior

**Current behavior:** Completed assistant messages in `render_assistant_message()` have a `.on_click()` handler that copies the entire message content to the clipboard when clicked. This is a user-facing feature that must be preserved.

**Change:** When `render_assistant_message()` is refactored to delegate to `AssistantBubble`, the click-to-copy behavior moves into `AssistantBubble::into_element()`. The behavior is **conditional on link content** (see [Top-Level Preconditions — Normative Click Handling Strategy](#normative-click-handling-strategy)):

- **Messages with no links:** The outermost container div in `AssistantBubble` gains the same `.on_click()` copy handler:
  ```rust
  .cursor_pointer()
  .on_click({
      let content = self.content.clone();
      move |_event, _window, cx| {
          cx.write_to_clipboard(gpui::ClipboardItem::new_string(content.clone()));
      }
  })
  ```

- **Messages with one or more links:** The bubble-level `.on_click()` handler is **not attached**. `InteractiveText::on_click()` handles link clicks exclusively. The cursor style remains default (no `cursor_pointer()`). Bubble-level copy is not available — this is an intentional trade-off to avoid UX-breaking click conflicts. Phase 2 adds a copy button affordance.

This means:
- **Completed messages without links:** Click anywhere on the bubble → copies raw markdown source (same as today).
- **Completed messages with links:** Click on a link → opens URL. Click elsewhere → no action. Copy via Cmd+C or future copy button.
- **Streaming messages:** No click-to-copy during streaming (streaming messages always use `InteractiveText` for the cursor, so bubble-level copy is never attached during streaming).

### 2.5 Click Event Precedence: Bubble Copy vs. Link Click — Implementation Gate

**Normative strategy:** `InteractiveText` handles link clicks; bubble-level copy is disabled when markdown content contains links (see [Top-Level Preconditions](#normative-click-handling-strategy)). This section defines the **verification gate** that confirms the strategy works correctly.

#### Pass/Fail Criteria

| Test Case | Expected Behavior | Pass Condition |
|---|---|---|
| Click on link text within rendered markdown | Opens URL in browser. Does NOT copy message to clipboard. | Only `InteractiveText::on_click` fires. Clipboard unchanged. |
| Click on non-link area within assistant bubble (message has links) | No action (bubble copy not attached for link-containing messages). | No handler fires. Clipboard unchanged. No browser opened. |
| Click on non-link area within assistant bubble (message has no links) | Copies raw markdown source to clipboard. | Bubble-level `on_click` fires. Clipboard updated. |
| Click on link text, then check clipboard | Clipboard retains its previous content (not overwritten by bubble copy). | Clipboard unchanged by link click. |

#### Implementation

The `AssistantBubble::into_element()` implementation checks whether `parse_markdown_blocks()` produced any blocks with non-empty `links` fields. If links are present, the bubble-level `.on_click()` is not attached. If no links are present, the bubble-level `.on_click()` is attached as before.

```rust
// In AssistantBubble::into_element():
let blocks = parse_markdown_blocks(&content_text);
let has_links = blocks.iter().any(|b| match b {
    MarkdownBlock::Paragraph { links, .. } | MarkdownBlock::Heading { links, .. } => !links.is_empty(),
    MarkdownBlock::List { items, .. } => items.iter().any(|item_blocks| {
        item_blocks.iter().any(|b| matches!(b, MarkdownBlock::Paragraph { links, .. } if !links.is_empty()))
    }),
    _ => false,
});

let container = div()
    // ... styling ...
    .children(blocks_to_elements(&blocks));

let container = if !has_links && !self.is_streaming {
    container
        .cursor_pointer()
        .on_click({
            let content = self.content.clone();
            move |_event, _window, cx| {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(content.clone()));
            }
        })
} else {
    container
};
```

#### Gate Resolution

This gate is resolved when:
- A `#[gpui::test]` demonstrating the chosen approach passes
- All four pass/fail criteria above are verified in tests
- The chosen approach is documented in a brief implementation note committed alongside the code

---

## 3. Technical Architecture

### 3.1 Two-Concern Separation

The design separates two independent concerns:

```
┌─────────────────────────────────────────────────────────────┐
│  Concern 1: Markdown → GPUI Elements (the IR + builder)     │
│                                                             │
│  Input:  &str (markdown text)                               │
│  Output: Vec<AnyElement> (GPUI element tree)                │
│                                                             │
│  Two-phase pipeline:                                        │
│  1. parse_markdown_blocks() → Vec<MarkdownBlock> (IR)       │
│     Walks pulldown-cmark events, builds intermediate model  │
│  2. blocks_to_elements() → Vec<AnyElement> (rendering)      │
│     Translates IR into GPUI elements with styling           │
│                                                             │
│  Location: src/ui_gpui/components/markdown_content.rs       │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Concern 2: Streaming Optimization (Phase B only)           │
│  *** Phase B only — Phase A works without this ***          │
│  *** CONDITIONAL DRAFT until Dependency Validation Gate *** │
│                                                             │
│  Input:  Incremental text chunks (tokens from LLM)          │
│  Output: Committed blocks + Pending block                   │
│                                                             │
│  Splits growing stream into:                                │
│  • Committed blocks: stable, won't change                   │
│  • Pending block: still receiving tokens                    │
│                                                             │
│  The builder is called per-block. Only the pending tail     │
│  gets re-parsed per token → O(n) total instead of O(n²).   │
│                                                             │
│  Location: src/ui_gpui/views/chat_view/mod.rs (view-local)  │
└─────────────────────────────────────────────────────────────┘
```

Both concerns share the same `render_markdown()` builder function. Completed messages call it once on the full text. Streaming messages call it per-block from the mdstream output [Phase B — Conditional, pending validation gate] or on the full content (Phase A fallback).

### 3.2 Data Flow: Completed Messages

```
ChatMessage.content (&str)
  │
  ▼
AssistantBubble::into_element()
  │
  ▼
parse_markdown_blocks(content) → Vec<MarkdownBlock>
  │
  ▼
blocks_to_elements(&blocks) → Vec<AnyElement>
  │
  ▼
AssistantBubble wraps elements in its styled container div
  (with conditional .on_click() — only if no links detected)
```

Note: `render_assistant_message()` in `render.rs` currently builds its own raw-text div for completed messages. After this change, it will delegate to `AssistantBubble` (see [Section 5.3](#53-modifications-to-renderrs-render_assistant_message)) to unify the rendering path through a single owner.

### 3.3 Data Flow: Streaming Messages

> **Phase B data flow — see [§6](#6-streaming-design-phase-b--conditional-draft) (Conditional Draft).** In Phase A, streaming messages call `render_markdown()` on the full `stream_buffer` content each frame.

**Phase A (no mdstream):**
```
LLM token arrives → StreamingState::Streaming { content, done: false }
  │
  ▼
ChatView.apply_store_snapshot() detects new bytes
  │
  ▼
AssistantBubble::new(content).streaming(true)
  │
  ▼
render_markdown(content)  ← full re-parse each frame (O(n²) for stream)
  │
  ▼
Rendered elements in scroll view
```

**Phase B (with mdstream — Conditional Draft):**
```
LLM token arrives → StreamingState::Streaming { content, done: false }
  │
  ▼
ChatView.apply_store_snapshot() detects new bytes
  │
  ▼
md_stream.append(delta)  →  Update { committed, pending, reset, invalidated }
  │                           │
  ▼                           ▼
render_markdown(block.raw)   render_markdown(pending.display_or_raw())
  for each committed block     for the pending block
  │                           │
  ▼                           ▼
Cached GPUI elements         Fresh GPUI elements (re-parsed each frame)
  │                           │
  └───────────┬───────────────┘
              ▼
  render_chat_area assembles both into the scroll view
```

**Note on stream completion:** The `done` field in `StreamingState::Streaming { content, done }` is always `false` in the current implementation. Stream completion is detected by the transition from `Streaming` to `Idle` — specifically, when `streaming_state_from_snapshot()` returns `Idle` because `active_target.is_none()` and `stream_buffer.is_empty()`. See [§6.3](#63-streaming-state-precedence) for details.

### 3.4 Canonical Rendering Owner: `AssistantBubble`

**`AssistantBubble` is the single canonical owner of markdown rendering for all assistant content.**

Both completed messages and streaming messages route through `AssistantBubble::into_element()`, which calls `render_markdown()`. There is no second call site in `render_assistant_message()` or elsewhere. This eliminates the risk of dual rendering paths or contradictory integration work.

### 3.5 Intermediate Model: Architecture Justification

The original issue (#62) states that "no intermediate representation is needed" and recommends a direct builder pattern. This spec introduces an intermediate model (`MarkdownBlock`/`MarkdownInline`) as its **normative architecture** (see [Top-Level Preconditions](#normative-architecture-two-phase-ir-model)). The trade-offs are:

**Why the spec diverges from the issue's "no IR needed" guidance:**

1. **Testability:** The intermediate model enables exhaustive `#[test]` coverage of the parsing logic without any GPUI dependency. Since `AnyElement` is opaque (no public inspection API), testing a direct builder requires `#[gpui::test]` for every case — which is slower, harder to write, and less deterministic. The intermediate model allows ~80% of test coverage through fast, pure-Rust `#[test]` assertions on `MarkdownBlock` values.

2. **Debuggability:** When rendering looks wrong, the intermediate model provides a `Debug`-printable data structure to inspect. With a direct builder, diagnosing whether the bug is in parsing or rendering requires instrumenting opaque GPUI element construction.

3. **Phase B compatibility (pending validation gate):** If/when mdstream splits content into committed + pending blocks, each block's `raw` text would be parsed independently. The intermediate model cleanly separates "what was parsed" from "how it renders," making block-level caching straightforward.

4. **Click handling:** The IR model enables the normative click handling strategy — `AssistantBubble` can inspect parsed blocks for link presence *before* building GPUI elements, which determines whether to attach the bubble-level copy handler.

**Costs of this approach:**

1. **Extra allocation:** Each message is parsed into `Vec<MarkdownBlock>` before being converted to GPUI elements. For typical messages (<10KB), this overhead is negligible compared to GPUI element construction.

2. **Additional complexity:** ~80 lines of enum/struct definitions and an extra translation pass. This is offset by ~200+ fewer lines of `#[gpui::test]` boilerplate.

3. **Divergence from issue guidance:** The issue author's "no IR needed" was a reasonable starting position. This spec upgrades it after evaluating the testing constraints of GPUI's opaque element types. The change is documented here so reviewers can evaluate the trade-off.

**Bottom line:** The intermediate model adds ~80 lines of type definitions and produces a net reduction in total code (fewer GPUI test fixtures) while substantially improving test coverage and debuggability.

---

## 4. GPUI API Usage & Verification

### 4.0 Verification Status Legend

All API claims in this section are categorized:

- **[VERIFICATION PROCEDURE PROVIDED — must re-run against current lock state]** — A verification procedure (grep commands against the GPUI source at pinned rev `c67328ab2e0d572718575e02ae07db37552e1cbe`) has been documented below. The results described are from a prior checkout. **Implementation must re-run these checks against the current `Cargo.lock` state**, as the resolved source path may differ between environments or if the lock file is updated.
- **[ASSUMED] Assumed from upstream docs** — Based on official documentation or crates.io API docs but not yet confirmed in the pinned checkout. May differ at the pinned rev.
- **[VALIDATE] To be validated during implementation** — Behavioral assumptions that require runtime testing or integration work to confirm.

### 4.1 How to Verify

To reproduce the API verification from any checkout of this project:

```bash
# 1. Confirm the pinned GPUI rev in Cargo.toml
grep -A2 'gpui' Cargo.toml

# 2. Ensure the dependency is fetched
cargo fetch

# 3. Use cargo tree to confirm the resolved rev
cargo tree -i gpui --depth 0

# 4. Find the checkout path via cargo metadata
cargo metadata --format-version=1 | jq -r '.packages[] | select(.name=="gpui") | .manifest_path'

# 5. Search for specific APIs (example: StyledText::with_runs)
GPUI_SRC=$(dirname $(cargo metadata --format-version=1 | jq -r '.packages[] | select(.name=="gpui") | .manifest_path'))/src
grep -n 'fn with_runs' "$GPUI_SRC/elements/text.rs"
grep -n 'fn on_click' "$GPUI_SRC/elements/text.rs"
grep -n 'fn grid\b' "$GPUI_SRC/styled.rs"
grep -n 'fn grid_cols' "$GPUI_SRC/styled.rs"
grep -n 'pub struct TextRun' "$GPUI_SRC/text_system.rs"
grep -n 'pub struct Font' "$GPUI_SRC/text_system.rs"
grep -n 'pub struct StrikethroughStyle' "$GPUI_SRC/style.rs"
grep -n 'fn open_url' "$GPUI_SRC/platform/mac/platform.rs"
```

**Implementation requirement:** Before writing any GPUI integration code, the implementer must run the above commands and confirm the outputs match the expectations below. If any API has moved, been renamed, or changed signature, update this spec section before proceeding.

### 4.2 StyledText::with_runs() — [VERIFICATION PROCEDURE PROVIDED — must re-run against current lock state]

**File:** `crates/gpui/src/elements/text.rs`
**Signature:** `pub fn with_runs(mut self, runs: Vec<TextRun>) -> Self`

This is the full-power text styling API. Each `TextRun` carries a `Font` struct with `family`, `weight`, and `style` per run. This enables monospace font for inline code within a paragraph that is otherwise in the system font — something the `HighlightStyle` API cannot do, as it lacks a `font_family` field.

**Verification:** `HighlightStyle` (at `style.rs`) has fields: `color`, `font_weight`, `font_style`, `background_color`, `underline`, `strikethrough`, `fade_out` — no `font_family`. The `TextRun` struct (`text_system.rs`) has `font: Font` which includes `family: SharedString`. This confirms the issue's assertion that `TextRun` + `StyledText::with_runs()` is the correct API.

**Used for:** All inline text rendering — paragraphs, headings, code blocks, blockquotes. Each text segment gets a `TextRun` with the appropriate font family, weight, style, color, and background.

### 4.3 InteractiveText — [VERIFICATION PROCEDURE PROVIDED — must re-run against current lock state]

**File:** `crates/gpui/src/elements/text.rs`
**Constructor:** `pub fn new(id: impl Into<ElementId>, text: StyledText) -> Self`
**Click handler:** `pub fn on_click(mut self, ranges: Vec<Range<usize>>, listener: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self`

Wraps a `StyledText` and adds clickable ranges. The `on_click` callback receives the index of the clicked range, which maps to a URL collected during the markdown walk.

**Used for:** Clickable links in any inline-text container — paragraphs, headings, list items, table cells, and blockquotes. When any of these contexts contain links, the text element is wrapped in `InteractiveText` instead of plain `StyledText`. The builder collects `(Range<usize>, String)` tuples for each link, and the click handler calls `cx.open_url()` (verified: `platform/mac/platform.rs`) for the clicked URL.

**[VALIDATE]** The exact click-handler dispatch behavior (whether `on_click` fires reliably for all range positions within wrapped text) should be confirmed with a `#[gpui::test]` during implementation.

### 4.4 CSS Grid: div().grid().grid_cols(n) — [VERIFICATION PROCEDURE PROVIDED — must re-run against current lock state]

**File:** `crates/gpui/src/styled.rs`
- `fn grid(mut self) -> Self`
- `fn grid_cols(mut self, cols: u16) -> Self`

**Used for:** Table rendering. The column count `n` is determined from the header row's cell count. Each table cell is a child div with conditional borders, optional header background, and alternating row stripe colors.

**[VALIDATE]** Grid layout with >10 columns and cells containing wrapped text should be tested during implementation to confirm GPUI grid behavior at scale.

### 4.5 TextRun — [VERIFICATION PROCEDURE PROVIDED — must re-run against current lock state]

**File:** `crates/gpui/src/text_system.rs`
**Fields:**
```rust
pub struct TextRun {
    pub len: usize,              // UTF-8 byte count
    pub font: Font,              // family, weight, style, features, fallbacks
    pub color: Hsla,
    pub background_color: Option<Hsla>,
    pub underline: Option<UnderlineStyle>,
    pub strikethrough: Option<StrikethroughStyle>,
}
```

### 4.6 Font — [VERIFICATION PROCEDURE PROVIDED — must re-run against current lock state]

**File:** `crates/gpui/src/text_system.rs`
**Fields:**
```rust
pub struct Font {
    pub family: SharedString,     // e.g., "Menlo", ".SystemUIFont"
    pub features: FontFeatures,
    pub fallbacks: Option<FontFallbacks>,
    pub weight: FontWeight,       // e.g., FontWeight::BOLD
    pub style: FontStyle,         // e.g., FontStyle::Italic
}
```

### 4.7 StrikethroughStyle — [VERIFICATION PROCEDURE PROVIDED — must re-run against current lock state]

**File:** `crates/gpui/src/style.rs`
**Fields:**
```rust
pub struct StrikethroughStyle {
    pub thickness: Pixels,
    pub color: Option<Hsla>,
}
```

### 4.8 cx.open_url() — [VERIFICATION PROCEDURE PROVIDED — must re-run against current lock state]

**File:** `crates/gpui/src/platform/mac/platform.rs`
Verified to exist at the pinned rev. Uses `NSWorkspace` to open URLs in the system default browser.

**[VALIDATE]** Behavior with internationalized domain names (IDN) and very long URLs (>2000 chars) should be tested during implementation, as `NSWorkspace` may have platform-specific URL length or encoding limits.

### 4.9 mdstream API — [ASSUMED, pinned to v0.2.0] — CONDITIONAL DRAFT

> ** CONDITIONAL DRAFT (Phase B):** This entire subsection is part of Phase B and is tentative until the Dependency Validation Gate passes. See [Top-Level Preconditions](#phase-b-non-authoritative-status).

The `mdstream` crate (v0.2.0) API has been reviewed against [docs.rs/mdstream/0.2.0](https://docs.rs/mdstream/0.2.0). The following signatures are taken from upstream documentation and verified against code examples in the repository, but have **not** been compiled against a local checkout:

- `MdStream::new(Options::default())` — construct a new stream
- `md_stream.append(chunk: &str) -> Update` — feed a chunk
- `md_stream.finalize() -> Update` — commit trailing pending content
- `md_stream.reset()` — clear all state for reuse

**The `Update` struct** (confirmed from docs.rs source):
```rust
pub struct Update {
    pub committed: Vec<Block>,
    pub pending: Option<Block>,
    pub reset: bool,           // if true, consumers must rebuild all rendered state
    pub invalidated: Vec<BlockId>,
}
```

Each `Block` has:
- `id: BlockId` — stable identifier
- `kind: BlockKind` — paragraph, heading, code fence, etc.
- `raw: String` — the raw markdown text for this block
- `display: Option<String>` — transformed display text (from pending transformers)

The `display_or_raw()` method returns `display` if set, otherwise `raw`.

**`DocumentState`:** mdstream also provides a `DocumentState` helper that tracks committed/pending blocks across updates via `state.apply(update)`. This is a convenience wrapper — the raw `Update` fields can also be managed directly. The implementer should evaluate whether `DocumentState` or manual tracking is simpler for the ChatView integration.

Feature flags: `mdstream` has an optional `pulldown` feature (depends on `pulldown-cmark ^0.13`) that provides a `PulldownAdapter` with `committed_events()` and `parse_pending()` methods. This should be evaluated during implementation; the adapter provides caching benefits for committed blocks but adds coupling.

**Dependency Validation Gate (Phase B prerequisite):** Before Phase B implementation begins, the following validation must pass:

1. Add `mdstream = "0.2.0"` to `Cargo.toml`
2. Run `cargo check` to confirm it resolves and compiles
3. Write a minimal integration test that calls `MdStream::new(Options::default())`, feeds a few chunks via `append()`, calls `finalize()`, and asserts the `Update` struct shape matches the assumptions above
4. If any API deviates from the above, update this spec section before proceeding
5. If the validation gate fails, execute the [Normative Fallback Architecture](#normative-fallback-architecture-if-mdstream-validation-fails)

---

## 5. Component Design

### 5.1 New Module: `markdown_content.rs`

**Location:** `src/ui_gpui/components/markdown_content.rs`

This is the core new file. It contains the two-phase pipeline that converts markdown text to GPUI elements via an intermediate representation.

#### Public API

```rust
/// Render a markdown string into a vector of GPUI elements.
///
/// Internally uses a two-phase pipeline:
/// 1. parse_markdown_blocks() → Vec<MarkdownBlock> (testable IR)
/// 2. blocks_to_elements() → Vec<AnyElement> (GPUI rendering)
pub fn render_markdown(content: &str) -> Vec<AnyElement>
```

#### Two-Phase Architecture

The builder uses a two-phase architecture (see [§3.5](#35-intermediate-model-architecture-justification) for the rationale and [Top-Level Preconditions](#normative-architecture-two-phase-ir-model) for the normative status of this design):

```rust
/// Phase 1: Markdown → Intermediate Model (pure, testable)
///
/// Converts pulldown-cmark events into a tree of MarkdownBlock/MarkdownInline
/// nodes. This is a plain data structure with no GPUI dependencies.
pub fn parse_markdown_blocks(content: &str) -> Vec<MarkdownBlock>

/// Phase 2: Intermediate Model → GPUI Elements (rendering)
///
/// Converts the intermediate model into GPUI AnyElement values.
/// This is a thin translation layer.
pub fn blocks_to_elements(blocks: &[MarkdownBlock]) -> Vec<AnyElement>

/// Public API: composes both phases.
pub fn render_markdown(content: &str) -> Vec<AnyElement> {
    let blocks = parse_markdown_blocks(content);
    blocks_to_elements(&blocks)
}
```

The intermediate model:

```rust
/// A block-level markdown construct.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MarkdownBlock {
    Paragraph { spans: Vec<MarkdownInline>, links: Vec<(Range<usize>, String)> },
    Heading { level: u8, spans: Vec<MarkdownInline>, links: Vec<(Range<usize>, String)> },
    CodeBlock { language: Option<String>, code: String },
    BlockQuote { children: Vec<MarkdownBlock> },
    List { ordered: bool, start: u64, items: Vec<Vec<MarkdownBlock>> },
    Table { alignments: Vec<Alignment>, header: Vec<Vec<MarkdownInline>>, rows: Vec<Vec<Vec<MarkdownInline>>> },
    ThematicBreak,
    ImageFallback { alt: String },
}

/// An inline span with styling information.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MarkdownInline {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub strikethrough: bool,
    pub code: bool,
    pub link_url: Option<String>,
}
```

This design enables Phase 1 to be tested exhaustively with `#[test]` (no GPUI context needed), while Phase 2 is verified through targeted `#[gpui::test]` behavioral tests (see [Section 11.2](#112-gpui-behavioral-tests-gpuitest)).

#### Builder Internals (parse_markdown_blocks implementation)

The `parse_markdown_blocks()` function walks `pulldown_cmark::Event` values from `Parser::new_ext(content, options)` with these pulldown-cmark options enabled:
- `Options::ENABLE_TABLES`
- `Options::ENABLE_STRIKETHROUGH`
- `Options::ENABLE_TASKLISTS`

Internally, it uses three stacks to track state during the event walk. These are implementation details of `parse_markdown_blocks()` — they produce the `Vec<MarkdownBlock>` IR, not GPUI elements directly:

1. **Block stack** (`Vec<BlockBuilder>`): Tracks block-level nesting. On `Start(Heading)`, push a new heading builder. On `End(Heading)`, pop it and finalize into a `MarkdownBlock::Heading`. Code blocks, blockquotes, and list items work similarly.

2. **Inline style stack** (`Vec<InlineStyleState>`): Tracks inline style nesting. On `Start(Strong)`, push bold. On `Start(Emphasis)`, push italic. On `Start(Strikethrough)`, push strikethrough. Current effective style = base state refined by all stack entries. Popped on corresponding `End` events.

3. **List stack** (`Vec<ListContext>`): Tracks list nesting for bullet/number generation.
   ```rust
   enum ListContext {
       Ordered { next_number: u64 },
       Unordered,
   }
   ```

#### Text Accumulation Strategy

Within a block, the builder accumulates inline spans as `Vec<MarkdownInline>`. As `Text(cow)` events arrive, text is appended to the current span. Each inline style transition (bold on/off, italic on/off, etc.) closes the current span and starts a new one with the updated style flags. When the block ends, the accumulated spans become the block's `spans` field.

For blocks containing links, the builder also collects `(Range<usize>, String)` tuples recording the byte range and URL of each link span, stored in the block's `links` field.

The `blocks_to_elements()` phase then translates each block's spans into `TextRun` values for `StyledText::with_runs()`. If the block has links, the text element is wrapped in `InteractiveText::new(id, styled_text).on_click(ranges, callback)`.

#### 5.1.1 pulldown-cmark HTML Event Handling

pulldown-cmark 0.13 emits **three distinct HTML-related event types**. Each must be handled explicitly to avoid dropped or mishandled content:

| pulldown-cmark Event | When Emitted | Builder Action |
|---|---|---|
| `Event::Html(text)` | Block-level HTML: lines starting with HTML block tags (`<div>`, `<table>`, `<pre>`, `<script>`, `<!-- -->`, etc.) or any tag on its own line followed by a blank line. Also emitted for `Start(Tag::HtmlBlock)` / `End(Tag::HtmlBlock)` wrapped content. | Strip tags via simple state machine, append extracted text content as a plain paragraph. Never interpret or render as HTML markup. |
| `Event::InlineHtml(text)` | Inline HTML fragments within a paragraph or other inline context (e.g., `This has <em>html</em> in it`). These appear as individual events interleaved with `Text` events. | Strip tags via the same state machine, append extracted text content to the current inline text buffer. Preserves surrounding text flow. |
| `Start(Tag::HtmlBlock)` / `End(Tag::HtmlBlock)` | Wraps a sequence of `Html` events that form a contiguous HTML block. The `Start`/`End` pair provides block-level framing. | On `Start(HtmlBlock)`: begin accumulating HTML text. On `End(HtmlBlock)`: flush accumulated text through tag-stripping state machine, emit as plain paragraph. |

**Tag-stripping state machine:** A simple `bool` flag tracks whether the scanner is inside a `<...>` tag. Characters inside tags are dropped; characters outside tags are accumulated as text content. This is intentionally not a full HTML parser — it handles the common cases (self-closing tags, attributes, nested angle brackets in attribute values) well enough for text extraction. Malformed HTML (e.g., unmatched `<` without `>`) degrades gracefully by treating the `<` as literal text.

**Test coverage:** The intermediate model tests (§11.1) include specific cases for:
- Block-level HTML (`<div>text</div>`) → text extracted
- Inline HTML (`This has <b>bold</b> text`) → text preserved in flow
- Script/style tags (`<script>alert(1)</script>`) → content stripped entirely (no text extraction from script/style)
- Malformed HTML (`<unclosed tag`) → degrades to literal text
- Empty HTML tags (`<br/>`, `<hr>`) → produce no text output

#### Handling Each Event Type

This table describes how `parse_markdown_blocks()` translates each pulldown-cmark event into IR nodes:

| pulldown-cmark Event | Builder Action (produces IR, not GPUI elements) |
|---|---|
| `Start(Paragraph)` | Push paragraph block builder |
| `End(Paragraph)` | Pop builder → `MarkdownBlock::Paragraph { spans, links }` |
| `Start(Heading { level, .. })` | Push heading block builder with level |
| `End(Heading)` | Pop builder → `MarkdownBlock::Heading { level, spans, links }` |
| `Start(CodeBlock(kind))` | Push code block builder; extract language from `kind` |
| `End(CodeBlock)` | Pop builder → `MarkdownBlock::CodeBlock { language, code }` |
| `Start(BlockQuote(_))` | Push blockquote container builder |
| `End(BlockQuote)` | Pop builder → `MarkdownBlock::BlockQuote { children }` |
| `Start(HtmlBlock)` | Begin HTML text accumulation (see [§5.1.1](#511-pulldown-cmark-html-event-handling)) |
| `End(HtmlBlock)` | Flush accumulated HTML through tag stripper → `MarkdownBlock::Paragraph` |
| `Start(List(Some(n)))` | Push `ListContext::Ordered { next_number: n }` |
| `Start(List(None))` | Push `ListContext::Unordered` |
| `End(List)` | Pop list context → `MarkdownBlock::List { ordered, start, items }` |
| `Start(Item)` | Push list item builder, prepend bullet/number |
| `End(Item)` | Pop item builder → add to current list's items |
| `Start(Table(alignments))` | Start table builder; record column count from `alignments.len()` |
| `Start(TableHead)` / `End(TableHead)` | Mark header row |
| `Start(TableRow)` / `End(TableRow)` | Increment row counter |
| `Start(TableCell)` / `End(TableCell)` | Accumulate cell spans |
| `End(Table)` | Pop builder → `MarkdownBlock::Table { alignments, header, rows }` |
| `Rule` | Emit `MarkdownBlock::ThematicBreak` |
| `Start(Strong)` | Push bold to inline style stack |
| `End(Strong)` | Pop from inline style stack |
| `Start(Emphasis)` | Push italic to inline style stack |
| `End(Emphasis)` | Pop from inline style stack |
| `Start(Strikethrough)` | Push strikethrough to inline style stack |
| `End(Strikethrough)` | Pop from inline style stack |
| `Start(Superscript)` | No-op (text content rendered plain) |
| `End(Superscript)` | No-op |
| `Start(Subscript)` | No-op (text content rendered plain) |
| `End(Subscript)` | No-op |
| `Start(Link { dest_url, .. })` | Push link style (record URL); mark link range start |
| `End(Link)` | Pop link style; register `(Range, url)` in block's links |
| `Start(Image { dest_url, title, .. })` | Switch to image-alt-text accumulation mode |
| `End(Image)` | Emit `MarkdownBlock::ImageFallback { alt }` |
| `Start(FootnoteDefinition(label))` | Begin footnote block; prepend `[^{label}]: ` |
| `End(FootnoteDefinition)` | End footnote block |
| `Start(DefinitionList)` | Treat as container, render children as blocks |
| `Start(DefinitionListTitle)` | Push bold-styled paragraph builder |
| `Start(DefinitionListDefinition)` | Push indented paragraph builder |
| `End(DefinitionList*)` | Pop respective containers |
| `Start(MetadataBlock(_))` | Begin skip mode |
| `End(MetadataBlock(_))` | End skip mode |
| `Code(text)` | Emit inline span: `MarkdownInline { code: true, text, ... }` |
| `Text(text)` | Append to current inline span text |
| `SoftBreak` | Append space to current span |
| `HardBreak` | Append newline to current span |
| `Html(html)` | Strip tags via state machine, append text content (see [§5.1.1](#511-pulldown-cmark-html-event-handling)) |
| `InlineHtml(html)` | Strip tags via state machine, append text content to current inline buffer (see [§5.1.1](#511-pulldown-cmark-html-event-handling)) |
| `InlineMath(text)` | Emit inline span: `MarkdownInline { code: true, text, ... }` |
| `DisplayMath(text)` | Emit `MarkdownBlock::CodeBlock { language: None, code: text }` |
| `TaskListMarker(checked)` | Prepend ballot-box character (U+2611 checked, U+2610 unchecked) to current span |
| `FootnoteReference(label)` | Append `[^label]` as text to current span |

#### blocks_to_elements() Rendering (Phase 2 of pipeline)

The `blocks_to_elements()` function translates each `MarkdownBlock` into GPUI elements:

| MarkdownBlock Variant | GPUI Element Construction |
|---|---|
| `Paragraph { spans, links }` | `StyledText::new(text).with_runs(runs)` wrapped in `InteractiveText` if links non-empty; wrapped in paragraph div with vertical margin |
| `Heading { level, spans, links }` | Same as paragraph but with `text_size(px(scale))` + bold weight; scale computed from level |
| `CodeBlock { language, code }` | Div with `bg(Theme::bg_darker())`, rounded corners, monospace `TextRun`; optional language label |
| `BlockQuote { children }` | Div with `border_l_2()` + `bg(Theme::bg_base())`; children recursively rendered |
| `List { ordered, start, items }` | Div per item with `pl(depth * indent)`, bullet/number prefix |
| `Table { alignments, header, rows }` | `div().grid().grid_cols(n)` with header + body cells |
| `ThematicBreak` | `div().h(px(1.0)).bg(Theme::border())` |
| `ImageFallback { alt }` | Styled text: `[image: {alt}]` in muted color |

### 5.2 Modifications to `message_bubble.rs` — Canonical Rendering Owner

**`AssistantBubble` is the single canonical owner of markdown rendering.** The `into_element()` implementation changes to call `render_markdown()` instead of passing the raw string:

**Before:**
```rust
.child(content_text)
```

**After:**
```rust
let blocks = parse_markdown_blocks(&content_text);
let has_links = /* check blocks for non-empty links fields */;
let elements = blocks_to_elements(&blocks);
// ... container.children(elements)
```

The streaming cursor (`▋`) is appended to the content string before calling `parse_markdown_blocks()`, exactly as it is today — this ensures the cursor appears at the end of the rendered markdown.

The `AssistantBubble` struct gains no new fields. Its external API is unchanged — callers still construct `AssistantBubble::new(content)`.

**Click-to-copy preservation (conditional):** The outermost container div in `AssistantBubble::into_element()` gains a `.on_click()` handler **only when the parsed markdown contains no links** (see [§2.4](#24-click-to-copy-behavior) and [§2.5](#25-click-event-precedence-bubble-copy-vs-link-click--implementation-gate)). This preserves click-to-copy for link-free messages while avoiding the link-click conflict for messages with links.

### 5.3 Modifications to `render.rs` — `render_assistant_message()`

**Current state:** `render_assistant_message()` in `render.rs` builds its own div with model label, optional thinking block, and a raw `.child(content)` for the response text. It includes a `.on_click()` handler for click-to-copy. It does NOT use `AssistantBubble` — it is a second, separate rendering path for completed messages.

**Change:** Refactor `render_assistant_message()` to delegate to `AssistantBubble`, mirroring the streaming path. This eliminates the dual rendering path. The click-to-copy behavior moves into `AssistantBubble` (see [§2.4](#24-click-to-copy-behavior)).

**Before (current code):**
```rust
pub(super) fn render_assistant_message(msg: &ChatMessage, show_thinking: bool) -> gpui::AnyElement {
    let model_id = msg.model_id.clone().unwrap_or_else(|| "Assistant".to_string());
    div()
        .w_full().flex().flex_col().gap(px(2.0))
        .child(div().text_size(px(10.0)).text_color(Theme::text_muted()).child(model_id))
        .when(msg.thinking.is_some() && show_thinking, |d| { /* thinking block */ })
        .child({
            let content = msg.content.clone();
            let text = content.clone();
            div()
                .id(SharedString::from(format!("abbl-{}", content.len())))
                // ... styling ...
                .cursor_pointer()
                .on_click(move |_event, _window, cx| {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()));
                })
                .child(content)  // ← raw string
        })
        .into_any_element()
}
```

**After:**
```rust
pub(super) fn render_assistant_message(msg: &ChatMessage, show_thinking: bool) -> gpui::AnyElement {
    let mut bubble = AssistantBubble::new(msg.content.clone());
    if let Some(ref model_id) = msg.model_id {
        bubble = bubble.model_id(model_id.clone());
    }
    if show_thinking {
        if let Some(ref thinking) = msg.thinking {
            bubble = bubble.thinking(thinking.clone()).show_thinking(true);
        }
    }
    bubble.into_any_element()
}
```

This ensures completed messages and streaming messages both flow through `AssistantBubble::into_element()` → `parse_markdown_blocks()` → `blocks_to_elements()`.

**Streaming path in `render_chat_area()`:** Already uses `AssistantBubble::new(content).streaming(true)`. No change needed other than `AssistantBubble` internally calling the two-phase pipeline.

### 5.4 Modifications to `components/mod.rs`

Add:
```rust
pub mod markdown_content;
pub use markdown_content::render_markdown;
```

---

## 6. Streaming Design (Phase B -- Conditional Draft)

> **[Phase B -- Conditional] This section has been extracted to [`spec-phase-b.md`](spec-phase-b.md)** to reduce document size and truncation risk. Phase A implementers do not need to read Phase B material.
>
> The extracted document contains sections 6.1 through 6.10:
> - 6.1 MdStream Placement
> - 6.2 mdstream API Usage
> - 6.3 Streaming State Precedence
> - 6.4 Authoritative Transition Handler Location (6.4.1-6.4.11)
> - 6.5 Streaming Finalization Transition Table
> - 6.6 Delta Feeding with UTF-8 Safety
> - 6.7 Cursor Invariants
> - 6.8 Rendering from mdstream Output
> - 6.9 Finalization
> - 6.10 Reset Matrix
>
> **All content in `spec-phase-b.md` is CONDITIONAL DRAFT -- pending mdstream validation gate (section 4.9).** Phase A is fully self-contained without any content from that document.

---



## 7. Theme Integration

**All markdown rendering colors must come from the `Theme` system.** No hardcoded hex values, no `rgb(0x...)` literals. This is critical for dark/light mode and custom theme support.

> **Note:** Other parts of the existing codebase (e.g., `render_bars.rs` exit button hover/active states) do contain hardcoded `gpui::rgb(...)` calls. This spec does not address those pre-existing instances. The constraint applies specifically to code introduced by this feature — `markdown_content.rs` and related modifications.

The `Theme` struct in `src/ui_gpui/theme.rs` provides runtime-backed color accessors that resolve from the active theme catalog (supporting bundled themes, mac-native system appearance, and custom themes).

### Color Token Mapping

All theme methods listed below have been verified to exist in the current `theme.rs` implementation. These are `pub fn` static methods on the `Theme` struct:

| Markdown Element | Color Token | Theme Method |
|---|---|---|
| Body text | Primary text | `Theme::text_primary()` |
| Heading text | Primary text | `Theme::text_primary()` |
| Code block background | Input background | `Theme::bg_darker()` |
| Code block text | Primary text | `Theme::text_primary()` |
| Code block language label | Muted text | `Theme::text_muted()` |
| Inline code background | Input background | `Theme::bg_darker()` |
| Inline code text | Primary text | `Theme::text_primary()` |
| Blockquote background | Base background | `Theme::bg_base()` |
| Blockquote border | Accent | `Theme::accent()` |
| Link text | Accent | `Theme::accent()` |
| Link underline | Accent | `Theme::accent()` |
| Table header background | Dark background | `Theme::bg_dark()` |
| Table border | Border | `Theme::border()` |
| Table alternate row | Base background | `Theme::bg_base()` |
| Thematic break | Border | `Theme::border()` |
| Strikethrough | Muted text | `Theme::text_muted()` |
| Image fallback text | Muted text | `Theme::text_muted()` |
| Bullet/number | Muted text | `Theme::text_muted()` |

### New Theme Methods Required

No new theme methods are required for Phase 1. All color needs are covered by existing accessors. The following existing methods cover the full markdown color mapping:

- **Backgrounds:** `bg_base()`, `bg_dark()`, `bg_darker()`, `bg_darkest()`
- **Text:** `text_primary()`, `text_secondary()`, `text_muted()`
- **Accents:** `accent()`, `accent_hover()`
- **UI:** `border()`, `selection_bg()`, `selection_fg()`

If markdown-specific tokens are needed in the future (e.g., a distinct `code_block_bg` for themes that want different code block coloring), they should be added to `Theme` following the existing `resolve_with_mac_native()` pattern, not hardcoded in the markdown builder.

### Font Tokens

All `FONT_SIZE_*` constants verified in `theme.rs`:

| Context | Font Family | Size | Weight |
|---|---|---|---|
| Body text | System UI (default) | `FONT_SIZE_MD` (13.0) | Normal |
| Heading 1 | System UI | `FONT_SIZE_LG` + 8.0 (24.0) | Bold |
| Heading 2 | System UI | `FONT_SIZE_LG` + 4.0 (20.0) | Bold |
| Heading 3 | System UI | `FONT_SIZE_LG` + 2.0 (18.0) | Bold |
| Heading 4 | System UI | `FONT_SIZE_LG` (16.0) | Bold |
| Heading 5 | System UI | `FONT_SIZE_BASE` (14.0) | Bold |
| Heading 6 | System UI | `FONT_SIZE_MD` (13.0) | Bold |
| Code (inline + block) | `"Menlo"` | Same as context | Normal |

Heading sizes use the existing `Theme::FONT_SIZE_*` constants where they align, with computed values for larger headings.

---

## 8. Security

### 8.1 URL Sanitization Policy

LLM responses can contain arbitrary URLs. This section defines the **normative policy** for URL handling before any URL is opened via `cx.open_url()`.

#### Allowed Schemes (Allowlist)

Only the following URL schemes are permitted:
- `https`
- `http`

All other schemes are **rejected**. This is a positive allowlist, not a denylist — any scheme not explicitly listed above is blocked.

#### Rejected Schemes (Non-Exhaustive Examples)

The following are examples of schemes that are blocked by the allowlist. This list is illustrative, not exhaustive — the allowlist is the normative control:
- `javascript:` — script execution
- `file://` — local file access
- `data:` — data URLs that could contain scripts
- `vbscript:` — Windows script execution
- `ftp:` — legacy protocol
- `mailto:` — email client invocation (may be reconsidered in future)
- Any custom or unknown scheme

#### URL Parsing and Validation

URLs are parsed and validated using `url::Url::parse`:

```rust
use url::Url;

fn is_safe_url(raw: &str) -> bool {
    let trimmed = raw.trim();
    match Url::parse(trimmed) {
        Ok(parsed) => {
            let scheme = parsed.scheme();
            scheme == "https" || scheme == "http"
        }
        Err(_) => false,
    }
}
```

This approach is superior to simple `starts_with` checks because `url::Url::parse`:
- Handles whitespace, control characters, and embedded newlines
- Normalizes scheme case (`HTTP://` → `http`)
- Rejects malformed URIs that could bypass prefix checks
- Provides proper RFC 3986 parsing

**Note:** The `url` crate must be added to `Cargo.toml`. It is a widely-used, well-audited crate (~150M downloads, maintained by the servo project).

#### Malformed URL Handling

URLs that fail `Url::parse` (returns `Err`) are treated as non-clickable:
- The link text is still rendered with link styling (underline, accent color) so the user can see it was intended as a link
- Clicking the link is a no-op (the `InteractiveText` click handler silently does nothing)
- No error message is shown to the user

#### Visible Affordance for Rejected Links

Links with rejected schemes (e.g., `javascript:`, `file://`) are rendered identically to valid links visually (underline, accent color). When clicked, nothing happens — the click is silently ignored. This is an intentional choice:
- Showing a warning/tooltip for rejected links would require additional GPUI tooltip infrastructure not currently available
- Visually distinguishing rejected links would leak security policy details to the LLM output, potentially enabling prompt injection to test for security boundaries
- The silent no-op matches browser behavior for blocked schemes in sandboxed contexts

Future improvement (Phase 2+): If tooltip infrastructure becomes available, consider showing a brief "Link blocked: unsupported scheme" tooltip on click for rejected URLs.

#### Click Handler Implementation

```rust
// In the InteractiveText on_click callback:
if is_safe_url(&url) {
    cx.open_url(&url);
}
// Silently ignore unsafe URLs — no error shown to user
```

### 8.2 URL Edge Cases

The following URL edge cases are handled by the combination of `url::Url::parse` and the `http`/`https` scheme allowlist:

| Edge Case | Behavior |
|---|---|
| **Relative URLs** (e.g., `./page`, `/path`) | `Url::parse` returns `Err(RelativeUrlWithoutBase)` → rejected by `is_safe_url`. Markdown links with relative URLs are rendered as non-clickable styled text. |
| **Extremely long URLs** (>2000 chars) | `Url::parse` succeeds (no length limit). Passed to `cx.open_url()`. Platform behavior depends on macOS `NSWorkspace` — if the URL exceeds OS limits, the open silently fails. No truncation is performed. |
| **Percent-encoding oddities** (e.g., `%2F`, `%00`, double-encoded) | `Url::parse` handles RFC 3986 percent-encoding normalization. Null bytes (`%00`) are preserved in the parsed URL but are harmless because the scheme allowlist restricts to `http`/`https`. |
| **Internationalized domain names** (e.g., `https://例え.jp`) | `Url::parse` converts to Punycode (IDNA). The Punycode form is passed to `cx.open_url()`. Display text in the rendered link shows the original markdown text, not the Punycode form. |
| **URLs with embedded credentials** (e.g., `https://user:pass@host`) | Passes `Url::parse` and scheme check. Passed to `cx.open_url()`. This is acceptable — the user sees the full URL text in the markdown and chooses to click it. |

### 8.3 HTML Content

Raw HTML in markdown (e.g., `<script>`, `<iframe>`) is stripped of tags and only text content is rendered. No HTML is interpreted or executed. See [§5.1.1](#511-pulldown-cmark-html-event-handling) for the complete enumeration of pulldown-cmark HTML event types and their handling.

---

## 9. Error Handling

### 9.1 Assert Panic Policy

This project uses `assert!` for invariant violations in two specific categories:

**Category 1 — Programmer-error invariants (assert! in all builds):**

The UTF-8 char-boundary check uses `assert!`, not `debug_assert!`:

```rust
assert!(
    stream_buffer.is_char_boundary(self.md_stream_fed_bytes),
    "md_stream feed offset is not a char boundary"
);
```

This is acceptable in a release build because:
1. It guards a condition that should **never** occur if the code is correct — `stream_buffer` only grows by appending valid UTF-8 strings from the LLM API.
2. If violated, it indicates a bug in the streaming pipeline, not a user-triggered error.
3. In safe Rust, slicing at a non-char-boundary would panic anyway (not UB); the `assert!` provides a clearer error message.
4. For a desktop application, a panic here is preferable to silently producing garbled text.

**Category 2 — Conditions that should gracefully degrade:**

All other error conditions in the markdown builder use graceful recovery, not panics:
- Unknown pulldown-cmark event types → extract text content as fallback
- Empty content → return empty `Vec<AnyElement>`
- Malformed markdown → pulldown-cmark handles gracefully by design (it never panics on input)
- mdstream errors → `append()` and `finalize()` are infallible (return `Update` directly)
- URL validation failures → silently skip the click handler (see [§8.1](#81-url-sanitization-policy))

**General policy:** `assert!` is reserved for invariant violations that indicate programmer bugs. User-triggered conditions (malformed input, network errors, unexpected data) must never panic — they gracefully degrade or log warnings.

### 9.2 Graceful Fallback for Unsupported Elements

Any pulldown-cmark event type not explicitly handled falls through to a default case that extracts any text content and appends it as plain text. The builder never panics on unknown events.

### 9.3 Empty Content

Empty or whitespace-only content produces an empty `Vec<AnyElement>`. The caller (`AssistantBubble`) handles the empty case gracefully — the bubble just renders as an empty bubble, which is acceptable.

### 9.4 Malformed Markdown

pulldown-cmark handles malformed/incomplete markdown gracefully by design — it never panics on input, always producing some event stream. The builder relies on this property and does not need its own error recovery for malformed input.

### 9.5 mdstream Errors

> **Phase B (Conditional Draft):** This subsection applies only to Phase B.

mdstream's `append()` and `finalize()` are infallible (they return `Update` directly, not `Result`). The only failure mode is programming errors in the delta-feeding logic, which are caught by the `assert!` boundary check.

---

## 10. Integration Points

### 10.1 How New Code Connects to Existing System

```
src/ui_gpui/components/markdown_content.rs  ← NEW (two-phase pipeline)
        │
        │ render_markdown() called by:
        │ (internally: parse_markdown_blocks() → blocks_to_elements())
        ▼
src/ui_gpui/components/message_bubble.rs    ← MODIFIED
│   AssistantBubble::into_element() calls parse_markdown_blocks()
│   then blocks_to_elements(); inspects IR for link presence
│   to determine click-to-copy attachment
│   (single canonical owner for all assistant content rendering)
│
src/ui_gpui/views/chat_view/render.rs       ← MODIFIED
│   render_assistant_message() delegates to AssistantBubble
│   render_chat_area() streaming path unchanged (already uses AssistantBubble)
│
src/ui_gpui/views/chat_view/mod.rs          ← MODIFIED (Phase B only)
│   ChatView struct gains md_stream, md_stream_fed_bytes, md_stream_finalized
│   apply_store_snapshot() delta-feeds mdstream
│   reset_md_stream() / finalize_and_reset_md_stream() helpers
│   Reset logic in all lifecycle transitions per §6.4
│
src/ui_gpui/components/mod.rs               ← MODIFIED
│   Exports markdown_content module
│
Cargo.toml                                  ← MODIFIED
    Phase A: adds pulldown-cmark, url
    Phase B: adds mdstream
```

### 10.2 Store Layer — No Changes

The store layer (`app_store.rs`, `ChatStoreSnapshot`, `StreamingStoreSnapshot`) is not modified. It continues to provide `stream_buffer: String` and `messages: Vec<ConversationMessagePayload>`. The markdown rendering is entirely a view-layer concern.

### 10.3 Presenter Layer — No Changes

The presenter layer continues to pass raw message content. It is not aware of markdown rendering.

### 10.4 AssistantBubble Compatibility

The `AssistantBubble` struct API is unchanged — callers still construct `AssistantBubble::new(content)` with a string. The rendering change is internal to `into_element()`. This means:
- The streaming bubble in `render_chat_area()` works without modification to its construction
- The completed message path in `render_assistant_message()` delegates to `AssistantBubble`
- All existing tests that construct `AssistantBubble` continue to compile

---

## 11. Testing Strategy

### 11.0 Testing Philosophy Alignment

This testing strategy aligns with the project's `dev-docs/RUST-RULES.md` behavioral testing philosophy:

- **Tests verify user-visible behavior**, not implementation internals. The intermediate model tests (§11.1) verify "what the user sees" (parsed structure that maps directly to visual output), not how the parser's internal stacks work.
- **No mocks of internal components.** Tests use real `pulldown_cmark::Parser` output and real GPUI contexts (for `#[gpui::test]`).
- **Tests are named for the behavior they verify** (e.g., `test_bold_text_produces_bold_span`, not `test_style_stack_push_pop`).
- **Avoid testing implementation details:** The tests do not assert on internal stack sizes, intermediate builder state, or the order of internal method calls. They assert on outputs given inputs.
- **Behavioral `#[gpui::test]` tests** focus on observable outcomes: does a link click open a URL? Does a table produce a grid? Does the element count match expectations? These test user-facing interactions and structural correctness, not GPUI internals.

### 11.1 Test Seam: Intermediate Model Tests (Phase 1, `#[test]`)

The primary test seam is the `parse_markdown_blocks()` function, which converts markdown to a plain Rust data structure (`Vec<MarkdownBlock>`) without any GPUI dependency. These tests run with standard `#[test]`, require no GPUI context, and can exhaustively verify:

1. **Inline style tests:**
   - `**bold**` → `MarkdownInline { bold: true, ... }`
   - `*italic*` → `MarkdownInline { italic: true, ... }`
   - `` `code` `` → `MarkdownInline { code: true, ... }`
   - `~~strike~~` → `MarkdownInline { strikethrough: true, ... }`
   - `[text](url)` → `MarkdownInline { link_url: Some("url"), ... }`
   - `***bold-italic***` → `MarkdownInline { bold: true, italic: true, ... }`

2. **Block structure tests:**
   - Paragraph → `MarkdownBlock::Paragraph { ... }`
   - Two paragraphs → two separate `MarkdownBlock::Paragraph` values
   - `# Heading` → `MarkdownBlock::Heading { level: 1, ... }`
   - ```` ```rust ... ``` ```` → `MarkdownBlock::CodeBlock { language: Some("rust"), ... }`
   - List with 3 items → `MarkdownBlock::List { items: vec![...] }` with 3 entries
   - Table → `MarkdownBlock::Table { ... }` with correct column count

3. **Link collection tests:**
   - Paragraph with links → `links` field contains correct `(Range, url)` tuples
   - Links in headings → heading's `links` field populated
   - Links in list items → links propagated through list structure

4. **Fallback tests:**
   - Images → `MarkdownBlock::ImageFallback { alt: "..." }`
   - HTML block-level → text content extracted, tags stripped
   - HTML inline → text content preserved in flow, tags stripped
   - `InlineHtml` event → text extracted correctly
   - `<script>` tags → content stripped entirely
   - Math → rendered as code-styled content

### 11.2 GPUI Behavioral Tests (`#[gpui::test]`)

Beyond the intermediate model tests, targeted `#[gpui::test]` tests verify that the GPUI element rendering layer behaves correctly for key interactive and structural behaviors. These tests exercise the full `render_markdown()` pipeline (not just the intermediate model):

1. **Clickable link dispatch:**
   - Render a paragraph containing `[click me](https://example.com)` via `render_markdown()`
   - Verify the output contains an `InteractiveText` element (not plain `StyledText`)
   - Verify the click handler is wired with the expected URL range count
   - Verify that `is_safe_url()` correctly gates dispatch (unit-testable separately)

2. **Table grid construction:**
   - Render a 3-column markdown table via `render_markdown()`
   - Verify the output produces a grid container element
   - Verify the grid has the expected number of child cell elements (header cells + body cells)
   - Render tables with varying column counts (1, 5, 10+) to confirm `grid_cols(n)` handles them

3. **Code block container creation:**
   - Render a fenced code block via `render_markdown()`
   - Verify the output produces a container div (not bare text)
   - Verify the container has monospace-styled text content
   - Verify code block with language label (`rust`) and without (plain fence) both render

4. **Click event precedence gate tests** (see [§2.5](#25-click-event-precedence-bubble-copy-vs-link-click--implementation-gate)):
   - Render a message with links → verify bubble-level `.on_click()` is NOT attached
   - Render a message without links → verify bubble-level `.on_click()` IS attached
   - Simulate a click on a link range → verify only `InteractiveText` handler fires
   - Simulate a click on non-link area (message without links) → verify bubble-level handler fires
   - These are pass/fail gate criteria, not optional

5. **Smoke tests:**
   - Produces non-empty `Vec<AnyElement>` for basic inputs
   - Does not panic on any standard markdown construct
   - Produces correct element count for simple multi-block inputs (e.g., 3 paragraphs → 3 elements)

These tests supplement (not replace) the intermediate model tests. The intermediate model tests provide exhaustive structural coverage; the `#[gpui::test]` tests provide confidence that the GPUI rendering layer correctly translates the model into interactive, styled elements.

### 11.3 URL Sanitization Tests (`#[test]`)

```rust
#[test]
fn url_sanitization() {
    // Valid URLs
    assert!(is_safe_url("https://example.com"));
    assert!(is_safe_url("http://example.com"));
    assert!(is_safe_url("  https://example.com  ")); // whitespace trimmed
    assert!(is_safe_url("HTTPS://EXAMPLE.COM"));     // case insensitive

    // Dangerous schemes
    assert!(!is_safe_url("javascript:alert(1)"));
    assert!(!is_safe_url("file:///etc/passwd"));
    assert!(!is_safe_url("data:text/html,<script>"));
    assert!(!is_safe_url("vbscript:MsgBox"));

    // Malformed or empty
    assert!(!is_safe_url(""));
    assert!(!is_safe_url("not a url"));
    assert!(!is_safe_url("java\nscript:alert(1)"));  // embedded newline

    // Edge cases (see §8.2)
    assert!(!is_safe_url("./relative/path"));          // relative URL
    assert!(!is_safe_url("/absolute/path"));            // relative URL
    assert!(is_safe_url("https://例え.jp/page"));       // IDN
    assert!(is_safe_url("https://example.com/path%20with%20spaces"));  // percent-encoded
}
```

### 11.4 Streaming Tests

> **Phase B (Conditional Draft):** These tests apply only to Phase B.

1. **Delta feeding:** Feed tokens one-at-a-time (e.g., "He", "llo", " **wor", "ld**"), verify committed blocks stabilize.
2. **Finalization:** Verify `finalize()` captures trailing pending content as committed.
3. **Finalization transition table:** Each row in the [Finalization Transition Table (§6.5)](#65-streaming-finalization-transition-table--acceptance-criteria) has a named test that verifies the correct `finalize_and_reset_md_stream()`/`reset_md_stream()` sequence.
4. **Reset matrix:** Each lifecycle transition (stream end, new conversation, conversation switch, stream error, Escape, Stop button, ConversationCleared) leaves clean state — `md_stream_fed_bytes == 0`, `md_stream_finalized == false`, mdstream internal state is empty.
5. **Cursor invariant:** Verify cursor `▋` never appears in committed block `raw` text or in finalized `ChatMessage.content`.
6. **Idempotency:** Verify `reset_md_stream()` is safe to call multiple times. Verify `finalize_and_reset_md_stream()` only calls `finalize()` once per stream.

### 11.5 Non-Regression Tests for Existing Chat Behaviors

Since markdown rendering changes hot render paths (`render_chat_area`, `render_assistant_message`, `AssistantBubble::into_element`), explicit non-regression tests must verify that existing chat behaviors are preserved:

1. **Streaming stop (Escape):** With a streaming message active, pressing Escape transitions `StreamingState` to `Idle` and emits `UserEvent::StopStreaming`. The chat area renders correctly after stop (no orphaned streaming elements, no panic).
2. **Streaming stop (Stop button):** Same as Escape but via the Stop button click handler.
3. **Conversation switching:** Selecting a different conversation from the dropdown clears the current messages, stops any active stream, and loads the new conversation's transcript. Markdown-rendered messages in the new conversation display correctly.
4. **Autoscroll:** During streaming, `chat_autoscroll_enabled` causes the chat to scroll to bottom on each update. After markdown rendering replaces raw text, autoscroll must still work (the triple-deferred `scroll_to_bottom()` mechanism must fire correctly with the new element tree).
5. **Click-to-copy:** Clicking on a completed assistant message (without links) copies its content to the clipboard. This must work after the `render_assistant_message()` → `AssistantBubble` delegation.
6. **New conversation (Cmd+N):** Creating a new conversation clears all messages and streaming state. The empty state renders correctly.
7. **Empty/whitespace messages:** An assistant message with empty or whitespace-only content renders as an empty bubble (no panic, no layout break).

These tests use `#[gpui::test]` and exercise the `ChatView` with `ChatState` configured to simulate each scenario. They verify state transitions and element construction, not pixel-perfect rendering.

### 11.6 Edge Cases

- Empty content → empty element vector, no panic
- Whitespace-only content → handled gracefully
- Unclosed markdown mid-stream (e.g., `**bold without closing`) → pulldown-cmark handles gracefully
- Multi-byte UTF-8 characters split across streaming chunks → `assert!` verifies char boundary
- Deeply nested lists (5+ levels) → indentation continues to increase
- Large tables (20+ columns) → grid renders without panic
- Very long code blocks → monospace text renders
- Mixed inline styles: `**bold _bold-italic_**` → correct style stacking

### 11.7 Test Framework

Tests use `#[test]` for pure logic tests (intermediate model, URL sanitization, streaming delta logic) and `#[gpui::test]` for behavioral tests requiring GPUI context (link dispatch, grid construction, code block containers, click event precedence, smoke tests, non-regression tests). The testing pyramid is weighted toward the testable intermediate model layer but includes meaningful GPUI-level verification for key interactive and structural behaviors.

---

## 12. Incremental Rollout Strategy

To reduce integration risk, the implementation is staged in two rollout phases:

### Rollout Phase A: Completed Messages (no mdstream dependency)

**Dependencies added:** `pulldown-cmark`, `url` only. **mdstream is NOT added in this phase.**

**Phase A is fully self-contained.** It can be implemented by reading §1–§5, §7–§13, §15–§22. Phase B (§6) need not be read at all.

1. Implement `markdown_content.rs` with `parse_markdown_blocks()` and `blocks_to_elements()` two-phase pipeline
2. Modify `AssistantBubble::into_element()` to call the two-phase pipeline; inspect IR for link presence to determine click-to-copy attachment
3. Refactor `render_assistant_message()` to delegate to `AssistantBubble`
4. **No mdstream** — streaming messages get markdown rendering via `AssistantBubble` calling `render_markdown()` on the full streaming content (same as completed, but re-parsed each frame)
5. Full test suite for intermediate model, GPUI behavioral tests, URL sanitization, non-regression tests, and edge cases
6. Verify visually: completed assistant messages render markdown correctly
7. Verify: streaming still works (just without the O(n) optimization)
8. Verify: click-to-copy works on completed messages without links
9. Verify: link clicks work on messages with links; bubble copy correctly not attached
10. **Resolve click event precedence gate** (§2.5) — must pass before merge

**Checkpoint:** All existing tests pass. Markdown renders correctly for completed messages. Streaming works (with O(n²) penalty on long streams, acceptable for validation). Click-to-copy preserved for link-free messages. Click event precedence verified. Phase A can be merged independently.

### Rollout Phase B: Streaming Optimization (adds mdstream) — Conditional Draft

> ** CONDITIONAL DRAFT — pending mdstream validation gate.** See [Top-Level Preconditions](#phase-b-non-authoritative-status).

**Prerequisites:**
- Phase A merged and validated
- **Dependency Validation Gate passed** (see [§4.9](#49-mdstream-api--assumed-pinned-to-v020)): `mdstream = "0.2.0"` compiles, minimal integration test passes, API matches spec assumptions

**Dependencies added:** `mdstream = "0.2.0"`

1. Add `mdstream` dependency to `Cargo.toml`
2. Add `md_stream`, `md_stream_fed_bytes`, and `md_stream_finalized` fields to `ChatView`
3. Implement `reset_md_stream()` and `finalize_and_reset_md_stream()` helper methods
4. Implement delta-feeding in `apply_store_snapshot()` (store-driven transitions)
5. Implement reset logic in all user-action lifecycle transitions (see [§6.4](#64-authoritative-transition-handler-location) and [§6.10](#610-reset-matrix))
6. Modify streaming rendering to use committed + pending block split
7. Test streaming-specific behavior: delta feeding, finalization, finalization transition table, cursor invariants, idempotency

**Checkpoint:** Streaming is now O(n) per token. All lifecycle transitions properly reset mdstream state. All finalization transition table tests pass. Idempotency verified.

This two-phase approach means Rollout Phase A can be merged and validated independently, reducing the blast radius of any streaming-related bugs.

### Phase A → Phase B Transition Decision [Phase B — Conditional]

The transition from Phase A to Phase B is **conditional** and should be driven by observable performance data, not a predetermined schedule. Phase B activation requires both a performance trigger **and** successful passage of the Dependency Validation Gate:

- **Trigger for Phase B prioritization:** If manual testing during Phase A reveals perceptible lag or stutter during streaming of responses longer than ~2,000 tokens, Phase B should be prioritized immediately (pending validation gate).
- **Monitoring:** During Phase A, add a `tracing::debug!` log in the streaming render path that reports `stream_buffer.len()` on each frame. This provides a lightweight telemetry signal for how large streaming buffers get in practice, without requiring formal metrics infrastructure.
- **If Phase A performance is acceptable:** Phase B can be deferred indefinitely to a subsequent milestone. The O(n²) cost is theoretical; for typical LLM responses (1,000–5,000 tokens), the wall-clock cost may be negligible on modern hardware with pulldown-cmark's speed.

---

## 13. Out of Scope (Phase 2)

The following features are explicitly deferred. Each is categorized as either **deferred by choice** (could be done now but is lower priority) or **blocked** (requires additional dependencies, APIs, or architectural work not yet available):

| Feature | Category | Rationale |
|---|---|---|
| **Code block copy button** | Deferred by choice | High UX value but requires additional GPUI layout work (absolute-positioned button overlay). Fast follow candidate. Note: click-to-copy on the entire assistant bubble is preserved for link-free messages (see [§2.4](#24-click-to-copy-behavior)). |
| **Bubble copy affordance for link-containing messages** | Deferred by choice | Messages with links currently have no copy affordance. A small copy icon button in the bubble header will restore this in Phase 2. |
| **Syntax highlighting** | Blocked (dependency) | Requires a highlighter crate (e.g., `syntect` or `tree-sitter`). Significant new dependency and complexity. Neither crate is currently in the project. |
| **Thinking block markdown rendering** | Deferred by choice | Thinking blocks already have their own styling. Adding markdown within them is independent and can be done as a fast follow once the main builder is stable. |
| **Completed message parse caching** | Deferred by choice | Currently, completed messages re-parse via pulldown-cmark on each GPUI frame. For typical conversations (<50 messages) pulldown-cmark is fast enough. A `HashMap<u64, Vec<AnyElement>>` keyed by content hash can be added if profiling shows need. No architectural changes required. |
| **Scroll anchoring during streaming** | Deferred by choice (may be unnecessary) | Re-rendering during streaming could cause scroll position jumps. The existing `chat_autoscroll_enabled` + `scroll_to_bottom()` triple-defer mechanism should handle most cases. Dedicated scroll anchoring may be needed only if issues arise. |
| **Image rendering** | Blocked (API complexity) | Rendering remote images requires async HTTP fetching, caching, and GPUI image element support. Out of scope for text rendering. |
| **Interactive task-list checkboxes** | Deferred by choice | Would require modifying message content, which conflicts with the read-only message model. AI response checkboxes are informational only. |

---

## 14. CodeRabbit Review Findings

The original CodeRabbit review raised 10 numbered items. During disposition analysis, an additional finding (#11, "mdstream crate not found") was identified from the same review pass — it was a late addition by CodeRabbit that arrived after the initial 10-item summary. All 11 items are documented here for completeness. The traceability table in [§15](#15-coderabbit-item--spec-section-traceability) covers all 11 items.

### Item 1 (Blocking): `debug_assert!` for UTF-8 boundary is unsafe in release

**CodeRabbit said:** `debug_assert!` is compiled away in release mode. A non-char-boundary feed could cause a panic downstream.

**Disposition: Accepted.** The specification mandates `assert!` (not `debug_assert!`). See [Section 9.1](#91-assert-panic-policy). Note: in safe Rust, slicing at a non-char-boundary is a panic, not undefined behavior — Rust's type system prevents UB in safe code.

### Item 2 (High): URL sanitization before `cx.open_url()`

**CodeRabbit said:** LLM responses can contain `javascript:`, `file://`, `data:` URLs. Need scheme allowlist.

**Disposition: Accepted.** See [Section 8.1](#81-url-sanitization-policy). The specification requires `url::Url::parse` with http/https scheme allowlist before calling `cx.open_url()`. URL edge cases documented in [Section 8.2](#82-url-edge-cases).

### Item 3 (High): Completed message re-parse on every frame

**CodeRabbit said:** Caching is deferred to Phase 2, but every GPUI frame re-parses all completed messages. Could become noticeable for long conversations.

**Disposition: Accepted as known risk.** For Phase 1, this is acceptable because:
1. pulldown-cmark is fast for typical message sizes. A 10KB message parses quickly.
2. Typical conversations have <50 messages.
3. GPUI already re-renders the entire chat area each frame (the existing `.child(content)` calls allocate strings each frame).
4. If profiling shows this is a bottleneck, a simple content-hash keyed cache can be added without architectural changes.

The specification explicitly calls out caching as a Phase 2 item with a clear implementation path: `HashMap<u64, Vec<AnyElement>>` keyed by content hash. Measurable acceptance criteria in [Section 19](#19-performance-acceptance-criteria).

### Item 4 (Medium): Theme-sourced colors not specified

**CodeRabbit said:** The issue mentions colors like "bg tint" and "accent" but doesn't specify they must come from the Theme system. Hardcoded hex values would break dark/light mode.

**Disposition: Accepted.** See [Section 7](#7-theme-integration). The specification mandates all colors through `Theme::*` methods. No hardcoded hex values in `markdown_content.rs`. All referenced methods verified against current `theme.rs`.

### Item 5 (Medium): Table column count during streaming

**CodeRabbit said:** CSS Grid requires knowing column count upfront. During streaming, the header row may not be complete.

**Disposition: Accepted.** This is handled naturally by mdstream: a table is a single block. The table header row arrives as part of the block's raw text. If the table is still in the pending block and the header isn't complete, pulldown-cmark won't emit `Start(Table)` events — it will treat the partial text as a paragraph. Once the full header row is committed, the correct column count is available. No special fallback is needed beyond the natural behavior of the markdown parser on incomplete input. See also [§2.1 Table Rendering During Incomplete Streaming](#table-rendering-during-incomplete-streaming) for the explicit fallback behavior.

### Item 6 (Medium): Strict Clippy compliance

**CodeRabbit said:** The project enforces `clippy::all = deny`, `pedantic = warn`, `cognitive_complexity = warn`. The builder's match arms will likely trigger `cognitive_complexity`.

**Disposition: Accepted.** The `parse_markdown_blocks()` implementation must be split into helper functions:
- `handle_block_start(event) -> ()`
- `handle_block_end(event) -> ()`
- `handle_inline_start(event) -> ()`
- `handle_inline_end(event) -> ()`
- `handle_text_event(event) -> ()`

This keeps each function under the cognitive complexity threshold. Targeted `#[allow(clippy::cognitive_complexity)]` may be used sparingly if unavoidable, but structural decomposition is preferred.

### Item 7 (Medium): GPUI rev pinning compatibility

**CodeRabbit said:** The project pins GPUI to rev `c67328ab`. APIs need verification against that specific rev.

**Disposition: Accepted.** See [Section 4](#4-gpui-api-usage--verification). All GPUI APIs have verification procedures documented and marked with [VERIFICATION PROCEDURE PROVIDED — must re-run against current lock state]. mdstream APIs are marked [ASSUMED] (from docs.rs). Reproducible verification instructions provided. Implementation-time validation items marked with [VALIDATE]. **Implementation must re-run all verification checks against the current lock state.**

### Item 8 (Minor): Code block copy button priority

**CodeRabbit said:** For a developer-facing AI agent, code block copy is arguably highest-value UX feature. Consider promoting to Phase 1.

**Disposition: Noted, remains Phase 2.** The copy button requires absolute positioning within a code block div (floating button), which is an additional GPUI layout concern. Phase 1 focuses on correct rendering. The copy button is marked as a fast-follow candidate. Note: click-to-copy on the entire assistant bubble is preserved for link-free messages (see [§2.4](#24-click-to-copy-behavior)).

### Item 9 (Minor): Scroll anchoring

**CodeRabbit said:** Re-parsing/re-rendering during streaming can cause scroll position jumps.

**Disposition: Noted, deferred.** The existing `chat_autoscroll_enabled` mechanism with the triple-deferred `scroll_to_bottom()` call handles the primary case (auto-scroll to bottom during streaming). If scroll-anchoring issues arise mid-conversation, they will be addressed as a follow-up.

### Item 10 (Minor): Task list markers not interactive

**CodeRabbit said:** Noted as a known UX gap.

**Disposition: Accepted as-is.** Task list checkboxes in AI responses are informational, not actionable. Making them interactive would require modifying the message content, which is a different concern.

### Item 11 (Blocking): mdstream crate not found

**CodeRabbit said:** "This crate does not appear to exist on crates.io."

**Disposition: CodeRabbit incorrect.** The `mdstream` crate exists on crates.io:
- URL: https://crates.io/crates/mdstream
- Version: 0.2.0
- Author: Latias94
- License: MIT OR Apache-2.0
- Published: 2025-12-29
- Downloads: 15,372
- Description: "Streaming-first Markdown middleware for LLM output (committed + pending blocks, render-agnostic)."
- Repository: https://github.com/Latias94/mdstream
- API docs: https://docs.rs/mdstream/0.2.0

This was verified by direct crates.io API query. CodeRabbit's search likely failed or returned no results due to the crate being relatively new.

---

## 15. CodeRabbit Item → Spec Section Traceability

The original CodeRabbit review produced 10 numbered items. Item 11 was a late-arriving finding from the same review pass (see [§14](#14-coderabbit-review-findings) preamble for explanation). All 11 items are traced below.

| # | CodeRabbit Item | Severity | Spec Section(s) | Resolution |
|---|---|---|---|---|
| 1 | `debug_assert!` UTF-8 boundary | Blocking | [§6.6](#66-delta-feeding-with-utf-8-safety), [§9.1](#91-assert-panic-policy) | Accepted — mandated `assert!` with clear rationale |
| 2 | URL sanitization | High | [§8.1](#81-url-sanitization-policy), [§8.2](#82-url-edge-cases), [§11.3](#113-url-sanitization-tests-test) | Accepted — `url::Url::parse` with scheme allowlist; edge cases documented |
| 3 | Frame re-parse cost | High | [§13](#13-out-of-scope-phase-2), [§19](#19-performance-acceptance-criteria) | Accepted as Phase 2; measurable acceptance criteria added |
| 4 | Theme-sourced colors | Medium | [§7](#7-theme-integration) | Accepted — full color mapping table with verified method references |
| 5 | Table column count streaming | Medium | [§6.8](#68-rendering-from-mdstream-output), [§2.1](#table-rendering-during-incomplete-streaming), Item 5 analysis | Accepted — handled by mdstream block splitting + pulldown-cmark behavior; explicit fallback documented |
| 6 | Clippy compliance | Medium | Item 6 analysis | Accepted — builder decomposed into helper functions |
| 7 | GPUI rev pinning | Medium | [§4](#4-gpui-api-usage--verification) | Accepted — reproducible verification; APIs categorized as [VERIFICATION PROCEDURE PROVIDED]/[ASSUMED]/[VALIDATE]; implementation must re-verify |
| 8 | Copy button priority | Minor | [§13](#13-out-of-scope-phase-2), [§2.4](#24-click-to-copy-behavior) | Noted — remains Phase 2; bubble-level copy preserved for link-free messages |
| 9 | Scroll anchoring | Minor | [§13](#13-out-of-scope-phase-2) | Deferred — existing autoscroll covers primary case |
| 10 | Task list non-interactive | Minor | [§2.1](#21-what-the-user-sees) | Accepted — informational only |
| 11 | mdstream crate existence | Blocking | [§16](#16-dependencies) | Rejected — CodeRabbit incorrect; crate verified on crates.io |

---

## 16. Dependencies

### New Dependencies — by Rollout Phase

#### Phase A Dependencies

| Crate | Version | Registry | License | Purpose |
|---|---|---|---|---|
| `pulldown-cmark` | `0.13` | crates.io | MIT | Standard Rust markdown parser. Produces event stream from markdown text. No features required beyond default. |
| `url` | `2` | crates.io | MIT OR Apache-2.0 | URL parsing and validation for link security. RFC 3986 compliant. No features required beyond default. |

**Cargo.toml additions for Phase A:**
```toml
pulldown-cmark = "0.13"
url = "2"
```

#### Phase B Dependencies (Conditional Draft — added when streaming optimization begins)

| Crate | Version | Registry | License | Purpose |
|---|---|---|---|---|
| `mdstream` | `= 0.2.0` | crates.io | MIT OR Apache-2.0 | Streaming markdown middleware. Splits growing LLM output into committed + pending blocks. **Phase B only. CONDITIONAL DRAFT — subject to Dependency Validation Gate.** Pinned to exact version `0.2.0` because the API is [ASSUMED] from docs at this version; a minor version bump could change the API surface. |

**Cargo.toml addition for Phase B:**
```toml
mdstream = "= 0.2.0"
```

**Note on version pinning:** mdstream is pinned to exact version `0.2.0` (using `=` prefix) rather than a semver-compatible range. This is because:
1. The spec's API assumptions ([§4.9](#49-mdstream-api--assumed-pinned-to-v020)) are verified only against 0.2.0 documentation
2. mdstream is a young crate (published 2025-12-29) and may make breaking changes in 0.3.x
3. The exact pin can be relaxed to `"0.2"` after Phase B implementation validates the API

mdstream's optional `pulldown` feature (depends on `pulldown-cmark ^0.13`) provides a `PulldownAdapter` for integrated event caching. This should be evaluated during Phase B implementation; the adapter provides caching benefits for committed blocks but adds coupling.

### Existing Dependencies Used

- `gpui` (pinned to rev `c67328ab`) — all UI rendering
- `shared_string` (via gpui) — for `SharedString` in text and font family

---

## 17. Files Touched

### Phase A Files

| File | Change Type | Description |
|---|---|---|
| `Cargo.toml` | MODIFIED | Add `pulldown-cmark` and `url` dependencies |
| `src/ui_gpui/components/mod.rs` | MODIFIED | Export `markdown_content` module and `render_markdown` |
| `src/ui_gpui/components/markdown_content.rs` | **NEW** | `parse_markdown_blocks()` IR model, `blocks_to_elements()` renderer, `render_markdown()` public API, `is_safe_url()`, helper functions |
| `src/ui_gpui/components/message_bubble.rs` | MODIFIED | `AssistantBubble::into_element()` uses two-phase pipeline; inspects IR for link presence; conditionally attaches `.on_click()` |
| `src/ui_gpui/views/chat_view/render.rs` | MODIFIED | `render_assistant_message()` refactored to delegate to `AssistantBubble` |
| `src/ui_gpui/theme.rs` | NOT MODIFIED | All required color tokens already exist |

### Phase B Files (Conditional Draft — additional)

| File | Change Type | Description |
|---|---|---|
| `Cargo.toml` | MODIFIED | Add `mdstream` dependency |
| `src/ui_gpui/views/chat_view/mod.rs` | MODIFIED | Add `md_stream: MdStream`, `md_stream_fed_bytes: usize`, `md_stream_finalized: bool`; `reset_md_stream()` and `finalize_and_reset_md_stream()` helpers; delta-feed logic in `apply_store_snapshot()`; reset logic in all lifecycle transitions per [§6.4](#64-authoritative-transition-handler-location) |
| `src/ui_gpui/views/chat_view/render.rs` | MODIFIED | Escape handler, Stop button handler gain `reset_md_stream()` |
| `src/ui_gpui/views/chat_view/render_bars.rs` | MODIFIED | "+" new conversation button handler gains `reset_md_stream()` |
| `src/ui_gpui/views/chat_view/command.rs` | MODIFIED | `ConversationCleared` handler gains `reset_md_stream()` |

Estimated magnitude: ~600–900 lines of new code in `markdown_content.rs` (including intermediate model), ~50–80 lines of Phase A modifications across existing files, ~100–150 lines of Phase B modifications across existing files, ~400–600 lines of tests (intermediate model + GPUI behavioral + URL sanitization + non-regression + streaming).

---

## 18. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| GPUI API difference at pinned rev vs. docs | Low | High | All GPUI APIs have verification procedures documented ([VERIFICATION PROCEDURE PROVIDED — must re-run against current lock state]). Implementation must re-run checks. See Section 4. |
| mdstream API mismatch with docs.rs documentation | Low | Medium | API signatures are [ASSUMED] from docs, not verified locally. Dependency Validation Gate ([§4.9](#49-mdstream-api--assumed-pinned-to-v020)) must pass before Phase B begins. Normative fallback architecture documented ([Top-Level Preconditions](#normative-fallback-architecture-if-mdstream-validation-fails)). Phase A works without mdstream. |
| Performance regression on long conversations | Medium | Medium | pulldown-cmark is fast for typical messages. Phase 2 caching available. Measurable acceptance criteria in [§19](#19-performance-acceptance-criteria). |
| Clippy cognitive_complexity violations | High | Low | Decompose builder into helper functions per Section 14 Item 6. |
| Scroll position issues during streaming re-render | Medium | Low | Existing autoscroll mechanism covers primary case. Defer dedicated anchoring if needed. |
| mdstream compatibility with pulldown-cmark version | Low | Medium | Both are stable crates. mdstream 0.2.0 depends on pulldown-cmark 0.13 which is the current release. |
| Table rendering edge cases (very wide, deeply nested) | Medium | Low | CSS Grid handles arbitrary column counts. Extremely wide tables may overflow the bubble width — acceptable for Phase 1. |
| Theme color resolution in tests | Medium | Low | Tests may need to set up theme state via `set_active_theme_slug()` or use known fallback colors. |
| `render_assistant_message()` refactor breaks existing behavior | Low | Medium | The refactor to delegate to `AssistantBubble` is straightforward and visually testable. Click-to-copy behavior explicitly preserved for link-free messages ([§2.4](#24-click-to-copy-behavior)). Width/styling divergence noted ([§1](#widthstyling-divergence-note)). Incremental rollout (Phase A) provides a checkpoint. Non-regression tests cover key behaviors ([§11.5](#115-non-regression-tests-for-existing-chat-behaviors)). |
| Streaming lifecycle reset missed in a code path | Medium | Medium | Comprehensive reset matrix ([§6.10](#610-reset-matrix)) with code-location cross-references. Finalization transition table ([§6.5](#65-streaming-finalization-transition-table--acceptance-criteria)) with named tests. Defensive guard at stream start ([§6.4.1](#641-transition-stream-start)). Idempotent `reset_md_stream()` and guarded `finalize_and_reset_md_stream()`. |
| Click-to-copy + InteractiveText dispatch conflict | Low | Low | Resolved by normative strategy: bubble copy disabled for link-containing messages ([§2.5](#25-click-event-precedence-bubble-copy-vs-link-click--implementation-gate)). No conflict possible — only one handler is ever active. |

---

## 19. Performance Acceptance Criteria

This section defines measurable acceptance criteria for performance.

### 19.1 Phase A (No mdstream)

| Scenario | Threshold | Acceptance Criterion | Measurement Method |
|---|---|---|---|
| Completed message, typical size | Up to ~10KB of markdown content per message | Frame-time delta ≤ 2ms above baseline (baseline = same message count with raw text rendering). No freeze, no dropped frames during scroll. | Profile with `cargo instruments` (Time Profiler) or `GPUI_FRAME_BUDGET` debug logging. Compare frame times with markdown rendering enabled vs. disabled. |
| Completed message, large | Up to ~50KB single message | Frame-time delta ≤ 8ms above baseline. Single frame spike acceptable on first render. | Same profiling method. Measure worst-case frame time during scroll through conversation containing the large message. |
| Conversation length | Up to 50 messages | No progressive degradation: frame time for 50 messages ≤ 2× frame time for 10 messages (for same average message size). | Profile with 10-message and 50-message conversations. Compare average frame times. |
| Streaming, short responses | Up to ~2,000 tokens | Full re-parse per token is acceptable. Frame-time delta ≤ 2ms above baseline during steady-state streaming. | Profile during active streaming. Measure frame times at 500, 1000, and 2000 token marks. |
| Streaming, long responses | 5,000–10,000 tokens | O(n²) re-parse is a known Phase A limitation. Frame-time delta at 10,000 tokens should be documented (not a pass/fail gate) to inform Phase B prioritization. | Profile during active streaming. Record frame time at 2000, 5000, 8000, and 10000 token marks. |
| Memory growth | Per completed message | Memory delta ≤ 2× raw string size per message (accounts for intermediate model + GPUI elements). No unbounded growth. | Use `instruments` (Allocations) or `jemalloc` stats. Measure resident memory before and after adding 50 messages. |

### 19.2 Phase B (With mdstream — Conditional Draft)

| Scenario | Threshold | Acceptance Criterion | Measurement Method |
|---|---|---|---|
| Streaming, any length | Up to 50,000+ tokens | O(n) total parsing cost. Frame-time at 50,000 tokens ≤ 2× frame-time at 1,000 tokens. No progressive degradation. | Profile at 1K, 10K, 25K, 50K tokens. Verify linear (not quadratic) growth in cumulative parse time. |
| Block commit latency | N/A | Committed blocks stabilize visually — they don't flicker or re-layout once committed. | Visual inspection + automated test asserting committed block content is stable across frames. |
| Delta feeding | Multi-byte UTF-8 tokens | Char-boundary assertion holds. No panics on any LLM output. | Feed synthetic multi-byte sequences (emoji, CJK, RTL) through streaming pipeline. |

### 19.3 Profiling Trigger and Method

**Profiling method:** Use `cargo instruments -t "Time Profiler"` on macOS to measure frame times. Alternatively, add a `tracing::debug!` span around `render_markdown()` calls and use `RUST_LOG=debug` to log per-call durations.

If any of the Phase A thresholds are exceeded during manual testing, the response is:
1. Profile with `cargo instruments` to identify the bottleneck
2. If pulldown-cmark parsing is the bottleneck → accelerate Phase B (mdstream) or add content-hash caching
3. If GPUI element construction is the bottleneck → investigate element pooling or reduce element count per message

---

## 20. Implementation Checklist

Ordered task list mirroring the architecture sections. Items marked (A) are Phase A, (B) are Phase B.

### Phase A

- [ ] **(A1)** Create `src/ui_gpui/components/markdown_content.rs` with `MarkdownBlock` / `MarkdownInline` intermediate model types
- [ ] **(A2)** Implement `parse_markdown_blocks()` — pulldown-cmark event walker producing `Vec<MarkdownBlock>`, handling all 13 `Event` variants and all `Tag` variants including `HtmlBlock`, `InlineHtml`, `InlineMath`, `DisplayMath`, `Superscript`, `Subscript`, `DefinitionList*`, `MetadataBlock`
- [ ] **(A3)** Implement `blocks_to_elements()` — intermediate model → `Vec<AnyElement>` using `StyledText::with_runs()`, `InteractiveText`, `div().grid().grid_cols()`
- [ ] **(A4)** Implement `render_markdown()` public API composing A2 + A3
- [ ] **(A5)** Implement `is_safe_url()` with `url::Url::parse` and http/https scheme allowlist
- [ ] **(A6)** Add `pulldown-cmark = "0.13"` and `url = "2"` to `Cargo.toml`
- [ ] **(A7)** Export module in `components/mod.rs`
- [ ] **(A8)** Modify `AssistantBubble::into_element()` to use two-phase pipeline; inspect IR for link presence; conditionally attach `.on_click()` for click-to-copy (only when no links present)
- [ ] **(A9)** Refactor `render_assistant_message()` to delegate to `AssistantBubble`; normalize width/styling divergence
- [ ] **(A10)** Write `#[test]` intermediate model tests (inline styles, block structures, link collection, fallbacks, HTML event handling, edge cases)
- [ ] **(A11)** Write `#[gpui::test]` behavioral tests (link dispatch, table grid, code block container, smoke tests)
- [ ] **(A12)** Write `#[test]` URL sanitization tests including edge cases
- [ ] **(A13)** Write `#[gpui::test]` non-regression tests (streaming stop, conversation switch, autoscroll, click-to-copy, new conversation)
- [ ] **(A14)** **Verify click event precedence gate** (§2.5): write `#[gpui::test]` confirming bubble copy not attached for link-containing messages and attached for link-free messages
- [ ] **(A15)** Run full verification: `cargo fmt`, `cargo clippy`, `cargo test`, visual testing
- [ ] **(A16)** Checkpoint: merge Phase A

### Phase B (Conditional Draft)

- [ ] **(B0)** **Dependency Validation Gate:** Add `mdstream = "= 0.2.0"` to `Cargo.toml`, run `cargo check`, write minimal integration test validating API surface matches [§4.9](#49-mdstream-api--assumed-pinned-to-v020). If validation fails, execute [Normative Fallback Architecture](#normative-fallback-architecture-if-mdstream-validation-fails) before proceeding.
- [ ] **(B1)** Add `md_stream: MdStream`, `md_stream_fed_bytes: usize`, and `md_stream_finalized: bool` fields to `ChatView`
- [ ] **(B2)** Implement `reset_md_stream()` and `finalize_and_reset_md_stream()` helper methods with idempotency guarantees
- [ ] **(B3)** Implement delta-feeding in `apply_store_snapshot()` with UTF-8 boundary assertion
- [ ] **(B4)** Implement mdstream reset in all lifecycle transitions per [§6.10](#610-reset-matrix), using `reset_md_stream()` for aborts and `finalize_and_reset_md_stream()` for successful completions
- [ ] **(B5)** Implement streaming render path using committed + pending block split
- [ ] **(B6)** Add defensive guard at stream start ([§6.4.1](#641-transition-stream-start))
- [ ] **(B7)** Write streaming tests: delta feeding, finalization transition table (all 9 rows), reset matrix, cursor invariant, idempotency (double-reset, double-finalize safety)
- [ ] **(B8)** Run full verification: `cargo fmt`, `cargo clippy`, `cargo test`, visual testing
- [ ] **(B9)** Checkpoint: merge Phase B

---

## 21. Implementation Minimum Slices

### Phase A Minimum Slice

The absolute minimum deliverable for Phase A (what must ship for the feature to be useful):

1. **`markdown_content.rs`** with `parse_markdown_blocks()` and `blocks_to_elements()` two-phase pipeline supporting: paragraphs, headings (1–6), bold, italic, inline code, fenced code blocks, unordered lists, ordered lists, links (clickable with URL sanitization), thematic breaks.
2. **`AssistantBubble` modification** to use two-phase pipeline with conditional click-to-copy (no bubble copy when links present).
3. **`render_assistant_message()` delegation** to `AssistantBubble`.
4. **Click-to-copy** preserved on `AssistantBubble` for link-free messages.
5. **Click event precedence gate** verified (§2.5).
6. **Tests:** Intermediate model tests for all supported constructs, URL sanitization tests, click event precedence test, basic smoke tests.

**Deferrable within Phase A** (can be fast-followed if needed to reduce initial PR size):
- Tables (grid layout)
- Blockquotes
- Strikethrough
- Task list markers
- Image fallbacks
- HTML tag stripping
- Math rendering
- Footnotes, definition lists, metadata blocks, superscript/subscript

### Phase B Minimum Slice (Conditional Draft)

The absolute minimum deliverable for Phase B:

1. **mdstream integration** (or fallback block splitter) in `ChatView`.
2. **`reset_md_stream()` and `finalize_and_reset_md_stream()`** helper methods with idempotency.
3. **Delta feeding** in `apply_store_snapshot()` with UTF-8 boundary assertion.
4. **Reset logic** for all lifecycle transitions (§6.10 full matrix).
5. **Finalization** on normal stream completion (guarded by `md_stream_finalized` flag).
6. **Tests:** Finalization transition table (all 9 rows), delta feeding test, cursor invariant test, idempotency tests.

**Deferrable within Phase B:**
- `DocumentState` usage (can use raw `Update` tracking)
- `pulldown` feature evaluation
- Block-level element caching (Phase 2)

---

## 22. References

- [Issue #62 — Markdown rendering for assistant messages](https://github.com/acoliver/personal-agent/issues/62)
- [mdstream crate on crates.io](https://crates.io/crates/mdstream)
- [mdstream API docs](https://docs.rs/mdstream/0.2.0)
- [pulldown-cmark on crates.io](https://crates.io/crates/pulldown-cmark)
- [pulldown-cmark 0.13.0 Event enum](https://docs.rs/pulldown-cmark/0.13.0/pulldown_cmark/enum.Event.html)
- [pulldown-cmark 0.13.0 Tag enum](https://docs.rs/pulldown-cmark/0.13.0/pulldown_cmark/enum.Tag.html)
- [url crate on crates.io](https://crates.io/crates/url)
- [GPUI pinned rev (zed-industries/zed)](https://github.com/zed-industries/zed/tree/c67328ab2e0d572718575e02ae07db37552e1cbe)
- [dev-docs/RUST-RULES.md](../../dev-docs/RUST-RULES.md) — project testing philosophy (behavioral testing)
