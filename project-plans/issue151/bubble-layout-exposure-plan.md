# Plan: Expose Bubble TextLayout for Transcript Hit-Testing (v6 — Final)

@plan PLAN-20260406-ISSUE151.P01

## Problem Statement

Issue #151 adds traditional text selection (drag, double-click word, triple-click paragraph) and copy (Cmd+C, right-click) to the GPUI chat transcript. Branch `issue151`, PR #159.

Three confirmed bugs stem from one architectural mismatch:

### Bug 1: Body selection cannot be initiated

`transcript_block_index_at_point` hit-tests `transcript_block_layouts`. For message bodies, this contains `None`. The bubble renders markdown internally and doesn't expose its `TextLayout`. Hit-test → `None` → no selection created. (Highlight rendering works when `selection` IS `Some(...)`.)

### Bug 2: Unmeasured layout panic (worked around)

Phantom `StyledText` → cloned `TextLayout` → dropped unrendered → paint never runs → `index_for_position` panics. Workaround: `None` for body layouts.

### Bug 3: Thinking block duplication

`build_thinking_row` + `AssistantBubble` internal thinking = two visible copies.

### Bug 4 (hypothesis): Only-first-conversation selection

Unconfirmed. Stage 5 adds tracing to diagnose.

## Root Cause

**ChatView has no access to measured `TextLayout` objects inside `UserBubble` / `AssistantBubble` after GPUI paints them.**

---

## Architecture Decision: Layout Sink in Bubbles

An overlay approach (ChatView renders transparent `StyledText` siblings for hit-testing) was considered and rejected:
- Overlay geometry would need pixel-perfect alignment with markdown-rendered bubbles (headings, lists, code blocks, tables). This is impractical — markdown produces complex div trees, not flat text.
- Width/padding constraints differ between `UserBubble` (max-width 300, right-aligned) and `AssistantBubble` (full-width, themed wrapper). Matching these exactly in an overlay is fragile.
- Pointer interactions (click-to-copy, links) would conflict with the overlay layer.

**Chosen approach: layout sink exposure from bubble components.**

Each bubble accepts optional `TextLayoutSink` handles. When the bubble renders flat `StyledText` (in selectable mode), it stores the layout handle in the sink. ChatView reads sinks after paint for hit-testing. Bubbles retain full ownership of their rendering.

---

## Design Decisions

### 1. Markdown flattening scope: active interaction only

Flat `StyledText` rendering (enabling sinks + highlights) activates only when:
- `state.text_selection.is_some()`, OR
- `self.transcript_selection_armed` is `true`

Otherwise bubbles render markdown. Markdown loss is transient — returns when selection is cleared and armed is false.

### 2. Layout sink type and placement

```rust
// In src/ui_gpui/components/selectable_text.rs (alongside existing find_word/paragraph helpers)
pub type TextLayoutSink = std::rc::Rc<std::cell::RefCell<Option<gpui::TextLayout>>>;
```

Placed in `components/selectable_text` — the module already owns selection-related helpers (`find_word_boundaries`, `find_paragraph_boundaries`, `is_word_char`). This avoids view→component dependency direction issues. Bubbles import from the same component module.

### 3. Centralized text builder with char-boundary safety

Single shared helper replaces both `render_transcript_text` (which currently does NOT snap to char boundaries) and `render_selection_styled_text` (which does):

```rust
// In src/ui_gpui/components/selectable_text.rs
pub fn build_selectable_styled_text(
    text: &str,
    selection: Option<&Range<usize>>,
    base_color: gpui::Hsla,
) -> StyledText
```

This helper:
- Snaps selection bounds to UTF-8 char boundaries (fixing the inconsistency between the two current implementations).
- Builds `TextRun` spans for before/selected/after regions.
- Returns a `StyledText` whose `layout()` handle can be cloned into a sink.

### 4. Armed state and first-click handling

On first mousedown when no flat layouts exist:

1. Set `transcript_selection_armed = true`.
2. `cx.notify()` → re-render with flat mode + sinks.
3. **Single click (click_count == 1):** No replay needed. The user will move the mouse (drag), and the `on_mouse_move` handler will resolve the offset against now-populated sinks. If the user clicks without dragging, `on_mouse_up` clears the armed state — no selection created, which is correct (single click without drag = no selection).
4. **Double/triple click (click_count >= 2):** Stash `PendingClick { position, click_count }` and use `cx.defer(move |this, cx| { ... })` to replay on the next frame:
   - Resolve position via `transcript_block_index_at_point` (now populated after paint).
   - Apply click_count: 2 → `select_word_at_offset`, 3 → `select_paragraph_at_offset`.
   - Clear pending click.

This scopes the deferred replay to multi-click gestures only, where immediate resolution is required. Single-click drag works naturally through the existing mouse-move handler.

### 5. Armed state clearing: centralized

Extend `clear_transcript_selection` to also clear `transcript_selection_armed` and `transcript_pending_click`. Route ALL direct `state.text_selection = None` assignments through this method:

| Call site | Current behavior | Action |
|---|---|---|
| `clear_transcript_selection` (mod.rs) | Clears text_selection + drag_anchor | Add: armed, pending |
| `on_mouse_down_out` (render.rs) | Calls clear_transcript_selection | OK |
| `on_chat_pointer_down_left` filter path (render.rs) | Calls clear_transcript_selection | OK |
| `apply_store_snapshot` Ready (mod.rs) | `state.text_selection = None` directly | Route through clear_transcript_selection |
| `apply_store_snapshot` Idle (mod.rs) | `state.text_selection = None` directly | Route through clear_transcript_selection |
| Cmd+N (render.rs) | `state.text_selection = None` directly | Route through clear_transcript_selection |
| New-conversation button (render_bars.rs) | `state.text_selection = None` directly | Route through clear_transcript_selection |
| `handle_conversation_cleared` (command.rs) | Calls clear_transcript_selection | OK |

### 6. Text equivalence invariant

```
transcript_text[block_range] == bubble_flat_text == sink_layout_text
```

- `build_transcript_buffer` and bubbles use identical `msg.content` / `msg.thinking` strings.
- Flat mode renders `self.content` directly — no markdown transforms, no streaming cursor.
- **Thinking**: flat mode renders raw thinking text without `"Thinking: "` prefix.

### 7. Selectable block model

Selectable blocks are **persisted message bodies** and **persisted thinking blocks** (when `show_thinking`). Streaming content is excluded:

- `build_transcript_buffer` may append a streaming-thinking range. This range gets no sink (not hit-testable).
- Vector alignment: after `build_message_rows` returns sinks for persisted blocks, push a placeholder `Rc::new(RefCell::new(None))` for any streaming-thinking range. OR exclude streaming-thinking from `transcript_block_ranges` entirely (cleaner — decide during implementation based on whether copy semantics need it).

### 8. Separator bytes

Newline characters between blocks in `transcript_text` are outside any block range. This is intentional:
- Hit-testing between bubbles returns `None` (no block owns that byte).
- Cross-block copy via `transcript_text[range]` includes separators — correct behavior.

### 9. Streaming excluded

No sinks, no flat mode for streaming bubbles. `render_streaming_message` unchanged.

### 10. Filter-emoji preserved

`filter_emoji` true → empty transcript, no sinks, no flat mode, no armed state.

---

## Implementation Stages

### Stage 1: Fix thinking block duplication

**Goal:** Remove standalone `build_thinking_row`. `AssistantBubble` is sole thinking renderer.

#### Tests first

**Test: `build_message_rows_one_row_per_message`**
- 1 user msg + 1 assistant msg with thinking, `show_thinking=true`.
- `build_message_rows` returns 2 rows (not 3).

**Test: `build_message_rows_count_unchanged_thinking_disabled`**
- Same, `show_thinking=false` → 2 rows.

#### Implementation

1. Delete `build_thinking_row` from `render.rs`.
2. `build_message_rows`: one row per message. No separate thinking rows.
3. `build_transcript_buffer` unchanged — thinking block ranges kept for copy.

### Stage 2: Consolidate text builder + add sinks to bubbles

**Goal:** Single `build_selectable_styled_text` with char-boundary safety. Bubbles accept `TextLayoutSink` handles. Flat mode populates sinks.

#### Tests first

**Test: `styled_text_no_selection_single_run`**
- `"Hello"`, no selection → one run, length 5, no background.

**Test: `styled_text_with_selection_correct_runs`**
- `"Hello world"`, selection `0..5` → two runs, first with selection_bg.

**Test: `styled_text_snaps_multibyte_boundaries`**
- `"café"`, selection `3..5` → no panic. Run lengths sum to byte length.

**Test: `styled_text_empty_selection_no_highlight`**
- `"Hello"`, selection `5..5` → one run, no background.

**Test: `thinking_flat_text_raw_no_prefix`**
- Extract logic for thinking flat text.
- `thinking: "deep reasoning"`, selectable → text is `"deep reasoning"`, not `"Thinking: deep reasoning"`.

**Test: `sink_not_populated_in_markdown_mode`**
- Bubble with `selectable: false`, sink provided.
- `into_element`.
- `sink.borrow().is_none()`.

#### Implementation

1. Add `build_selectable_styled_text` to `components/selectable_text.rs`. Includes char-boundary snapping.
2. Add `TextLayoutSink` type to `components/selectable_text.rs`.
3. Update `components/mod.rs` to re-export both.
4. Add to `UserBubble`: `body_layout_sink: Option<TextLayoutSink>`, `selectable: bool` + builders.
5. Add to `AssistantBubble`: `body_layout_sink`, `thinking_layout_sink`, `selectable` + builders.
6. `UserBubble::into_element`: if `selectable` → `build_selectable_styled_text(self.content, self.selection, ...)`, capture `styled.layout().clone()` in sink, render flat. Else → existing markdown path.
7. `AssistantBubble::into_element`: body same. Thinking: if `selectable` and thinking exists and `show_thinking` → render raw thinking via shared helper, capture in thinking_sink. Else → `Theme::badge` with prefix.
8. Remove `render_selection_styled_text` from `message_bubble.rs`.
9. Replace `render_transcript_text` in `render_messages.rs` with calls to shared helper.

### Stage 3: Wire sinks through ChatView + armed/pending state

**Goal:** `build_message_rows` creates sinks, threads to bubbles. Armed state + `cx.defer` replay.

#### Tests first

**Test: `hit_test_no_panic_empty_sinks`**
- Fresh sinks → `transcript_block_index_at_point` returns `None`, no panic.

**Test: `selectable_flag_logic`**
- `transcript_selectable(filter_emoji)`: no selection + not armed → false. Selection → true. Armed → true. filter_emoji → always false.

**Test: `clear_transcript_selection_resets_all`**
- Set text_selection + drag_anchor + armed + pending. Clear. Assert all gone.

**Test: `conversation_switch_clears_all`**
- Set armed + pending + selection. Trigger load. Assert cleared.

**Test: `pending_double_click_replay_creates_word_selection`**
- Install transcript. Stash pending click_count=2 at a known offset.
- Simulate deferred replay callback.
- Assert: `text_selection` covers expected word.

**Test: `single_click_does_not_stash_pending`**
- First mousedown with click_count=1 and no sinks.
- Assert: `transcript_selection_armed = true` but `transcript_pending_click = None`.
- Selection resolves via subsequent mousemove (not via deferred replay).

**Test: `vector_alignment_persisted_plus_streaming`**
- Transcript buffer with streaming thinking range.
- Layouts for persisted only + placeholder.
- Assert: `ranges.len() == layouts.len()`.

#### Implementation

1. Add `ChatView` fields: `transcript_selection_armed: bool`, `transcript_pending_click: Option<PendingClick>`.
2. `transcript_block_layouts` → `Vec<TextLayoutSink>`.
3. `transcript_block_index_at_point`: `sink.borrow().as_ref()?.index_for_position(...)`.
4. Add `transcript_selectable(&self, filter_emoji: bool) -> bool`.
5. Extend `clear_transcript_selection` (armed + pending). Audit all `text_selection = None` → route through it.
6. `build_message_rows`: accept `selectable`, create body/thinking sinks, thread through `render_message` → bubbles.
7. Push placeholder sink for streaming thinking range if present.
8. `on_chat_pointer_down_left`: if no hit and not armed → arm + `cx.notify()`. If click_count >= 2, also stash `PendingClick` and `cx.defer(replay)`.
9. Deferred replay (multi-click only): resolve offset, apply click_count, clear pending. Single-click drag resolves naturally via `on_chat_pointer_move` on the next frame.
10. Update `render_message` / `render_user_message` / `render_assistant_message` signatures.

### Stage 4: Verify block model + cross-block selection

#### Tests first

**Test: `transcript_blocks_alternate_body_and_thinking`**
- 2 messages with thinking → 4 ranges, contiguous, text slices match.

**Test: `copy_across_body_and_thinking`**
- Selection spanning body → thinking → clipboard includes separator.

**Test: `word_select_respects_boundary`**
- Offset near boundary → word within one block.

**Test: `multibyte_across_blocks_valid_utf8`**
- Cross-block selection on multibyte content → valid UTF-8.

#### Implementation

Verify existing functions. Fix only if tests reveal regressions.

### Stage 5: Diagnose only-first-conversation bug

1. `tracing::debug!` in hit-test miss, pointer-down miss, conversation-switch clear.
2. Reproduce, collect logs, file evidence-based fix.

---

## Files to change

| File | Changes |
|---|---|
| `components/selectable_text.rs` | Add `TextLayoutSink` type. Add `build_selectable_styled_text` (char-boundary-safe, replaces both `render_transcript_text` and `render_selection_styled_text`). |
| `components/mod.rs` | Re-export `TextLayoutSink` and `build_selectable_styled_text`. |
| `components/message_bubble.rs` | Add `body_layout_sink`, `thinking_layout_sink`, `selectable` fields + builders. Update `into_element` for both bubbles. Remove `render_selection_styled_text`. |
| `chat_view/mod.rs` | `transcript_selection_armed`, `transcript_pending_click` fields. `transcript_block_layouts` → `Vec<TextLayoutSink>`. `transcript_selectable` helper. Extended `clear_transcript_selection`. Audit text_selection=None sites. |
| `chat_view/render.rs` | Delete `build_thinking_row`. `build_message_rows` creates/threads sinks + selectable flag. Armed/pending in `on_chat_pointer_down_left`. `cx.defer` replay. Placeholder for streaming thinking. |
| `chat_view/render_messages.rs` | Replace `render_transcript_text` with `build_selectable_styled_text`. Update `render_message` chain signatures (sinks + selectable). |
| `chat_view/command.rs` | Verify clearing routes through `clear_transcript_selection`. |
| `chat_view/mod_tests.rs` | New tests per stage (~20 tests). |

## Estimated scope

~300–400 LoC changed across 7 files. ~200–250 LoC of new tests. No new files (extends existing `selectable_text.rs`). No new dependencies.

## Risk mitigation

| Risk | Mitigation |
|---|---|
| One-frame latency | Sinks `None` on frame 1. `cx.defer` replays double/triple click on frame 2. Single-click resolves via mousemove. ~16ms. |
| Markdown loss during selection | Only when armed/selecting. Returns on clear. |
| Thinking text mismatch | Flat mode uses raw text (no prefix), matching transcript. |
| Char-boundary panics | Shared helper snaps to boundaries. Tests cover multibyte. |
| Separator bytes | Outside block ranges. Intentional. Cross-block copy correct. |
| Streaming | Excluded from sinks/flat mode entirely. |
| Filter-emoji | No sinks, no flat mode, no armed state. |
| Armed state leaks | Centralized clearing via `clear_transcript_selection`. |
| User bubble width mismatch | Current code uses `w(px(400.0))` in selection path vs `max_w(px(300.0))` in markdown path. Fix during Stage 2: flat mode must use same `max_w(px(300.0))` constraints as markdown to ensure line breaks and hit-test coordinates align. |
| Click-to-copy precedence | In flat/selectable mode, bubble click-to-copy is suppressed (no `on_mouse_down` for copy). Selection drag takes priority. Click-to-copy returns in markdown mode. |
