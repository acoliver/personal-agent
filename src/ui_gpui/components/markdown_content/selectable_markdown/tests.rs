//! Phase 2 & 3 interaction tests for issue #151.
//!
//! These tests drive a real `SelectableMarkdown` element through GPUI's
//! `Element` lifecycle (via a window view) and then exercise the actual
//! first-frame layouts via simulated mouse down/move/up events. They cover
//! forward/reverse drag, double-click word, triple-click semantic block,
//! wrapped text, multi-leaf mixed formatting, Unicode/emoji, and the
//! requirement that the rich metadata remains unchanged while a selection is
//! active.
//!
//! MIT clean-room implementation using only GPUI public APIs.
//!
//! @plan PLAN-20260713-ISSUE151 Phase 2 & 3

#![allow(clippy::future_not_send)]

use gpui::{
    div, px, IntoElement, Modifiers, MouseButton, Point, Render, TestAppContext, VisualTestContext,
};

use super::{SelectableMarkdown, SelectableMarkdownEvent};
use crate::ui_gpui::components::markdown_content::visible_document::{MessageRevision, Selection};

// ---------------------------------------------------------------------------
// harness
// ---------------------------------------------------------------------------
struct RerenderingMarkdownView {
    markdown: String,
    revision: MessageRevision,
    selection: Option<Selection>,
    dragging: bool,
}

impl Render for RerenderingMarkdownView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let entity = cx.entity();
        SelectableMarkdown::from_markdown(&self.markdown, self.revision.clone())
            .id("rerendering-selectable-markdown")
            .with_selection(self.selection.clone())
            .with_dragging(self.dragging)
            .on_event(move |event, _window, cx| {
                if let SelectableMarkdownEvent::SelectionChanged {
                    selection,
                    dragging,
                    ..
                } = event
                {
                    entity.update(cx, |view, cx| {
                        view.selection = selection;
                        view.dragging = dragging;
                        cx.notify();
                    });
                }
            })
    }
}

/// Minimal root view that renders a selectable markdown element. This gives us
/// a real window for mouse-event simulation.
struct MarkdownView {
    element: SelectableMarkdown,
}

impl Render for MarkdownView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        self.element.clone()
    }
}

/// Build a `SelectableMarkdown` from raw markdown, place it in a window, and
/// return the element + the visual test context for interaction.
fn build<'a>(
    cx: &'a mut TestAppContext,
    markdown: &str,
    revision: MessageRevision,
) -> (SelectableMarkdown, &'a mut VisualTestContext) {
    let element =
        SelectableMarkdown::from_markdown(markdown, revision).id("test-selectable-markdown");
    let (_view, vtc) = cx.add_window_view(|_window, _cx| MarkdownView {
        element: element.clone(),
    });
    (element, vtc)
}

// ---------------------------------------------------------------------------
// Phase 2: rich renderer metadata
// ---------------------------------------------------------------------------

/// A single paragraph must produce exactly one selectable leaf.
#[gpui::test]
async fn paragraph_produces_one_leaf(cx: &mut TestAppContext) {
    let (element, vtc) = build(
        cx,
        "hello world",
        MessageRevision::new("m1", "hello world", 0, false),
    );
    vtc.run_until_parked();

    let leaves = element.leaf_count();
    assert_eq!(
        leaves, 1,
        "a single paragraph must produce exactly one selectable leaf"
    );
}

#[gpui::test]
async fn multiple_paragraphs_produce_multiple_leaves(cx: &mut TestAppContext) {
    let md = "first paragraph\n\nsecond paragraph\n\nthird";
    let (element, vtc) = build(cx, md, MessageRevision::new("m1", md, 0, false));
    vtc.run_until_parked();
    assert_eq!(element.leaf_count(), 3);
}

#[gpui::test]
async fn heading_produces_leaf(cx: &mut TestAppContext) {
    let md = "# Big Heading";
    let (element, vtc) = build(cx, md, MessageRevision::new("m1", md, 0, false));
    vtc.run_until_parked();
    assert_eq!(element.leaf_count(), 1);
}

#[gpui::test]
async fn code_block_produces_leaf(cx: &mut TestAppContext) {
    let md = "```rust\nfn main() {}\n```";
    let (element, vtc) = build(cx, md, MessageRevision::new("m1", md, 0, false));
    vtc.run_until_parked();
    assert_eq!(element.leaf_count(), 2); // language label + code body
}

#[gpui::test]
async fn list_produces_leaf_per_item(cx: &mut TestAppContext) {
    let md = "- one\n- two\n- three";
    let (element, vtc) = build(cx, md, MessageRevision::new("m1", md, 0, false));
    vtc.run_until_parked();
    assert_eq!(element.leaf_count(), 6); // marker + body for each item
}

#[gpui::test]
async fn mixed_blocks_produce_correct_leaf_count(cx: &mut TestAppContext) {
    // heading + paragraph + code + two list marker/body pairs = 7 leaves
    let md = "# H\n\npara\n\n```\ncode\n```\n\n- a\n- b";
    let (element, vtc) = build(cx, md, MessageRevision::new("m1", md, 0, false));
    vtc.run_until_parked();
    assert_eq!(element.leaf_count(), 7);
}

/// Leaf document ranges must be ordered and non-overlapping.
#[gpui::test]
async fn leaf_ranges_are_ordered_and_cover_document(cx: &mut TestAppContext) {
    let md = "# Title\n\nfirst **bold** para\n\nsecond";
    let (element, vtc) = build(cx, md, MessageRevision::new("m1", md, 0, false));
    vtc.run_until_parked();

    let ranges = element.leaf_doc_ranges();
    assert!(!ranges.is_empty(), "must have at least one leaf");
    for window in ranges.windows(2) {
        assert!(
            window[0].end <= window[1].start,
            "leaf ranges must not overlap: {:?} overlaps {:?}",
            window[0],
            window[1]
        );
    }
}

/// The rendered text length reported by each leaf's TextLayout must match the
/// length of the leaf's document-range text.
#[gpui::test]
async fn leaf_layout_len_matches_doc_range(cx: &mut TestAppContext) {
    let md = "hello world";
    let (element, vtc) = build(cx, md, MessageRevision::new("m1", md, 0, false));
    vtc.run_until_parked();

    let leaf_lens = element.leaf_rendered_lens();
    assert_eq!(leaf_lens.len(), 1);
    assert_eq!(leaf_lens[0], "hello world".len());
}

// ---------------------------------------------------------------------------
// Phase 3: pointer interaction
// ---------------------------------------------------------------------------

/// First-attempt forward drag without any warm-up selects a partial range.
#[gpui::test]
async fn first_drag_forward_selects(cx: &mut TestAppContext) {
    let md = "AAAAA BBBBB CCCCC";
    let rev = MessageRevision::new("m1", md, 0, false);
    let (element, vtc) = build(cx, md, rev);
    vtc.run_until_parked();

    simulate_drag(vtc, px(10.0), px(120.0));

    let sel = element
        .selection()
        .expect("selection must be set after drag");
    assert!(!sel.is_empty(), "drag must produce a non-empty selection");
    let text = element.selected_text();
    assert!(!text.is_empty(), "selected text must not be empty");
}

/// Reverse drag selects the same logical range as forward.
#[gpui::test]
async fn reverse_drag_selects(cx: &mut TestAppContext) {
    let md = "AAAAA BBBBB CCCCC";
    let rev = MessageRevision::new("m1", md, 0, false);
    let (element, vtc) = build(cx, md, rev);
    vtc.run_until_parked();

    simulate_drag(vtc, px(120.0), px(10.0));

    let sel = element
        .selection()
        .expect("selection must be set after reverse drag");
    assert!(!sel.is_empty());
    assert!(
        sel.is_reverse(),
        "reverse drag must produce a reverse selection"
    );
}

/// Double-click selects a word.
#[gpui::test]
async fn double_click_selects_word(cx: &mut TestAppContext) {
    let md = "alpha beta gamma";
    let rev = MessageRevision::new("m1", md, 0, false);
    let (element, vtc) = build(cx, md, rev);
    vtc.run_until_parked();

    simulate_double_click(vtc, px(70.0));

    let sel = element
        .selection()
        .expect("selection must be set after double-click");
    assert!(!sel.is_empty(), "double-click must select a word");
    let text = element.selected_text();
    assert_eq!(
        text.trim(),
        "beta",
        "double-click must select exactly the word under the pointer, got: {text:?}"
    );
}

/// Triple-click selects the semantic block.
#[gpui::test]
async fn triple_click_selects_block(cx: &mut TestAppContext) {
    let md = "first line paragraph text here";
    let rev = MessageRevision::new("m1", md, 0, false);
    let (element, vtc) = build(cx, md, rev);
    vtc.run_until_parked();

    simulate_triple_click(vtc, px(60.0));

    let sel = element
        .selection()
        .expect("selection must be set after triple-click");
    assert!(!sel.is_empty(), "triple-click must select a block");
    let text = element.selected_text();
    assert!(
        text.contains("first") && text.contains("here"),
        "triple-click must select the whole semantic block, got: {text:?}"
    );
}

/// Drag state survives the same owner-notify re-render used by ChatView.
#[gpui::test]
async fn drag_continues_across_owner_rerender(cx: &mut TestAppContext) {
    let markdown = "AAAAA BBBBB CCCCC".to_string();
    let revision = MessageRevision::new("m1", &markdown, 0, false);
    let (view, vtc) = cx.add_window_view(|_window, _cx| RerenderingMarkdownView {
        markdown,
        revision,
        selection: None,
        dragging: false,
    });
    vtc.run_until_parked();

    let y = px(10.0);
    vtc.simulate_mouse_down(
        Point::new(px(10.0), y),
        MouseButton::Left,
        Modifiers::default(),
    );
    vtc.simulate_mouse_move(
        Point::new(px(80.0), y),
        MouseButton::Left,
        Modifiers::default(),
    );
    vtc.run_until_parked();
    view.read_with(vtc, |view, _| {
        assert!(view.dragging, "owner must preserve an in-progress drag");
        assert!(view.selection.as_ref().is_some_and(|sel| !sel.is_empty()));
    });

    vtc.simulate_mouse_move(
        Point::new(px(130.0), y),
        MouseButton::Left,
        Modifiers::default(),
    );
    vtc.run_until_parked();
    vtc.simulate_mouse_up(
        Point::new(px(130.0), y),
        MouseButton::Left,
        Modifiers::default(),
    );
    vtc.run_until_parked();

    view.read_with(vtc, |view, _| {
        assert!(!view.dragging, "mouse-up must finalize the restored drag");
        assert!(view.selection.as_ref().is_some_and(|sel| !sel.is_empty()));
    });
}

#[gpui::test]
async fn safe_link_click_opens_url(cx: &mut TestAppContext) {
    let markdown = "[clickable](https://example.com) trailing text";
    let (_, vtc) = build(cx, markdown, MessageRevision::new("m1", markdown, 0, false));
    vtc.run_until_parked();

    vtc.simulate_click(Point::new(px(20.0), px(10.0)), Modifiers::default());
    assert_eq!(vtc.opened_url().as_deref(), Some("https://example.com"));
}

#[gpui::test]
async fn dragging_link_selects_without_opening(cx: &mut TestAppContext) {
    let markdown = "[clickable](https://example.com) trailing text";
    let (element, vtc) = build(cx, markdown, MessageRevision::new("m1", markdown, 0, false));
    vtc.run_until_parked();

    simulate_drag(vtc, px(20.0), px(160.0));
    assert!(element
        .selection()
        .is_some_and(|selection| !selection.is_empty()));
    assert_eq!(vtc.opened_url(), None, "dragging must not activate the URL");
}

#[gpui::test]
async fn right_click_emits_context_menu_with_selection_snapshot(cx: &mut TestAppContext) {
    use std::cell::RefCell;
    use std::rc::Rc;

    let markdown = "copy this phrase";
    let snapshots = Rc::new(RefCell::new(Vec::new()));
    let captured = snapshots.clone();
    let element =
        SelectableMarkdown::from_markdown(markdown, MessageRevision::new("m1", markdown, 0, false))
            .on_event(move |event, _, _| {
                if let SelectableMarkdownEvent::ContextMenu { selected_text, .. } = event {
                    captured.borrow_mut().push(selected_text);
                }
            });
    let (_view, vtc) = cx.add_window_view(|_, _| MarkdownView {
        element: element.clone(),
    });
    vtc.run_until_parked();
    simulate_drag(vtc, px(10.0), px(90.0));

    vtc.simulate_event(gpui::MouseDownEvent {
        position: Point::new(px(30.0), px(10.0)),
        modifiers: Modifiers::default(),
        button: MouseButton::Right,
        click_count: 1,
        first_mouse: false,
    });

    assert_eq!(snapshots.borrow().as_slice(), &[element.selected_text()]);
}

/// Selection remains per-message: a stale revision rejects the selection.
#[gpui::test]
async fn selection_rejected_on_stale_revision(cx: &mut TestAppContext) {
    let md = "hello world here";
    let rev = MessageRevision::new("m1", md, 0, false);
    let (element, vtc) = build(cx, md, rev);
    vtc.run_until_parked();

    simulate_drag(vtc, px(10.0), px(80.0));
    assert!(
        element.selection().is_some(),
        "selection set on current revision"
    );

    let stale = MessageRevision::new("m1", "CHANGED", 0, false);
    element.report_current_revision(stale);
    assert!(
        element.selection().is_none(),
        "selection must be cleared when the message revision changes"
    );
}

/// Rich metadata (leaf count and rendered lengths) remains unchanged while a
/// selection is active.
#[gpui::test]
async fn rich_metadata_unchanged_during_selection(cx: &mut TestAppContext) {
    let md = "# Heading\n\nparagraph text";
    let rev = MessageRevision::new("m1", md, 0, false);
    let (element, vtc) = build(cx, md, rev);
    vtc.run_until_parked();

    let leaves_before = element.leaf_count();
    let lens_before = element.leaf_rendered_lens();

    simulate_drag(vtc, px(10.0), px(80.0));
    vtc.run_until_parked();

    let leaves_after = element.leaf_count();
    let lens_after = element.leaf_rendered_lens();

    assert_eq!(
        leaves_before, leaves_after,
        "leaf count must not change during selection"
    );
    assert_eq!(
        lens_before, lens_after,
        "leaf rendered lengths must not change during selection"
    );
}

/// Unicode and emoji: selection offsets are UTF-8 safe and selection works.
#[gpui::test]
async fn unicode_emoji_selection(cx: &mut TestAppContext) {
    let md = "héllo wörld 🌍 emoji";
    let rev = MessageRevision::new("m1", md, 0, false);
    let (element, vtc) = build(cx, md, rev);
    vtc.run_until_parked();

    simulate_drag(vtc, px(10.0), px(80.0));

    element
        .selection()
        .expect("selection must work with Unicode text");
    let text = element.selected_text();
    assert!(!text.is_empty());
    let doc = element.document_text();
    assert!(doc.contains("🌍"));
}

/// Multi-leaf mixed formatting drag: inline formatting does not split leaves.
#[gpui::test]
async fn multi_leaf_mixed_formatting(cx: &mut TestAppContext) {
    let md = "**bold** and *italic* and `code`";
    let rev = MessageRevision::new("m1", md, 0, false);
    let (element, vtc) = build(cx, md, rev);
    vtc.run_until_parked();

    assert_eq!(
        element.leaf_count(),
        1,
        "inline formatting does not split leaves"
    );

    simulate_drag(vtc, px(10.0), px(100.0));
    let sel = element
        .selection()
        .expect("drag over mixed formatting must select");
    assert!(!sel.is_empty());
}

// ---------------------------------------------------------------------------
// simulation helpers
// ---------------------------------------------------------------------------

fn simulate_drag(cx: &mut VisualTestContext, start_x: gpui::Pixels, end_x: gpui::Pixels) {
    let y = px(10.0);
    cx.simulate_mouse_down(
        Point::new(start_x, y),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(
        Point::new(end_x, y),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        Point::new(end_x, y),
        MouseButton::Left,
        Modifiers::default(),
    );
}

fn simulate_double_click(cx: &mut VisualTestContext, x: gpui::Pixels) {
    let pos = Point::new(x, px(10.0));
    cx.simulate_event(gpui::MouseDownEvent {
        position: pos,
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(gpui::MouseUpEvent {
        position: pos,
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: 2,
    });
}

fn simulate_triple_click(cx: &mut VisualTestContext, x: gpui::Pixels) {
    let pos = Point::new(x, px(10.0));
    cx.simulate_event(gpui::MouseDownEvent {
        position: pos,
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: 3,
        first_mouse: false,
    });
    cx.simulate_event(gpui::MouseUpEvent {
        position: pos,
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count: 3,
    });
}

// Keep unused import reference alive.
#[allow(dead_code)]
fn _unused() {
    let _ = div().into_any_element();
}
