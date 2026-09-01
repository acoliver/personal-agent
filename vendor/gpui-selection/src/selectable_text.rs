// NOTICE: PersonalAgent modified this file from gpui-component commit c5ade48.
// Changes: removed the upstream Theme dependency, require explicit surface
// colors, build one StyledText from caller-provided runs, register scroll/copy
// metadata, recolor selected glyphs in the surface background color, and retain
// one owning-window refresh subscription per selectable-text element. Selected
// runs now suppress their own backgrounds, and low-contrast surface pairs use a
// black-or-white selected glyph fallback. Safe links now activate on clicks while
// pointer movement beyond the drag threshold remains a text-selection gesture,
// participant-defined content keys are attached to selection endpoints, and
// callers can install source-copy projections for virtualized text.

use std::{cell::RefCell, ops::Range, rc::Rc};

use gpui::{
    hsla, transparent_black, App, BorderStyle, Bounds, Corners, CursorStyle, DispatchPhase, Edges,
    Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior, Hsla, InspectorElementId,
    IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad,
    Pixels, Point, SharedString, StyledText, Subscription, TextLayout, TextRun, Window,
};

use crate::{
    GlobalState, TextSelection, TextSelectionContentKey, TextSelectionHandle,
    TextSelectionRegistration, TextSelectionRun,
};

type SourceCopyCallback =
    dyn Fn(Option<crate::TextSelectionSnapshot>, Option<String>, &mut App) -> Option<String>;

/// Styled text that participates in window-scoped text selection.
pub struct SelectableText {
    id: ElementId,
    text: SharedString,
    styled_text: Option<StyledText>,
    links: Vec<(Range<usize>, String)>,
    text_runs: Vec<TextRun>,
    document_order: u64,
    scroll_offset: Point<Pixels>,
    copy_separator_before: SharedString,
    content_key: Option<TextSelectionContentKey>,
    copy: Option<Rc<SourceCopyCallback>>,
    selection_color: Hsla,
    selected_text_color: Hsla,
}

/// Layout state retained between the element lifecycle phases.
#[doc(hidden)]
pub struct SelectableTextLayoutState {
    handle: TextSelectionHandle,
    selected_ranges: Vec<Option<Range<usize>>>,
    link_press: Rc<RefCell<Option<LinkPress>>>,
}

impl SelectableText {
    /// Creates a selectable leaf from plain text and its exact styling runs.
    pub fn new(
        id: impl Into<ElementId>,
        text: impl Into<SharedString>,
        text_runs: Vec<TextRun>,
        surface_background: Hsla,
        surface_foreground: Hsla,
    ) -> Self {
        let (selection_color, selected_text_color) =
            legible_selection_colors(surface_foreground, surface_background);
        Self {
            id: id.into(),
            text: text.into(),
            styled_text: None,
            links: Vec::new(),
            text_runs,
            document_order: 0,
            scroll_offset: Point::default(),
            copy_separator_before: "\n".into(),
            content_key: None,
            copy: None,
            selection_color,
            selected_text_color,
        }
    }

    /// Adds safe URL targets keyed by byte ranges into the element's text.
    pub fn links(mut self, links: Vec<(Range<usize>, String)>) -> Self {
        self.links = links;
        self
    }

    /// Places this leaf in reading order among all window participants.
    pub fn document_order(mut self, order: u64) -> Self {
        self.document_order = order;
        self
    }

    /// Records the containing scroll view's current content offset.
    pub fn scroll_offset(mut self, offset: Point<Pixels>) -> Self {
        self.scroll_offset = offset;
        self
    }

    /// Sets the visible-text separator inserted before this leaf when copied
    /// after another selected leaf.
    pub fn copy_separator_before(mut self, separator: impl Into<SharedString>) -> Self {
        self.copy_separator_before = separator.into();
        self
    }

    /// Attaches participant-defined stable content identity to selection endpoints.
    pub fn content_key(mut self, content_key: TextSelectionContentKey) -> Self {
        self.content_key = Some(content_key);
        self
    }

    /// Exports source text, including virtualized content that is not painted.
    pub fn copy_with(
        mut self,
        callback: impl Fn(Option<crate::TextSelectionSnapshot>, Option<String>, &mut App) -> Option<String>
            + 'static,
    ) -> Self {
        self.copy = Some(Rc::new(callback));
        self
    }

    fn paint_selection(
        layout: &gpui::TextLayout,
        range: Range<usize>,
        color: Hsla,
        window: &mut Window,
    ) {
        let (Some(start), Some(end)) = (
            layout.position_for_index(range.start),
            layout.position_for_index(range.end),
        ) else {
            return;
        };
        for bounds in selection_quad_bounds(start, end, layout.bounds(), layout.line_height()) {
            window.paint_quad(PaintQuad {
                bounds,
                background: color.into(),
                corner_radii: Corners::default(),
                border_widths: Edges::default(),
                border_color: transparent_black(),
                border_style: BorderStyle::default(),
            });
        }
    }
}

const LINK_DRAG_THRESHOLD: f64 = 2.0;

struct LinkPress {
    position: Point<Pixels>,
    link_index: usize,
    dragged: bool,
}

impl LinkPress {
    fn new(position: Point<Pixels>, link_index: usize) -> Self {
        Self {
            position,
            link_index,
            dragged: false,
        }
    }

    fn update(&mut self, position: Point<Pixels>) -> bool {
        let crossed_threshold =
            !self.dragged && (position - self.position).magnitude() > LINK_DRAG_THRESHOLD;
        self.dragged |= crossed_threshold;
        crossed_threshold
    }

    fn finish(mut self, position: Point<Pixels>, link_index: Option<usize>) -> Option<usize> {
        self.update(position);
        (!self.dragged && link_index == Some(self.link_index)).then_some(self.link_index)
    }
}

fn link_at_position(
    layout: &TextLayout,
    links: &[(Range<usize>, String)],
    position: Point<Pixels>,
) -> Option<usize> {
    let byte_index = layout.index_for_position(position).ok()?;
    links
        .iter()
        .position(|(range, _)| range.contains(&byte_index))
}

const MIN_SELECTION_CONTRAST: f32 = 4.5;

fn legible_selection_colors(quad: Hsla, glyph: Hsla) -> (Hsla, Hsla) {
    if contrast_ratio(quad, glyph) >= MIN_SELECTION_CONTRAST {
        return (quad, glyph);
    }

    let black = hsla(0.0, 0.0, 0.0, 1.0);
    let white = hsla(0.0, 0.0, 1.0, 1.0);
    let glyph = if contrast_ratio(quad, black) >= contrast_ratio(quad, white) {
        black
    } else {
        white
    };
    (quad, glyph)
}

fn contrast_ratio(first: Hsla, second: Hsla) -> f32 {
    let first = relative_luminance(first);
    let second = relative_luminance(second);
    let lighter = first.max(second);
    let darker = first.min(second);
    (lighter + 0.05) / (darker + 0.05)
}

fn relative_luminance(color: Hsla) -> f32 {
    let rgba = color.to_rgb();
    0.2126 * linearized(rgba.r) + 0.7152 * linearized(rgba.g) + 0.0722 * linearized(rgba.b)
}

fn linearized(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn selection_quad_bounds(
    start: Point<Pixels>,
    end: Point<Pixels>,
    bounds: Bounds<Pixels>,
    line_height: Pixels,
) -> Vec<Bounds<Pixels>> {
    if start.y == end.y {
        return vec![Bounds::from_corners(
            start,
            Point::new(end.x, end.y + line_height),
        )];
    }

    let mut quads = vec![Bounds::from_corners(
        start,
        Point::new(bounds.right(), start.y + line_height),
    )];
    if end.y > start.y + line_height {
        quads.push(Bounds::from_corners(
            Point::new(bounds.left(), start.y + line_height),
            Point::new(bounds.right(), end.y),
        ));
    }
    quads.push(Bounds::from_corners(
        Point::new(bounds.left(), end.y),
        Point::new(end.x, end.y + line_height),
    ));
    quads
}

fn recolor_selected_runs(
    runs: &[TextRun],
    selected: Option<&Range<usize>>,
    selected_text_color: Hsla,
) -> Vec<TextRun> {
    let Some(selected) = selected.filter(|range| !range.is_empty()) else {
        return runs.to_vec();
    };
    let mut offset = 0;
    let mut result = Vec::with_capacity(runs.len().saturating_mul(3));
    for run in runs {
        let run_end = offset + run.len;
        let selected_start = offset.max(selected.start).min(run_end);
        let selected_end = offset.max(selected.end).min(run_end);
        append_run_segment(&mut result, run, offset, selected_start, None);
        append_run_segment(
            &mut result,
            run,
            selected_start,
            selected_end,
            Some(selected_text_color),
        );
        append_run_segment(&mut result, run, selected_end, run_end, None);
        offset = run_end;
    }
    result
}

fn append_run_segment(
    result: &mut Vec<TextRun>,
    source: &TextRun,
    start: usize,
    end: usize,
    color: Option<Hsla>,
) {
    if end <= start {
        return;
    }
    let mut run = source.clone();
    run.len = end - start;
    if let Some(color) = color {
        run.color = color;
        run.background_color = None;
    }
    result.push(run);
}

fn register_link_handlers(
    links: Rc<Vec<(Range<usize>, String)>>,
    layout: TextLayout,
    hitbox: Hitbox,
    link_press: Rc<RefCell<Option<LinkPress>>>,
    window: &mut Window,
) {
    let mouse_down_links = links.clone();
    let mouse_down_layout = layout.clone();
    let mouse_down_hitbox = hitbox.clone();
    let mouse_down_press = link_press.clone();
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble
            || event.button != MouseButton::Left
            || event.click_count != 1
        {
            return;
        }
        let link_index = if mouse_down_hitbox.is_hovered(window) {
            link_at_position(&mouse_down_layout, &mouse_down_links, event.position)
        } else {
            None
        };
        *mouse_down_press.borrow_mut() =
            link_index.map(|index| LinkPress::new(event.position, index));
        if link_index.is_some() {
            GlobalState::suppress_text_selection(cx);
        }
    });

    let mouse_move_press = link_press.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble || !event.dragging() {
            return;
        }
        let drag_anchor = mouse_move_press
            .borrow_mut()
            .as_mut()
            .and_then(|press| press.update(event.position).then_some(press.position));
        if let Some(anchor) = drag_anchor {
            GlobalState::reset_text_selection_suppression(cx);
            TextSelection::begin_drag(anchor, event.position, window, cx);
        }
    });

    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
            return;
        }
        let Some(press) = link_press.borrow_mut().take() else {
            return;
        };
        let release_link = if hitbox.is_hovered(window) {
            link_at_position(&layout, &links, event.position)
        } else {
            None
        };
        if let Some(index) = press.finish(event.position, release_link) {
            cx.open_url(&links[index].1);
        }
    });
}

impl IntoElement for SelectableText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SelectableText {
    type RequestLayoutState = SelectableTextLayoutState;
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let (handle, link_press) = window.with_element_state(
            global_id.expect("SelectableText must have a stable element id"),
            |retained: Option<(
                TextSelectionHandle,
                Subscription,
                Rc<RefCell<Option<LinkPress>>>,
            )>,
             window| {
                let state = retained.unwrap_or_else(|| {
                    let handle = TextSelectionHandle::new(self.text.clone(), cx);
                    let subscription = handle.refresh_window_on_change(window, cx);
                    (handle, subscription, Rc::default())
                });
                ((state.0.clone(), state.2.clone()), state)
            },
        );
        if let Some(content_key) = self.content_key {
            handle.resolve_content_key_with(move |_, _| Some(content_key), cx);
        }
        if let Some(copy) = self.copy.clone() {
            handle.copy_projection_with(
                move |snapshot, projected, cx| copy(snapshot, projected, cx),
                cx,
            );
        }
        let projection = handle.project_cached_runs(cx);
        let selected_ranges = projection.ranges().to_vec();
        let selected_range = selected_ranges.first().and_then(Option::as_ref);
        self.styled_text = Some(StyledText::new(self.text.clone()).with_runs(
            recolor_selected_runs(&self.text_runs, selected_range, self.selected_text_color),
        ));
        let styled_text = self
            .styled_text
            .as_mut()
            .expect("SelectableText creates its StyledText before layout");
        let (layout_id, ()) = styled_text.request_layout(global_id, inspector_id, window, cx);
        (
            layout_id,
            SelectableTextLayoutState {
                handle,
                selected_ranges,
                link_press,
            },
        )
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let styled_text = self
            .styled_text
            .as_mut()
            .expect("SelectableText must complete layout before prepaint");
        styled_text.prepaint(global_id, inspector_id, bounds, &mut (), window, cx);
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        state.handle.register(
            TextSelectionRegistration::new(hitbox.clone(), bounds)
                .with_document_order(self.document_order)
                .with_scroll_offset(self.scroll_offset)
                .with_text_bounds(vec![styled_text.layout().bounds()])
                .with_copy_separator_before(self.copy_separator_before.to_string()),
            window,
            cx,
        );
        hitbox
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let styled_text = self
            .styled_text
            .as_mut()
            .expect("SelectableText must complete prepaint before paint");
        let layout = styled_text.layout().clone();
        if !self.links.is_empty() {
            if link_at_position(&layout, &self.links, window.mouse_position()).is_some() {
                window.set_cursor_style(CursorStyle::PointingHand, hitbox);
            }
            register_link_handlers(
                Rc::new(self.links.clone()),
                layout.clone(),
                hitbox.clone(),
                state.link_press.clone(),
                window,
            );
        }
        let projection = state.handle.update_runs(
            &[
                TextSelectionRun::new(self.text.clone(), layout.clone(), bounds)
                    .with_document_order(self.document_order),
            ],
            cx,
        );
        if projection.ranges() != state.selected_ranges {
            window.refresh();
        }
        for range in projection.ranges().iter().flatten().cloned() {
            Self::paint_selection(&layout, range, self.selection_color, window);
        }
        styled_text.paint(
            global_id,
            inspector_id,
            bounds,
            &mut (),
            &mut (),
            window,
            cx,
        );
    }
}

#[cfg(test)]
mod tests {
    use gpui::{
        font, hsla, point, px, size, Bounds, FontFeatures, FontStyle, FontWeight,
        StrikethroughStyle, TextRun, UnderlineStyle,
    };

    #[test]
    fn wrapped_selection_paints_full_width_middle_lines() {
        let bounds = Bounds::new(point(px(10.), px(20.)), size(px(100.), px(100.)));
        let quads = super::selection_quad_bounds(
            point(px(40.), px(20.)),
            point(px(30.), px(80.)),
            bounds,
            px(20.),
        );

        assert_eq!(
            quads,
            vec![
                Bounds::from_corners(point(px(40.), px(20.)), point(px(110.), px(40.))),
                Bounds::from_corners(point(px(10.), px(40.)), point(px(110.), px(80.))),
                Bounds::from_corners(point(px(10.), px(80.)), point(px(30.), px(100.))),
            ]
        );
    }

    #[test]
    fn selected_run_recolor_removes_the_run_background() {
        let mut source = TextRun {
            len: 10,
            font: font("Inline Code"),
            color: hsla(0.1, 0.2, 0.3, 1.0),
            background_color: Some(hsla(0.4, 0.5, 0.6, 1.0)),
            underline: Some(UnderlineStyle {
                thickness: px(1.5),
                color: Some(hsla(0.2, 0.3, 0.4, 1.0)),
                wavy: true,
            }),
            strikethrough: Some(StrikethroughStyle {
                thickness: px(2.0),
                color: Some(hsla(0.3, 0.4, 0.5, 1.0)),
            }),
        };
        source.font.weight = FontWeight::BOLD;
        source.font.style = FontStyle::Italic;
        source.font.features = FontFeatures::disable_ligatures();
        let selected_color = hsla(0.7, 0.8, 0.9, 1.0);
        let runs = super::recolor_selected_runs(&[source.clone()], Some(&(2..7)), selected_color);

        let mut before = source.clone();
        before.len = 2;
        let mut selected = source.clone();
        selected.len = 5;
        selected.color = selected_color;
        selected.background_color = None;
        let mut after = source.clone();
        after.len = 3;
        assert_eq!(runs, [before, selected, after]);
    }

    #[test]
    fn absent_selection_returns_original_runs() {
        let source = TextRun {
            len: 4,
            color: hsla(0.1, 0.2, 0.3, 1.0),
            ..Default::default()
        };

        assert_eq!(
            super::recolor_selected_runs(&[source.clone()], None, hsla(0., 0., 0., 1.)),
            [source]
        );
    }

    #[test]
    fn low_contrast_selection_uses_black_or_white_glyphs() {
        let black = hsla(0.0, 0.0, 0.0, 1.0);
        let (quad, glyph) = super::legible_selection_colors(black, black);

        assert_eq!(quad, black);
        assert_eq!(glyph, hsla(0.0, 0.0, 1.0, 1.0));
        assert!(super::contrast_ratio(quad, glyph) >= super::MIN_SELECTION_CONTRAST);
    }

    #[test]
    fn contrasting_surface_selection_pair_is_preserved() {
        let quad = hsla(0.28, 0.29, 0.47, 1.0);
        let glyph = hsla(0.0, 0.0, 0.0, 1.0);

        assert_eq!(super::legible_selection_colors(quad, glyph), (quad, glyph));
    }

    #[test]
    fn link_press_activates_only_without_dragging() {
        let mut click = super::LinkPress::new(point(px(10.), px(10.)), 2);

        assert!(!click.update(point(px(12.), px(10.))));
        assert_eq!(click.finish(point(px(12.), px(10.)), Some(2)), Some(2));

        let mut drag = super::LinkPress::new(point(px(10.), px(10.)), 2);
        assert!(drag.update(point(px(12.1), px(10.))));
        assert_eq!(drag.finish(point(px(10.), px(10.)), Some(2)), None);

        let other_link = super::LinkPress::new(point(px(10.), px(10.)), 2);
        assert_eq!(other_link.finish(point(px(10.), px(10.)), Some(3)), None);
    }
}
