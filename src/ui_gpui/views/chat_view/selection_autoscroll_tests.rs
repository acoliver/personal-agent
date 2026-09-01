//! Focused tests for transcript drag auto-scroll ownership.
//!
//! The vendored engine's command-stream behavior (mouse-up/clear publishing
//! `None`, stationary-pointer ticks advancing the endpoint) is covered by
//! `gpui-selection`'s `window_auto_scroll_tests`; these tests pin the
//! `ChatView`-owned scrolling side: step sign and clamping, the
//! scrollbar-point round trip through a real `ListState`, single-loop
//! start/stop semantics, one subscription per view, and the end-to-end
//! scheduled loop scrolling a real `ChatView` transcript list.

use super::selection_autoscroll::{selection_auto_scroll_target, SELECTION_AUTO_SCROLL_TICK_MS};
use super::state::ChatState;
use super::ChatView;
use gpui::{
    div, point, prelude::*, px, relative, size, App, AppContext, Bounds, Element, ElementId,
    Entity, GlobalElementId, HitboxBehavior, InspectorElementId, IntoElement, LayoutId,
    ListAlignment, ListOffset, ListState, Modifiers, MouseButton, Pixels, Point, Style,
    TestAppContext, VisualTestContext, Window,
};
use gpui_selection_vendor::{
    AutoScroll, TextSelectionHandle, TextSelectionLayer, TextSelectionRegistration,
};
use std::sync::Arc;
use std::time::Duration;

/// A window root hosting one `gpui::list`, mirroring how `ChatView` mounts
/// its transcript viewport. The viewport box is fixed so the test does not
/// depend on platform window sizing.
struct ListView {
    list: ListState,
}

impl gpui::Render for ListView {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        // Mirrors how `render_chat_area` mounts the transcript list inside a
        // fixed-size column so the viewport box is deterministic.
        div().h(px(200.0)).w(px(200.0)).flex().flex_col().child(
            gpui::list(self.list.clone(), |index: usize, _, _| {
                div().id(index).h(px(50.0)).w_full().into_any_element()
            })
            .flex_1()
            .min_h_0()
            .w_full(),
        )
    }
}

/// A window root hosting one `gpui::list` with production padding and
/// unequal row heights, mirroring how `ChatView` mounts its transcript
/// viewport for bottom-aligned scroll tests.
struct PaddedListView {
    list: ListState,
}

impl gpui::Render for PaddedListView {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let padding = super::TRANSCRIPT_LIST_PADDING_VERTICAL;
        div().h(px(200.0)).w(px(200.0)).flex().flex_col().child(
            gpui::list(self.list.clone(), |index: usize, _, _| {
                // Unequal row heights: 30, 50, 70, 90, ...
                #[allow(clippy::cast_precision_loss, clippy::suboptimal_flops)]
                let h = 30.0 + 20.0 * index as f32;
                div().id(index).h(px(h)).w_full().into_any_element()
            })
            .flex_1()
            .min_h_0()
            .w_full()
            .p(px(padding)),
        )
    }
}

/// Measured geometry of a mounted bottom-aligned [`PaddedListView`]: the
/// inputs every bottom-aligned target computation derives from.
struct PaddedListGeometry {
    viewport_height: Pixels,
    row_count: usize,
    px_offset: Point<Pixels>,
    padding: Pixels,
    actual_max: Pixels,
}

/// Splices ten rows into a bottom-aligned [`PaddedListView`], draws one
/// frame, and returns the list together with its measured geometry
/// (viewport height, row count, bottom-pinned scrollbar offset, vertical
/// padding, and the true scroll max including padding).
fn mount_padded_bottom_list(cx: &mut TestAppContext) -> (ListState, PaddedListGeometry) {
    let list = ListState::new(0, ListAlignment::Bottom, px(2048.0));
    let (_, window_cx) = cx.add_window_view({
        let list = list.clone();
        move |_, _| PaddedListView { list }
    });
    window_cx.update(|window, cx| {
        list.splice(0..0, 10);
        _ = window.draw(cx);
    });
    let viewport_height = list.viewport_bounds().size.height;
    let row_count = list.item_count();
    let px_offset = list.scroll_px_offset_for_scrollbar();
    let content_height = -px_offset.y;
    let padding = px(2.0 * super::TRANSCRIPT_LIST_PADDING_VERTICAL);
    let actual_max = (content_height + padding - viewport_height).max(px(0.));
    (
        list,
        PaddedListGeometry {
            viewport_height,
            row_count,
            px_offset,
            padding,
            actual_max,
        },
    )
}
#[test]
fn target_step_follows_compute_delta_sign() {
    let viewport = Bounds::new(point(px(0.), px(0.)), size(px(400.), px(400.)));
    let down = AutoScroll::compute_delta(px(390.), viewport)
        .expect("pointer below the bottom trigger scrolls down");
    let up = AutoScroll::compute_delta(px(10.), viewport)
        .expect("pointer above the top trigger scrolls up");
    assert!(down > Pixels::ZERO);
    assert!(up < Pixels::ZERO);

    // The scrollbar point negates scroll-top, so down lowers y and up raises it.
    let current = point(px(0.), px(-100.));
    let down_target =
        selection_auto_scroll_target(current, down, false, px(800.)).expect("steps down");
    let up_target = selection_auto_scroll_target(current, up, false, px(800.)).expect("steps up");
    assert!(down_target.y < current.y);
    assert!(up_target.y > current.y);
    assert_eq!(down_target.x, current.x);
    assert_eq!(up_target.x, current.x);
}

#[test]
fn target_step_clamps_at_top_and_stops_at_edges() {
    let top = point(px(0.), px(0.));
    assert_eq!(
        selection_auto_scroll_target(top, px(-12.), false, px(800.)),
        None,
        "already at the top edge: stop instead of spinning"
    );

    let near_top = point(px(0.), px(-10.));
    let stepped_to_top = selection_auto_scroll_target(near_top, px(-12.), false, px(800.))
        .expect("clamps to the top");
    assert_eq!(stepped_to_top.y, px(0.));
    assert_eq!(
        selection_auto_scroll_target(stepped_to_top, px(-12.), false, px(800.)),
        None
    );
}

#[gpui::test]
fn rendered_top_aligned_list_round_trips_owner_targets(cx: &mut TestAppContext) {
    let list = ListState::new(0, ListAlignment::Top, px(2048.0));
    let (_, window_cx) = cx.add_window_view({
        let list = list.clone();
        move |_, _| ListView { list }
    });

    window_cx.update(|window, cx| {
        list.splice(0..0, 20);
        // An unset logical scroll-top pins a list at its end; start from the
        // top the way `ChatView::scroll_chat_to_top` does.
        list.scroll_to(ListOffset::default());
        _ = window.draw(cx);
    });

    let max_top = list.max_offset_for_scrollbar().height;
    assert_eq!(
        list.viewport_bounds().size.height,
        px(200.),
        "the list viewport tracks its layout box"
    );
    assert_eq!(
        max_top,
        px(800.),
        "20 measured 50px rows minus the 200px viewport"
    );

    let current = list.scroll_px_offset_for_scrollbar();
    assert_eq!(
        current.y,
        px(0.),
        "a top-aligned list starts at scroll-top zero"
    );

    let down = selection_auto_scroll_target(current, px(12.), false, max_top)
        .expect("steps down in the list");
    assert_eq!(down, point(px(0.), px(-12.)));
    list.set_offset_from_scrollbar(down);
    assert_eq!(list.scroll_px_offset_for_scrollbar().y, px(-12.));
    assert_eq!(
        list.logical_scroll_top().item_ix,
        0,
        "a 12px step stays inside the first 50px row"
    );

    let up = selection_auto_scroll_target(point(px(0.), px(-100.)), px(-12.), false, max_top)
        .expect("steps up in the list");
    assert_eq!(up, point(px(0.), px(-88.)));
    list.set_offset_from_scrollbar(up);
    assert_eq!(list.scroll_px_offset_for_scrollbar().y, px(-88.));

    assert_eq!(
        selection_auto_scroll_target(current, px(-12.), false, max_top),
        None,
        "clamped at the list top"
    );
}

#[gpui::test]
fn bottom_aligned_pinned_list_with_padding_steps_up_exactly(cx: &mut TestAppContext) {
    // Production rendering applies `.p(px(12.0))` around the transcript list.
    // The test list mirrors that so viewport_bounds and scrollbar offsets
    // include the same padding.
    let (list, geometry) = mount_padded_bottom_list(cx);
    let PaddedListGeometry {
        row_count,
        px_offset,
        actual_max,
        ..
    } = geometry;

    // Pinned at bottom: logical_scroll_top.item_ix >= row_count.
    let scroll_top = list.logical_scroll_top();
    assert!(
        scroll_top.item_ix >= row_count,
        "a bottom-aligned list with unset logical scroll-top is pinned at the bottom"
    );

    // Upward step (negative delta) from bottom-pinned: target_y = min(0, -actual_max - delta).
    // With delta = -12: target_y = min(0, -actual_max + 12) = -actual_max + 12.
    let up_target = selection_auto_scroll_target(px_offset, px(-12.), true, actual_max)
        .expect("upward step from bottom");
    assert_eq!(
        up_target.y,
        -actual_max + px(12.),
        "upward step moves exactly 12px from the bottom-pinned position"
    );
    assert!(up_target.y < px(0.));

    // Apply and verify it actually moved.
    list.set_offset_from_scrollbar(up_target);
    let after_up = list.scroll_px_offset_for_scrollbar();
    assert_ne!(
        after_up.y, px_offset.y,
        "the list offset changed after an upward step"
    );

    // After stepping up, we're no longer bottom-pinned.
    let scroll_top_after = list.logical_scroll_top();
    assert!(
        scroll_top_after.item_ix < row_count,
        "after stepping up the list is no longer pinned at the bottom"
    );

    // Downward step from non-bottom-pinned: target_y = min(0, current.y - delta).
    // With delta = +12: target_y = min(0, current.y - 12), which lands exactly
    // on the true max; `set_offset_from_scrollbar` then restores bottom
    // alignment (the pinned sentinel offset), the documented clamp path.
    let current_after_up = list.scroll_px_offset_for_scrollbar();
    let down_target = selection_auto_scroll_target(current_after_up, px(12.), false, actual_max)
        .expect("downward step back toward bottom");
    list.set_offset_from_scrollbar(down_target);
    let after_down = list.scroll_px_offset_for_scrollbar();
    assert_eq!(
        after_down.y, px_offset.y,
        "a downward step onto the true max restores the bottom-pinned position"
    );
    assert!(
        list.logical_scroll_top().item_ix >= row_count,
        "the list is bottom-pinned again after the downward step"
    );

    // A further downward step at the bottom must be a round-trip no-op.
    assert_overshoot_at_bottom_is_round_trip_no_op(&list, &geometry);
}

/// Applies a deliberately overshooting downward step at the bottom-pinned
/// position and asserts the round trip is a no-op. The pure target
/// intentionally does not lower-clamp — that is
/// `set_offset_from_scrollbar`'s job — but applying the overshooting
/// target must leave the pinned state untouched so the owner loop's
/// before/after comparison stops it instead of oscillating.
fn assert_overshoot_at_bottom_is_round_trip_no_op(list: &ListState, geometry: &PaddedListGeometry) {
    let at_bottom = list.scroll_px_offset_for_scrollbar();
    let is_bottom_again = list.logical_scroll_top().item_ix >= geometry.row_count;
    let actual_max_again = if is_bottom_again {
        (-at_bottom.y + geometry.padding - geometry.viewport_height).max(px(0.))
    } else {
        list.max_offset_for_scrollbar().height
    };
    let further_down =
        selection_auto_scroll_target(at_bottom, px(50.), is_bottom_again, actual_max_again)
            .expect("the pure target keeps stepping past the bottom max");
    assert!(
        further_down.y < -actual_max_again,
        "the target overshoots the true max instead of wrapping or reflecting"
    );
    list.set_offset_from_scrollbar(further_down);
    assert_eq!(
        list.scroll_px_offset_for_scrollbar().y,
        at_bottom.y,
        "applying the overshooting target is a no-op (no oscillation)"
    );
    assert!(
        list.logical_scroll_top().item_ix >= geometry.row_count,
        "still bottom-pinned after the overshooting step"
    );
}

#[gpui::test]
fn bottom_aligned_top_clamp_does_not_reflect(cx: &mut TestAppContext) {
    let (list, geometry) = mount_padded_bottom_list(cx);
    let PaddedListGeometry {
        px_offset,
        actual_max,
        ..
    } = geometry;
    assert!(
        list.logical_scroll_top().item_ix >= geometry.row_count,
        "starts bottom-pinned"
    );

    // A large upward step beyond the top edge: target_y should clamp at 0
    // (the top), not reflect/abs-wrap to a negative value.
    let huge_up = selection_auto_scroll_target(px_offset, px(-10000.), true, actual_max)
        .expect("clamps to top");
    assert_eq!(
        huge_up.y,
        px(0.),
        "upward step beyond top clamps to zero, not a reflected negative"
    );

    // At the top, further upward is a no-op.
    let at_top = point(px(0.), px(0.));
    assert_eq!(
        selection_auto_scroll_target(at_top, px(-12.), false, actual_max),
        None,
        "already at top: stop"
    );
}

#[gpui::test]
fn repeated_some_commands_keep_one_owner_loop_and_none_stops_it(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut window_cx = cx.add_empty_window().clone();
    let handle = window_cx.update(|window, _| window.window_handle());

    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.apply_selection_auto_scroll_command(Some(px(20.)), handle, cx);
            view.apply_selection_auto_scroll_command(Some(px(20.)), handle, cx);
            assert!(view.selection_auto_scroll.is_active());
        });
    });

    // One 16ms window must run exactly one tick; a duplicated loop would
    // record two.
    cx.executor()
        .advance_clock(Duration::from_millis(SELECTION_AUTO_SCROLL_TICK_MS));
    cx.run_until_parked();
    window_cx.update(|_, app| {
        view.update(app, |view, _cx| {
            assert_eq!(view.selection_auto_scroll_ticks.get(), 1);
            // The empty window has no drag, so the tick stops the loop.
            assert!(!view.selection_auto_scroll.is_active());
        });
    });

    // Some restarts after that stop, and None stops it again before any tick.
    window_cx.update(|_, app| {
        view.update(app, |view, cx| {
            view.apply_selection_auto_scroll_command(Some(px(20.)), handle, cx);
            assert!(view.selection_auto_scroll.is_active());
            view.apply_selection_auto_scroll_command(None, handle, cx);
            assert!(!view.selection_auto_scroll.is_active());
        });
    });
    cx.executor()
        .advance_clock(Duration::from_millis(5 * SELECTION_AUTO_SCROLL_TICK_MS));
    cx.run_until_parked();
    window_cx.update(|_, app| {
        view.update(app, |view, _cx| {
            assert_eq!(
                view.selection_auto_scroll_ticks.get(),
                1,
                "None stops the loop before its next tick"
            );
        });
    });
}

#[gpui::test]
fn chat_view_subscribes_to_auto_scroll_commands_once(cx: &mut TestAppContext) {
    let (view, window_cx) = cx.add_window_view(|_, cx| ChatView::new(ChatState::default(), cx));

    let opened =
        window_cx.update(|_, app| view.read(app).selection_auto_scroll_subscriptions.get());
    assert_eq!(opened, 1, "the window's opening render subscribes once");

    window_cx.update(|window, app| {
        view.update(app, |view, cx| {
            let _ = gpui::Render::render(view, window, cx);
            let _ = gpui::Render::render(view, window, cx);
        });
    });

    let rerendered =
        window_cx.update(|_, app| view.read(app).selection_auto_scroll_subscriptions.get());
    assert_eq!(rerendered, opened, "repeated renders must not resubscribe");
    assert!(window_cx.update(|_, app| view.read(app).selection_auto_scroll_subscription.is_some()));
}

/// Rows mounted by [`ChatSelectionAutoScrollHarness`], matching production's
/// transcript of many short rows.
const HARNESS_ROW_COUNT: usize = 20;

/// Fixed height of each [`SelectionRowElement`].
const HARNESS_ROW_HEIGHT: f32 = 20.0;

/// One transcript row that registers real prepaint-time geometry.
///
/// The hitbox and bounds come from the list's layout, so registrations track
/// the list's actual scroll position the way production selectable rows do.
struct SelectionRowElement {
    selection: TextSelectionHandle,
    document_order: u64,
}

impl IntoElement for SelectionRowElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SelectionRowElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = px(HARNESS_ROW_HEIGHT).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        self.selection.register(
            TextSelectionRegistration::new(hitbox, bounds)
                .with_document_order(self.document_order)
                .with_text_bounds(vec![bounds]),
            window,
            cx,
        );
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        (): &mut Self::PrepaintState,
        _: &mut Window,
        _: &mut App,
    ) {
    }
}

/// Mounts a real `ChatView`'s transcript `ListState` inside a fixed
/// 100x80 overflow-hidden viewport with the production selection layer, so
/// the `ChatView`-owned auto-scroll loop drives a genuinely scrolling list.
struct ChatSelectionAutoScrollHarness {
    chat: Entity<ChatView>,
    rows: Arc<[TextSelectionHandle]>,
}

impl gpui::Render for ChatSelectionAutoScrollHarness {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        // Acquire the command subscription before the first paint, exactly
        // as `ChatView`'s own render does, so no early command is dropped.
        self.chat.update(cx, |view, cx| {
            view.ensure_selection_auto_scroll_subscription(window, cx);
        });
        let list = self.chat.read(cx).transcript_list_state.clone();
        let rows = Arc::clone(&self.rows);
        div()
            .w(px(100.))
            .h(px(80.))
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(
                gpui::list(list, move |index: usize, _, _| {
                    SelectionRowElement {
                        selection: rows[index].clone(),
                        document_order: index as u64,
                    }
                    .into_any_element()
                })
                .flex_1()
                .min_h_0()
                .w_full(),
            )
            // Production mounts the selection layer as the root's final
            // child; rows register during prepaint, before the layer's
            // paint-phase mouse listeners exist.
            .child(TextSelectionLayer)
    }
}

/// Opens the harness window and returns the real `ChatView`, its selection
/// row handles, the `ChatView`'s own transcript `ListState` spliced to
/// [`HARNESS_ROW_COUNT`] rows and scrolled to the top, and the window.
fn setup_actual_chat_view_loop_window(
    cx: &mut TestAppContext,
) -> (
    Entity<ChatView>,
    Arc<[TextSelectionHandle]>,
    ListState,
    VisualTestContext,
) {
    let chat = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let rows: Arc<[TextSelectionHandle]> = cx.update(|cx| {
        (0..HARNESS_ROW_COUNT)
            .map(|_| TextSelectionHandle::new("row", cx))
            .collect()
    });
    let (_, window_cx) = cx.add_window_view({
        let chat = chat.clone();
        let rows = Arc::clone(&rows);
        move |_, _| ChatSelectionAutoScrollHarness { chat, rows }
    });
    let mut window_cx = window_cx.clone();
    window_cx.simulate_resize(size(px(100.), px(80.)));
    let list = window_cx.update(|window, cx| {
        let list = chat.read(cx).transcript_list_state.clone();
        list.splice(0..0, HARNESS_ROW_COUNT);
        // A fresh bottom-aligned list pins at its end; start from the top
        // the way `ChatView::scroll_chat_to_top` does.
        list.scroll_to(ListOffset::default());
        _ = window.draw(cx);
        list
    });
    (chat, rows, list, window_cx)
}

/// Returns whether the window selection's cursor endpoint sits on `rows[ix]`.
fn cursor_is_on_row(rows: &[TextSelectionHandle], ix: usize, cx: &App) -> bool {
    rows[ix]
        .snapshot(cx)
        .and_then(|snapshot| snapshot.cursor().entity_id())
        .is_some_and(|id| id == rows[ix].entity_id())
}

/// Advances the virtual clock by `windows` 16 ms tick windows, parks so the
/// owner loop's timer fires, and redraws so the list mounts the scrolled row
/// geometry the next tick hit-tests against.
fn advance_owner_tick_windows(
    cx: &mut TestAppContext,
    window_cx: &mut VisualTestContext,
    windows: u64,
) {
    cx.executor().advance_clock(Duration::from_millis(
        windows * SELECTION_AUTO_SCROLL_TICK_MS,
    ));
    cx.run_until_parked();
    window_cx.update(|window, cx| {
        _ = window.draw(cx);
    });
}

/// Returns the first row whose measured bounds contain `pointer`, matching
/// how the owner loop hit-tests the stationary pointer against the
/// currently scrolled layout.
fn row_containing_pointer(list: &ListState, pointer: Point<Pixels>) -> Option<usize> {
    (0..HARNESS_ROW_COUNT).find(|&ix| {
        list.bounds_for_item(ix)
            .is_some_and(|b| b.contains(&pointer))
    })
}

/// Handles shared by the drag-gesture helpers below: the real `ChatView`
/// driving the loop, its selection row handles, and the transcript list the
/// loop scrolls.
struct OwnerLoopHarness<'a> {
    chat: &'a Entity<ChatView>,
    rows: &'a [TextSelectionHandle],
    list: &'a ListState,
}

/// Presses the left button on `anchor` and drags-held to `pointer` in the
/// bottom edge zone, asserting the real `ChatView` subscription received
/// the published command and started the owner loop before any tick window
/// elapsed while nothing has scrolled yet; returns the per-tick step delta
/// the held pointer publishes.
fn start_drag_and_assert_owner_loop_started(
    window_cx: &mut VisualTestContext,
    harness: &OwnerLoopHarness<'_>,
    anchor: Point<Pixels>,
    pointer: Point<Pixels>,
    starting_row: usize,
) -> Pixels {
    window_cx.simulate_mouse_down(anchor, MouseButton::Left, Modifiers::default());
    window_cx.simulate_mouse_move(pointer, Some(MouseButton::Left), Modifiers::default());
    window_cx.update(|_, cx| {
        let view = harness.chat.read(cx);
        assert!(view.selection_auto_scroll.is_active());
        assert_eq!(view.selection_auto_scroll_ticks.get(), 0);
        assert_eq!(view.selection_auto_scroll_subscriptions.get(), 1);
        assert!(cursor_is_on_row(harness.rows, starting_row, cx));
    });
    assert_eq!(
        harness.list.scroll_px_offset_for_scrollbar(),
        point(px(0.), px(0.)),
        "nothing has scrolled yet"
    );
    let delta = window_cx
        .update(|_, cx| {
            let viewport = harness
                .chat
                .read(cx)
                .transcript_list_state
                .viewport_bounds();
            AutoScroll::compute_delta(pointer.y, viewport)
        })
        .expect("the held pointer sits inside the bottom edge zone");
    assert!(delta > Pixels::ZERO);
    delta
}

/// Advances exactly one tick window, then asserts the owner loop fired tick
/// number `tick` and scrolled the real list by exactly one `delta` step
/// from `prev_offset_y`; returns the new scrollbar offset.
fn advance_and_assert_owner_tick(
    cx: &mut TestAppContext,
    window_cx: &mut VisualTestContext,
    harness: &OwnerLoopHarness<'_>,
    prev_offset_y: Pixels,
    delta: Pixels,
    tick: usize,
) -> Point<Pixels> {
    advance_owner_tick_windows(cx, window_cx, 1);
    let offset = harness.list.scroll_px_offset_for_scrollbar();
    assert_eq!(
        offset,
        point(px(0.), prev_offset_y - delta),
        "each tick scrolls the real list by exactly one delta step"
    );
    window_cx.update(|_, cx| {
        assert_eq!(
            harness.chat.read(cx).selection_auto_scroll_ticks.get(),
            tick
        );
    });
    offset
}

/// Releases the button at `pointer` and asserts the gesture's final `None`
/// command stopped the owner loop: three further tick windows fire no tick,
/// the scroll position freezes where the gesture ended, and the endpoint
/// stays on `revealed_row`.
fn assert_mouse_up_stops_and_freezes(
    cx: &mut TestAppContext,
    window_cx: &mut VisualTestContext,
    harness: &OwnerLoopHarness<'_>,
    revealed_row: usize,
    pointer: Point<Pixels>,
) {
    window_cx.simulate_mouse_up(pointer, MouseButton::Left, Modifiers::default());
    window_cx.update(|_, cx| {
        assert!(!harness.chat.read(cx).selection_auto_scroll.is_active());
    });
    // The final cursor update happens on mouse-up, so capture afterwards.
    let offset_after_up = harness.list.scroll_px_offset_for_scrollbar();
    assert!(window_cx.update(|_, cx| cursor_is_on_row(harness.rows, revealed_row, cx)));

    advance_owner_tick_windows(cx, window_cx, 3);
    window_cx.update(|_, cx| {
        assert_eq!(
            harness.chat.read(cx).selection_auto_scroll_ticks.get(),
            2,
            "no further ticks after mouse-up"
        );
    });
    assert_eq!(
        harness.list.scroll_px_offset_for_scrollbar(),
        offset_after_up,
        "the scroll position is frozen after mouse-up"
    );
    assert!(window_cx.update(|_, cx| cursor_is_on_row(harness.rows, revealed_row, cx)));
}

#[gpui::test]
fn actual_chat_view_loop_scrolls_twice_advances_endpoint_then_mouse_up_stops(
    cx: &mut TestAppContext,
) {
    let (chat, rows, list, mut window_cx) = setup_actual_chat_view_loop_window(cx);
    let harness = OwnerLoopHarness {
        chat: &chat,
        rows: rows.as_ref(),
        list: &list,
    };

    // Draw first: derive the gesture points from the measured viewport
    // rather than assuming them.
    let viewport = list.viewport_bounds();
    assert_eq!(viewport.size, size(px(100.), px(80.)));
    let anchor = point(
        viewport.left() + viewport.size.width * 0.5,
        viewport.top() + px(5.),
    );
    let pointer = point(
        viewport.left() + viewport.size.width * 0.5,
        viewport.bottom() - px(10.),
    );
    let starting_row =
        row_containing_pointer(&list, pointer).expect("the pointer starts inside a measured row");
    assert_eq!(
        starting_row, 3,
        "20px rows in an 80px viewport put the bottom-edge pointer on row 3"
    );

    let delta = start_drag_and_assert_owner_loop_started(
        &mut window_cx,
        &harness,
        anchor,
        pointer,
        starting_row,
    );

    // Tick 1: the loop advances the endpoint against the previous frame's
    // geometry and then scrolls the real list by exactly one delta step.
    let offset1 = advance_and_assert_owner_tick(cx, &mut window_cx, &harness, px(0.), delta, 1);
    window_cx.update(|_, cx| {
        assert!(
            cursor_is_on_row(harness.rows, starting_row, cx),
            "tick 1 advances the endpoint before scrolling, so it stays on the starting row"
        );
    });

    // The first tick's real scroll moved a new row under the stationary
    // pointer; derive which one from the measured layout.
    let revealed_row = row_containing_pointer(&list, pointer)
        .expect("the stationary pointer sits inside a row of the scrolled list");
    assert!(
        revealed_row > starting_row,
        "the real list scroll, not pointer motion, moved a new row under the stationary pointer"
    );

    // Tick 2: the endpoint advances onto the revealed row while the pointer
    // stays stationary, and the list scrolls a second delta step.
    let _offset2 = advance_and_assert_owner_tick(cx, &mut window_cx, &harness, offset1.y, delta, 2);
    window_cx.update(|_, cx| {
        assert!(
            cursor_is_on_row(harness.rows, revealed_row, cx),
            "tick 2 advances the endpoint onto the row the first tick revealed"
        );
    });

    // Mouse up ends the gesture; its `None` command must stop the owner
    // loop, freeze the scroll position, and leave the endpoint where the
    // gesture ended.
    assert_mouse_up_stops_and_freezes(cx, &mut window_cx, &harness, revealed_row, pointer);
}
