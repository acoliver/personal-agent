//! `SelectableMarkdown` custom Element (Phase 3).
//!
//! This element owns the real rendered rich Markdown child and drives it
//! through the standard GPUI `Element` lifecycle (`request_layout` →
//! `prepaint` → `paint`). Each frame it:
//!
//! 1. Clears the [`LeafRegistry`] and rebuilds the rich child (which registers
//!    all inline-text leaves with their live `TextLayout` handles).
//! 2. Lays out and paints the unchanged rich child.
//! 3. Registers current-frame mouse listeners that hit-test against the
//!    leaves' live layouts.
//! 4. Updates the per-message [`Selection`].
//! 5. Paints translucent selection quads over the unchanged child using
//!    `position_for_index` from each leaf's layout.
//!
//! There is no flat renderer, no phantom layout, no warm-up click, no arming,
//! and no deferred replay. Selection is confined to a single message.
//!
//! MIT clean-room implementation using only GPUI public APIs.
//!
//! @plan PLAN-20260713-ISSUE151 Phase 3
//! @requirement REQ-151-001, REQ-151-002, REQ-151-004

#![allow(
    clippy::doc_markdown,
    clippy::module_name_repetitions,
    clippy::redundant_pub_crate,
    clippy::use_self
)]

#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    fill, px, App, Bounds, DispatchPhase, Element, ElementId, GlobalElementId, Hsla,
    InspectorElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, Window,
};

use super::leaf_meta::{LeafMeta, LeafRegistry};
use super::rich_builder::build_selectable_rich_tree;
use super::visible_document::{
    clamp_to_char_boundary, MessageRevision, Selection, VisibleDocument,
};
use super::MarkdownBlock;

const DRAG_THRESHOLD: f32 = 4.0;

/// Interaction emitted by a selectable message body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectableMarkdownEvent {
    /// The active selection changed. The text is rendered without Markdown syntax.
    SelectionChanged {
        selection: Option<Selection>,
        selected_text: String,
        dragging: bool,
    },
    /// A secondary click requested the message selection context menu.
    ContextMenu {
        position: Point<Pixels>,
        selected_text: String,
    },
}

type EventHandler = Rc<dyn Fn(SelectableMarkdownEvent, &mut Window, &mut App)>;

#[derive(Clone)]
struct PointerGesture {
    anchor: usize,
    down_position: Point<Pixels>,
    link_url: Option<String>,
    dragged: bool,
}

/// Selection color: translucent accent.
fn selection_color() -> Hsla {
    crate::ui_gpui::theme::Theme::accent().alpha(0.25)
}

/// State held outside the per-frame lifecycle so it persists across frames.
#[derive(Clone)]
struct SharedState {
    /// The visible document (text + links + blocks) built from the same IR.
    doc: Rc<RefCell<VisibleDocument>>,
    /// Registry of leaf layouts, populated each frame.
    registry: LeafRegistry,
    /// Current selection for this message, if any.
    selection: Rc<RefCell<Option<Selection>>>,
    /// The revision this message was built against.
    revision: Rc<RefCell<MessageRevision>>,
    /// The latest reported "current" revision (to detect staleness).
    current_revision: Rc<RefCell<MessageRevision>>,
    /// Parsed blocks cached by the owning message.
    blocks: Rc<Vec<MarkdownBlock>>,
    /// Bubble-specific base text color.
    text_color: Hsla,
    /// Cached leaves from the most recent layout (for painting quads).
    leaves: Rc<RefCell<Vec<LeafMeta>>>,
    /// Active pointer gesture. This contains offsets only, never geometry.
    gesture: Rc<RefCell<Option<PointerGesture>>>,
    /// Optional owner callback used by the chat view.
    on_event: Option<EventHandler>,
}

/// A custom GPUI element that renders a real rich Markdown child and supports
/// per-message text selection.
///
/// Clone the element to share the same selection state (e.g. across re-renders
/// of the same message). Each clone drives the same shared selection.
///
/// @plan PLAN-20260713-ISSUE151 Phase 3
pub struct SelectableMarkdown {
    state: SharedState,
    element_id: Option<ElementId>,
}

impl Clone for SelectableMarkdown {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            element_id: self.element_id.clone(),
        }
    }
}

impl IntoElement for SelectableMarkdown {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl SelectableMarkdown {
    /// Build a selectable markdown element from raw markdown text.
    ///
    /// The visible document and rich tree are derived from the same IR.
    /// `revision` identifies the message content revision for freshness checks.
    ///
    /// @plan PLAN-20260713-ISSUE151 Phase 2 & 3
    #[must_use]
    pub fn from_markdown(markdown: &str, revision: MessageRevision) -> Self {
        let blocks = super::parse_markdown_blocks(markdown);
        Self::from_blocks(
            markdown,
            blocks,
            revision,
            crate::ui_gpui::theme::Theme::text_primary(),
        )
    }

    /// Build from already-parsed blocks so production chat rendering shares its cache.
    #[must_use]
    pub fn from_blocks(
        _markdown: &str,
        blocks: Vec<MarkdownBlock>,
        revision: MessageRevision,
        text_color: Hsla,
    ) -> Self {
        let doc = VisibleDocument::from_blocks(&blocks);
        Self {
            state: SharedState {
                doc: Rc::new(RefCell::new(doc)),
                registry: LeafRegistry::default(),
                selection: Rc::new(RefCell::new(None)),
                revision: Rc::new(RefCell::new(revision.clone())),
                current_revision: Rc::new(RefCell::new(revision)),
                blocks: Rc::new(blocks),
                text_color,
                leaves: Rc::new(RefCell::new(Vec::new())),
                gesture: Rc::new(RefCell::new(None)),
                on_event: None,
            },
            element_id: None,
        }
    }

    /// Assign a stable per-message identity.
    #[must_use]
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.element_id = Some(id.into());
        self
    }

    /// Restore an externally owned selection after a chat-view re-render.
    #[must_use]
    pub fn with_selection(self, selection: Option<Selection>) -> Self {
        *self.state.selection.borrow_mut() = selection;
        self
    }

    /// Restore an in-progress drag after the owner re-renders with the latest selection.
    #[must_use]
    pub fn with_dragging(self, dragging: bool) -> Self {
        if dragging {
            if let Some(selection) = self.state.selection.borrow().as_ref() {
                *self.state.gesture.borrow_mut() = Some(PointerGesture {
                    anchor: selection.anchor(),
                    down_position: Point::default(),
                    link_url: None,
                    dragged: true,
                });
            }
        }
        self
    }

    /// Receive selection and context-menu events in the owning view.
    #[must_use]
    pub fn on_event<F>(mut self, handler: F) -> Self
    where
        F: Fn(SelectableMarkdownEvent, &mut Window, &mut App) + 'static,
    {
        self.state.on_event = Some(Rc::new(handler));
        self
    }

    /// Return the current selection, or `None` if there is no selection or the
    /// selection is stale.
    #[must_use]
    pub fn selection(&self) -> Option<Selection> {
        if self.is_stale() {
            return None;
        }
        self.state.selection.borrow().clone()
    }

    /// Return the selected visible text, or empty string if no selection.
    #[must_use]
    pub fn selected_text(&self) -> String {
        self.selection().map_or_else(String::new, |selection| {
            self.state.doc.borrow().selected_text(&selection)
        })
    }

    /// Return the visible-document text.
    #[must_use]
    pub fn document_text(&self) -> String {
        self.state.doc.borrow().text().to_string()
    }

    /// Return the number of selectable leaves from the most recent layout.
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        self.state.leaves.borrow().len()
    }

    /// Return the document byte ranges of all leaves from the most recent layout.
    #[must_use]
    pub fn leaf_doc_ranges(&self) -> Vec<Range<usize>> {
        self.state
            .leaves
            .borrow()
            .iter()
            .map(|l| l.doc_range.clone())
            .collect()
    }

    /// Return the rendered text lengths (from the live `TextLayout`) of all
    /// leaves from the most recent layout.
    #[must_use]
    pub fn leaf_rendered_lens(&self) -> Vec<usize> {
        self.state
            .leaves
            .borrow()
            .iter()
            .map(|l| l.layout.len())
            .collect()
    }

    /// Report the current message revision. If it differs from the revision
    /// this element was built against, the selection is cleared.
    pub fn report_current_revision(&self, rev: MessageRevision) {
        *self.state.current_revision.borrow_mut() = rev;
        if self.is_stale() {
            *self.state.selection.borrow_mut() = None;
            *self.state.gesture.borrow_mut() = None;
        }
    }

    /// Return `true` when the message content has changed since this element
    /// was built.
    fn is_stale(&self) -> bool {
        let rev = self.state.revision.borrow();
        let current = self.state.current_revision.borrow();
        *rev != *current
    }

    /// Clear the selection.
    #[allow(dead_code)]
    pub fn clear_selection(&self) {
        *self.state.selection.borrow_mut() = None;
    }

    /// Paint selection quads over the leaves using the current selection.
    fn paint_selection(&self, window: &mut Window, _cx: &mut App) {
        let sel = match self.selection() {
            Some(s) if !s.is_empty() => s,
            _ => return,
        };
        let doc = self.state.doc.borrow();
        let clamped = sel.clamped(doc.text());
        let range = clamped.ordered_range();

        let leaves = self.state.leaves.borrow();
        let color = selection_color();

        for leaf in leaves.iter() {
            // Intersect the selection range with this leaf's document range.
            let sel_start = range.start.max(leaf.doc_range.start);
            let sel_end = range.end.min(leaf.doc_range.end);
            if sel_start >= sel_end {
                continue;
            }
            // Convert document offsets to leaf-local offsets.
            let local_start = sel_start - leaf.doc_range.start;
            let local_end = sel_end - leaf.doc_range.start;

            for bounds in selection_bounds_for_leaf(leaf, local_start..local_end) {
                window.paint_quad(fill(bounds, color));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Element trait implementation
// ---------------------------------------------------------------------------

/// Per-frame layout state: the built rich child for the current frame.
pub struct MarkdownLayoutState {
    child: gpui::AnyElement,
    /// Child element layout ID.
    _layout_id: LayoutId,
}

impl Element for SelectableMarkdown {
    type RequestLayoutState = MarkdownLayoutState;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        self.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        // Clear registry and build the rich child. The child's leaves will
        // register their layouts during the child's request_layout traversal.
        self.state.registry.clear();
        let document = self.state.doc.borrow();
        let child = build_selectable_rich_tree(
            &self.state.blocks,
            &document,
            self.state.text_color,
            &self.state.registry,
        );

        let mut child_elem = child;
        let layout_id = Element::request_layout(&mut child_elem, None, None, window, cx).0;

        // Snapshot the registered leaves for painting + hit-testing this frame.
        let leaves = self.state.registry.leaves();
        *self.state.leaves.borrow_mut() = leaves;

        (
            layout_id,
            MarkdownLayoutState {
                child: child_elem,
                _layout_id: layout_id,
            },
        )
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        Element::prepaint(
            &mut request_layout.child,
            None,
            None,
            bounds,
            &mut (),
            window,
            cx,
        );
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        Element::paint(
            &mut request_layout.child,
            None,
            None,
            bounds,
            &mut (),
            &mut (),
            window,
            cx,
        );

        let gesture = persistent_gesture(global_id, &self.state, window);
        self.paint_selection(window, cx);

        let mut state = self.state.clone();
        state.gesture = gesture;
        let doc_text = self.state.doc.borrow().text().to_string();
        register_mouse_listeners(state, bounds, doc_text, window);
    }
}

// ---------------------------------------------------------------------------
// Free helpers
// ---------------------------------------------------------------------------

fn persistent_gesture(
    global_id: Option<&GlobalElementId>,
    state: &SharedState,
    window: &mut Window,
) -> Rc<RefCell<Option<PointerGesture>>> {
    let Some(global_id) = global_id else {
        return state.gesture.clone();
    };
    let frame_gesture = state.gesture.clone();
    window.with_element_state::<Rc<RefCell<Option<PointerGesture>>>, _>(
        global_id,
        |stored, _window| {
            let persistent = stored.unwrap_or(frame_gesture);
            (persistent.clone(), persistent)
        },
    )
}

fn register_mouse_listeners(
    state: SharedState,
    bounds: Bounds<Pixels>,
    doc_text: String,
    window: &mut Window,
) {
    register_secondary_click(state.clone(), bounds, window);
    register_primary_down(state.clone(), bounds, doc_text.clone(), window);
    register_primary_move(state.clone(), bounds, doc_text, window);
    register_primary_up(state, window);
}

fn register_secondary_click(state: SharedState, bounds: Bounds<Pixels>, window: &mut Window) {
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble
            || event.button != MouseButton::Right
            || !bounds.contains(&event.position)
        {
            return;
        }
        let Some(offset) = hit_test_leaves(&state, event.position) else {
            return;
        };
        let selection = state.selection.borrow().clone();
        let Some(selection) = selection.filter(|selection| selection.contains(offset)) else {
            return;
        };
        let selected_text = state.doc.borrow().selected_text(&selection);
        emit_event(
            &state,
            SelectableMarkdownEvent::ContextMenu {
                position: event.position,
                selected_text,
            },
            window,
            cx,
        );
    });
}

fn register_primary_down(
    state: SharedState,
    bounds: Bounds<Pixels>,
    doc_text: String,
    window: &mut Window,
) {
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble
            || event.button != MouseButton::Left
            || !bounds.contains(&event.position)
        {
            return;
        }
        let Some(offset) = hit_test_leaves(&state, event.position) else {
            return;
        };
        match event.click_count {
            count if count >= 3 => select_semantic_block(&state, offset, window, cx),
            2 => select_word(&state, &doc_text, offset, window, cx),
            _ => start_pointer_gesture(&state, &doc_text, offset, event.position),
        }
    });
}

fn select_semantic_block(state: &SharedState, offset: usize, window: &mut Window, cx: &mut App) {
    let selection = state
        .doc
        .borrow()
        .semantic_blocks()
        .iter()
        .find(|block| block.range.contains(&offset) || block.range.end == offset)
        .map(|block| Selection::block(block.range.clone()));
    set_selection(state, selection, false, window, cx);
    *state.gesture.borrow_mut() = None;
}

fn select_word(state: &SharedState, text: &str, offset: usize, window: &mut Window, cx: &mut App) {
    let range = super::visible_document::word_range_at(text, offset);
    set_selection(state, Some(Selection::word(range)), false, window, cx);
    *state.gesture.borrow_mut() = None;
}

fn start_pointer_gesture(state: &SharedState, text: &str, offset: usize, position: Point<Pixels>) {
    let anchor = clamp_to_char_boundary(text, offset);
    let link_url = link_at_offset(state, anchor);
    *state.gesture.borrow_mut() = Some(PointerGesture {
        anchor,
        down_position: position,
        link_url,
        dragged: false,
    });
    *state.selection.borrow_mut() = Some(Selection::char(anchor));
}

fn register_primary_move(
    state: SharedState,
    bounds: Bounds<Pixels>,
    doc_text: String,
    window: &mut Window,
) {
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble || event.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let mut gesture_ref = state.gesture.borrow_mut();
        let Some(gesture) = gesture_ref.as_mut() else {
            return;
        };
        let dx = f32::from(event.position.x - gesture.down_position.x);
        let dy = f32::from(event.position.y - gesture.down_position.y);
        if !gesture.dragged && dx.hypot(dy) < DRAG_THRESHOLD {
            return;
        }
        gesture.dragged = true;
        gesture.link_url = None;
        let anchor = gesture.anchor;
        drop(gesture_ref);

        let point = clamp_point(event.position, bounds);
        let offset = hit_test_leaves(&state, point).unwrap_or(anchor);
        let head = clamp_to_char_boundary(&doc_text, offset);
        set_selection(&state, Some(Selection::new(anchor, head)), true, window, cx);
    });
}

fn register_primary_up(state: SharedState, window: &mut Window) {
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
            return;
        }
        let Some(gesture) = state.gesture.borrow_mut().take() else {
            return;
        };
        if gesture.dragged {
            let selection = state.selection.borrow().clone();
            set_selection(&state, selection, false, window, cx);
            return;
        }
        open_link_if_same_target(&state, &gesture, event.position, cx);
        set_selection(&state, None, false, window, cx);
    });
}

fn open_link_if_same_target(
    state: &SharedState,
    gesture: &PointerGesture,
    position: Point<Pixels>,
    cx: &mut App,
) {
    let Some(url) = &gesture.link_url else {
        return;
    };
    let same_link = hit_test_leaves(state, position)
        .and_then(|offset| link_at_offset(state, offset))
        .is_some_and(|candidate| candidate == *url);
    if same_link && super::is_safe_url(url) {
        cx.open_url(url);
    }
}

/// Hit-test a point against the shared state's leaves and return a document
/// byte offset.
fn hit_test_leaves(state: &SharedState, point: Point<Pixels>) -> Option<usize> {
    let leaves = state.leaves.borrow();
    // First, find the leaf whose vertical band contains the point.
    for leaf in leaves.iter() {
        let layout = &leaf.layout;
        let bounds = layout.bounds();
        if point.y >= bounds.top() && point.y <= bounds.bottom() {
            let offset = match layout.index_for_position(point) {
                Ok(idx) | Err(idx) => idx,
            };
            let local = offset.min(leaf.doc_range.len());
            return Some(leaf.doc_range.start + local);
        }
    }
    // If no leaf's vertical band matches, find the nearest leaf by distance.
    let mut best: Option<(f32, usize)> = None;
    for leaf in leaves.iter() {
        let layout = &leaf.layout;
        let bounds = layout.bounds();

        let dy = (f32::from(point.y) - f32::from(bounds.origin.y)).abs();
        match best {
            None => best = Some((dy, leaf.doc_range.start)),
            Some((bd, _)) if dy < bd => best = Some((dy, leaf.doc_range.start)),
            _ => {}
        }
    }
    best.map(|(_, o)| o)
}

/// Clamp a point to the interior of `bounds`.
fn clamp_point(point: Point<Pixels>, bounds: Bounds<Pixels>) -> Point<Pixels> {
    Point::new(
        point.x.max(bounds.left()).min(bounds.right()),
        point.y.max(bounds.top()).min(bounds.bottom()),
    )
}

/// Build one highlight rectangle per rendered line segment.
fn selection_bounds_for_leaf(leaf: &LeafMeta, range: Range<usize>) -> Vec<Bounds<Pixels>> {
    let layout = &leaf.layout;
    let text = layout.text();
    let line_height = layout.line_height();
    let layout_bounds = layout.bounds();
    let mut result: Vec<Bounds<Pixels>> = Vec::new();

    for (relative, ch) in text[range.clone()].char_indices() {
        if ch == '\n' {
            continue;
        }
        let start = range.start + relative;
        let end = start + ch.len_utf8();
        let Some(start_pos) = layout.position_for_index(start) else {
            continue;
        };
        let Some(end_pos) = layout.position_for_index(end) else {
            continue;
        };
        let right = if (end_pos.y - start_pos.y).abs() < line_height * 0.5 {
            end_pos.x
        } else {
            layout_bounds.right()
        };
        if right <= start_pos.x {
            continue;
        }
        if let Some(previous) = result.last_mut() {
            let same_line = (previous.origin.y - start_pos.y).abs() < line_height * 0.5;

            let adjacent = start_pos.x <= previous.right() + px(1.0);
            if same_line && adjacent {
                previous.size.width = right - previous.left();
                continue;
            }
        }
        result.push(Bounds::from_corners(
            start_pos,
            Point::new(right, start_pos.y + line_height),
        ));
    }
    result
}

fn link_at_offset(state: &SharedState, offset: usize) -> Option<String> {
    state
        .doc
        .borrow()
        .links()
        .iter()
        .find(|link| link.range.contains(&offset))
        .map(|link| link.url.clone())
}

fn emit_event(
    state: &SharedState,
    event: SelectableMarkdownEvent,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(handler) = state.on_event.as_ref() {
        handler(event, window, cx);
    }
}

fn set_selection(
    state: &SharedState,
    selection: Option<Selection>,
    dragging: bool,
    window: &mut Window,
    cx: &mut App,
) {
    state.selection.borrow_mut().clone_from(&selection);
    let selected_text = selection
        .as_ref()
        .map(|selection| state.doc.borrow().selected_text(selection))
        .unwrap_or_default();
    emit_event(
        state,
        SelectableMarkdownEvent::SelectionChanged {
            selection,
            selected_text,
            dragging,
        },
        window,
        cx,
    );
    window.refresh();
}
