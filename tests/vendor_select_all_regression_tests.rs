//! Root-level public API integration regression tests for the vendored
//! `gpui-selection` crate's logical select-all behavior.
//!
//! These tests ensure that critical vendor behavior is exercised by the
//! root crate's normal test gates (`cargo test --lib --tests`) without
//! requiring the standalone vendor test suite to be run separately.
//!
//! Coverage:
//! - Logical select-all publishes frozen copy text, reports all keys, and
//!   marks matching in-scope participants Full.
//! - Empty key lists and blank text are rejected without touching state.
//! - Registrations whose content key is outside the frozen set, or outside
//!   the active scope, are never marked.
//! - Virtualized participants are not retained; remounting the same content
//!   key rejoins the logical selection.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gpui::{
    div, hsla, point, px, size, App, Bounds, Element, ElementId, GlobalElementId, HitboxBehavior,
    InspectorElementId, IntoElement, LayoutId, Modifiers, MouseButton, ParentElement, Pixels,
    Render, SharedString, Styled, TestAppContext, TextRun, Window,
};
use gpui_selection_vendor::{
    SelectableText, TextSelection, TextSelectionContentKey, TextSelectionCoverage,
    TextSelectionHandle, TextSelectionLayer, TextSelectionRegistration, TextSelectionScopeId,
};

const fn key(value: u64) -> TextSelectionContentKey {
    TextSelectionContentKey::new(value)
}

#[derive(Clone)]
struct KeyedRegistration {
    selection: TextSelectionHandle,
    y: f32,
    document_order: u64,
    content_key: TextSelectionContentKey,
    scope: TextSelectionScopeId,
}

struct SelectAllView {
    registrations: Rc<RefCell<Vec<KeyedRegistration>>>,
}

struct SelectAllElement {
    registrations: Rc<RefCell<Vec<KeyedRegistration>>>,
}

impl IntoElement for SelectAllElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SelectAllElement {
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
        (): &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        for registration in self.registrations.borrow().iter() {
            let bounds = Bounds::new(point(px(0.), px(registration.y)), size(px(100.), px(10.)));
            let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
            registration.selection.register(
                TextSelectionRegistration::new(hitbox, bounds)
                    .with_document_order(registration.document_order)
                    .with_text_bounds(vec![bounds])
                    .with_content_key(registration.content_key)
                    .with_scope(registration.scope),
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
        (): &mut Self::RequestLayoutState,
        (): &mut Self::PrepaintState,
        _: &mut Window,
        _: &mut App,
    ) {
    }
}

impl Render for SelectAllView {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(TextSelectionLayer)
            .child(SelectAllElement {
                registrations: Rc::clone(&self.registrations),
            })
    }
}

fn keyed(
    selection: &TextSelectionHandle,
    y: f32,
    document_order: u64,
    content_key: TextSelectionContentKey,
) -> KeyedRegistration {
    KeyedRegistration {
        selection: selection.clone(),
        y,
        document_order,
        content_key,
        scope: TextSelectionScopeId::default(),
    }
}

fn scoped(registration: KeyedRegistration, scope: TextSelectionScopeId) -> KeyedRegistration {
    KeyedRegistration {
        scope,
        ..registration
    }
}

#[gpui::test]
fn select_all_serves_frozen_copy_and_reports_all_keys(cx: &mut TestAppContext) {
    let registrations = Rc::new(RefCell::new(Vec::new()));
    let (_, window_cx) = cx.add_window_view({
        let registrations = Rc::clone(&registrations);
        move |_, _| SelectAllView { registrations }
    });
    window_cx.update(|window, cx| {
        let first = TextSelectionHandle::new("first text", cx);
        let second = TextSelectionHandle::new("second text", cx);
        *registrations.borrow_mut() =
            vec![keyed(&first, 0., 0, key(1)), keyed(&second, 20., 1, key(2))];
        _ = window.draw(cx);

        TextSelection::select_all(&[key(1), key(2)], "frozen whole transcript", window, cx);

        assert!(TextSelection::has_selection(window, cx));
        assert_eq!(
            TextSelection::selected_text(window, cx),
            "frozen whole transcript"
        );
        assert_eq!(
            TextSelection::selected_content_keys(window, cx),
            Some(vec![key(1), key(2)])
        );
        assert_eq!(
            first.snapshot(cx).expect("keyed participant").coverage(),
            TextSelectionCoverage::Full
        );
        assert_eq!(
            second.snapshot(cx).expect("keyed participant").coverage(),
            TextSelectionCoverage::Full
        );

        TextSelection::clear(window, cx);
        assert!(!TextSelection::has_selection(window, cx));
        assert_eq!(TextSelection::selected_text(window, cx), "");
        assert_eq!(TextSelection::selected_content_keys(window, cx), None);
    });
}

#[gpui::test]
fn select_all_rejects_empty_keys_and_blank_text(cx: &mut TestAppContext) {
    let registrations = Rc::new(RefCell::new(Vec::new()));
    let (_, window_cx) = cx.add_window_view({
        let registrations = Rc::clone(&registrations);
        move |_, _| SelectAllView { registrations }
    });
    window_cx.update(|window, cx| {
        let participant = TextSelectionHandle::new("text", cx);
        *registrations.borrow_mut() = vec![keyed(&participant, 0., 0, key(1))];
        _ = window.draw(cx);

        TextSelection::select_all(&[], "text", window, cx);
        assert!(!TextSelection::has_selection(window, cx));
        assert!(participant.snapshot(cx).is_none());

        TextSelection::select_all(&[key(1)], "", window, cx);
        assert!(!TextSelection::has_selection(window, cx));
        assert!(participant.snapshot(cx).is_none());

        // Blank-only text is rejected too: copy resolution drops blank
        // items, so a keyful but blank logical selection could otherwise
        // exist with no copyable text. The app layer additionally refuses
        // blank-only transcript payloads before ever calling select_all.
        TextSelection::select_all(&[key(1)], "   ", window, cx);
        assert!(!TextSelection::has_selection(window, cx));
        assert!(participant.snapshot(cx).is_none());
    });
}

#[gpui::test]
fn select_all_marks_only_frozen_keys_in_the_active_scope(cx: &mut TestAppContext) {
    let registrations = Rc::new(RefCell::new(Vec::new()));
    let (_, window_cx) = cx.add_window_view({
        let registrations = Rc::clone(&registrations);
        move |_, _| SelectAllView { registrations }
    });
    window_cx.update(|window, cx| {
        let inside = TextSelectionHandle::new("inside", cx);
        let outside_scope = TextSelectionHandle::new("outside scope", cx);
        let outside_set = TextSelectionHandle::new("outside set", cx);
        *registrations.borrow_mut() = vec![
            keyed(&inside, 0., 0, key(1)),
            scoped(
                keyed(&outside_scope, 20., 1, key(1)),
                TextSelectionScopeId::new(),
            ),
            keyed(&outside_set, 40., 2, key(2)),
        ];
        _ = window.draw(cx);

        TextSelection::select_all(&[key(1)], "frozen", window, cx);

        assert_eq!(
            inside
                .snapshot(cx)
                .expect("matching key and scope")
                .coverage(),
            TextSelectionCoverage::Full
        );
        assert!(outside_scope.snapshot(cx).is_none());
        assert!(outside_set.snapshot(cx).is_none());
    });
}

#[gpui::test]
fn virtualized_participant_rejoins_by_content_key(cx: &mut TestAppContext) {
    let registrations = Rc::new(RefCell::new(Vec::new()));
    let (_, window_cx) = cx.add_window_view({
        let registrations = Rc::clone(&registrations);
        move |_, _| SelectAllView { registrations }
    });
    let participant = window_cx.update(|window, cx| {
        let participant = TextSelectionHandle::new("virtualized", cx);
        *registrations.borrow_mut() = vec![keyed(&participant, 0., 0, key(5))];
        _ = window.draw(cx);
        participant
    });

    // Install the logical selection, then virtualize the participant away.
    // The post-frame sweep runs when this update closure ends.
    window_cx.update(|window, cx| {
        TextSelection::select_all(&[key(5)], "frozen", window, cx);
        assert!(participant.snapshot(cx).is_some());
        registrations.borrow_mut().clear();
        _ = window.draw(cx);
    });

    // The selection survives without retaining the unmounted participant,
    // and the frozen copy still resolves.
    window_cx.update(|window, cx| {
        assert!(participant.snapshot(cx).is_none());
        assert!(TextSelection::has_selection(window, cx));
        assert_eq!(TextSelection::selected_text(window, cx), "frozen");
        assert_eq!(
            TextSelection::selected_content_keys(window, cx),
            Some(vec![key(5)])
        );
    });

    // Remounting the same content key rejoins as Full; a registration whose
    // key is outside the frozen set stays unmarked.
    window_cx.update(|window, cx| {
        let outsider = TextSelectionHandle::new("outsider", cx);
        *registrations.borrow_mut() = vec![
            keyed(&participant, 0., 0, key(5)),
            keyed(&outsider, 20., 1, key(6)),
        ];
        _ = window.draw(cx);

        assert_eq!(
            participant
                .snapshot(cx)
                .expect("remounted content key")
                .coverage(),
            TextSelectionCoverage::Full
        );
        assert!(outsider.snapshot(cx).is_none());
    });
}

const REMOUNT_BRIDGE_TEXT: &str = "bridge text";

/// Renders a real `SelectableText` leaf that can be unmounted and remounted
/// with the same content key, standing in for a virtualized transcript leaf.
struct RemountBridgeView {
    mounted: Rc<Cell<bool>>,
}

impl Render for RemountBridgeView {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let mut root = div().size_full().child(TextSelectionLayer);
        if self.mounted.get() {
            root = root.child(
                SelectableText::new(
                    SharedString::from("bridge-leaf"),
                    REMOUNT_BRIDGE_TEXT,
                    vec![TextRun {
                        len: REMOUNT_BRIDGE_TEXT.len(),
                        color: hsla(0., 0., 0., 1.),
                        ..Default::default()
                    }],
                    hsla(0., 0., 1., 1.),
                    hsla(0., 0., 0., 1.),
                )
                .content_key(key(7))
                .document_order(0),
            );
        }
        root
    }
}

/// Rendered-`SelectableText` bridge for logical select-all: the element joins
/// by `content_key`, survives unmounting with the frozen copy intact, and a
/// remounted element with the same key participates again (proven by a
/// pointer drag whose copy text and endpoint keys come from the remounted
/// element's own registration). The Full-snapshot/highlight projection itself
/// is proven in the vendor crate's `selectable_text_bridge_tests`, where the
/// participant handle is observable.
#[gpui::test]
fn rendered_selectable_text_bridge_keeps_logical_select_all_across_remount(
    cx: &mut TestAppContext,
) {
    let mounted = Rc::new(Cell::new(true));
    let (_, window_cx) = cx.add_window_view({
        let mounted = mounted.clone();
        move |_, _| RemountBridgeView { mounted }
    });

    window_cx.update(|window, cx| {
        _ = window.draw(cx);
        assert!(!TextSelection::has_selection(window, cx));

        TextSelection::select_all(&[key(7)], "frozen bridge copy", window, cx);
        assert!(TextSelection::has_selection(window, cx));
        assert_eq!(
            TextSelection::selected_text(window, cx),
            "frozen bridge copy"
        );
        assert_eq!(
            TextSelection::selected_content_keys(window, cx),
            Some(vec![key(7)])
        );
    });

    // Unmount the leaf: the logical selection survives on its frozen payload.
    window_cx.update(|window, cx| {
        mounted.set(false);
        _ = window.draw(cx);
    });
    window_cx.update(|window, cx| {
        assert!(TextSelection::has_selection(window, cx));
        assert_eq!(
            TextSelection::selected_text(window, cx),
            "frozen bridge copy"
        );
        assert_eq!(
            TextSelection::selected_content_keys(window, cx),
            Some(vec![key(7)])
        );
    });

    // Remount a fresh leaf with the same content key, then drag across it:
    // the pointer selection replaces the logical one and resolves its copy
    // text and endpoint content keys through the remounted element's own
    // geometry, runs, and content-key resolver.
    window_cx.update(|window, cx| {
        mounted.set(true);
        _ = window.draw(cx);
    });
    window_cx.simulate_mouse_down(
        point(px(2.), px(2.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    window_cx.simulate_mouse_move(
        point(px(40.), px(2.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    window_cx.simulate_mouse_up(
        point(px(40.), px(2.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    window_cx.update(|window, cx| {
        let dragged = TextSelection::selected_text(window, cx);
        assert!(
            !dragged.is_empty(),
            "the remounted leaf participates in pointer selection"
        );
        assert!(
            REMOUNT_BRIDGE_TEXT.contains(&dragged),
            "copy text {dragged:?} comes from the remounted element"
        );
        assert_ne!(dragged, "frozen bridge copy");
        assert_eq!(
            TextSelection::selected_content_keys(window, cx),
            Some(vec![key(7), key(7)]),
            "the remounted element's content key rides the endpoints"
        );

        TextSelection::clear(window, cx);
        assert!(!TextSelection::has_selection(window, cx));
    });
}
