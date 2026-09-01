//! Root-level public API integration regression tests for the vendored
//! `gpui-selection` crate's drag auto-scroll behavior.
//!
//! These tests ensure that critical vendor behavior is exercised by the
//! root crate's normal test gates (`cargo test --tests` or `cargo test --lib`)
//! without requiring the standalone vendor test suite to be run separately.
//!
//! Coverage:
//! - Some/None mouse-up: a drag into the edge zone publishes Some, mouse-up
//!   publishes None, and no further commands arrive after.
//! - Restart through participant churn: replacing the anchor's leaf during a
//!   drag does not produce spurious None/Some; the stream survives.
//! - Stationary update: `update_drag_at_pointer` advances the selection
//!   endpoint against newly mounted participants while the pointer is
//!   stationary.

use std::{cell::RefCell, rc::Rc, time::Duration};

use gpui::{
    div, point, px, size, App, Bounds, Element, ElementId, GlobalElementId, HitboxBehavior,
    InspectorElementId, IntoElement, LayoutId, Modifiers, MouseButton, ParentElement, Pixels,
    Render, Styled, TestAppContext, Window,
};
use gpui_selection_vendor::{
    AutoScrollLease, TextSelection, TextSelectionHandle, TextSelectionLayer,
    TextSelectionRegistration,
};

// ── Test infrastructure (mirrors the vendor's test harness) ─────────────

#[derive(Clone)]
struct Registration {
    selection: TextSelectionHandle,
    y: f32,
    document_order: u64,
}

struct AutoScrollView {
    registrations: Rc<RefCell<Vec<Registration>>>,
}

struct AutoScrollElement {
    registrations: Rc<RefCell<Vec<Registration>>>,
}

impl IntoElement for AutoScrollElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for AutoScrollElement {
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
        (window.request_layout(gpui::Style::default(), [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        for registration in self.registrations.borrow().iter() {
            let bounds = Bounds::new(point(px(0.), px(registration.y)), size(px(100.), px(10.)));
            let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
            // Use the public register API so the root test does not depend on
            // private window state accessors.
            registration.selection.register(
                TextSelectionRegistration::new(hitbox, bounds)
                    .with_document_order(registration.document_order)
                    .with_text_bounds(vec![bounds]),
                window,
                cx,
            );
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        _: &mut Window,
        _: &mut App,
    ) {
    }
}

impl Render for AutoScrollView {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(TextSelectionLayer)
            .child(AutoScrollElement {
                registrations: Rc::clone(&self.registrations),
            })
    }
}

fn registration(selection: &TextSelectionHandle, y: f32, document_order: u64) -> Registration {
    Registration {
        selection: selection.clone(),
        y,
        document_order,
    }
}

#[allow(clippy::type_complexity)]
fn setup_window<'a>(
    cx: &'a mut TestAppContext,
    registrations: &Rc<RefCell<Vec<Registration>>>,
) -> (
    &'a mut gpui::VisualTestContext,
    Rc<RefCell<Option<AutoScrollLease>>>,
    Rc<RefCell<Vec<Option<Pixels>>>>,
) {
    let commands = Rc::new(RefCell::new(Vec::<Option<Pixels>>::new()));
    let lease = Rc::new(RefCell::new(None));
    let (_, window_cx) = cx.add_window_view({
        let registrations = Rc::clone(registrations);
        move |_, _| AutoScrollView { registrations }
    });
    window_cx.simulate_resize(size(px(100.), px(40.)));
    (window_cx, lease, commands)
}

// ── Tests ──────────────────────────────────────────────────────────────

#[gpui::test]
fn drag_publishes_some_then_mouse_up_publishes_none(cx: &mut TestAppContext) {
    let registrations = Rc::new(RefCell::new(Vec::new()));
    let (window_cx, lease, commands) = setup_window(cx, &registrations);
    window_cx.update(|window, cx| {
        let observed = commands.clone();
        *lease.borrow_mut() = Some(TextSelection::subscribe_auto_scroll(
            window,
            cx,
            move |delta, _| {
                observed.borrow_mut().push(delta);
            },
        ));
        let participant = TextSelectionHandle::new("line", cx);
        registrations
            .borrow_mut()
            .push(registration(&participant, 0., 0));
        _ = window.draw(cx);
    });

    // Drag into the bottom edge zone: publishes Some.
    window_cx.simulate_mouse_down(
        point(px(5.), px(5.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    window_cx.simulate_mouse_move(
        point(px(5.), px(30.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    assert!(
        commands.borrow().iter().any(Option::is_some),
        "drag into the bottom edge publishes Some"
    );

    // Mouse up: publishes None.
    window_cx.simulate_mouse_up(
        point(px(5.), px(30.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert_eq!(
        commands.borrow().last(),
        Some(&None),
        "mouse up publishes None"
    );

    // No further commands after mouse up.
    let count = commands.borrow().len();
    window_cx.simulate_mouse_move(point(px(5.), px(32.)), None, Modifiers::default());
    assert_eq!(
        commands.borrow().len(),
        count,
        "no further commands after mouse up"
    );
}

#[gpui::test]
fn restart_through_participant_churn_survives(cx: &mut TestAppContext) {
    let registrations = Rc::new(RefCell::new(Vec::new()));
    let (window_cx, lease, commands) = setup_window(cx, &registrations);
    window_cx.update(|window, cx| {
        let observed = commands.clone();
        *lease.borrow_mut() = Some(TextSelection::subscribe_auto_scroll(
            window,
            cx,
            move |delta, _| {
                observed.borrow_mut().push(delta);
            },
        ));
        let anchor = TextSelectionHandle::new("anchor", cx);
        registrations
            .borrow_mut()
            .push(registration(&anchor, 0., 0));
        _ = window.draw(cx);
    });

    // Start drag.
    window_cx.simulate_mouse_down(
        point(px(5.), px(5.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    window_cx.simulate_mouse_move(
        point(px(5.), px(25.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    assert_eq!(commands.borrow().len(), 1);
    assert!(commands.borrow()[0].is_some());

    // Replace the anchor's leaf entirely during the drag.
    window_cx.update(|window, cx| {
        let replacement = TextSelectionHandle::new("replacement", cx);
        *registrations.borrow_mut() = vec![registration(&replacement, 0., 0)];
        _ = window.draw(cx);
    });

    // Repeating the same pointer position must not re-emit or produce a
    // spurious None.
    window_cx.simulate_mouse_move(
        point(px(5.), px(25.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    assert_eq!(
        commands.borrow().len(),
        1,
        "participant churn does not produce spurious commands"
    );

    // Mouse up publishes None to end the gesture.
    window_cx.simulate_mouse_up(
        point(px(5.), px(25.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert_eq!(commands.borrow().last(), Some(&None));
}

#[gpui::test]
fn stationary_update_advances_endpoint(cx: &mut TestAppContext) {
    let registrations = Rc::new(RefCell::new(Vec::new()));
    let (window_cx, lease, commands) = setup_window(cx, &registrations);
    let _first = window_cx.update(|window, cx| {
        let observed = commands.clone();
        *lease.borrow_mut() = Some(TextSelection::subscribe_auto_scroll(
            window,
            cx,
            move |delta, _| {
                observed.borrow_mut().push(delta);
            },
        ));
        let first = TextSelectionHandle::new("first", cx);
        registrations.borrow_mut().push(registration(&first, 0., 0));
        _ = window.draw(cx);
        first
    });

    // Start drag.
    window_cx.simulate_mouse_down(
        point(px(5.), px(5.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    window_cx.simulate_mouse_move(
        point(px(5.), px(25.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    assert_eq!(commands.borrow().len(), 1);

    // The scroll host mounts a new row under the stationary pointer.
    let second = window_cx.update(|window, cx| {
        let second = TextSelectionHandle::new("second", cx);
        registrations
            .borrow_mut()
            .push(registration(&second, 20., 1));
        _ = window.draw(cx);
        second
    });
    window_cx.run_until_parked();

    // update_drag_at_pointer advances the endpoint against the newly
    // mounted participant even though the pointer hasn't moved.
    let is_selecting = window_cx.update(TextSelection::update_drag_at_pointer);
    assert!(is_selecting, "drag is still active after stationary update");

    let cursor = window_cx.update(|window, cx| {
        second
            .snapshot(cx)
            .expect("newly mounted participant joins the selection")
            .cursor()
            .entity_id()
            .map(|id| {
                (
                    id == second.entity_id(),
                    TextSelection::has_selection(window, cx),
                )
            })
    });
    assert_eq!(
        cursor,
        Some((true, true)),
        "stationary update advances the cursor to the newly mounted participant"
    );

    // Mouse up stops the gesture.
    window_cx.simulate_mouse_up(
        point(px(5.), px(25.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert_eq!(commands.borrow().last(), Some(&None));
}

#[gpui::test]
fn sustained_drag_two_ticks_advance_endpoint_then_stop(cx: &mut TestAppContext) {
    let registrations = Rc::new(RefCell::new(Vec::new()));
    let (window_cx, lease, commands) = setup_window(cx, &registrations);
    let first = window_cx.update(|window, cx| {
        let observed = commands.clone();
        *lease.borrow_mut() = Some(TextSelection::subscribe_auto_scroll(
            window,
            cx,
            move |delta, _| {
                observed.borrow_mut().push(delta);
            },
        ));
        let first = TextSelectionHandle::new("first", cx);
        registrations.borrow_mut().push(registration(&first, 0., 0));
        _ = window.draw(cx);
        first
    });

    // Start drag at the bottom edge.
    window_cx.simulate_mouse_down(
        point(px(5.), px(5.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    window_cx.simulate_mouse_move(
        point(px(5.), px(30.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    assert!(commands.borrow().last().is_some_and(Option::is_some));

    // Tick 1: mount a new participant and advance the drag.
    let second = window_cx.update(|window, cx| {
        let second = TextSelectionHandle::new("second", cx);
        registrations
            .borrow_mut()
            .push(registration(&second, 20., 1));
        _ = window.draw(cx);
        second
    });
    window_cx.run_until_parked();
    let tick1 = window_cx.update(TextSelection::update_drag_at_pointer);
    assert!(tick1, "tick 1: drag still active");

    let cursor1 =
        window_cx.update(|_, cx| second.snapshot(cx).and_then(|s| s.cursor().entity_id()));
    assert_eq!(
        cursor1,
        Some(second.entity_id()),
        "tick 1: endpoint advanced to the new participant"
    );

    // Tick 2: the previous tick scrolled the host, so the row under the
    // stationary pointer is replaced by a newly mounted one (the anchor row
    // stays registered). The endpoint must advance to that new row on the
    // second tick; an exact-overlap push could never win the hit-test's
    // document-order tie-break.
    let third = window_cx.update(|window, cx| {
        let third = TextSelectionHandle::new("third", cx);
        *registrations.borrow_mut() =
            vec![registration(&first, 0., 0), registration(&third, 20., 1)];
        _ = window.draw(cx);
        third
    });
    window_cx.run_until_parked();
    let tick2 = window_cx.update(TextSelection::update_drag_at_pointer);
    assert!(tick2, "tick 2: drag still active");

    let cursor2 = window_cx.update(|_, cx| third.snapshot(cx).and_then(|s| s.cursor().entity_id()));
    assert_eq!(
        cursor2,
        Some(third.entity_id()),
        "tick 2: endpoint advanced to the newly mounted participant"
    );

    // Mouse up: publishes None, no further ticks.
    window_cx.simulate_mouse_up(
        point(px(5.), px(30.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert_eq!(commands.borrow().last(), Some(&None));
    let count = commands.borrow().len();

    cx.executor().advance_clock(Duration::from_millis(48));
    cx.run_until_parked();
    assert_eq!(
        commands.borrow().len(),
        count,
        "no further commands after mouse up"
    );
}
