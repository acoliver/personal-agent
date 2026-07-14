//! Composer text element with live-layout pointer selection.

use std::sync::Arc;

use gpui::{
    fill, App, Bounds, DispatchPhase, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, SharedString, StyledText, TextLayout, Window,
};

use crate::ui_gpui::theme::Theme;

use super::composer_selection::{clamp_to_char_boundary, ComposerSelection};

pub(super) struct LayoutState {
    child: StyledText,
}

#[derive(Clone)]
pub(super) struct ComposerFieldConfig {
    pub text: SharedString,
    pub selection: ComposerSelection,
    pub line_height: Pixels,
    pub show_caret: bool,
}

type SelectionHandler = Arc<dyn Fn(ComposerSelection, &mut Window, &mut App)>;

pub(super) struct ComposerField {
    config: ComposerFieldConfig,
    layout: TextLayout,
    on_selection_change: Option<SelectionHandler>,
    interactive: bool,
}

impl ComposerField {
    pub(super) fn new(config: ComposerFieldConfig) -> Self {
        Self {
            config,
            layout: TextLayout::default(),
            on_selection_change: None,
            interactive: true,
        }
    }

    pub(super) fn on_selection_change<F>(mut self, handler: F) -> Self
    where
        F: Fn(ComposerSelection, &mut Window, &mut App) + 'static,
    {
        self.on_selection_change = Some(Arc::new(handler));
        self
    }

    pub(super) const fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }
}

impl IntoElement for ComposerField {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ComposerField {
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
        let selection = self.config.selection.clamped(&self.config.text);
        let mut child = StyledText::new(self.config.text.clone());
        if !selection.is_collapsed() {
            child = child.with_highlights([(
                selection.start()..selection.end(),
                gpui::HighlightStyle {
                    color: Some(Theme::selection_fg()),
                    ..Default::default()
                },
            )]);
        }
        let layout_id = Element::request_layout(&mut child, None, None, window, cx).0;
        self.layout = child.layout().clone();
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
        self.paint_selection(window);
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
        self.paint_caret(bounds, window);
        self.register_pointer_listeners(bounds, window);
    }
}

impl ComposerField {
    fn paint_selection(&self, window: &mut Window) {
        if self.config.selection.is_collapsed() {
            return;
        }
        let text = self.layout.text();
        let selection = self.config.selection.clamped(&text);
        let range = selection.start()..selection.end();
        for bounds in selection_bounds(&self.layout, range, self.config.line_height) {
            window.paint_quad(fill(bounds, Theme::selection_bg()));
        }
    }

    fn paint_caret(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        if !self.config.show_caret || !self.config.selection.is_collapsed() {
            return;
        }
        let text = self.layout.text();
        let offset = clamp_to_char_boundary(&text, self.config.selection.head);
        let origin = self
            .layout
            .position_for_index(offset)
            .unwrap_or(bounds.origin);
        let caret = Bounds::from_corners(
            origin,
            Point::new(origin.x + gpui::px(1.0), origin.y + self.config.line_height),
        );
        window.paint_quad(fill(caret, Theme::text_primary()));
    }

    fn register_pointer_listeners(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        if !self.interactive {
            return;
        }
        let Some(handler) = self.on_selection_change.clone() else {
            return;
        };
        let text = self.config.text.clone();
        let layout = self.layout.clone();
        let down_handler = Arc::clone(&handler);
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble
                || event.button != MouseButton::Left
                || !bounds.contains(&event.position)
            {
                return;
            }
            let offset = hit_test(&layout, event.position, text.as_ref());
            down_handler(ComposerSelection::caret(offset), window, cx);
        });

        let text = self.config.text.clone();
        let layout = self.layout.clone();
        let anchor = self.config.selection.anchor;
        let move_handler = Arc::clone(&handler);
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble || event.pressed_button != Some(MouseButton::Left) {
                return;
            }
            let point = Point::new(
                event.position.x.max(bounds.left()).min(bounds.right()),
                event.position.y.max(bounds.top()).min(bounds.bottom()),
            );
            let offset = hit_test(&layout, point, text.as_ref());
            move_handler(ComposerSelection::new(anchor, offset), window, cx);
        });

        let text = self.config.text.clone();
        let layout = self.layout.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
                return;
            }
            let point = Point::new(
                event.position.x.max(bounds.left()).min(bounds.right()),
                event.position.y.max(bounds.top()).min(bounds.bottom()),
            );
            let offset = hit_test(&layout, point, text.as_ref());
            handler(ComposerSelection::new(anchor, offset), window, cx);
        });
    }
}

fn hit_test(layout: &TextLayout, position: Point<Pixels>, text: &str) -> usize {
    let offset = match layout.index_for_position(position) {
        Ok(index) | Err(index) => index,
    };
    clamp_to_char_boundary(text, offset)
}

fn selection_bounds(
    layout: &TextLayout,
    range: std::ops::Range<usize>,
    line_height: Pixels,
) -> Vec<Bounds<Pixels>> {
    let text = layout.text();
    let layout_bounds = layout.bounds();
    let mut result: Vec<Bounds<Pixels>> = Vec::new();
    for (relative, ch) in text[range.clone()].char_indices() {
        if ch == '\n' {
            continue;
        }
        let start = range.start + relative;
        let end = start + ch.len_utf8();
        let (Some(start_position), Some(end_position)) = (
            layout.position_for_index(start),
            layout.position_for_index(end),
        ) else {
            continue;
        };
        let wrapped = (end_position.y - start_position.y).abs() >= line_height * 0.5;
        let (origin, right) = if wrapped {
            (
                Point::new(layout_bounds.left(), end_position.y),
                end_position.x,
            )
        } else {
            (start_position, end_position.x)
        };
        if right <= origin.x {
            continue;
        }
        if let Some(previous) = result.last_mut() {
            let same_line = (previous.top() - origin.y).abs() < line_height * 0.5;
            if same_line && origin.x <= previous.right() + gpui::px(1.0) {
                previous.size.width = right - previous.left();
                continue;
            }
        }
        result.push(Bounds::from_corners(
            origin,
            Point::new(right, origin.y + line_height),
        ));
    }
    result
}
