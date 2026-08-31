# Cross-transcript text selection: post-mortem and research

Date: 2026-08-31
Issue: https://github.com/acoliver/personal-agent/issues/151
Prior attempts: PR #159 (closed), PR #209 (open, stale since 2026-07-14)

This document records why two attempts at real select-and-copy failed, what the
current rendering architecture makes possible, and what the evidence says about
extending selection across the whole conversation rather than one message at a
time.

## 0. Sources

Everything below is read from primary sources rather than summarised from
secondary ones. Specifically:

- Our own code on `main`, and the two abandoned branches read with `git show`.
- GitHub issue #151 and pull requests #159 and #209, including their full comment threads and CI check status.
- The pinned GPUI source on disk at `~/.cargo/git/checkouts/zed-a70e2ad075855582/c67328a`, which is the whole Zed repository, so `crates/gpui`, `crates/markdown` and `crates/agent_ui` were read directly at the exact revision we build against.
- Alacritty's `alacritty_terminal/src/selection.rs` from `master`.
- `longbridge/gpui-component` through the GitHub contents API and crates.io API, plus its published documentation.

Claims that could not be verified are marked as such where they appear.

## 1. What "real select and copy" means here

The requested behavior is browser-grade: press the mouse anywhere in the
transcript, drag in any direction across message boundaries, see the covered
text highlighted, and press Cmd+C (Ctrl+C on Linux) to get the visible text on
the clipboard. Partial selections, whole-transcript selections, and everything
in between.

What exists today is different. `AssistantBubble` and the user message div each
carry an `on_mouse_down` handler that copies the entire message
(`src/ui_gpui/components/message_bubble.rs:219-231`,
`src/ui_gpui/views/chat_view/render.rs:411-419`), gated off when the message
contains links or is still streaming. The top bar has a copy-conversation button
(`render_bars.rs:61`). Cmd+C never touches the transcript: `handle_platform_key`
(`render.rs:159`, `"c"` branch at `render.rs:211`) copies the sidebar search
query, the conversation title input, or the composer text. So the only
granularity available is "one whole message" or "the whole conversation".

## 2. Why this is hard in this codebase

### 2.1 There is no document to select over

The transcript is not text. It is a tree of independent elements, each with its
own isolated `TextLayout` and its own byte-offset space starting at zero.

The markdown pipeline (`src/ui_gpui/components/markdown_content.rs`) parses
markdown into a `MarkdownBlock` IR and then converts each block into GPUI
elements:

| Block | Renderer | Text primitive |
|---|---|---|
| Paragraph | `render_paragraph_with_color` (L479) | 1 `StyledText`, wrapped in `InteractiveText` if it has links |
| Heading | `render_heading_with_color` (L491) | 1 `StyledText` |
| Code block | `render_code_block` (L517) | language label as `SharedString` child + body as `SharedString` child |
| Block quote | `render_blockquote_with_color` (L545) | recursive, border-left div |
| List | `render_list_with_color` (L561) | per item: bullet prefix `SharedString` + recursive content |
| Table | `render_table_with_color` (L604) | one `StyledText` **per cell** |
| Thematic break | L684 | no text |
| Image fallback | L694 | text div |

A typical assistant message with three paragraphs, a three-item list and one
fenced code block is roughly twelve independent text elements. A 3x3 table on
its own is twelve. A long conversation is hundreds. None of them share an index
space, and the app holds no handle to any of their layouts.

### 2.2 GPUI ships nothing that does this

At the pinned revision (`Cargo.toml:15`, zed rev `c67328ab`, gpui 0.2.2) the
`gpui` package contains no selectable text element and no `Editor`. Searching
`crates/gpui/src` for `selectable` returns nothing. What it does provide:

- `StyledText::layout() -> &TextLayout` (`elements/text.rs:163`)
- `TextLayout::index_for_position(Point<Pixels>) -> Result<usize, usize>` (`elements/text.rs:492`)
- `TextLayout::position_for_index(usize) -> Option<Point<Pixels>>` (~L522)
- `StyledText::with_highlights(ranges, HighlightStyle)`, where `HighlightStyle.background_color` is the only built-in per-range background paint
- `InteractiveText` (`elements/text.rs:638`), whose `paint` at L780-860 is the reference pattern for paint-time hit testing: clone the layout, register `window.on_mouse_event`, check `hitbox.is_hovered(window)`, call `index_for_position`. It is click-only and carries no selection model.

`index_for_position` requires that the specific element instance was laid out
and prepainted in the current frame. That single constraint is what killed the
first attempt.

### 2.3 Zed's own implementation is GPL and is scoped to one element

Zed solves this in `crates/markdown` (`LICENSE-GPL`, `license =
"GPL-3.0-or-later"` in its `Cargo.toml`). PersonalAgent is MIT, so that code can
only be read as an API and concept reference. The architecture, for reference:

- `RenderedText { lines: Rc<[RenderedLine]>, links }` (`markdown.rs:1900`)
- each `RenderedLine` carries `source_mappings: Vec<SourceMapping { rendered_index, source_index }>` (`markdown.rs:1795-1797`)
- `Selection { start, end, reversed, pending, mode }` (`markdown.rs:457`) stores **source** byte offsets, not rendered offsets, and `set_head(head, rendered_text)` (L466) handles reverse drag by swapping ends
- `RenderedText::source_index_for_position` (L1912) walks lines in order; when the point falls in the vertical gap between two lines it returns `Err(line.source_end)`, i.e. it snaps to the end of the line above
- `text_for_range` (L1990) reconstructs the copied string from rendered lines through the mappings

The important structural point: one `Markdown` entity holds one source string
and one `Selection`. Zed's agent panel creates one `Markdown` per message and
renders `MarkdownElement::new(markdown, style)` per message
(`crates/agent_ui/src/acp/thread_view.rs:4713`, `:4830`, `:5869`). Selection
therefore cannot cross message boundaries in Zed either. Zed issue #58319,
"Agent Panel: make copying multiple messages or transcript ranges easy", asked
for exactly this and was closed without comment.

So cross-message selection over rich markdown is not a solved problem anywhere
in the GPUI ecosystem. It has to be built.

## 3. Attempt 1: PR #159, branch `issue151` (April 2026)

Closed by the author with: "this was trash and didn't work properly."

### Approach

Flatten the whole transcript into one plain-text buffer held on `ChatView`
(`transcript_text`, `transcript_block_ranges`, `transcript_block_layouts`,
`transcript_drag_anchor`). When a selection was active, each bubble swapped from
the rich markdown tree to a flat `StyledText` with the selected range
highlighted. To get at the layouts, it introduced `TextLayoutSink`
(`Rc<RefCell<Option<TextLayout>>>`) handles that bubbles populated during
`into_element` and `ChatView` read back after paint.

### What went wrong

The commit message for `b2cf619` ("Implement layout sink pattern for transcript
text selection") documents three interlocking failures in its own words:

1. Body selection was invisible, because bubbles only rendered `StyledText` when
   a selection was supplied, but the mouse handlers could not create one without
   access to a measured layout. Chicken and egg.
2. Phantom `StyledText` layouts were created and cloned without being rendered,
   which panicked in `index_for_position`. The workaround pushed `None` layouts,
   which disabled body hit-testing entirely.
3. Thinking blocks rendered twice.

The workarounds it needed tell the story: an "armed" state so the first click
after a frame would not be lost, and `cx.defer` replay for double and triple
click. Structural gates forced further churn: `render.rs` had to be split to
stay under the 1000-line cap (`fe70806`), selection tests had to be split
(`54c1b11`), and `is_word_char` needed a lizard whitelist entry.

### Root cause

Selecting text changed what was rendered. Tables, code blocks and inline styles
became flat text the instant a drag began. Everything else followed from
fighting that inversion.

## 4. Attempt 2: PR #209, branch `issue151-selectable-markdown` (July 2026)

Still open. All eight CI checks pass: Linux, Windows, format, clippy plus
structural, lib and integration tests, coverage gate, and provider-backed E2E.
It is not blocked on CI.

### Approach

A clean-room rewrite whose plan
(`project-plans/issue151/selectable-markdown-plan.md` on that branch) forbids
attempt 1's mechanisms by name: no flattened cross-message transcript, no
alternate flat renderer, no phantom layouts, no layout sinks into `ChatView`, no
arming, no deferred replay.

Three pieces:

- `VisibleDocument` (`markdown_content/visible_document/`) is a pure data layer
  with no GPUI dependency and no geometry. It derives visible plain text, link
  ranges and semantic block ranges from the same `MarkdownBlock` IR the renderer
  uses. Copy slices this, never raw markdown.
- `LeafRegistry` / `LeafMeta { doc_range: Range<usize>, layout: TextLayout }`
  (`markdown_content/leaf_meta.rs`). Each rendered inline text leaf registers its
  live layout during the layout phase; the registry is cleared and repopulated
  each frame, so hit-testing always uses current-frame geometry.
- `SelectableMarkdown` (`markdown_content/selectable_markdown/mod.rs`) is a
  custom `Element` that drives the unchanged rich child through
  `request_layout` / `prepaint` / `paint`, registers current-frame mouse
  listeners, and paints selection quads over the child using
  `position_for_index`. Rendering does not change when you select.

Delivered: forward and reverse drag, wrapped lines, double-click word,
triple-click semantic block, right-click Copy menu, link-click versus link-drag
arbitration, `MessageRevision` freshness tokens that discard a selection when
content, conversation, stream state or the emoji filter changes, and an X11
automation script that drives the compiled app and asserts the real clipboard.

### What went wrong

Not correctness. The PR comment thread from 2026-07-13 to 2026-07-14 records
three rounds of regressions found by hand after CI was green:

- selected glyphs rendered green on green in the Green Screen theme, fixed in `76e2df3` by using the theme selection foreground
- popup open time regressed to 1.9-3.2 seconds, brought back to about 0.51s in `76e2df3` by reusing Arc-backed markdown and `VisibleDocument` caches and not cloning the whole message vector
- popup focus-steal on Linux, fixed in `b64d05b` by making popup creation non-focusing
- held-drag latency, coalesced to one owner update per 16ms frame in `b64d05b`, measured at 240ms event-to-visible

The PR also grew to 6,670 added lines and absorbed unrelated Linux tray, popup
focus and composer-selection work. CodeRabbit's pre-merge check flagged exactly
that: "The PR also changes tray/popup focus and Linux GUI harness code that is
unrelated to chat-message selection in #151."

Work stopped on 2026-07-14. No CI failure, no review rejection.

### Root cause

Two things. First, the transcript is not virtualized, so per-frame selection
work multiplies by conversation length; that is where the popup latency came
from. Second, the change set stopped being reviewable, so each hand-tested
regression cost a full re-verification cycle over 6,670 lines.

## 5. Recurring root causes across both attempts

1. **No shared index space.** Hundreds of independent `TextLayout`s, each
   starting at byte zero. A logical document has to be built before selection is
   even expressible.
2. **Layouts are only valid in the frame that produced them.** Any design that
   stores a layout, or fabricates one, breaks. Hit-testing has to happen inside
   the element's own paint phase.
3. **The transcript is laid out and painted in full, every frame.**
   `render_chat_area` (`render.rs:258`) is a plain `div().id("chat-area")
   .overflow_y_scroll().track_scroll(&self.chat_scroll_handle)` with one child
   per message (L313-321). There is no culling. Selection amplifies the existing
   cost.
4. **Existing affordances fight the gesture.** Whole-bubble copy fires on
   `on_mouse_down` anywhere in the bubble, and links are `InteractiveText`. Both
   have to be renegotiated before a press-and-drag can mean "select".
5. **Scope creep.** Both PRs pulled in refactors, structural-gate fixes and
   unrelated platform work.

## 5a. Prior art: how terminal emulators model scrollback selection

A terminal is the closest working analogue to what we want: a long scrollback of
independently styled cells with real drag-select, semantic and line modes, and
copy. Alacritty's `alacritty_terminal/src/selection.rs` (Apache-2.0) is worth
reading in full. The transferable decisions:

**Anchors carry a side, not just a position.**

```rust
struct Anchor { point: Point, side: Side }   // Side is Left or Right of the cell
pub struct Selection { pub ty: SelectionType, region: Range<Anchor> }
```

The side is what makes "press in the middle of a glyph" behave correctly and
what makes empty-selection detection exact: a selection is empty when
`start.side == Right && end.side == Left` on adjacent cells, not just when the
positions match. GPUI hands us the equivalent signal already, since
`TextLayout::index_for_position` returns `Result<usize, usize>` where `Err`
means the point was outside the glyph run and got clamped.

**Anchors are stored unordered and sorted at read time.**

`Selection::update` only overwrites `region.end`. Ordering happens in
`to_range`, which does `mem::swap` if `start.point > end.point`. Reverse drag
needs no special case and no `reversed` flag. Zed's markdown crate uses an
explicit `reversed: bool` instead; Alacritty's version is simpler and is what
attempt 2's anchor/head model is already closest to.

**Selection modes are an enum on the selection, not on the gesture.**

`SelectionType { Simple, Block, Semantic, Lines }`. `Semantic` keeps expanding
to word boundaries as the drag continues, and `Lines` keeps expanding to whole
lines. This maps directly onto attempt 2's `SelectionMode { Char, Word, Block }`
and confirms that mode belongs on the persisted selection so that continuing a
double-click drag extends word-by-word rather than character-by-character.

**Scrolling rewrites the anchors, and out-of-range selections are dropped.**

```rust
pub fn rotate<D: Dimensions>(mut self, dimensions: &D, range: &Range<Line>, delta: i32)
    -> Option<Selection>
```

`rotate` shifts both anchors by the scroll delta, clamps them to the region, and
returns `None` when the selection has scrolled entirely out of the grid. The
module doc states the policy plainly: "The selection should be cleared when text
is added/removed/scrolled on the screen."

This is the reference answer for our streaming-mutation question. If we key
transcript positions on `message_index` plus a byte offset, appending a new
message is inherently stable (indices do not shift, the new content is at the
end), and streaming tokens only mutate the last message. Deleting a message,
switching conversations, or toggling the emoji filter shifts or rewrites content
and should drop the selection, which is what attempt 2's `MessageRevision`
freshness token already does per message. Extending it to a transcript-scoped
selection means checking the revision of both endpoint messages.

**Renderers ask "does the selection touch this range?" before doing work.**

```rust
pub fn intersects_range<R: RangeBounds<Line>>(&self, range: R) -> bool
```

Alacritty's renderer uses this to skip lines the selection cannot touch. The
equivalent for us is the per-message test in section 6 step 4: a message asks
whether the global selection intersects its own document range, and paints
nothing if not. With virtualization this is also the cheap path for deciding
whether an off-screen message needs any work at all.

**Wide glyphs get explicit handling.**

`contains_cell` treats a wide char's trailing spacer as selected when the wide
char is. Our analogue is UTF-8 boundary snapping, which attempt 2 already
implements in `clamp_to_char_boundary` and covers with tests over ASCII,
accented, CJK, combining and emoji samples.

## 5b. An existing Apache-2.0 implementation of exactly this

`longbridge/gpui-component` contains a window-level cross-element text selection
engine. It is not a per-widget selection like Zed's; it is explicitly designed
for selection that starts in one element and continues into others, including
virtualized documents.

**License.** The repository ships `LICENSE-APACHE`, "Copyright 2024 - 2026
Longbridge, Licensed under the Apache License, Version 2.0". GitHub's API reports
`NOASSERTION` only because the file is not named `LICENSE`. Apache-2.0 is
compatible with an MIT project: it can be depended on, vendored, or adapted, so
long as the license text and attribution are retained and modified files are
marked. This is a real option in a way that Zed's GPL markdown crate is not.

**Where the code is.** `crates/base` in that repository, which is the `gpui-base`
crate. File sizes on `main`:

| File | Size | Purpose |
|---|---|---|
| `crates/base/src/text_selection.rs` | 126,389 bytes | the selection engine |
| `crates/base/src/selectable_text.rs` | 10,277 bytes | a selectable text element built on it |
| `crates/base/src/auto_scroll.rs` | 5,068 bytes | auto-scroll during drag |
| `crates/base/src/text_boundary.rs` | 2,411 bytes | word boundaries |
| `crates/base/src/virtual_list.rs` | 33,921 bytes | virtualized list |

**The API**, from https://longbridge.github.io/gpui-component/base/text-selection:

| Type | Lifetime | Purpose |
|---|---|---|
| `TextSelectionLayer` | once per window | installs window-level pointer handling and holds selection state |
| `TextSelection` | static | query and control the window selection |
| `TextSelectionHandle` | once per selectable participant | identity, callbacks, projected selection |
| `TextSelectionRegistration` | recreated each frame | hitbox, bounds, scroll offset, scope, document order |
| `TextSelectionRun` | during paint | laid-out text plus its `TextLayout` and bounds |
| `TextSelectionProjection` | returned by `update_runs` | UTF-8-safe byte range per run |
| `TextSelectionSnapshot` | on selection change | endpoints and coverage |
| `TextSelectionEvent` | to subscribers | selection changes, clearing, auto-scroll requests |
| `TextSelectionContentKey` | stable identity | identifies virtualized content at a selection endpoint |

The integration contract is five steps: retain one `TextSelectionLayer` at the
window root; create one `TextSelectionHandle` per participant; call
`TextSelectionHandle::register` during prepaint with the current hitbox, bounds,
scroll offset and `document_order`; call `TextSelectionHandle::update_runs`
during paint with the laid-out runs; paint each returned byte range behind its
glyphs.

Several points in the documentation address the exact problems from section 4:

- `text_bounds` is separate from `bounds`, and "blank-only drags do not start a text selection". That is the gap-between-elements problem.
- `document_order` is required and the docs warn against deriving it from a `HashMap` or from paint order. That is the ordering problem for cross-participant copy.
- `TextSelectionEvent` carries auto-scroll requests, and `auto_scroll.rs` exists as a sibling module. That is the drag-past-the-edge problem.
- `copy_with` is documented as the hook to "export source text or include virtualized content that is not currently painted", and `TextSelectionSnapshot::coverage()` distinguishes a participant selected in part, from its start, to its end, or in full. That is the off-screen-content problem, solved the same way section 7 proposes.
- The docs state that wrapped selections need three highlight rectangles: the remainder of the first line, full-width middle lines, and the prefix of the last line. Attempt 2 hit this and fixed it in `76e2df3` ("Corrected selection rectangles at GPUI soft-wrap boundaries").

**Caveats, and they matter.**

1. `gpui-base` on crates.io is a name reservation, not a library. Version 0.1.0,
   published 2026-08-11, 788 bytes, three lines of Rust, `has_lib: false`. The
   real code is only in the repository workspace. Using it means a git
   dependency, a vendored copy, or waiting for a real release. We already take a
   git dependency on `gpui` from the Zed repository, so a git dependency is not a
   new kind of risk, but pinning a rev of a 12.9k-star repository under active
   development is a maintenance cost.
2. `gpui-component` targets `gpui = "0.2.2"`, which is the version our pinned Zed
   rev reports. That needs verifying against our exact rev, not just the version
   string, because we pin a git rev and they pin a crates.io release.
3. Taking the whole of `gpui-component` for one subsystem is not sensible. The
   question is whether `gpui-base`'s selection module can be used alone, and
   whether it drags in that crate's `Theme`, `ActiveTheme` and global state.
   `crates/base/src/theme.rs` and `global_state.rs` both exist and the selection
   code likely touches at least the theme for highlight color.
4. `longbridge/gpui-component` issue #2110 asks for a Flutter-style
   `SelectionArea` and describes the current state as per-element only. The
   window-level engine documented above appears to be the answer to that request.
   How complete it is in practice, and how well it behaves with a rich markdown
   tree of the shape we render, is unverified. Reading `text_selection.rs` and
   building a spike against our transcript is the next step.

**What this changes.** Section 6 describes building `TranscriptPosition`, a
transcript-scoped leaf registry, gap resolution, per-participant highlight
slicing and autoscroll. That is a description of `gpui-base`'s public API.
Before writing it a third time, spike the existing implementation against our
transcript and find out whether it works. If it does, attempt 2's
`VisibleDocument` still earns its place: it is what produces the syntax-free
visible text and the `TextSelectionRun` contents, and it is what `copy_with`
would slice for off-screen messages.

## 6. What has to change for whole-history selection

Attempt 2 scoped selection to one message deliberately. The relevant types:

```rust
// chat_view/mod.rs
struct ActiveMessageSelection {
    message_index: usize,
    revision: MessageRevision,
    selection: Selection,
    dragging: bool,
}

// markdown_content/visible_document/selection.rs
pub struct Selection {
    anchor: usize,   // byte offset into ONE message's visible document
    head: usize,
    mode: SelectionMode,
}
```

The delta to cross-history selection:

1. **Position type.** `anchor` and `head` become
   `TranscriptPosition { message_index: usize, revision: MessageRevision, offset: usize }`.
   Ordering across the transcript is `(message_index, offset)`.
2. **Registry scope.** `LeafRegistry` moves from per-element to transcript scope,
   and `LeafMeta` gains the owning message index. Every visible leaf in the
   window registers into one registry per frame.
3. **Hit testing across gaps.** A transcript-level resolver picks the leaf whose
   bounds contain the pointer; when the pointer is in bubble padding, between
   bubbles, or past the last message, it snaps to the nearest leaf in reading
   order. Zed's `source_index_for_position` gap rule (return the end of the line
   above) is the reference behavior.
4. **Painting.** Each message computes the intersection of the global selection
   range with its own document range and paints only that slice. Messages
   entirely inside the range paint full coverage without needing per-character
   geometry.
5. **Copy.** Concatenate `VisibleDocument::selected_text` slices in message
   order, with a blank-line separator between messages. Role prefixes are a
   question to settle: raw text matches browser behavior, role labels match the
   existing copy-conversation export.
6. **Autoscroll during drag.** When the pointer sits at the viewport edge, no
   further mouse-move events arrive. GPUI provides `window.request_animation_frame()`
   and `window.on_next_frame()` (`window.rs:1816`, `:1826`), and
   `cx.background_executor().timer(Duration)` (`executor.rs:272`). Drive a
   per-frame tick while a drag is active and the pointer is outside the
   viewport, adjusting `chat_scroll_handle` and re-resolving the head each tick.

The part that makes this tractable: `VisibleDocument` has no geometry. Offsets
exist for messages that are scrolled off screen or never laid out. Geometry is
needed only to paint the visible slice and to resolve the pointer.

## 7. Virtualization is a prerequisite, not a conflict

The obvious worry is that virtualizing the transcript would break selection over
off-screen content. It does not, because the document model is decoupled from
geometry. Selection over an off-screen message is expressible; it simply is not
painted until you scroll to it.

GPUI's `gpui::list` with `ListState` (`elements/list.rs:54`) is the
variable-height virtualized list, and it exposes `bounds_for_item` (L443) and
`logical_scroll_top` (L368). `uniform_list` requires equal item heights and is
not applicable to chat bubbles.

Zed's own agent panel uses exactly this:
`ListState::new(0, gpui::ListAlignment::Bottom, px(2048.0))`
(`crates/agent_ui/src/acp/thread_view.rs:451`), rendered with `list(...)` at
L7426. Bottom alignment, 2048px of overdraw.

Moving `render_chat_area` from a plain scrolling div to `list` would fix the
latency class that stalled attempt 2, and would give per-item bounds that the
selection resolver can use directly. It is also a change that stands on its own
merit and can be verified independently.

## 8. Recommendation

Do not start a third clean-room attempt from nothing. Two things already exist
that a third attempt would otherwise reinvent: attempt 2's `VisibleDocument` and
`SelectableMarkdown`, which are CI-green, and `gpui-base`'s window-level
selection engine, which is Apache-2.0 and solves the cross-element half.

The branch is four commits behind main; `git merge-tree` reports conflicts in
exactly two files, `chat_view/mod.rs` and `chat_view/render.rs`.

### Step 0: spike before committing to an architecture

Before any of the steps below, answer one question with code: can `gpui-base`'s
`TextSelectionLayer` / `TextSelectionHandle` be used against our GPUI rev, in
isolation from the rest of `gpui-component`, with our markdown tree as the
participants? Time-box it. The output is a yes or no plus a list of what it
drags in (theme, global state, other modules).

- If yes, sections 6 and 7's `TranscriptPosition` and transcript-level registry
  are replaced by their API, and our work becomes producing correct
  `TextSelectionRun`s and correct `copy_with` slices from `VisibleDocument`.
- If no, section 6 stands as the build plan, informed by their documented
  contract and by Alacritty's anchor model in section 5a.

### Then, split so each piece is reviewable and hand-testable

1. **Land the selection primitives, per-message, nothing else.** Rebase
   `issue151-selectable-markdown` onto main, move the tray, popup-focus and
   composer-selection commits to their own branches, and ship `VisibleDocument`
   plus per-message selection. This is the reviewable core of a 6,670-line PR
   reduced to its subject. Do this whichever way the spike goes: the visible
   document is needed either way.
2. **Virtualize the transcript** with `gpui::list` and `ListState`, bottom
   aligned. Measure popup open time and drag latency before and after. This is
   independently valuable and removes the performance ceiling that stalled
   attempt 2.
3. **Promote the selection to transcript scope**, either through `gpui-base` or
   through `TranscriptPosition` plus a transcript-level leaf registry, gap
   resolution, per-message highlight slicing and ordered copy concatenation.
4. **Add autoscroll during drag**, either `TextSelectionEvent`'s auto-scroll
   request or a `request_animation_frame` tick.
5. **Retire the conflicting affordances**: whole-bubble click-to-copy has to go
   or move to an explicit control, and link activation becomes press-release
   under the drag threshold.

### Verification, given how attempt 2 failed

CI being green was not sufficient last time. Every step needs a hand-test pass
covering the four things that were found manually and never by a test:

- selection colors in every shipped theme, Green Screen first
- popup open time, with a number, before and after
- popup focus behavior on Linux
- press-to-visible-highlight latency, with a number

### Open questions to settle before step 3

- Does copy include role labels, or only the text?
- Does Cmd+A in the transcript select the whole conversation, and how does that
  interact with the composer's existing select-all?
- Does selection survive a new message arriving mid-drag, or is it invalidated?
  Alacritty's answer is to invalidate; Zed's markdown crate has no streaming case.
- Should code blocks keep a dedicated copy button even once selection works?

## 9. Related defect found while reading this code

Issue #223: in Green Screen, links inside the user bubble are invisible.
`inline_to_text_run` (`markdown_content.rs:390-427`) hardcodes `Theme::accent()`
for the link glyph and underline, ignoring the `text_color` passed by the
caller. Green Screen sets `accent.primary` and `message.userBorder` to the same
`#6a9955`, and `user_bubble_bg()` resolves from `message.userBorder`. The link
is drawn in the bubble's own background color. Body text is unaffected because
it uses `user_bubble_text()`, which resolves from `selection.fg` (`#000000`).

Attempt 2 hit the same class of bug with selected glyphs and fixed it in
`76e2df3` by routing through the theme's selection foreground.
