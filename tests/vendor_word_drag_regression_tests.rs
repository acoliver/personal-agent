//! Root-level public API integration regression tests for the vendored
//! `gpui-selection` crate's word-drag selection behavior.
//!
//! These tests ensure that the held double-click word-granular drag (Stage 4B)
//! is exercised by the root crate's normal test gates (`cargo test --tests`)
//! without requiring the standalone vendor test suite to be run separately.
//!
//! Coverage:
//! - Held double-click drag within one participant extends and reverses by
//!   whole words and persists after mouse-up.
//! - Held double-click drag across participants keeps both participants'
//!   content keys in both drag directions.
//! - UAX #29-specific Unicode words (apostrophe-joined and decomposed
//!   accents) double-click and drag to exact whole segments.
//! - Virtualizing the original participant mid-drag plus a stationary
//!   `TextSelection::update_drag_at_pointer` tick advances the remounted
//!   row with word snap and stops cleanly on mouse-up.
//! - Triple and quadruple clicks stay static line selections while the
//!   button is held, after release, and on later moves.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    div, point, px, size, App, AppContext, Bounds, Element, ElementId, GlobalElementId,
    HitboxBehavior, InspectorElementId, IntoElement, LayoutId, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Render, Styled,
    StyledText, TestAppContext, TextLayout, Window,
};
use gpui_selection_vendor::{
    TextSelection, TextSelectionContentKey, TextSelectionHandle, TextSelectionLayer,
    TextSelectionRegistration, TextSelectionRun,
};

// ── Test infrastructure (mirrors the vendor's test harness) ─────────────

#[derive(Clone)]
struct Participant {
    selection: TextSelectionHandle,
    text: gpui::SharedString,
    layout: TextLayout,
    document_order: u64,
    content_key: TextSelectionContentKey,
}

impl Participant {
    fn run(&self) -> TextSelectionRun {
        TextSelectionRun::new(self.text.clone(), self.layout.clone(), self.layout.bounds())
            .with_document_order(self.document_order)
    }

    /// A window point inside the glyph at a byte index.
    fn glyph_center(&self, index: usize) -> gpui::Point<Pixels> {
        let start = self.layout.position_for_index(index).unwrap();
        let end = self.layout.position_for_index(index + 1).unwrap();
        point(
            px(f32::midpoint(f32::from(start.x), f32::from(end.x))),
            start.y,
        )
    }

    /// The participant-relative point for a byte index.
    fn content_point(&self, index: usize) -> gpui::Point<Pixels> {
        self.layout.position_for_index(index).unwrap() - self.layout.bounds().origin
    }
}

/// Lays out participant texts in a scratch window to obtain real text runs.
struct RunsLayoutView {
    runs: Vec<(gpui::SharedString, f32)>,
    layouts: Rc<RefCell<Vec<TextLayout>>>,
}

impl Render for RunsLayoutView {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        self.layouts.borrow_mut().clear();
        let children = self
            .runs
            .iter()
            .map(|(text, y)| {
                let styled = StyledText::new(text.clone());
                self.layouts.borrow_mut().push(styled.layout().clone());
                div().absolute().top(px(*y)).child(styled)
            })
            .collect::<Vec<_>>();
        div().size_full().children(children)
    }
}

fn laid_out_runs(
    texts: &[(&'static str, f32)],
    cx: &mut TestAppContext,
) -> Vec<(gpui::SharedString, TextLayout)> {
    let runs = texts
        .iter()
        .map(|(text, y)| (gpui::SharedString::from(*text), *y))
        .collect::<Vec<_>>();
    let layouts = Rc::new(RefCell::new(Vec::new()));
    let view = cx.add_window({
        let runs = runs.clone();
        let layouts = layouts.clone();
        move |_, _| RunsLayoutView { runs, layouts }
    });
    cx.update_window(*view, |_, window, cx| {
        let _ = window.draw(cx);
    })
    .unwrap();
    cx.update(|_| {
        runs.into_iter()
            .map(|(text, _)| text)
            .zip(layouts.borrow_mut().drain(..))
            .collect()
    })
}

struct WordDragView {
    participants: Rc<RefCell<Vec<Participant>>>,
}

struct WordDragElement {
    participants: Rc<RefCell<Vec<Participant>>>,
}

impl IntoElement for WordDragElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for WordDragElement {
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
        for participant in self.participants.borrow().iter() {
            let bounds = participant.layout.bounds();
            let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
            participant.selection.register(
                TextSelectionRegistration::new(hitbox, bounds)
                    .with_document_order(participant.document_order)
                    .with_text_bounds(vec![bounds])
                    .with_content_key(participant.content_key),
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
        cx: &mut App,
    ) {
        for participant in self.participants.borrow().iter() {
            participant.selection.update_runs(&[participant.run()], cx);
        }
    }
}

impl Render for WordDragView {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(TextSelectionLayer)
            .child(WordDragElement {
                participants: Rc::clone(&self.participants),
            })
    }
}

/// A word-drag event window over `(text, y, document_order, content_key)`
/// specs; the first `mounted_count` participants start registered and the
/// rest are returned as unmounted spares for virtualization scenarios.
fn word_drag_window<'a>(
    cx: &'a mut TestAppContext,
    specs: &[(&'static str, f32, u64, u64)],
    mounted_count: usize,
) -> (
    Rc<RefCell<Vec<Participant>>>,
    Vec<Participant>,
    &'a mut gpui::VisualTestContext,
) {
    let layouts = laid_out_runs(
        &specs
            .iter()
            .map(|(text, y, _, _)| (*text, *y))
            .collect::<Vec<_>>(),
        cx,
    );
    let participants = Rc::new(RefCell::new(Vec::new()));
    let (_, window_cx) = cx.add_window_view({
        let participants = Rc::clone(&participants);
        move |_, _| WordDragView { participants }
    });
    let spares = window_cx.update(|window, cx| {
        let entries = layouts
            .into_iter()
            .zip(specs.iter())
            .map(
                |((text, layout), (_, _, document_order, content_key))| Participant {
                    selection: TextSelectionHandle::new(text.to_string(), cx),
                    text,
                    layout,
                    document_order: *document_order,
                    content_key: TextSelectionContentKey::new(*content_key),
                },
            )
            .collect::<Vec<_>>();
        for entry in &entries {
            let content_key = entry.content_key;
            entry
                .selection
                .resolve_content_key_with(move |_, _| Some(content_key), cx);
        }
        *participants.borrow_mut() = entries.iter().take(mounted_count).cloned().collect();
        _ = window.draw(cx);
        entries.into_iter().skip(mounted_count).collect::<Vec<_>>()
    });
    (participants, spares, window_cx)
}

fn multi_click_down(
    window_cx: &mut gpui::VisualTestContext,
    position: gpui::Point<Pixels>,
    click_count: usize,
) {
    window_cx.simulate_event(MouseDownEvent {
        position,
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count,
        first_mouse: false,
    });
}

fn mouse_up_at(
    window_cx: &mut gpui::VisualTestContext,
    position: gpui::Point<Pixels>,
    click_count: usize,
) {
    window_cx.simulate_event(MouseUpEvent {
        position,
        modifiers: Modifiers::default(),
        button: MouseButton::Left,
        click_count,
    });
}

fn double_click(window_cx: &mut gpui::VisualTestContext, position: gpui::Point<Pixels>) {
    multi_click_down(window_cx, position, 2);
}

fn drag_to(window_cx: &mut gpui::VisualTestContext, position: gpui::Point<Pixels>) {
    window_cx.simulate_event(MouseMoveEvent {
        position,
        modifiers: Modifiers::default(),
        pressed_button: Some(MouseButton::Left),
    });
}

fn release_at(window_cx: &mut gpui::VisualTestContext, position: gpui::Point<Pixels>) {
    mouse_up_at(window_cx, position, 2);
}

fn selected_text_after_repaint(window_cx: &mut gpui::VisualTestContext) -> String {
    window_cx.update(|window, cx| {
        _ = window.draw(cx);
        TextSelection::selected_text(window, cx)
    })
}

// ── Tests ──────────────────────────────────────────────────────────────

#[gpui::test]
fn held_double_click_extends_by_whole_words_and_persists(cx: &mut TestAppContext) {
    let text = "alpha words beta";
    let (participants, _spares, window_cx) = word_drag_window(cx, &[(text, 0., 0, 1)], 1);
    let first = participants.borrow()[0].clone();
    let words = text.find("words").unwrap();
    let beta = text.find("beta").unwrap();

    double_click(window_cx, first.glyph_center(words + 1));
    assert_eq!(selected_text_after_repaint(window_cx), "words");

    // Dragging forward extends to the target word's end.
    drag_to(window_cx, first.glyph_center(beta + 1));
    assert_eq!(selected_text_after_repaint(window_cx), "words beta");

    // Dragging back inside the original word restores the whole word.
    drag_to(window_cx, first.glyph_center(words + 1));
    assert_eq!(selected_text_after_repaint(window_cx), "words");

    // Dragging before the original word reverses by whole words.
    drag_to(window_cx, first.glyph_center(1));
    assert_eq!(selected_text_after_repaint(window_cx), "alpha words");

    // Releasing keeps the selection, and later moves do not extend it.
    release_at(window_cx, first.glyph_center(1));
    window_cx.update(|window, cx| {
        assert!(!first.selection.snapshot(cx).unwrap().is_selecting());
        assert!(TextSelection::has_selection(window, cx));
    });
    assert_eq!(selected_text_after_repaint(window_cx), "alpha words");
    window_cx.simulate_mouse_move(first.glyph_center(beta + 1), None, Modifiers::default());
    assert_eq!(selected_text_after_repaint(window_cx), "alpha words");
}

#[gpui::test]
fn word_drag_spans_participants_in_both_directions_with_content_keys(cx: &mut TestAppContext) {
    let first_text = "first words tail";
    let second_text = "second words tail";
    let (participants, _spares, window_cx) =
        word_drag_window(cx, &[(first_text, 0., 0, 1), (second_text, 40., 1, 2)], 2);
    let first = participants.borrow()[0].clone();
    let second = participants.borrow()[1].clone();
    let first_words = first_text.find("words").unwrap();
    let second_words = second_text.find("words").unwrap();

    double_click(window_cx, first.glyph_center(first_words + 1));
    drag_to(window_cx, second.glyph_center(second_words + 1));
    assert_eq!(
        selected_text_after_repaint(window_cx),
        "words tail\nsecond words"
    );
    window_cx.update(|window, cx| {
        assert_eq!(
            TextSelection::content_keys(window, cx),
            Some([
                TextSelectionContentKey::new(1),
                TextSelectionContentKey::new(2)
            ])
        );
    });

    // Dragging back into the first participant reverses by whole words.
    drag_to(window_cx, first.glyph_center(1));
    assert_eq!(selected_text_after_repaint(window_cx), "first words");
    window_cx.update(|window, cx| {
        assert_eq!(
            TextSelection::content_keys(window, cx),
            Some([
                TextSelectionContentKey::new(1),
                TextSelectionContentKey::new(1)
            ])
        );
    });

    release_at(window_cx, first.glyph_center(1));
    assert_eq!(selected_text_after_repaint(window_cx), "first words");
}

#[gpui::test]
fn unicode_words_double_click_and_drag_select_exact_uax_segments(cx: &mut TestAppContext) {
    let apostrophe_text = "see l’objectif now";
    let decomposed_text = "vote cafe\u{0301} non";
    let (participants, _spares, window_cx) = word_drag_window(
        cx,
        &[(apostrophe_text, 0., 0, 1), (decomposed_text, 40., 1, 2)],
        2,
    );
    let first = participants.borrow()[0].clone();
    let second = participants.borrow()[1].clone();

    // The apostrophe joins `l’objectif` into one UAX #29 word segment.
    double_click(window_cx, first.glyph_center(9));
    assert_eq!(selected_text_after_repaint(window_cx), "l’objectif");
    drag_to(window_cx, first.glyph_center(18));
    assert_eq!(selected_text_after_repaint(window_cx), "l’objectif now");
    drag_to(window_cx, first.glyph_center(1));
    assert_eq!(selected_text_after_repaint(window_cx), "see l’objectif");

    // The decomposed combining accent joins `cafe` into one segment without
    // splitting between the base letter and the combining mark.
    double_click(window_cx, second.glyph_center(6));
    assert_eq!(selected_text_after_repaint(window_cx), "cafe\u{0301}");
    drag_to(window_cx, second.glyph_center(13));
    assert_eq!(selected_text_after_repaint(window_cx), "cafe\u{0301} non");
    drag_to(window_cx, second.glyph_center(1));
    assert_eq!(selected_text_after_repaint(window_cx), "vote cafe\u{0301}");
}

#[gpui::test]
fn virtualized_original_word_drag_remounts_via_stationary_pointer_and_stops_on_mouse_up(
    cx: &mut TestAppContext,
) {
    let (participants, spares, window_cx) = word_drag_window(
        cx,
        &[
            ("first words tail", 0., 0, 1),
            ("second words tail", 40., 1, 2),
        ],
        1,
    );
    window_cx.simulate_resize(size(px(100.), px(60.)));
    let commands = Rc::new(RefCell::new(Vec::<Option<Pixels>>::new()));
    let lease = Rc::new(RefCell::new(None));
    let first = participants.borrow()[0].clone();
    let second = spares.into_iter().next().unwrap();
    window_cx.update(|window, cx| {
        let observed = commands.clone();
        *lease.borrow_mut() = Some(TextSelection::subscribe_auto_scroll(
            window,
            cx,
            move |delta, _| observed.borrow_mut().push(delta),
        ));
    });

    let first_words = "first words tail".find("words").unwrap();
    let second_words = "second words tail".find("words").unwrap();
    double_click(window_cx, first.glyph_center(first_words + 1));

    // Hold the pointer stationary in the bottom auto-scroll zone,
    // horizontally at the unmounted row's `words`: the drag extends over
    // the gap to the mounted terminal run end and starts auto-scrolling.
    let pointer = point(second.glyph_center(second_words + 1).x, px(50.));
    drag_to(window_cx, pointer);
    assert!(commands.borrow().last().is_some_and(Option::is_some));
    assert_eq!(selected_text_after_repaint(window_cx), "words tail");

    // Virtualize the original participant while the drag stays held, then
    // remount the unmounted row under the stationary pointer.
    window_cx.update(|window, cx| {
        participants.borrow_mut().remove(0);
        _ = window.draw(cx);
    });
    window_cx.run_until_parked();
    window_cx.update(|window, cx| {
        participants.borrow_mut().push(second.clone());
        _ = window.draw(cx);
    });
    window_cx.run_until_parked();

    // The stationary tick advances the drag onto the remounted participant
    // and snaps the cursor to the target word's outer edge without
    // recomputing the auto-scroll command; the anchor still names the
    // virtualized original participant.
    assert!(window_cx.update(TextSelection::update_drag_at_pointer));
    assert_eq!(
        commands.borrow().len(),
        1,
        "the stationary tick never recomputes auto-scroll"
    );
    window_cx.update(|_, cx| {
        let snapshot = second.selection.snapshot(cx).unwrap();
        assert!(snapshot.is_selecting());
        assert_eq!(
            snapshot.anchor().entity_id(),
            Some(first.selection.entity_id())
        );
        assert_eq!(
            snapshot.cursor().entity_id(),
            Some(second.selection.entity_id())
        );
        assert_eq!(
            snapshot.cursor().content_point(),
            second.content_point(second_words + 5)
        );
    });

    // Remounting the original participant converges its layout-side
    // projection, so the copy spans the whole word-drag selection.
    window_cx.update(|window, cx| {
        participants.borrow_mut().insert(0, first.clone());
        _ = window.draw(cx);
    });
    window_cx.run_until_parked();
    assert_eq!(
        selected_text_after_repaint(window_cx),
        "words tail\nsecond words"
    );

    // Releasing stops the gesture and the auto-scroll stream, and the
    // selection persists.
    release_at(window_cx, pointer);
    assert_eq!(commands.borrow().last(), Some(&None));
    assert!(!window_cx.update(TextSelection::update_drag_at_pointer));
    assert_eq!(
        selected_text_after_repaint(window_cx),
        "words tail\nsecond words"
    );
}

#[gpui::test]
fn triple_and_quadruple_clicks_move_while_held_stay_static_line_selections(
    cx: &mut TestAppContext,
) {
    let text = "alpha words beta";
    let (participants, _spares, window_cx) = word_drag_window(cx, &[(text, 0., 0, 1)], 1);
    let first = participants.borrow()[0].clone();
    let words = text.find("words").unwrap();
    let beta = text.find("beta").unwrap();

    for click_count in [3usize, 4usize] {
        multi_click_down(window_cx, first.glyph_center(words + 1), click_count);
        // Moving while the button is still held keeps the whole-line
        // selection static: word-drag mode never engages past a double
        // click.
        drag_to(window_cx, first.glyph_center(beta + 1));
        assert_eq!(selected_text_after_repaint(window_cx), text);
        mouse_up_at(window_cx, first.glyph_center(beta + 1), click_count);
        assert_eq!(selected_text_after_repaint(window_cx), text);
        window_cx.update(|_, cx| {
            assert!(!first.selection.snapshot(cx).unwrap().is_selecting());
        });

        // Moves after release do not extend the line either.
        drag_to(window_cx, first.glyph_center(words + 1));
        assert_eq!(selected_text_after_repaint(window_cx), text);
    }
}
