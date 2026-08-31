// NOTICE: PersonalAgent modified this file from gpui-component commit c5ade48.
// Changes: removed the upstream Theme dependency, require explicit surface
// colors, build one StyledText from caller-provided runs, register scroll/copy
// metadata, recolor selected glyphs in the surface background color, and retain
// one owning-window refresh subscription per selectable-text element.

use std::ops::Range;

use gpui::{
    transparent_black, App, BorderStyle, Bounds, Corners, Edges, Element, ElementId,
    GlobalElementId, Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId,
    PaintQuad, Pixels, Point, SharedString, StyledText, Subscription, TextRun, Window,
};

use crate::{TextSelectionHandle, TextSelectionRegistration, TextSelectionRun};

/// Styled text that participates in window-scoped text selection.
pub struct SelectableText {
    id: ElementId,
    text: SharedString,
    styled_text: Option<StyledText>,
    text_runs: Vec<TextRun>,
    document_order: u64,
    scroll_offset: Point<Pixels>,
    copy_separator_before: SharedString,
    selection_color: Hsla,
    selected_text_color: Hsla,
}

/// Layout state retained between the element lifecycle phases.
#[doc(hidden)]
pub struct SelectableTextLayoutState {
    handle: TextSelectionHandle,
    selected_ranges: Vec<Option<Range<usize>>>,
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
        Self {
            id: id.into(),
            text: text.into(),
            styled_text: None,
            text_runs,
            document_order: 0,
            scroll_offset: Point::default(),
            copy_separator_before: "\n".into(),
            selection_color: surface_foreground,
            selected_text_color: surface_background,
        }
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
    }
    result.push(run);
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
        let handle = window.with_element_state(
            global_id.expect("SelectableText must have a stable element id"),
            |retained: Option<(TextSelectionHandle, Subscription)>, window| {
                let state = retained.unwrap_or_else(|| {
                    let handle = TextSelectionHandle::new(self.text.clone(), cx);
                    let subscription = handle.refresh_window_on_change(window, cx);
                    (handle, subscription)
                });
                (state.0.clone(), state)
            },
        );
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
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let styled_text = self
            .styled_text
            .as_mut()
            .expect("SelectableText must complete prepaint before paint");
        let layout = styled_text.layout().clone();
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
    fn selected_run_recolor_preserves_every_other_property() {
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
}
