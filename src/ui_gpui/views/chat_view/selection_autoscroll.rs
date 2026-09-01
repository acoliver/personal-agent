//! Transcript drag auto-scroll ownership.
//!
//! The vendored selection engine publishes window-scoped auto-scroll commands
//! (`Some(delta)` while the pointer sits in a viewport edge zone during a
//! drag, `None` when the gesture ends). `ChatView` owns the scrolling side of
//! that contract: one retained command subscription plus exactly one 16 ms
//! repeat loop that advances the selection endpoint into rows the previous
//! tick revealed and then scrolls the transcript list.

use super::ChatView;
use super::TRANSCRIPT_LIST_PADDING_VERTICAL;
use gpui::{point, px, AnyWindowHandle, Context, Pixels, Point, Window};
use gpui_selection_vendor::{AutoScroll, TextSelection};

/// Auto-scroll interval shared with the vendored engine's drag loop.
#[cfg(test)]
pub(super) const SELECTION_AUTO_SCROLL_TICK_MS: u64 = 16;

/// Computes the scrollbar target for one auto-scroll step on a bottom-aligned
/// list.
///
/// `ListState` reports the scroll position as a scrollbar point whose y axis
/// is the negated scroll-top (see `ListState::scroll_px_offset_for_scrollbar`
/// and `ListState::set_offset_from_scrollbar`). A positive `delta` (pointer
/// near the bottom edge) moves y further negative (scrolling down), and a
/// negative delta (pointer near the top edge) moves y toward zero (scrolling
/// up).
///
/// For a non-bottom-pinned list, `target_y = min(0, current.y - delta)` — no
/// abs-wrapping across top crossings. A bottom-pinned list
/// (`logical_scroll_top.item_ix >= row_count`) reports its scrollbar y as
/// `-content_height`, an out-of-range sentinel; it is remapped to
/// `-actual_max`, the true scroll max including padding:
/// `max(0, content_height + top_padding + bottom_padding - viewport_height)`.
/// This corrects the original clamp design which assumed `current.y` was in
/// `[-max_offset, 0]` while `max_offset_for_scrollbar` omits padding.
///
/// `set_offset_from_scrollbar` internally clamps and restores bottom
/// alignment, so we only need to detect actual no-ops (no state change before
/// and after) to stop the loop.
///
/// Returns `None` when the target equals the current y, indicating the list
/// is pinned at an edge.
pub(super) fn selection_auto_scroll_target(
    current: Point<Pixels>,
    delta: Pixels,
    is_bottom_pinned: bool,
    actual_max: Pixels,
) -> Option<Point<Pixels>> {
    let effective_y = if is_bottom_pinned {
        -actual_max
    } else {
        current.y
    };
    let target_y = (effective_y - delta).min(Pixels::ZERO);
    (target_y != current.y).then(|| point(current.x, target_y))
}

impl ChatView {
    /// Subscribes to window-scoped drag auto-scroll commands, once.
    ///
    /// Called from the render lifecycle so the command source is acquired
    /// before the selection layer's first paint. The subscription is retained
    /// for the `ChatView`'s lifetime; dropping the view drops it.
    pub(super) fn ensure_selection_auto_scroll_subscription(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selection_auto_scroll_subscription.is_some() {
            return;
        }
        #[cfg(test)]
        self.selection_auto_scroll_subscriptions
            .set(self.selection_auto_scroll_subscriptions.get() + 1);

        let view = cx.weak_entity();
        let window_handle = window.window_handle();
        self.selection_auto_scroll_subscription = Some(TextSelection::subscribe_auto_scroll(
            window,
            cx,
            move |delta, cx| {
                let Some(view) = view.upgrade() else {
                    return;
                };
                view.update(cx, |view, cx| {
                    view.apply_selection_auto_scroll_command(delta, window_handle, cx);
                });
            },
        ));
    }

    /// Applies one engine auto-scroll command.
    ///
    /// `Some(delta)` starts or retargets the single owner loop; `None` stops
    /// it immediately. The vendored [`AutoScroll`] state keeps the loop at one
    /// task regardless of how many `Some` commands arrive between ticks.
    pub(super) fn apply_selection_auto_scroll_command(
        &mut self,
        delta: Option<Pixels>,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        self.selection_auto_scroll
            .set(delta, cx, move |command_delta, this, cx| {
                this.tick_selection_auto_scroll(command_delta, window, cx);
            });
    }

    /// Stops the owner loop, e.g. when the drag ended or the window is gone.
    fn stop_selection_auto_scroll(&mut self) {
        self.selection_auto_scroll.stop();
    }

    /// Runs one owner-loop tick.
    ///
    /// The command delta is recomputed from the live pointer and viewport each
    /// tick (the published command may be stale after participant churn), so
    /// the incoming command value is only used as the loop's pacing value.
    fn tick_selection_auto_scroll(
        &mut self,
        _command_delta: Pixels,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        #[cfg(test)]
        self.selection_auto_scroll_ticks
            .set(self.selection_auto_scroll_ticks.get() + 1);

        // First advance the drag against the geometry mounted by the previous
        // frame, so the selection head crosses into rows the last tick
        // revealed even while the pointer is stationary.
        let Ok((drag_active, pointer_y)) = window.update(cx, |_, window, cx| {
            let active = TextSelection::update_drag_at_pointer(window, cx);
            (active, window.mouse_position().y)
        }) else {
            // The window closed under us; nothing left to scroll.
            self.stop_selection_auto_scroll();
            return;
        };
        if !drag_active {
            self.stop_selection_auto_scroll();
            return;
        }

        let viewport = self.transcript_list_state.viewport_bounds();
        let Some(delta) = AutoScroll::compute_delta(pointer_y, viewport) else {
            self.stop_selection_auto_scroll();
            return;
        };

        // Capture logical/scroll state before applying the step so we can
        // detect actual no-ops (the loop must stop, not spin).
        let scroll_before = self.transcript_list_state.logical_scroll_top();
        let px_before = self.transcript_list_state.scroll_px_offset_for_scrollbar();

        let row_count = self.transcript_list_state.item_count();
        let is_bottom_pinned = scroll_before.item_ix >= row_count;

        // For bottom-pinned state, derive the actual scroll max from measured
        // content height, viewport bounds, and the centralized transcript
        // vertical padding. `max_offset_for_scrollbar` omits padding, so we
        // compute it ourselves.
        let actual_max = if is_bottom_pinned {
            let content_height = -self
                .transcript_list_state
                .scroll_px_offset_for_scrollbar()
                .y;
            let viewport_height = viewport.size.height;
            let padding = px(2.0 * TRANSCRIPT_LIST_PADDING_VERTICAL);
            (content_height + padding - viewport_height).max(px(0.))
        } else {
            // For non-bottom-pinned, the scrollbar max from ListState is
            // sufficient because we're not at the bottom edge.
            self.transcript_list_state.max_offset_for_scrollbar().height
        };

        let Some(target) =
            selection_auto_scroll_target(px_before, delta, is_bottom_pinned, actual_max)
        else {
            // Target equals current: the list is at an edge. Stop the loop
            // instead of spinning against the boundary.
            self.stop_selection_auto_scroll();
            return;
        };

        self.transcript_list_state.set_offset_from_scrollbar(target);

        // Compare logical/scroll state before and after. If set_offset did a
        // no-op (e.g. internal clamp kept us at the same position), stop the
        // loop — there's nothing left to scroll.
        let scroll_after = self.transcript_list_state.logical_scroll_top();
        let px_after = self.transcript_list_state.scroll_px_offset_for_scrollbar();
        if scroll_before.item_ix == scroll_after.item_ix
            && scroll_before.offset_in_item == scroll_after.offset_in_item
            && px_before == px_after
        {
            self.stop_selection_auto_scroll();
            return;
        }

        self.refresh_autoscroll_state_from_list();
        cx.notify();
    }
}
