//! Selectable leaf — Phase 0 first-frame hit-test element (issue #151).
//!
//! This is the minimal, reusable custom GPUI `Element` that proves a
//! `StyledText` layout belonging to the ACTUAL retained/rendered rich child can
//! be hit-tested on the very first mouse-down after the first draw. It is the
//! seed for the eventual `SelectableMarkdown` element.
//!
//! ## How first-frame hit testing works (no sink, no phantom)
//!
//! `TextLayout` is `Rc<RefCell<Option<TextLayoutInner>>>`. When this element
//! builds its real `StyledText` child it clones that shared handle *before*
//! driving the child through the standard `Element` lifecycle. The parent then
//! runs the real child directly:
//!
//! 1. `request_layout` — measures the real child (populates line layout/size).
//! 2. `prepaint`        — the real child records its painted `bounds` in the
//!    same shared `TextLayout`.
//! 3. `paint`           — the real child paints itself, then this element
//!    registers a current-frame `MouseDownEvent` listener that hit-tests the
//!    now-fully-populated live layout.
//!
//! Because the layout handle is `Rc`-shared with the painted child, the
//! listener sees measured-and-bound-registered geometry on the first frame.
//! There is no warm-up click, no arming, no deferred replay, no separate
//! phantom layout, and no alternate flat renderer.
//!
//! This is an MIT clean-room implementation using only GPUI public APIs.
//!
//! @plan PLAN-20260713-ISSUE151 Phase 0 (blocking gate)

#[cfg(test)]
mod tests;

use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    MouseDownEvent, Pixels, SharedString, StyledText, TextLayout, TextRun, Window,
};

/// The result of hit-testing a mouse-down against the live layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    /// Byte offset into the rendered text, or `None` when the pointer misses
    /// every glyph.
    pub offset: Option<usize>,
    /// Byte length of the rendered StyledText child's text at hit time. This
    /// proves the hit-tested layout belongs to the real rendered child rather
    /// than a separately constructed phantom.
    pub rendered_len: usize,
}

/// Callback invoked on each mouse-down with the hit-test result.
pub type HitHandler = Box<dyn Fn(Hit) + 'static>;

/// A custom GPUI element owning a real rendered `StyledText` child plus a
/// shared handle to that child's live `TextLayout`.
///
/// Cloning this element clones the immutable text, the (cheap) shared layout
/// handle, and the hit handler. The element is deliberately minimal and free
/// of any selection state — that belongs to the future `SelectableMarkdown`
/// and `ChatView`.
#[derive(Clone)]
pub struct SelectableLeaf {
    text: SharedString,
    runs: Vec<TextRun>,
    /// `Rc`-shared handle to the layout of the actually-painted child. It is
    /// populated during the first frame's `request_layout`.
    layout: TextLayout,
    on_hit: Option<std::sync::Arc<HitHandler>>,
}

impl SelectableLeaf {
    /// Construct a selectable leaf with plain (single-run) text.
    #[must_use]
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            runs: Vec::new(),
            layout: TextLayout::default(),
            on_hit: None,
        }
    }

    /// Provide custom text runs for the real child. Used in later phases to
    /// render rich inline styles without flattening to plain text.
    #[must_use]
    pub fn with_runs(mut self, runs: Vec<TextRun>) -> Self {
        self.runs = runs;
        self
    }

    /// Register the current-frame mouse-down hit handler.
    #[must_use]
    pub fn on_mouse_down_hit<F>(mut self, handler: F) -> Self
    where
        F: Fn(Hit) + 'static,
    {
        self.on_hit = Some(std::sync::Arc::new(Box::new(handler)));
        self
    }

    /// Build the real `StyledText` child and clone its live layout so this
    /// element shares geometry with the painted child.
    fn build_child(&self) -> StyledText {
        let mut styled = StyledText::new(self.text.clone());
        if !self.runs.is_empty() {
            styled = styled.with_runs(self.runs.clone());
        }
        styled
    }
}

impl IntoElement for SelectableLeaf {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Per-frame layout state: the built real child for the current frame.
///
/// This is an implementation detail exposed only because the `Element` trait
/// requires its associated `RequestLayoutState` to be reachable.
#[doc(hidden)]
pub struct LayoutState {
    child: StyledText,
}

impl Element for SelectableLeaf {
    type RequestLayoutState = LayoutState;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
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
        let mut child = self.build_child();
        // Capture the live layout handle *before* measuring the child. This is
        // an Rc-clone of the same handle the painted child populates.
        self.layout = child.layout().clone();

        let layout_id = Element::request_layout(&mut child, None, None, window, cx).0;
        (layout_id, LayoutState { child })
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
        _id: Option<&GlobalElementId>,
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

        // Register a current-frame mouse-down listener against the live layout
        // shared with the just-painted child. Because `prepaint` already set
        // the child's bounds on this same handle, hit testing succeeds on the
        // very first interaction.
        let Some(handler) = self.on_hit.clone() else {
            return;
        };
        let layout = self.layout.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, _window, _cx| {
            if !phase.bubble() || event.button != gpui::MouseButton::Left {
                return;
            }
            let local = event.position - bounds.origin;
            let offset = match layout.index_for_position(local) {
                Ok(idx) | Err(idx) => Some(idx),
            };
            let rendered_len = layout.len();
            handler(Hit {
                offset,
                rendered_len,
            });
        });
    }
}
