# Migrating the PersonalAgent renderer onto the selection engine

Date: 2026-08-31
Branch: `issue151-notepad-prototype`
Issue: https://github.com/acoliver/personal-agent/issues/151

The `notepad_selection` harness proves the mechanism: press-and-drag selects
continuously across message boundaries, the highlight tracks the pointer live,
rich rendering is unchanged while selecting, and Cmd+C copies the visible text.
This document is how that reaches the shipping app.

## 1. How much is already done

More than it looks. The leaf factory landed in the real renderer, not in a fork
of it.

`src/ui_gpui/components/markdown_content.rs` now exposes three entry points:

| Function | Line | Used by |
|---|---|---|
| `blocks_to_elements` | 359 | `AssistantBubble` (message_bubble.rs:209) |
| `blocks_to_elements_with_color` | 368 | `render_user_message` (render.rs:400) |
| `blocks_to_elements_with_leaf_factory` | 391 | the notepad harness only |

The first two are the shipping paths and their output is unchanged; the full
library suite passes at 1,104 tests. The third is the same renderer with every
visible text leaf handed to a `MarkdownLeafFactory` instead of being turned
directly into a `StyledText`.

So there is exactly one renderer, already, with a seam in it. The app is not
using the seam yet.

The vendored engine lives at `vendor/gpui-selection/` as a path dependency
outside `src/`, which keeps house-style gates off third-party code without any
CI workflow edit. `cargo tree -i gpui` shows a single GPUI.

## 2. The actual migration, in the app

Two call sites move:

- `message_bubble.rs:209`, `AssistantBubble` → `blocks_to_elements_with_leaf_factory`
- `render.rs:400`, `render_user_message` → the same

Plus, per window that shows a transcript:

- call `GlobalState::init(cx)` once at startup
- render one `TextSelectionLayer` as the LAST child of the window root, so it
  paints above content
- thread a single `document_order` counter through the transcript in reading
  order, so a drag from message 2 into message 7 orders correctly
- pass `chat_scroll_handle.offset()` into each registration

That is the whole mechanical change. Everything below is the part that actually
takes the work.

## 3. Blockers. None of these are optional

### 3.1 Links are dead in the selectable path

The app shipped clickable links in PR #160 (issue #153). In the selectable path
the renderer returns `factory.create_leaf(...)` before it ever constructs
`InteractiveText::on_click`, so link activation is discarded. The audit
confirmed this: clicking a link in the harness does nothing, while a drag that
starts on a link correctly selects.

Regressing clickable links to gain selection is not a trade worth making. The
fix is the standard one and both prior attempts specified it: a press and
release on the same link, with movement under a small threshold, activates the
URL; movement beyond the threshold starts a selection and cancels activation.

Blocking.

### 3.2 Whole-bubble click-to-copy fights every drag

`message_bubble.rs:227` and `render.rs:417` install `on_mouse_down` handlers
that copy the entire message when you press anywhere in the bubble. A press is
also how a selection starts. These cannot coexist.

This is a user-visible behaviour change and needs an explicit decision:

- remove it, and let selection plus Cmd+C be the way to copy
- or move it to an explicit control, a hover copy button on the bubble

The top bar copy-conversation button (`render_bars.rs:61`) is unaffected either
way and should stay.

Blocking, and it needs a decision rather than an implementation.

### 3.3 Autoscroll does not work

Confirmed broken in both directions by the audit. Press inside a message, drag
past the bottom edge, hold: the transcript does not scroll and the selection
stops extending. A six-second hold produced byte-identical clipboard output to
an immediate release.

The cause is known. When the vendored engine was ported, upstream's
`Window::dispatch_event` could not be called at our pinned GPUI rev because its
return type is `pub(crate)`, so auto-scroll was re-plumbed through the
participant callback. Nothing then consumes it: the engine emits
`TextSelectionEvent::AutoScroll(delta)` through the anchor participant, but
`SelectableText` only retains the `refresh_window_on_change` subscription, which
reacts to `SelectionChanged` and ignores `AutoScroll`.

The fix is to subscribe a scroll owner that applies the delta to the transcript
`ScrollHandle` on a frame timer while the pointer is held outside the viewport.

In the harness this is a nuisance. In the real app, where transcripts are long,
selecting more than one screen of text is the common case. Blocking.

### 3.4 Cmd+C precedence

`handle_platform_key` (render.rs:159), `"c"` branch at line 211, copies the
sidebar search query, the conversation title input, or the composer text. It
never touches the transcript. Transcript selection has to take precedence when
one exists, falling through to today's behaviour when it does not.

Small, but easy to get wrong, and it is the single choke point.

### 3.5 Selection invalidation

The harness is static. The app is not. A selection has to be dropped or held
correctly across:

- streaming tokens appended to the last message
- a new message arriving mid-drag
- conversation switch (`snapshot.rs:159` replaces `state.messages` wholesale)
- message deletion, which shifts every index after it
- the emoji filter toggling, which rewrites displayed content

PR #209 solved this with a per-message `MessageRevision` freshness token. That
approach is sound and should be reused. Alacritty's answer for the analogous
scrollback case is simply to clear the selection on mutation, which is a
legitimate fallback for the streaming case.

## 4. The performance question, which is what killed PR #209

The transcript is not virtualized. `render_chat_area` (render.rs:258) is a plain
`overflow_y_scroll` div with one child per message, and every message is laid
out and painted every frame. Adding a selection participant per text leaf
multiplies per-frame work by transcript length.

This is exactly what stalled the last attempt: popup open time regressed to
1.9-3.2 seconds and had to be clawed back to about 0.5 with Arc caches.

Two things follow.

First, measure before integrating, not after. Popup open time and
press-to-visible-highlight latency, as numbers, on a long conversation.

Second, virtualize the transcript with `gpui::list` and `ListState`, bottom
aligned. Zed's own agent panel does exactly this,
`ListState::new(0, ListAlignment::Bottom, px(2048.0))`. Virtualization does not
conflict with cross-message selection, because the selection model is logical
and geometry-free; an off-screen message simply is not painted until you scroll
to it. The engine anticipates this: `copy_with` is documented as the hook for
including virtualized content that is not currently painted, and
`TextSelectionSnapshot::coverage()` distinguishes a participant selected in
part, from its start, to its end, or in full.

Virtualization should land as its own change, independently verifiable, before
selection goes near the app.

## 5. Two windows

`TextSelectionLayer` is per-window. The app has both the menu-bar popup and the
popout window. Each needs its own layer, and selection state is per-window, so
selecting in one must not disturb the other. Worth an explicit test, given the
popup focus regressions that plagued PR #209.

## 6. Known gaps that are not blockers

From the audit, with evidence in `artifacts/issue151/audit/`:

| Behaviour | State |
|---|---|
| double-click word | works, on ASCII and dotted tokens; word classification is hand-written rather than full Unicode word-break |
| triple-click | works, selects the logical leaf line |
| shift-click extend | works |
| blank-only drag rejection | works, via `with_text_bounds` |
| single click clears selection | works |
| drag-extend after double-click | character-granular, should be word-granular |
| Cmd+A | absent |
| Escape to clear | absent |

These are polish. They can land after the feature is in the app.

## 7. Sequencing

Deliberately small, because both prior attempts died partly of size. PR #209 was
6,670 lines and absorbed unrelated tray, popup-focus and composer work, which
the pre-merge check flagged as out of scope.

1. **Virtualize the transcript.** `gpui::list` + `ListState`, bottom aligned.
   No selection. Measure popup open time and scroll smoothness before and after.
   Independently valuable.
2. **Retire whole-bubble click-to-copy.** Behaviour change, needs the decision in
   3.2. Tiny diff, obvious to review, ships on its own.
3. **Land the vendored engine and the leaf factory wiring.** Selection enabled in
   the app, links restored, Cmd+C precedence, invalidation. The bulk of the work.
4. **Autoscroll.** Scroll-owner subscription applying `AutoScroll` deltas.
5. **Polish.** Cmd+A, Escape, word-granular drag extension, Unicode word breaks.

## 8. Verification, given how this has failed before

CI being green was not sufficient for PR #209, and it will not be sufficient
here. Three of the four defects found in this prototype were colour and paint
bugs that no unit test would have caught, and every one of them was found by
eye:

- selected glyphs invisible because a second `StyledText` shared one element id
- highlight not appearing until mouse-up because a redraw subscription was never retained
- inline-code run backgrounds repainting over the selection quad, black on black

So the gate has to include pixels, not just clipboard contents. The harness
already demonstrates the technique: drive real input with `cliclick`, capture
mid-drag with `screencapture`, and assert programmatically that no selected
region has glyph and quad colours that collapse together. The last check
measured eleven regions with a minimum contrast of 6.39:1.

Every stage above needs, at minimum:

- popup open time and press-to-highlight latency as numbers, before and after
- a mid-drag pixel-contrast assertion across every markdown block type
- selection colours checked in every shipped theme, Green Screen first
- popup focus behaviour on Linux

## 9. Open decisions

1. Whole-bubble click-to-copy: remove it, or move it to an explicit control?
2. Does virtualization land first, as argued in section 4, or does selection go
   in on the current unvirtualized transcript and accept the latency?
3. Cmd+A in the transcript: select the whole conversation? And how does that
   interact with the composer's existing select-all, which today is composer-only
   and does not even paint a selection?
4. Does a new message arriving mid-drag invalidate the selection, or extend it?
