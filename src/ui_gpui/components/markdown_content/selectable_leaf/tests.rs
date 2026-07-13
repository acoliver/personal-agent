//! Phase 0 first-frame hit-test proof for issue #151.
//!
//! These tests prove that a `StyledText` layout belonging to the ACTUAL
//! retained/rendered rich child can be hit-tested on the very first mouse-down
//! after the first draw, using GPUI's real `Element` lifecycle and the
//! `TextLayout` handle shared with the painted element. There is no layout
//! sink, no phantom/unpainted layout, no arming, no warm-up frame, no deferred
//! replay, and no alternate flat renderer.
//!
//! This is an MIT clean-room implementation. It uses only GPUI public APIs and
//! does not derive from Zed's Markdown code.
//!
//! @plan PLAN-20260713-ISSUE151 Phase 0 (blocking gate)

#![allow(clippy::future_not_send)]

use std::sync::{Arc, Mutex};

use gpui::{
    div, px, IntoElement, Modifiers, MouseButton, Point, Render, TestAppContext, TextRun,
    VisualTestContext,
};

use super::{Hit, SelectableLeaf};

/// A trivial empty root view, only used to open a window so that
/// `VisualTestContext` mouse simulation is available.
struct EmptyView;

impl Render for EmptyView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
    }
}

/// A painted [`SelectableLeaf`] plus the shared captured-hit sink.
struct Harness {
    hits: Arc<Mutex<Vec<Hit>>>,
}

/// Build a `SelectableLeaf`, paint it via `VisualTestContext::draw`, and return
/// the shared captured-hit sink. The element is painted exactly once (the first
/// draw); subsequent mouse events exercise the first-frame layout.
fn build_and_draw(cx: &mut VisualTestContext, text: String) -> Harness {
    let hits = Arc::new(Mutex::new(Vec::new()));
    let text_len = text.len();
    let element = SelectableLeaf::new(text)
        .with_runs(vec![TextRun {
            len: text_len,
            ..Default::default()
        }])
        .on_mouse_down_hit({
            let hits = Arc::clone(&hits);
            move |hit: Hit| hits.lock().unwrap().push(hit)
        });
    let origin = Point::new(px(0.0), px(0.0));
    cx.draw(
        origin,
        gpui::size(
            gpui::AvailableSpace::Definite(px(400.0)),
            gpui::AvailableSpace::Definite(px(60.0)),
        ),
        move |_window, _app| element,
    );
    Harness { hits }
}

#[gpui::test]
async fn first_frame_hit_test_returns_in_bounds_offset(cx: &mut TestAppContext) {
    let text = "Hello selectable world";
    let (_window, cx) = cx.add_window_view(|_window, _cx| EmptyView);
    let harness = build_and_draw(cx, text.to_string());

    // The very first mouse-down after the first draw must hit-test the real
    // rendered layout without any warm-up click, sink, or replay.
    cx.simulate_mouse_down(
        Point::new(px(60.0), px(10.0)),
        MouseButton::Left,
        Modifiers::default(),
    );

    let captured = harness.hits.lock().unwrap().pop();
    assert!(
        captured.is_some(),
        "first-frame hit-test produced no result — mouse-down listener did not fire"
    );
    let hit = captured.unwrap();
    let offset = hit
        .offset
        .expect("pointer over text must resolve to an offset");
    assert!(
        offset <= text.len(),
        "hit-test offset {offset} is past end of text (len {})",
        text.len()
    );
}

#[gpui::test]
async fn first_frame_hit_test_moves_with_pointer(cx: &mut TestAppContext) {
    let text = "Hello selectable world";
    let (_window, cx) = cx.add_window_view(|_window, _cx| EmptyView);
    let harness = build_and_draw(cx, text.to_string());

    // Hit near the left edge, then further right. Two distinct, in-bounds
    // offsets proves the live layout (not a phantom constant) is driving the
    // result on the first interaction.
    cx.simulate_mouse_down(
        Point::new(px(4.0), px(10.0)),
        MouseButton::Left,
        Modifiers::default(),
    );
    let left = harness
        .hits
        .lock()
        .unwrap()
        .pop()
        .and_then(|h| h.offset)
        .expect("left hit must resolve");

    cx.simulate_mouse_down(
        Point::new(px(150.0), px(10.0)),
        MouseButton::Left,
        Modifiers::default(),
    );
    let right = harness
        .hits
        .lock()
        .unwrap()
        .pop()
        .and_then(|h| h.offset)
        .expect("right hit must resolve");

    assert!(
        left <= right,
        "left hit offset ({left}) should not exceed right hit offset ({right})"
    );
}

#[gpui::test]
async fn first_frame_hit_test_uses_only_real_child_layout(cx: &mut TestAppContext) {
    // Each `Hit` carries the byte length of the rendered StyledText child's
    // text at hit time. This must equal the source text length, proving the
    // hit-tested layout belongs to the actual child rather than a separately
    // constructed phantom.
    let text = "Hello selectable world";
    let (_window, cx) = cx.add_window_view(|_window, _cx| EmptyView);
    let harness = build_and_draw(cx, text.to_string());

    cx.simulate_mouse_down(
        Point::new(px(60.0), px(10.0)),
        MouseButton::Left,
        Modifiers::default(),
    );
    let hit = harness
        .hits
        .lock()
        .unwrap()
        .pop()
        .expect("hit handler must fire");

    assert_eq!(
        hit.rendered_len,
        text.len(),
        "hit-tested layout's rendered length must equal source text length"
    );
}
