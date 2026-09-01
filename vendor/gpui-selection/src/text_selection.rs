// NOTICE: PersonalAgent modified this file from gpui-component commit c5ade48.
// Changes: adjusted crate-relative imports, rewrote Rust 2024 let chains for this
// project's Rust 2021 edition, routed window auto-scroll through participant
// callbacks because pinned GPUI exposes a private return type from
// Window::dispatch_event, added participant-defined copy separators, exposed
// cached run projection, added delayed drag start for selectable links, exposed
// window-selection endpoint content keys, and gated tests requiring newer GPUI
// test-support APIs. Virtualized selected participants are strongly retained with
// their copy callbacks after paint registration ends; one virtual endpoint gets
// synthetic projection geometry, and unresolved callbacks fail the whole copy.
// Auto-scroll commands are also published through a window-scoped source that
// outlives transient participants, drag updates are gated on an active gesture,
// and a stationary-drag tick advances the endpoint at the window pointer.
// Registrations can carry a stable content key, `TextSelection::select_all`
// installs a window-owned logical whole-content selection matched by that key
// with a frozen copy text, `TextSelection::selected_content_keys` reports every
// key a copy depends on, and full-coverage snapshots project every run byte
// without endpoint geometry while logical participants are never retained in
// the virtual copy set. Painted runs now also cache under geometry-free
// snapshots so participants remounting into an active logical selection still
// converge their layout-side projection instead of repainting forever. A held
// double-click now enters word-drag mode: both original word endpoints are
// retained, every drag update snaps to whole UAX #29 word segments (forward,
// reverse, cross-participant, and virtualized), geometric outside-text drags
// extend only to edges covering the whole terminal run of the hit
// participant, and deferred endpoint content keys are synchronized into the
// retained originals.

use std::{
    cmp,
    collections::{HashMap, HashSet},
    ops::Range,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use gpui::{
    point, px, App, AppContext as _, Bounds, Context, Element, ElementId, Entity, EntityId,
    EventEmitter, Global, GlobalElementId, Half, Hitbox, InspectorElementId, IntoElement, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollWheelEvent,
    SharedString, Style, Subscription, TextLayout, WeakEntity, Window,
};

use super::text_boundary::{line_range_at, word_range_at, WordSegments};
use super::{AutoScroll, GlobalState};

/// An opaque selection layer identifier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextSelectionScopeId(u64);

impl TextSelectionScopeId {
    /// Allocates a process-unique scope identifier.
    ///
    /// Keep the returned identifier for the semantic lifetime of the scope;
    /// do not allocate a new identifier on every frame.
    pub fn new() -> Self {
        static NEXT_SCOPE_ID: AtomicU64 = AtomicU64::new(1);
        let value = NEXT_SCOPE_ID
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .expect("text selection scope identifiers exhausted");
        Self(value)
    }

    #[cfg(test)]
    const fn from_raw(value: u64) -> Self {
        Self(value)
    }
}

/// Stable participant-defined identity for virtualized participant content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextSelectionContentKey(u64);

impl TextSelectionContentKey {
    /// Creates a key from a participant-defined stable content identity.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the participant-defined value.
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// A selection endpoint anchored to a participant's content coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextSelectionEndpoint {
    entity_id: Option<EntityId>,
    point: Point<Pixels>,
    content_key: Option<TextSelectionContentKey>,
}

impl TextSelectionEndpoint {
    /// Creates an endpoint at a participant-relative content point.
    pub(crate) const fn new(entity_id: Option<EntityId>, point: Point<Pixels>) -> Self {
        Self {
            entity_id,
            point,
            content_key: None,
        }
    }

    /// Sets participant-defined endpoint metadata.
    pub(crate) const fn with_content_key(mut self, content_key: TextSelectionContentKey) -> Self {
        self.content_key = Some(content_key);
        self
    }

    /// Returns the participant which owns this endpoint, when it hit one.
    pub const fn entity_id(&self) -> Option<EntityId> {
        self.entity_id
    }

    /// Returns the participant-relative content point.
    pub const fn content_point(&self) -> Point<Pixels> {
        self.point
    }

    /// Returns participant-defined endpoint metadata captured when it hit a participant.
    pub const fn content_key(&self) -> Option<TextSelectionContentKey> {
        self.content_key
    }
}

/// Window-coordinate anchor and cursor points for painting a selection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextSelectionWindowPoints {
    anchor: Point<Pixels>,
    cursor: Point<Pixels>,
}

impl TextSelectionWindowPoints {
    /// Returns the stable anchor in window coordinates.
    pub const fn anchor(&self) -> Point<Pixels> {
        self.anchor
    }

    /// Returns the moving cursor in window coordinates.
    pub const fn cursor(&self) -> Point<Pixels> {
        self.cursor
    }
}

/// Participant-relative selection endpoints with an optional rendering projection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextSelectionSnapshot {
    anchor: TextSelectionEndpoint,
    cursor: TextSelectionEndpoint,
    is_selecting: bool,
    window_points: Option<TextSelectionWindowPoints>,
    coverage: TextSelectionCoverage,
}

/// How much of one participant participates in a window selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextSelectionCoverage {
    /// Only the interval between this participant's two endpoints is selected.
    #[default]
    Bounded,
    /// The participant is selected from its beginning through its endpoint.
    FromStart,
    /// The participant is selected from its endpoint through its end.
    ToEnd,
    /// The entire participant lies between endpoints in other participants.
    Full,
}

impl TextSelectionSnapshot {
    /// Creates a snapshot from stable participant-relative endpoints.
    pub(crate) const fn new(anchor: TextSelectionEndpoint, cursor: TextSelectionEndpoint) -> Self {
        Self {
            anchor,
            cursor,
            is_selecting: false,
            window_points: None,
            coverage: TextSelectionCoverage::Bounded,
        }
    }

    /// Sets whether the pointer gesture is still active.
    pub(crate) const fn with_selecting(mut self, is_selecting: bool) -> Self {
        self.is_selecting = is_selecting;
        self
    }

    /// Sets the current window-coordinate rendering projection.
    pub(crate) const fn with_window_points(
        mut self,
        window_points: Option<TextSelectionWindowPoints>,
    ) -> Self {
        self.window_points = window_points;
        self
    }

    /// Sets the portion of the receiving participant covered by this selection.
    pub(crate) const fn with_coverage(mut self, coverage: TextSelectionCoverage) -> Self {
        self.coverage = coverage;
        self
    }

    /// Returns the stable anchor endpoint.
    pub const fn anchor(&self) -> TextSelectionEndpoint {
        self.anchor
    }

    /// Returns the moving cursor endpoint.
    pub const fn cursor(&self) -> TextSelectionEndpoint {
        self.cursor
    }

    /// Returns whether the pointer gesture is still active.
    pub const fn is_selecting(&self) -> bool {
        self.is_selecting
    }

    /// Returns the window-coordinate endpoints for participants that need them.
    pub const fn window_points(&self) -> Option<TextSelectionWindowPoints> {
        self.window_points
    }

    /// Returns the portion of the receiving participant covered by this selection.
    pub const fn coverage(&self) -> TextSelectionCoverage {
        self.coverage
    }
}

/// Per-frame geometry reported by a [`TextSelectionHandle`] participant.
pub struct TextSelectionRegistration {
    hitbox: Hitbox,
    bounds: Bounds<Pixels>,
    scroll_offset: Point<Pixels>,
    scope: TextSelectionScopeId,
    document_order: u64,
    text_bounds: Vec<Bounds<Pixels>>,
    copy_separator_before: String,
    content_key: Option<TextSelectionContentKey>,
}

impl TextSelectionRegistration {
    /// Creates a registration with default scope, order, and scroll offset.
    pub fn new(hitbox: Hitbox, bounds: Bounds<Pixels>) -> Self {
        Self {
            hitbox,
            bounds,
            scroll_offset: Point::default(),
            scope: TextSelectionScopeId::default(),
            document_order: 0,
            text_bounds: Vec::new(),
            copy_separator_before: "\n".to_string(),
            content_key: None,
        }
    }

    /// Sets the participant's content scroll offset.
    pub fn with_scroll_offset(mut self, scroll_offset: Point<Pixels>) -> Self {
        self.scroll_offset = scroll_offset;
        self
    }

    /// Sets the opaque selection scope.
    pub fn with_scope(mut self, scope: TextSelectionScopeId) -> Self {
        self.scope = scope;
        self
    }

    /// Sets the stable logical document order.
    pub fn with_document_order(mut self, document_order: u64) -> Self {
        self.document_order = document_order;
        self
    }

    /// Sets the glyph-bearing bounds used to reject blank-only gestures.
    pub fn with_text_bounds(mut self, text_bounds: Vec<Bounds<Pixels>>) -> Self {
        self.text_bounds = text_bounds;
        self
    }

    /// Sets the text inserted before this participant when earlier selected
    /// participants precede it in document order.
    pub fn with_copy_separator_before(mut self, separator: impl Into<String>) -> Self {
        self.copy_separator_before = separator.into();
        self
    }

    /// Attaches the participant's stable content identity.
    ///
    /// Logical whole-content selections match registered participants by
    /// this key, so virtualized participants rejoin when they remount with
    /// the same content.
    pub const fn with_content_key(mut self, content_key: TextSelectionContentKey) -> Self {
        self.content_key = Some(content_key);
        self
    }

    /// Returns the participant hitbox.
    pub fn hitbox(&self) -> &Hitbox {
        &self.hitbox
    }

    /// Returns the participant's window-coordinate bounds.
    pub const fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    /// Returns the participant's content scroll offset.
    pub const fn scroll_offset(&self) -> Point<Pixels> {
        self.scroll_offset
    }

    /// Returns the opaque selection scope.
    pub const fn scope(&self) -> TextSelectionScopeId {
        self.scope
    }

    /// Returns the stable logical document order.
    pub const fn document_order(&self) -> u64 {
        self.document_order
    }

    /// Returns the glyph-bearing bounds used to reject blank-only gestures.
    pub fn text_bounds(&self) -> &[Bounds<Pixels>] {
        &self.text_bounds
    }

    /// Returns the copy separator preceding this participant.
    pub fn copy_separator_before(&self) -> &str {
        &self.copy_separator_before
    }

    /// Returns the participant's stable content identity, when it has one.
    pub const fn content_key(&self) -> Option<TextSelectionContentKey> {
        self.content_key
    }
}

/// Laid-out text reported by a plain selection participant during paint.
#[derive(Clone)]
pub struct TextSelectionRun {
    /// Logical order within the containing participant.
    document_order: u64,
    /// The exact text used to produce `layout`.
    text: SharedString,
    /// Laid-out glyph geometry in window coordinates.
    layout: TextLayout,
    /// The run's window-coordinate paint bounds.
    bounds: Bounds<Pixels>,
}

impl TextSelectionRun {
    /// Creates a laid-out text run.
    pub fn new(text: impl Into<SharedString>, layout: TextLayout, bounds: Bounds<Pixels>) -> Self {
        Self {
            document_order: 0,
            text: text.into(),
            layout,
            bounds,
        }
    }

    /// Sets the run's logical order within the participant.
    pub const fn with_document_order(mut self, document_order: u64) -> Self {
        self.document_order = document_order;
        self
    }

    /// Returns the run's logical order within its participant.
    pub const fn document_order(&self) -> u64 {
        self.document_order
    }

    /// Returns the exact text used to produce the layout.
    pub fn text(&self) -> &SharedString {
        &self.text
    }

    /// Returns the laid-out glyph geometry.
    pub fn layout(&self) -> &TextLayout {
        &self.layout
    }

    /// Returns the run's window-coordinate paint bounds.
    pub const fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }
}

/// Selection projected onto a participant's laid-out text runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextSelectionProjection {
    /// Selected UTF-8 byte ranges paired with the input runs.
    ranges: Vec<Option<Range<usize>>>,
    /// Whether the participant participates in the current selection.
    is_active: bool,
}

impl TextSelectionProjection {
    /// Returns selected UTF-8 byte ranges paired with the input runs.
    pub fn ranges(&self) -> &[Option<Range<usize>>] {
        &self.ranges
    }

    /// Returns whether the participant participates in the selection.
    pub const fn is_active(&self) -> bool {
        self.is_active
    }
}

/// Projects a participant selection snapshot onto laid-out plain-text runs.
///
/// The returned states retain the input order so callers can pair every state
/// with its run. The ranges are always character boundaries; `order` is used
/// only when a participant caches selected text for copying.
fn project_ranges(
    snapshot: Option<TextSelectionSnapshot>,
    runs: &[TextSelectionRun],
) -> TextSelectionProjection {
    let Some(snapshot) = snapshot else {
        return TextSelectionProjection {
            ranges: vec![None; runs.len()],
            is_active: false,
        };
    };
    let Some(window_points) = snapshot.window_points() else {
        // Without endpoint geometry only a whole-content selection knows its
        // extent: every run contributes all of its bytes.
        let ranges = match snapshot.coverage() {
            TextSelectionCoverage::Full => runs.iter().map(|run| Some(0..run.text.len())).collect(),
            TextSelectionCoverage::Bounded
            | TextSelectionCoverage::FromStart
            | TextSelectionCoverage::ToEnd => vec![None; runs.len()],
        };
        return TextSelectionProjection {
            ranges,
            is_active: true,
        };
    };

    TextSelectionProjection {
        ranges: runs
            .iter()
            .map(|run| selection_range_for_run(run, window_points.anchor, window_points.cursor))
            .collect(),
        is_active: true,
    }
}

fn selection_range_for_run(
    run: &TextSelectionRun,
    selection_start: Point<Pixels>,
    selection_end: Point<Pixels>,
) -> Option<Range<usize>> {
    if run.text.len() != run.layout.len() {
        return None;
    }

    let line_height = run.layout.line_height();
    let mut range = None;
    for (offset, character) in run.text.char_indices() {
        let next_offset = offset + character.len_utf8();
        let Some(position) = run.layout.position_for_index(offset) else {
            continue;
        };

        let char_width = run
            .layout
            .position_for_index(next_offset)
            .filter(|next| next.y == position.y)
            .map_or_else(|| line_height.half(), |next| next.x - position.x);

        if point_in_selection_band(
            position,
            char_width,
            selection_start,
            selection_end,
            line_height,
        ) {
            range.get_or_insert(offset..offset).end = next_offset;
        }
    }
    range
}

fn points_for_multi_click(
    runs: &[TextSelectionRun],
    position: Point<Pixels>,
    click_count: usize,
) -> Option<(Point<Pixels>, Point<Pixels>)> {
    let run = runs.iter().find(|run| run.bounds.contains(&position))?;
    if run.text.len() != run.layout.len() {
        return None;
    }
    let offset = run.layout.index_for_position(position).ok()?;
    let range = match click_count {
        2 => word_range_at(&run.text, offset)?,
        3.. => line_range_at(&run.text, offset),
        _ => return None,
    };
    if range.is_empty() {
        return None;
    }
    Some((
        run.layout.position_for_index(range.start)?,
        run.layout.position_for_index(range.end)?,
    ))
}

/// Returns window points at the start of the first run and the end of the
/// last run, in run document order, for whole-run word-drag edges.
fn terminal_run_points(runs: &[TextSelectionRun]) -> Option<(Point<Pixels>, Point<Pixels>)> {
    let first = runs
        .iter()
        .filter(|run| run.text.len() == run.layout.len())
        .min_by_key(|run| run.document_order)?;
    let last = runs
        .iter()
        .filter(|run| run.text.len() == run.layout.len())
        .max_by_key(|run| run.document_order)?;
    Some((
        first.layout.position_for_index(0)?,
        last.layout.position_for_index(last.text.len())?,
    ))
}

fn point_in_selection_band(
    position: Point<Pixels>,
    char_width: Pixels,
    selection_start: Point<Pixels>,
    selection_end: Point<Pixels>,
    line_height: Pixels,
) -> bool {
    let point_in_line =
        |point: Point<Pixels>| point.y >= position.y && point.y < position.y + line_height;
    let top = selection_start.y.min(selection_end.y);
    let bottom = selection_start.y.max(selection_end.y);
    let x = position.x + char_width.half();

    if position.y + line_height <= top || position.y > bottom {
        return false;
    }

    if point_in_line(selection_start) && point_in_line(selection_end) {
        let left = selection_start.x.min(selection_end.x);
        let right = selection_start.x.max(selection_end.x);
        return x >= left && x <= right;
    }

    let (top_point, bottom_point) = if selection_start.y < selection_end.y {
        (selection_start, selection_end)
    } else {
        (selection_end, selection_start)
    };
    if point_in_line(top_point) {
        x >= top_point.x
    } else if point_in_line(bottom_point) {
        x <= bottom_point.x
    } else {
        true
    }
}
fn projection_is_unchanged(
    previous: TextSelectionSnapshot,
    next: TextSelectionSnapshot,
    participant: EntityId,
) -> bool {
    if previous.coverage() != next.coverage() {
        return false;
    }
    match next.coverage() {
        TextSelectionCoverage::Bounded => {
            previous.anchor() == next.anchor() && previous.cursor() == next.cursor()
        }
        TextSelectionCoverage::FromStart | TextSelectionCoverage::ToEnd => {
            let endpoint_for = |snapshot: TextSelectionSnapshot| {
                [snapshot.anchor(), snapshot.cursor()]
                    .into_iter()
                    .find(|endpoint| endpoint.entity_id() == Some(participant))
            };
            endpoint_for(previous) == endpoint_for(next)
        }
        TextSelectionCoverage::Full => true,
    }
}


type FocusCallback = Rc<dyn Fn(&mut Window, &mut App)>;
type ClearHandler = Rc<dyn Fn(&mut App)>;
type CopyCallback = Rc<dyn Fn(&mut App) -> Option<String>>;
type ContentKeyResolver = Rc<dyn Fn(Point<Pixels>, &App) -> Option<TextSelectionContentKey>>;

/// Notifications emitted by a text-selection participant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextSelectionEvent {
    /// The participant's window-selection projection changed.
    SelectionChanged(Option<TextSelectionSnapshot>),
    /// The active drag requests vertical auto-scroll, or `None` to stop.
    AutoScroll(Option<Pixels>),
    /// Window selection cleared the participant's participant-local state.
    Cleared,
}

struct CopyItem {
    document_order: u64,
    callback: Option<CopyCallback>,
    fallback: String,
    separator_before: String,
}

fn resolve_copy_item(item: CopyItem, cx: &mut App) -> Option<(String, String)> {
    let text = if let Some(callback) = item.callback {
        callback(cx)?
    } else {
        item.fallback
    };
    Some((item.separator_before, text))
}

fn resolve_copy_items(mut items: Vec<CopyItem>, cx: &mut App) -> String {
    items.sort_by_key(|item| item.document_order);
    let Some(resolved) = items
        .into_iter()
        .map(|item| resolve_copy_item(item, cx))
        .collect::<Option<Vec<_>>>()
    else {
        return String::new();
    };
    let resolved = resolved
        .into_iter()
        .filter(|(_, text)| !text.trim().is_empty())
        .collect::<Vec<_>>();
    let mut output = String::new();
    for (index, (separator, text)) in resolved.into_iter().enumerate() {
        if index > 0 {
            output.push_str(&separator);
        }
        output.push_str(&text);
    }
    output
}

fn dispatch_clear_handlers(handlers: Vec<ClearHandler>, cx: &mut App) {
    for handler in handlers {
        handler(cx);
    }
}

struct SelectableTextState {
    entity_id: EntityId,
    fallback_copy_text: String,
    projected_copy_text: Option<String>,
    runs: Vec<TextSelectionRun>,
    /// Lazily built UAX #29 word segments per current run, invalidated when
    /// run text changes so drag updates never re-segment unchanged text.
    word_segments: Vec<Option<WordSegments>>,
    local_selection: bool,
    snapshot: Option<TextSelectionSnapshot>,
    on_focus: Option<FocusCallback>,
    clear: Option<ClearHandler>,
    copy: Option<CopyCallback>,
    content_key_resolver: Option<ContentKeyResolver>,
}

impl EventEmitter<TextSelectionEvent> for SelectableTextState {}

impl SelectableTextState {
    fn new(entity_id: EntityId, fallback_copy_text: impl Into<String>) -> Self {
        Self {
            entity_id,
            fallback_copy_text: fallback_copy_text.into(),
            projected_copy_text: None,
            runs: Vec::new(),
            word_segments: Vec::new(),
            local_selection: false,
            snapshot: None,
            on_focus: None,
            clear: None,
            copy: None,
            content_key_resolver: None,
        }
    }

    /// The current geometry selection snapshot for this participant.
    fn snapshot(&self) -> Option<TextSelectionSnapshot> {
        self.snapshot
    }

    /// Sets the text copied by this participant when it participates in selection.
    fn set_fallback_copy_text(&mut self, text: impl Into<String>) {
        self.fallback_copy_text = text.into();
        self.projected_copy_text = None;
    }

    /// Marks participant-local selection (for example select-all) as active.
    fn set_local_selection(&mut self, active: bool) {
        self.local_selection = active;
    }

    /// Projects this participant's current snapshot onto plain-text runs and caches
    /// their selected substrings for the window selection query.
    ///
    /// Call this once per painted run. A snapshot change or
    /// Clearing window selection invalidates the cache immediately, so copy
    /// never returns text from a previous projection while waiting to repaint.
    fn update_runs(&mut self, runs: &[TextSelectionRun]) -> TextSelectionProjection {
        let texts_changed = self.runs.len() != runs.len()
            || self
                .runs
                .iter()
                .zip(runs)
                .any(|(previous, next)| previous.text != next.text);
        self.runs = runs.to_vec();
        if texts_changed {
            self.word_segments.clear();
        }
        self.word_segments.resize_with(self.runs.len(), || None);
        if self.snapshot.is_some_and(|snapshot| snapshot.window_points().is_none()) {
            // Geometry-free snapshots (logical whole-content selections) keep
            // their copy text frozen at the window, so the projected-substring
            // cache stays unset, but the runs themselves must cache: a
            // participant that first paints while already logically selected
            // (for example remounted by virtualization) otherwise never
            // caches runs, leaves its layout-side projection empty, and
            // repaints forever against the mismatch.
            return project_ranges(self.snapshot, runs);
        }
        let states = project_ranges(self.snapshot, runs);
        let mut selected_runs = runs
            .iter()
            .zip(states.ranges())
            .enumerate()
            .filter_map(|(index, (run, state))| {
                state.as_ref().map(|range| {
                    debug_assert!(run.text.is_char_boundary(range.start));
                    debug_assert!(run.text.is_char_boundary(range.end));
                    (
                        run.document_order,
                        index,
                        run.text[range.clone()].to_string(),
                    )
                })
            })
            .collect::<Vec<_>>();
        selected_runs.sort_by_key(|(order, index, _)| (*order, *index));
        self.projected_copy_text =
            Some(selected_runs.into_iter().map(|(_, _, text)| text).collect());
        states
    }

    /// Installs the callback which focuses the participant when a drag begins in it.
    fn set_focus_handler(&mut self, callback: impl Fn(&mut Window, &mut App) + 'static) {
        self.on_focus = Some(Rc::new(callback));
    }

    /// Window points at the edges of the multi-click unit under `position`.
    ///
    /// Word clicks binary-search the cached UAX #29 segments for the current
    /// run instead of re-segmenting the whole run from byte 0 on every drag
    /// update.
    fn multi_click_points(
        &mut self,
        position: Point<Pixels>,
        click_count: usize,
    ) -> Option<(Point<Pixels>, Point<Pixels>)> {
        let index = self
            .runs
            .iter()
            .position(|run| run.bounds.contains(&position))?;
        if self.runs[index].text.len() != self.runs[index].layout.len() {
            return None;
        }
        let offset = self.runs[index]
            .layout
            .index_for_position(position)
            .ok()?;
        let range = match click_count {
            2 => self.word_range_at(index, offset)?,
            3.. => line_range_at(&self.runs[index].text, offset),
            _ => return None,
        };
        if range.is_empty() {
            return None;
        }
        let run = &self.runs[index];
        Some((
            run.layout.position_for_index(range.start)?,
            run.layout.position_for_index(range.end)?,
        ))
    }

    /// The cached UAX #29 word segment containing `offset` in run `index`.
    ///
    /// The per-run cache is invalidated by [`Self::update_runs`] whenever run
    /// text changes; its construction is the source of truth, so lookups
    /// never revalidate the text.
    fn word_range_at(&mut self, index: usize, offset: usize) -> Option<Range<usize>> {
        let (segments, text) = (&mut self.word_segments[index], &self.runs[index].text);
        let segments = segments.get_or_insert_with(|| WordSegments::new(text));
        segments.range_at(text, offset)
    }

    fn clear_with(&mut self, callback: impl Fn(&mut App) + 'static) {
        self.clear = Some(Rc::new(callback));
    }

    /// Installs a participant-specific copy projection.
    fn copy_with(&mut self, callback: impl Fn(&mut App) -> Option<String> + 'static) {
        self.copy = Some(Rc::new(callback));
    }

    /// Installs a participant-specific lookup for stable virtualized content keys.
    fn resolve_content_key_with(
        &mut self,
        callback: impl Fn(Point<Pixels>, &App) -> Option<TextSelectionContentKey> + 'static,
    ) {
        self.content_key_resolver = Some(Rc::new(callback));
    }

    fn set_snapshot(&mut self, snapshot: Option<TextSelectionSnapshot>, cx: &mut Context<Self>) {
        if self.snapshot == snapshot {
            return;
        }
        let preserves_projection = self
            .snapshot
            .zip(snapshot)
            .is_some_and(|(previous, next)| {
                projection_is_unchanged(previous, next, self.entity_id)
            });
        self.snapshot = snapshot;
        if !preserves_projection {
            self.projected_copy_text = None;
        }
        cx.emit(TextSelectionEvent::SelectionChanged(snapshot));
    }

    fn clear_state(&mut self, cx: &mut Context<Self>) -> Option<ClearHandler> {
        self.snapshot = None;
        self.projected_copy_text = None;
        self.local_selection = false;
        cx.emit(TextSelectionEvent::Cleared);
        cx.emit(TextSelectionEvent::SelectionChanged(None));
        self.clear.clone()
    }

    fn set_auto_scroll(&self, delta: Option<Pixels>, cx: &mut Context<Self>) {
        cx.emit(TextSelectionEvent::AutoScroll(delta));
    }

    fn focus(&self, window: &mut Window, cx: &mut App) {
        if let Some(callback) = self.on_focus.clone() {
            window.defer(cx, move |window, cx| callback(window, cx));
        }
    }

    fn copy_item(&self, document_order: u64, separator_before: &str) -> Option<CopyItem> {
        (self.snapshot.is_some() || self.local_selection).then(|| CopyItem {
            document_order,
            callback: self.copy.clone(),
            separator_before: separator_before.to_string(),
            fallback: self
                .projected_copy_text
                .clone()
                .unwrap_or_else(|| self.fallback_copy_text.clone()),
        })
    }
}

/// A stable, participant-neutral handle for text that participates in window selection.
#[derive(Clone)]
pub struct TextSelectionHandle(Entity<SelectableTextState>);

impl TextSelectionHandle {
    /// Creates a selection participant handle with fallback text for copying.
    pub fn new(fallback_copy_text: impl Into<String>, cx: &mut App) -> Self {
        Self(cx.new(|cx| {
            SelectableTextState::new(cx.entity_id(), fallback_copy_text)
        }))
    }

    /// Returns this participant's stable identity.
    pub fn entity_id(&self) -> EntityId {
        self.0.entity_id()
    }

    /// Returns the current geometry selection snapshot for this participant.
    pub fn snapshot(&self, cx: &App) -> Option<TextSelectionSnapshot> {
        self.0.read(cx).snapshot()
    }

    /// Sets the fallback text copied while this participant participates.
    pub fn set_fallback_copy_text(&self, text: impl Into<String>, cx: &mut App) {
        self.0
            .update(cx, |state, _| state.set_fallback_copy_text(text));
    }

    /// Marks participant-local selection, such as select-all, as active.
    pub fn set_local_selection(&self, active: bool, cx: &mut App) {
        self.0
            .update(cx, |state, _| state.set_local_selection(active));
    }

    /// Returns whether participant-local selection is active.
    pub fn has_local_selection(&self, cx: &App) -> bool {
        self.0.read(cx).local_selection
    }

    /// Registers this participant and its geometry for the current frame.
    pub fn register(
        &self,
        mut registration: TextSelectionRegistration,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(scope) = current_text_selection_scope(window.window_handle().window_id(), cx) {
            registration.scope = scope;
        }
        let Some(state) = WindowSelectionState::existing(window, cx) else {
            return;
        };
        state.update(cx, |state, cx| {
            state.register_participant(self.clone(), registration, cx)
        });
    }

    /// Projects the current snapshot onto plain-text runs and caches their copy text.
    pub fn update_runs(&self, runs: &[TextSelectionRun], cx: &mut App) -> TextSelectionProjection {
        self.0.update(cx, |state, _| state.update_runs(runs))
    }

    /// Projects the current snapshot onto the runs retained from the previous
    /// paint. This lets elements prepare selected-glyph styling during layout.
    pub fn project_cached_runs(&self, cx: &App) -> TextSelectionProjection {
        let state = self.0.read(cx);
        project_ranges(state.snapshot, &state.runs)
    }

    /// Subscribes to participant selection notifications.
    pub fn subscribe(
        &self,
        mut callback: impl FnMut(&TextSelectionEvent, &mut App) + 'static,
        cx: &mut App,
    ) -> Subscription {
        cx.subscribe(&self.0, move |_, event, cx| callback(event, cx))
    }

    /// Subscribes `window` to refresh whenever this participant's selection changes.
    #[must_use = "retain the subscription or explicitly detach it"]
    pub fn refresh_window_on_change(&self, window: &Window, cx: &mut App) -> Subscription {
        let window = window.window_handle();
        self.subscribe(
            move |event, cx| {
                if matches!(event, TextSelectionEvent::SelectionChanged(_)) {
                    _ = window.update(cx, |_, window, _| window.refresh());
                }
            },
            cx,
        )
    }

    /// Sets the callback which focuses the participant when a drag begins in it.
    pub fn focus_with(&self, callback: impl Fn(&mut Window, &mut App) + 'static, cx: &mut App) {
        self.0
            .update(cx, |state, _| state.set_focus_handler(callback));
    }

    /// Sets the synchronous participant cleanup command used by window clear.
    pub fn clear_with(&self, callback: impl Fn(&mut App) + 'static, cx: &mut App) {
        self.0.update(cx, |state, _| state.clear_with(callback));
    }

    /// Sets a participant-specific copy projection.
    ///
    /// Returning `None` marks the complete window copy as unresolved, so callers
    /// can refuse clipboard writes instead of returning a partial selection.
    pub fn copy_with(
        &self,
        callback: impl Fn(&mut App) -> Option<String> + 'static,
        cx: &mut App,
    ) {
        self.0.update(cx, |state, _| state.copy_with(callback));
    }

    /// Sets a source-copy projection without retaining this handle inside itself.
    pub fn copy_projection_with(
        &self,
        callback: impl Fn(
                Option<TextSelectionSnapshot>,
                Option<String>,
                &mut App,
            ) -> Option<String>
            + 'static,
        cx: &mut App,
    ) {
        let weak = self.downgrade();
        self.copy_with(
            move |cx| {
                let participant = weak.upgrade()?;
                let (snapshot, projected) = {
                    let state = participant.read(cx);
                    (state.snapshot, state.projected_copy_text.clone())
                };
                callback(snapshot, projected, cx)
            },
            cx,
        );
    }

    /// Sets a participant-specific lookup for stable virtualized content keys.
    pub fn resolve_content_key_with(
        &self,
        callback: impl Fn(Point<Pixels>, &App) -> Option<TextSelectionContentKey> + 'static,
        cx: &mut App,
    ) {
        self.0
            .update(cx, |state, _| state.resolve_content_key_with(callback));
    }

    fn downgrade(&self) -> WeakEntity<SelectableTextState> {
        self.0.downgrade()
    }
}

#[derive(Clone)]
struct ParticipantRegistration {
    participant: WeakEntity<SelectableTextState>,
    retained_selection: Option<TextSelectionHandle>,
    registration: Rc<TextSelectionRegistration>,
    generation: u64,
}

#[derive(Clone)]
struct SelectionEndpoint {
    participant: Option<WeakEntity<SelectableTextState>>,
    point: Point<Pixels>,
    document_order: u64,
    inside: bool,
    inside_text: bool,
    content_key: Option<TextSelectionContentKey>,
    content_key_resolver: Option<(ContentKeyResolver, Point<Pixels>)>,
}

impl SelectionEndpoint {
    fn snapshot(&self) -> TextSelectionEndpoint {
        let snapshot = TextSelectionEndpoint::new(self.entity_id(), self.point);
        if let Some(content_key) = self.content_key {
            snapshot.with_content_key(content_key)
        } else {
            snapshot
        }
    }

    fn resolve(
        &self,
        participants: &HashMap<EntityId, ParticipantRegistration>,
    ) -> Option<Point<Pixels>> {
        let participant = self.participant.as_ref()?;
        let registration = participants.get(&participant.entity_id())?;
        participant.upgrade()?;
        Some(
            self.point
                + registration.registration.scroll_offset
                + registration.registration.bounds.origin,
        )
    }

    fn entity_id(&self) -> Option<EntityId> {
        self.participant
            .as_ref()
            .map(|participant| participant.entity_id())
    }

    /// Whether both endpoints anchor the same participant-relative point.
    fn same_point(&self, other: &Self) -> bool {
        self.entity_id() == other.entity_id()
            && self.document_order == other.document_order
            && self.point == other.point
    }
}

/// The retained endpoints of the word a held double-click selected.
#[derive(Clone)]
struct WordDragState {
    original_start: SelectionEndpoint,
    original_end: SelectionEndpoint,
}

/// Orders endpoints by document order, then by participant-relative point.
fn compare_selection_positions(
    first: &SelectionEndpoint,
    second: &SelectionEndpoint,
) -> cmp::Ordering {
    first
        .document_order
        .cmp(&second.document_order)
        .then_with(|| {
            f32::from(first.point.y)
                .partial_cmp(&f32::from(second.point.y))
                .unwrap_or(cmp::Ordering::Equal)
        })
        .then_with(|| {
            f32::from(first.point.x)
                .partial_cmp(&f32::from(second.point.x))
                .unwrap_or(cmp::Ordering::Equal)
        })
}

/// Opaque lease on a window's drag auto-scroll command stream.
///
/// Created by [`TextSelection::subscribe_auto_scroll`], it retains both the
/// window selection state entity and the GPUI subscription so neither is
/// collected while the host holds the lease. The window state entity may not
/// have been created by the `TextSelectionLayer`'s first prepaint yet; this
/// lease ensures it stays alive from the moment the host subscribes.
pub struct AutoScrollLease {
    state: Entity<WindowSelectionState>,
    _subscription: Subscription,
}

/// Window-scoped emitter of drag auto-scroll commands.
///
/// Participant events follow whichever leaf currently holds the drag anchor,
/// which virtualization replaces while the drag continues. This source is
/// retained by the window selection state, so one host subscription observes
/// the whole gesture across participant churn.
struct WindowAutoScrollSource {
    last: Option<Pixels>,
}

impl EventEmitter<TextSelectionEvent> for WindowAutoScrollSource {}

impl WindowAutoScrollSource {
    fn update_delta(&mut self, delta: Option<Pixels>, cx: &mut Context<Self>) {
        if self.last == delta {
            return;
        }
        self.last = delta;
        cx.emit(TextSelectionEvent::AutoScroll(delta));
    }
}

/// Window-owned selection of whole participant contents by stable identity.
///
/// Installed by [`TextSelection::select_all`]. The complete copy text is
/// frozen at install time, and registered participants match by content key,
/// so virtualized participants neither need to stay mounted nor enter the
/// virtual copy retention set: they rejoin by key when they register again.
struct LogicalSelection {
    keys: Vec<TextSelectionContentKey>,
    key_set: HashSet<TextSelectionContentKey>,
    text: String,
}

impl LogicalSelection {
    fn new(keys: Vec<TextSelectionContentKey>, text: String) -> Self {
        Self {
            key_set: keys.iter().copied().collect(),
            keys,
            text,
        }
    }

    fn contains(&self, key: TextSelectionContentKey) -> bool {
        self.key_set.contains(&key)
    }

    /// The participant snapshot for a logically selected participant.
    ///
    /// Coverage is `Full` and no endpoint geometry is attached, so run
    /// projection selects every byte of each run.
    fn participant_snapshot() -> TextSelectionSnapshot {
        let endpoint = TextSelectionEndpoint::new(None, Point::default());
        TextSelectionSnapshot::new(endpoint, endpoint).with_coverage(TextSelectionCoverage::Full)
    }
}

/// Window-local generic text-selection state.
#[derive(Default)]
struct WindowSelectionState {
    participants: HashMap<EntityId, ParticipantRegistration>,
    virtual_copy_participants: HashMap<EntityId, ParticipantRegistration>,
    logical_selection: Option<LogicalSelection>,
    active_scope: TextSelectionScopeId,
    anchor: Option<SelectionEndpoint>,
    cursor: Option<SelectionEndpoint>,
    pending_extension_anchor: Option<SelectionEndpoint>,
    word_drag: Option<WordDragState>,
    is_selecting: bool,
    did_hit_text: bool,
    frame_generation: u64,
    finish_frame_scheduled: bool,
    mouse_down_prepared: bool,
    auto_scroll: AutoScroll,
    auto_scroll_source: Option<Entity<WindowAutoScrollSource>>,
}

impl WindowSelectionState {
    fn resolve_content_keys(state: &Entity<Self>, cx: &mut App) {
        let pending = state.update(cx, |state, _| {
            [
                state
                    .anchor
                    .as_ref()
                    .and_then(|endpoint| endpoint.content_key_resolver.clone()),
                state
                    .cursor
                    .as_ref()
                    .and_then(|endpoint| endpoint.content_key_resolver.clone()),
            ]
        });
        let resolved =
            pending.map(|pending| pending.and_then(|(callback, point)| callback(point, cx)));
        state.update(cx, |state, cx| {
            if let (Some(endpoint), Some(key)) = (state.anchor.as_mut(), resolved[0]) {
                endpoint.content_key = Some(key);
                endpoint.content_key_resolver = None;
            }
            if let (Some(endpoint), Some(key)) = (state.cursor.as_mut(), resolved[1]) {
                endpoint.content_key = Some(key);
                endpoint.content_key_resolver = None;
            }
            state.sync_word_drag_content_keys();
            state.publish_snapshots(cx);
        });
    }

    /// Synchronizes resolved endpoint content keys into the retained
    /// word-drag originals, so reversal and virtualization clone endpoints
    /// that already carry their resolved identity.
    ///
    /// Reverse drags anchor on the original word's end, so every newly
    /// resolved endpoint is matched against both retained originals rather
    /// than a positional pairing.
    fn sync_word_drag_content_keys(&mut self) {
        let Some(word) = self.word_drag.as_mut() else {
            return;
        };
        for endpoint in [self.anchor.as_ref(), self.cursor.as_ref()]
            .into_iter()
            .flatten()
        {
            let Some(key) = endpoint.content_key else {
                continue;
            };
            for original in [&mut word.original_start, &mut word.original_end] {
                if original.same_point(endpoint) {
                    original.content_key = Some(key);
                    original.content_key_resolver = None;
                }
            }
        }
    }
    fn acquire(window_id: gpui::WindowId, cx: &mut App) -> Entity<Self> {
        if !cx.has_global::<SelectionStateRegistry>() {
            cx.set_global(SelectionStateRegistry::default());
        }
        if let Some(state) = cx
            .global::<SelectionStateRegistry>()
            .0
            .get(&window_id)
            .and_then(WeakEntity::upgrade)
        {
            return state;
        }

        let active_scope = if cx.has_global::<PendingTextSelectionScopes>() {
            cx.global_mut::<PendingTextSelectionScopes>()
                .0
                .remove(&window_id)
                .unwrap_or_default()
        } else {
            TextSelectionScopeId::default()
        };

        let state = cx.new(move |cx| {
            let entity_id = cx.entity_id();
            cx.on_release(move |state: &mut WindowSelectionState, cx| {
                let handlers = state.clear_state(cx);
                if cx.has_global::<SelectionStateRegistry>() {
                    let registry = &mut cx.global_mut::<SelectionStateRegistry>().0;
                    if registry
                        .get(&window_id)
                        .is_some_and(|state| state.entity_id() == entity_id)
                    {
                        registry.remove(&window_id);
                    }
                }
                if !handlers.is_empty() {
                    cx.defer(move |cx| dispatch_clear_handlers(handlers, cx));
                }
            })
            .detach();
            Self {
                active_scope,
                ..Self::default()
            }
        });
        cx.global_mut::<SelectionStateRegistry>()
            .0
            .insert(window_id, state.downgrade());
        state
    }

    #[cfg(test)]
    fn ensure(window: &Window, cx: &mut App) -> Entity<Self> {
        Self::acquire(window.window_handle().window_id(), cx)
    }

    fn existing(window: &Window, cx: &App) -> Option<Entity<Self>> {
        if !cx.has_global::<SelectionStateRegistry>() {
            return None;
        }
        cx.global::<SelectionStateRegistry>()
            .0
            .get(&window.window_handle().window_id())
            .and_then(WeakEntity::upgrade)
    }

    /// Updates the active scope. Participants from other scopes cannot participate.
    #[cfg(test)]
    fn set_active_scope(&mut self, scope: TextSelectionScopeId, cx: &mut App) {
        let handlers = self.set_active_scope_state(scope, cx);
        dispatch_clear_handlers(handlers, cx);
    }

    fn set_active_scope_state(
        &mut self,
        scope: TextSelectionScopeId,
        cx: &mut App,
    ) -> Vec<ClearHandler> {
        if self.active_scope == scope {
            return Vec::new();
        }
        let handlers = self.clear_state(cx);
        self.active_scope = scope;
        self.publish_snapshots(cx);
        handlers
    }

    /// Sweeps participants after a rendered frame has completed.
    ///
    /// Registrations are stamped with the current generation while any sibling
    /// is painting. Sweeping only after paint makes registration independent of
    /// whether a participant or the lifecycle element paints first.
    pub fn finish_frame(&mut self, cx: &mut App) -> Vec<ClearHandler> {
        self.finish_frame_scheduled = false;
        let stale = self
            .participants
            .iter()
            .filter_map(|(id, registration)| {
                (registration.generation != self.frame_generation)
                    .then(|| (*id, registration.participant.clone()))
            })
            .collect::<Vec<_>>();
        let mut handlers = Vec::new();
        for (id, participant) in stale {
            let mut registration = self
                .participants
                .remove(&id)
                .expect("stale participant disappeared during frame sweep");
            let Some(participant) = participant.upgrade() else {
                continue;
            };
            if participant.read(cx).snapshot.is_some() && self.logical_selection.is_none() {
                registration.retained_selection = Some(TextSelectionHandle(participant));
                self.virtual_copy_participants.insert(id, registration);
            } else if let Some(handler) = participant.update(cx, |state, cx| state.clear_state(cx)) {
                handlers.push(handler);
            }
        }
        self.publish_snapshots(cx);
        self.frame_generation = self.frame_generation.wrapping_add(1);
        handlers
    }

    fn schedule_finish_frame(&mut self) -> bool {
        if self.finish_frame_scheduled {
            return false;
        }
        self.finish_frame_scheduled = true;
        true
    }

    /// Registers this frame's geometry for a participant.
    pub fn register_participant(
        &mut self,
        selection: TextSelectionHandle,
        registration: TextSelectionRegistration,
        cx: &mut App,
    ) {
        self.prune_dead_participants();
        self.virtual_copy_participants.remove(&selection.entity_id());
        self.participants.insert(
            selection.entity_id(),
            ParticipantRegistration {
                participant: selection.downgrade(),
                retained_selection: None,
                registration: Rc::new(registration),
                generation: self.frame_generation,
            },
        );
        self.publish_snapshots(cx);
    }

    /// Starts a selection gesture using bounds hit testing (useful to adapters/tests).
    #[cfg(test)]
    fn begin(&mut self, position: Point<Pixels>, extend: bool, cx: &mut App) {
        self.begin_impl(position, extend, false, None, cx);
    }

    /// Updates the current gesture using bounds hit testing.
    #[cfg(test)]
    fn update(&mut self, position: Point<Pixels>, cx: &mut App) {
        self.update_impl(position, None, cx);
    }

    /// Ends the current gesture and keeps its selection visible.
    pub fn end(&mut self, cx: &mut App) {
        self.pending_extension_anchor = None;
        self.word_drag = None;
        if !self.is_selecting {
            return;
        }
        self.is_selecting = false;
        if !self.did_hit_text {
            self.anchor = None;
            self.cursor = None;
        }
        self.stop_anchor_auto_scroll(cx);
        self.publish_snapshots(cx);
    }

    /// Clears both window selection and every participant's local selection.
    pub fn clear(&mut self, cx: &mut App) {
        let handlers = self.clear_state(cx);
        dispatch_clear_handlers(handlers, cx);
    }

    fn clear_state(&mut self, cx: &mut App) -> Vec<ClearHandler> {
        self.stop_anchor_auto_scroll(cx);
        self.anchor = None;
        self.cursor = None;
        self.pending_extension_anchor = None;
        self.word_drag = None;
        self.is_selecting = false;
        self.did_hit_text = false;
        self.logical_selection = None;
        self.prune_dead_participants();
        let handlers = self
            .participants
            .values()
            .chain(self.virtual_copy_participants.values())
            .filter_map(|registration| registration.participant.upgrade())
            .filter_map(|participant| participant.update(cx, |state, cx| state.clear_state(cx)))
            .collect();
        self.virtual_copy_participants.clear();
        handlers
    }

    fn copy_items(&self, cx: &App) -> Vec<CopyItem> {
        if let Some(logical) = self.logical_selection.as_ref() {
            // The logical selection carries its own frozen complete copy; the
            // participating leaves only paint its highlight.
            return vec![CopyItem {
                document_order: 0,
                callback: None,
                fallback: logical.text.clone(),
                separator_before: String::new(),
            }];
        }
        self.participants
            .values()
            .chain(self.virtual_copy_participants.values())
            .filter_map(|registration| {
                let participant = registration.participant.upgrade()?;
                participant.read(cx).copy_item(
                    registration.registration.document_order,
                    &registration.registration.copy_separator_before,
                )
            })
            .collect()
    }

    #[cfg(test)]
    fn selected_text(&self, cx: &mut App) -> String {
        resolve_copy_items(self.copy_items(cx), cx)
    }

    /// Returns whether a drag, a logical whole-content selection, or a
    /// participant-local selection is active.
    pub fn has_selection(&self, cx: &App) -> bool {
        self.logical_selection.is_some()
            || self.snapshot().is_some()
            || self.participants.values().any(|registration| {
                registration
                    .participant
                    .upgrade()
                    .is_some_and(|participant| participant.read(cx).local_selection)
            })
    }

    fn content_keys(&self) -> Option<[TextSelectionContentKey; 2]> {
        self.did_hit_text.then_some([
            self.anchor.as_ref()?.content_key?,
            self.cursor.as_ref()?.content_key?,
        ])
    }

    /// Returns every content identity the current selection depends on.
    ///
    /// A logical whole-content selection reports all of its frozen keys, so
    /// any stale interior participant refuses the copy; a pointer selection
    /// falls back to its two endpoint keys.
    fn selected_content_keys(&self) -> Option<Vec<TextSelectionContentKey>> {
        if let Some(logical) = self.logical_selection.as_ref() {
            return Some(logical.keys.clone());
        }
        self.content_keys()
            .map(|[anchor, cursor]| vec![anchor, cursor])
    }

    /// Returns the current resolved selection endpoints.
    pub fn snapshot(&self) -> Option<TextSelectionSnapshot> {
        if !self.did_hit_text {
            return None;
        }
        let anchor_endpoint = self.anchor.as_ref()?;
        let cursor_endpoint = self.cursor.as_ref()?;
        let anchor = anchor_endpoint.snapshot();
        let cursor = cursor_endpoint.snapshot();
        (anchor != cursor).then(|| {
            TextSelectionSnapshot::new(anchor, cursor)
                .with_selecting(self.is_selecting)
                .with_window_points(self.window_points(anchor_endpoint, cursor_endpoint))
        })
    }

    fn window_points(
        &self,
        anchor_endpoint: &SelectionEndpoint,
        cursor_endpoint: &SelectionEndpoint,
    ) -> Option<TextSelectionWindowPoints> {
        let anchor = anchor_endpoint.resolve(&self.participants);
        let cursor = cursor_endpoint.resolve(&self.participants);
        match (anchor, cursor) {
            (Some(anchor), Some(cursor)) => Some(TextSelectionWindowPoints { anchor, cursor }),
            (Some(anchor), None) => Some(TextSelectionWindowPoints {
                anchor,
                cursor: self.virtual_endpoint_point(
                    cursor_endpoint.document_order < anchor_endpoint.document_order,
                )?,
            }),
            (None, Some(cursor)) => Some(TextSelectionWindowPoints {
                anchor: self.virtual_endpoint_point(
                    anchor_endpoint.document_order < cursor_endpoint.document_order,
                )?,
                cursor,
            }),
            (None, None) => None,
        }
    }

    fn virtual_endpoint_point(&self, before: bool) -> Option<Point<Pixels>> {
        let mut registrations = self.participants.values();
        let first = registrations.next()?.registration.bounds;
        let mut edge = if before { first.top() } else { first.bottom() };
        for registration in registrations {
            let candidate = if before {
                registration.registration.bounds.top()
            } else {
                registration.registration.bounds.bottom()
            };
            if (before && candidate < edge) || (!before && candidate > edge) {
                edge = candidate;
            }
        }
        let outside = if before {
            edge - px(1.)
        } else {
            edge + px(1.)
        };
        Some(point(px(0.), outside))
    }

    /// Returns whether a drag is currently in progress.
    #[cfg(test)]
    fn is_selecting(&self) -> bool {
        self.is_selecting
    }

    fn prepare_for_mouse_down(&mut self, extend: bool, cx: &mut App) -> Vec<ClearHandler> {
        let pending_extension_anchor = extend.then(|| self.anchor.clone()).flatten();
        self.stop_anchor_auto_scroll(cx);
        self.anchor = None;
        self.cursor = None;
        self.pending_extension_anchor = None;
        self.word_drag = None;
        self.is_selecting = false;
        self.did_hit_text = false;
        self.logical_selection = None;
        self.prune_dead_participants();
        let handlers = self
            .participants
            .values()
            .chain(self.virtual_copy_participants.values())
            .filter_map(|registration| registration.participant.upgrade())
            .filter_map(|participant| participant.update(cx, |state, cx| state.clear_state(cx)))
            .collect();
        self.virtual_copy_participants.clear();
        self.pending_extension_anchor = pending_extension_anchor;
        handlers
    }

    fn begin_in_window(
        &mut self,
        position: Point<Pixels>,
        extend: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.begin_impl(position, extend, true, Some(window), cx);
    }

    fn update_in_window(
        &mut self,
        position: Point<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if !cx.has_active_drag() {
            self.update_impl(position, Some(window), cx);
            self.update_auto_scroll(position, Some(window), cx);
        }
    }

    fn select_at(
        &mut self,
        position: Point<Pixels>,
        click_count: usize,
        window: &mut Window,
        cx: &mut App,
    ) {
        GlobalState::init(cx);
        if GlobalState::is_text_selection_suppressed(cx) {
            return;
        }
        let hit = self.endpoint(position, Some(window), cx);
        if !hit.inside_text {
            return;
        }
        let Some(participant) = hit
            .participant
            .and_then(|participant| participant.upgrade())
        else {
            return;
        };
        // A multi-click resolves its unit once per gesture, so it can segment
        // directly; the per-update drag path uses the cached segments.
        let points = points_for_multi_click(&participant.read(cx).runs, position, click_count);
        let Some((anchor_point, cursor_point)) = points else {
            return;
        };
        let Some(registration) = self
            .participants
            .get(&participant.entity_id())
            .map(|entry| Rc::clone(&entry.registration))
        else {
            return;
        };
        let anchor = Self::participant_endpoint(&participant, &registration, anchor_point, cx);
        let cursor = Self::participant_endpoint(&participant, &registration, cursor_point, cx);
        self.anchor = Some(anchor.clone());
        self.cursor = Some(cursor.clone());
        self.did_hit_text = true;
        self.is_selecting = false;
        if click_count == 2 {
            // A held double-click keeps extending by whole words; triple and
            // further clicks remain static line selections.
            self.word_drag = Some(WordDragState {
                original_start: anchor,
                original_end: cursor,
            });
            self.is_selecting = true;
        }
        participant.update(cx, |state, cx| state.focus(window, cx));
        self.publish_snapshots(cx);
    }

    #[cfg(test)]
    fn update_in_window_with_active_drag(
        &mut self,
        position: Point<Pixels>,
        active_drag: bool,
        window: &Window,
        cx: &mut App,
    ) {
        if !active_drag {
            self.update_impl(position, Some(window), cx);
        }
    }

    fn begin_impl(
        &mut self,
        position: Point<Pixels>,
        extend: bool,
        already_prepared: bool,
        window: Option<&mut Window>,
        cx: &mut App,
    ) {
        GlobalState::init(cx);
        if GlobalState::is_text_selection_suppressed(cx) {
            self.pending_extension_anchor = None;
            return;
        }
        let previous_anchor = extend
            .then(|| {
                self.pending_extension_anchor
                    .take()
                    .or_else(|| self.anchor.clone())
            })
            .flatten()
            .filter(|anchor| anchor.resolve(&self.participants).is_some());
        if !extend && !already_prepared {
            self.clear(cx);
        }
        self.word_drag = None;
        let endpoint = self.endpoint(position, window.as_deref(), cx);
        let focus_participant = endpoint
            .inside
            .then(|| endpoint.participant.clone())
            .flatten();
        let anchor = previous_anchor.unwrap_or_else(|| endpoint.clone());
        self.anchor = Some(anchor.clone());
        self.cursor = Some(endpoint.clone());
        self.did_hit_text = anchor.inside_text || endpoint.inside_text;
        self.is_selecting = true;
        if let Some(participant) = focus_participant.and_then(|participant| participant.upgrade()) {
            if let Some(window) = window {
                participant.update(cx, |state, cx| state.focus(window, cx));
            }
        }
        self.publish_snapshots(cx);
    }

    fn update_impl(&mut self, position: Point<Pixels>, window: Option<&Window>, cx: &mut App) {
        if !self.is_selecting {
            return;
        }
        if self.word_drag.is_some() {
            self.update_word_drag(position, window, cx);
        } else {
            let endpoint = self.endpoint(position, window, cx);
            self.did_hit_text |= endpoint.inside_text;
            self.cursor = Some(endpoint);
        }
        if window.is_none() {
            self.update_participant_auto_scroll(position, cx);
        }
        self.publish_snapshots(cx);
    }

    /// Builds a selection endpoint at a participant text-layout window point.
    fn participant_endpoint(
        participant: &Entity<SelectableTextState>,
        registration: &TextSelectionRegistration,
        window_point: Point<Pixels>,
        cx: &App,
    ) -> SelectionEndpoint {
        let bounds = &registration.bounds;
        let content_point = window_point - bounds.origin - registration.scroll_offset;
        SelectionEndpoint {
            participant: Some(participant.downgrade()),
            point: content_point,
            document_order: registration.document_order,
            inside: true,
            inside_text: true,
            content_key: None,
            content_key_resolver: participant
                .read(cx)
                .content_key_resolver
                .clone()
                .map(|resolver| (resolver, content_point)),
        }
    }

    /// Advances a held double-click drag by whole word segments.
    ///
    /// Both endpoints of the originally double-clicked word are retained.
    /// Every update resolves the word segment under the pointer and moves
    /// only the selection edge the pointer crossed: dragging before the
    /// original word anchors on the original end and snaps the cursor to the
    /// target word's start, dragging after anchors on the original start and
    /// snaps to the target word's end, and dragging back inside restores the
    /// original word. Mapping failures keep the last valid word range, and
    /// geometric outside-text drags extend only to an edge covering the hit
    /// participant's whole terminal run, never a raw character endpoint.
    fn update_word_drag(
        &mut self,
        position: Point<Pixels>,
        window: Option<&Window>,
        cx: &mut App,
    ) {
        let Some(word) = self.word_drag.clone() else {
            return;
        };
        let hit = self.endpoint(position, window, cx);
        self.did_hit_text |= hit.inside_text;
        let Some(participant) = hit.participant.as_ref().and_then(WeakEntity::upgrade) else {
            return;
        };
        let Some(registration) = self
            .participants
            .get(&participant.entity_id())
            .map(|entry| Rc::clone(&entry.registration))
        else {
            return;
        };
        // Extract only the target word or terminal-run window points while
        // the participant is borrowed; the selection state mutates after the
        // borrow ends. Over text the compared endpoints and cursor candidates
        // are the pointer's target word; geometrically outside text the hit
        // position itself orders the gesture and the candidates are edges
        // covering the hit participant's whole terminal run.
        let points = participant.update(cx, |state, _| {
            if hit.inside_text {
                state.multi_click_points(position, 2)
            } else {
                terminal_run_points(&state.runs)
            }
        });
        let Some((start_point, end_point)) = points else {
            return;
        };
        let start = Self::participant_endpoint(&participant, &registration, start_point, cx);
        let end = Self::participant_endpoint(&participant, &registration, end_point, cx);
        let (reference_start, reference_end, before_cursor, after_cursor) = if hit.inside_text {
            (start.clone(), end.clone(), start, end)
        } else {
            (hit.clone(), hit, start, end)
        };
        let (anchor, cursor) = match (
            compare_selection_positions(&reference_start, &word.original_start),
            compare_selection_positions(&reference_end, &word.original_end),
        ) {
            (cmp::Ordering::Less, _) => (word.original_end, before_cursor),
            (_, cmp::Ordering::Greater) => (word.original_start, after_cursor),
            _ => (word.original_start, word.original_end),
        };
        self.anchor = Some(anchor);
        self.cursor = Some(cursor);
    }

    fn endpoint(
        &mut self,
        position: Point<Pixels>,
        window: Option<&Window>,
        cx: &App,
    ) -> SelectionEndpoint {
        self.prune_dead_participants();
        let mut hit: Option<(
            WeakEntity<SelectableTextState>,
            Rc<TextSelectionRegistration>,
            f32,
        )> = None;
        let mut predecessor: Option<(
            WeakEntity<SelectableTextState>,
            Rc<TextSelectionRegistration>,
        )> = None;
        let mut first: Option<(
            WeakEntity<SelectableTextState>,
            Rc<TextSelectionRegistration>,
        )> = None;

        for registration in self.participants.values() {
            if registration.registration.scope != self.active_scope
                || registration.participant.upgrade().is_none()
            {
                continue;
            }
            let participant_geometry = &registration.registration;
            let hovered = window.map_or_else(
                || participant_geometry.bounds.contains(&position),
                |window| participant_geometry.hitbox.is_hovered(window),
            );
            if hovered {
                let area = f32::from(participant_geometry.bounds.size.width)
                    * f32::from(participant_geometry.bounds.size.height);
                if hit.as_ref().is_none_or(|(_, best, best_area)| {
                    area < *best_area
                        || (area == *best_area
                            && participant_geometry.document_order < best.document_order)
                }) {
                    hit = Some((
                        registration.participant.clone(),
                        participant_geometry.clone(),
                        area,
                    ));
                }
            }
            if participant_geometry.bounds.top() <= position.y
                && predecessor.as_ref().is_none_or(|(_, best)| {
                    participant_geometry.bounds.top() > best.bounds.top()
                        || (participant_geometry.bounds.top() == best.bounds.top()
                            && participant_geometry.document_order < best.document_order)
                })
            {
                predecessor = Some((
                    registration.participant.clone(),
                    participant_geometry.clone(),
                ));
            }
            if first.as_ref().is_none_or(|(_, best)| {
                participant_geometry.bounds.top() < best.bounds.top()
                    || (participant_geometry.bounds.top() == best.bounds.top()
                        && participant_geometry.document_order < best.document_order)
            }) {
                first = Some((
                    registration.participant.clone(),
                    participant_geometry.clone(),
                ));
            }
        }

        let selection = hit
            .map(|(participant, registration, _)| (participant, registration, true))
            .or_else(|| {
                predecessor
                    .or(first)
                    .map(|(participant, registration)| (participant, registration, false))
            });
        match selection {
            Some((participant, registration, inside)) => {
                let point = position - registration.bounds.origin - registration.scroll_offset;
                let content_key_resolver = participant.upgrade().and_then(|participant| {
                    participant
                        .read(cx)
                        .content_key_resolver
                        .clone()
                        .map(|callback| (callback, point))
                });
                SelectionEndpoint {
                    point,
                    participant: Some(participant),
                    document_order: registration.document_order,
                    inside,
                    inside_text: inside
                        && registration
                            .text_bounds
                            .iter()
                            .any(|bounds| bounds.contains(&position)),
                    content_key: None,
                    content_key_resolver,
                }
            }
            None => SelectionEndpoint {
                participant: None,
                point: position,
                document_order: 0,
                inside: false,
                inside_text: false,
                content_key: None,
                content_key_resolver: None,
            },
        }
    }

    fn publish_snapshots(&mut self, cx: &mut App) {
        self.prune_dead_participants();
        if let Some(logical) = self.logical_selection.as_ref() {
            let snapshot = LogicalSelection::participant_snapshot();
            for registration in self.participants.values() {
                let Some(participant) = registration.participant.upgrade() else {
                    continue;
                };
                let selected = registration.registration.scope == self.active_scope
                    && registration
                        .registration
                        .content_key
                        .is_some_and(|key| logical.contains(key));
                participant.update(cx, |state, cx| {
                    state.set_snapshot(selected.then_some(snapshot), cx)
                });
            }
            return;
        }
        let snapshot = self.snapshot();
        let single_participant = self.single_participant();
        for (id, registration) in self
            .participants
            .iter()
            .chain(self.virtual_copy_participants.iter())
        {
            let Some(participant) = registration.participant.upgrade() else {
                continue;
            };
            let order = registration.registration.document_order;
            let participant_snapshot = (registration.registration.scope == self.active_scope
                && self.participates(*id, order)
                && single_participant.is_none_or(|single| single == *id))
            .then_some(snapshot)
            .flatten()
            .map(|mut snapshot| {
                snapshot.coverage = self.coverage_for(*id, order);
                snapshot
            });
            participant.update(cx, |state, cx| state.set_snapshot(participant_snapshot, cx));
        }
    }

    fn coverage_for(&self, id: EntityId, order: u64) -> TextSelectionCoverage {
        let Some(anchor) = self.anchor.as_ref() else {
            return TextSelectionCoverage::Bounded;
        };
        let Some(cursor) = self.cursor.as_ref() else {
            return TextSelectionCoverage::Bounded;
        };
        let (Some(anchor_id), Some(cursor_id)) = (anchor.entity_id(), cursor.entity_id()) else {
            return TextSelectionCoverage::Bounded;
        };
        if anchor_id == cursor_id {
            return TextSelectionCoverage::Bounded;
        }
        if id != anchor_id && id != cursor_id {
            TextSelectionCoverage::Full
        } else if (id == anchor_id) == (anchor.document_order < cursor.document_order) {
            TextSelectionCoverage::ToEnd
        } else if order == anchor.document_order || order == cursor.document_order {
            TextSelectionCoverage::FromStart
        } else {
            TextSelectionCoverage::Full
        }
    }

    fn single_participant(&self) -> Option<EntityId> {
        let anchor = self.anchor.as_ref()?.entity_id()?;
        let cursor = self.cursor.as_ref()?.entity_id()?;
        (anchor == cursor).then_some(anchor)
    }

    fn participates(&self, id: EntityId, order: u64) -> bool {
        let Some(anchor) = self.anchor.as_ref() else {
            return false;
        };
        let Some(cursor) = self.cursor.as_ref() else {
            return false;
        };
        let start = anchor.document_order.min(cursor.document_order);
        let end = anchor.document_order.max(cursor.document_order);
        (start..=end).contains(&order)
            || anchor.entity_id() == Some(id)
            || cursor.entity_id() == Some(id)
    }

    fn update_auto_scroll(
        &mut self,
        position: Point<Pixels>,
        _window: Option<&Window>,
        cx: &mut Context<Self>,
    ) {
        if !self.is_selecting {
            return;
        }

        // Resolve a viewport for the window-scoped command, trying in order:
        // 1. The registered cursor participant (survives anchor virtualization).
        // 2. The live anchor participant (original behavior).
        // 3. The first active-scope participant (fallback for any registered leaf).
        // The participant-local compatibility event is only published when the
        // live anchor is resolvable, preserving the original contract.
        let viewport = self
            .cursor
            .as_ref()
            .and_then(|endpoint| self.participant_viewport(&endpoint.participant))
            .or_else(|| {
                self.anchor
                    .as_ref()
                    .filter(|anchor| anchor.inside)
                    .and_then(|anchor| self.participant_viewport(&anchor.participant))
            })
            .or_else(|| {
                self.participants
                    .values()
                    .find(|registration| {
                        registration.registration.scope == self.active_scope
                            && registration.participant.upgrade().is_some()
                    })
                    .map(|registration| registration.registration.hitbox.content_mask.bounds)
            });

        let Some(viewport) = viewport else {
            return;
        };

        let delta = AutoScroll::compute_delta(position.y, viewport);
        self.publish_auto_scroll(delta, cx);

        // Keep participant-local compatibility event conditional on the live
        // anchor, matching the original contract for participant listeners.
        if let Some(participant) = self
            .anchor
            .as_ref()
            .filter(|anchor| anchor.inside)
            .and_then(|anchor| anchor.participant.as_ref())
            .and_then(WeakEntity::upgrade)
        {
            participant.update(cx, |state, cx| state.set_auto_scroll(delta, cx));
        }
    }

    /// Resolves a participant's viewport bounds if its registration exists.
    fn participant_viewport(
        &self,
        participant: &Option<WeakEntity<SelectableTextState>>,
    ) -> Option<Bounds<Pixels>> {
        let participant = participant.as_ref()?.upgrade()?;
        let registration = self.participants.get(&participant.entity_id())?;
        Some(registration.registration.hitbox.content_mask.bounds)
    }

    fn update_participant_auto_scroll(&self, position: Point<Pixels>, cx: &mut App) {
        let Some(anchor) = self.anchor.as_ref().filter(|anchor| anchor.inside) else {
            return;
        };
        let Some(participant) = anchor.participant.as_ref().and_then(WeakEntity::upgrade) else {
            return;
        };
        let Some(registration) = self.participants.get(&participant.entity_id()) else {
            return;
        };
        let delta = AutoScroll::compute_delta(position.y, registration.registration.bounds);
        participant.update(cx, |state, cx| state.set_auto_scroll(delta, cx));
    }

    fn stop_anchor_auto_scroll(&mut self, cx: &mut App) {
        self.auto_scroll.stop();
        self.publish_auto_scroll(None, cx);
        let Some(participant) = self
            .anchor
            .as_ref()
            .filter(|anchor| anchor.inside)
            .and_then(|anchor| anchor.participant.as_ref())
            .and_then(WeakEntity::upgrade)
        else {
            return;
        };
        participant.update(cx, |state, cx| state.set_auto_scroll(None, cx));
    }

    fn ensure_auto_scroll_source(&mut self, cx: &mut App) -> Entity<WindowAutoScrollSource> {
        self.auto_scroll_source
            .get_or_insert_with(|| cx.new(|_| WindowAutoScrollSource { last: None }))
            .clone()
    }

    fn publish_auto_scroll(&mut self, delta: Option<Pixels>, cx: &mut App) {
        let source = self.ensure_auto_scroll_source(cx);
        source.update(cx, |source, cx| source.update_delta(delta, cx));
    }

    fn prune_dead_participants(&mut self) {
        self.participants
            .retain(|_, registration| registration.participant.upgrade().is_some());
        self.virtual_copy_participants
            .retain(|_, registration| registration.participant.upgrade().is_some());
    }
}

#[derive(Default)]
/// Non-owning window locator; retained [`TextSelection`] element state owns
/// each live selection entity.
struct SelectionStateRegistry(HashMap<gpui::WindowId, WeakEntity<WindowSelectionState>>);

impl Global for SelectionStateRegistry {}

#[derive(Default)]
struct PendingTextSelectionScopes(HashMap<gpui::WindowId, TextSelectionScopeId>);

impl Global for PendingTextSelectionScopes {}

#[derive(Default)]
struct TextSelectionScopeStacks(HashMap<gpui::WindowId, Vec<TextSelectionScopeId>>);

impl Global for TextSelectionScopeStacks {}

fn push_text_selection_scope(window_id: gpui::WindowId, scope: TextSelectionScopeId, cx: &mut App) {
    if !cx.has_global::<TextSelectionScopeStacks>() {
        cx.set_global(TextSelectionScopeStacks::default());
    }
    cx.global_mut::<TextSelectionScopeStacks>()
        .0
        .entry(window_id)
        .or_default()
        .push(scope);
}

fn pop_text_selection_scope(window_id: gpui::WindowId, cx: &mut App) {
    let stacks = &mut cx.global_mut::<TextSelectionScopeStacks>().0;
    let remove_stack = stacks.get_mut(&window_id).is_some_and(|stack| {
        stack.pop();
        stack.is_empty()
    });
    if remove_stack {
        stacks.remove(&window_id);
    }
}

fn current_text_selection_scope(
    window_id: gpui::WindowId,
    cx: &App,
) -> Option<TextSelectionScopeId> {
    cx.has_global::<TextSelectionScopeStacks>()
        .then(|| {
            cx.global::<TextSelectionScopeStacks>()
                .0
                .get(&window_id)
                .and_then(|stack| stack.last().copied())
        })
        .flatten()
}

fn with_text_selection_scope<T>(
    window_id: gpui::WindowId,
    scope: TextSelectionScopeId,
    cx: &mut App,
    callback: impl FnOnce(&mut App) -> T,
) -> T {
    push_text_selection_scope(window_id, scope, cx);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(cx)));
    pop_text_selection_scope(window_id, cx);
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// Window-level operations for text selection.
pub struct TextSelection;

impl TextSelection {
    /// Returns the currently selected text in logical document order.
    pub fn selected_text(window: &mut Window, cx: &mut App) -> String {
        let Some(state) = live_text_selection_state(window, cx) else {
            return String::new();
        };
        let items = state.read(cx).copy_items(cx);
        resolve_copy_items(items, cx)
    }


    /// Returns the stable content identities captured at both selection endpoints.
    ///
    /// The keys remain available while endpoint participants are temporarily
    /// unregistered, allowing callers to reject stale virtualized selections.
    pub fn content_keys(
        window: &mut Window,
        cx: &mut App,
    ) -> Option<[TextSelectionContentKey; 2]> {
        live_text_selection_state(window, cx)?.read(cx).content_keys()
    }

    /// Returns every stable content identity the current selection depends on.
    ///
    /// A logical whole-content selection (see [`Self::select_all`]) reports
    /// all of its frozen participant keys in logical order, so a stale
    /// interior participant refuses the copy; a pointer selection reports
    /// its two endpoint keys.
    pub fn selected_content_keys(
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Vec<TextSelectionContentKey>> {
        live_text_selection_state(window, cx)?.read(cx).selected_content_keys()
    }

    /// Selects whole participant contents by stable identity.
    ///
    /// `keys` lists the participant content identities in logical order and
    /// `text` is the complete copy text, both frozen at install time. Any
    /// pointer selection and its drag auto-scroll are cleared first.
    /// Registered participants in the active scope whose registration content
    /// key is in `keys` receive a whole-content (`Full`) snapshot; every other
    /// participant is left unselected. Virtualized participants rejoin by
    /// content key when they register again, so they are never retained in
    /// the virtual copy set. Empty `keys`, or empty or blank-only `text`, is
    /// rejected without changing the current selection: copy resolution drops
    /// blank items, so a keyful but blank logical selection could otherwise
    /// exist with no copyable text.
    pub fn select_all(
        keys: &[TextSelectionContentKey],
        text: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        if keys.is_empty() || text.trim().is_empty() {
            return;
        }
        let window_id = window.window_handle().window_id();
        let state = WindowSelectionState::acquire(window_id, cx);
        let handlers = state.update(cx, |state, cx| {
            let handlers = state.clear_state(cx);
            state.logical_selection = Some(LogicalSelection::new(keys.to_vec(), text.to_string()));
            state.publish_snapshots(cx);
            handlers
        });
        dispatch_clear_handlers(handlers, cx);
        window.refresh();
    }

    /// Returns whether the window has a geometry selection or any participant
    /// has an active participant-local selection such as select-all.
    pub fn has_selection(window: &mut Window, cx: &mut App) -> bool {
        live_text_selection_state(window, cx).is_some_and(|state| state.read(cx).has_selection(cx))
    }

    /// Clears window selection and every participant's local selection.
    pub fn clear(window: &mut Window, cx: &mut App) {
        if let Some(state) = live_text_selection_state(window, cx) {
            let handlers = state.update(cx, |state, cx| state.clear_state(cx));
            dispatch_clear_handlers(handlers, cx);
        }
    }

    /// Clears selection for a known window identifier.
    ///
    /// Prefer [`Self::clear`] when a window reference is available. This
    /// narrow entry point supports hosts retiring deprecated window wrappers.
    pub fn clear_for_window(window_id: gpui::WindowId, cx: &mut App) {
        clear_window_text_selection(window_id, cx);
    }

    /// Ends the current drag while leaving its selection visible.
    pub fn end(window: &mut Window, cx: &mut App) {
        if let Some(state) = live_text_selection_state(window, cx) {
            state.update(cx, |state, cx| state.end(cx));
        }
    }
    /// Starts a drag after an interactive participant crosses its movement threshold.
    pub(crate) fn begin_drag(
        anchor: Point<Pixels>,
        cursor: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(state) = live_text_selection_state(window, cx) else {
            return;
        };
        state.update(cx, |state, cx| {
            state.begin_in_window(anchor, false, window, cx);
            state.update_in_window(cursor, window, cx);
        });
        WindowSelectionState::resolve_content_keys(&state, cx);
        window.refresh();
    }

    /// Advances an in-progress drag to the window's current pointer position.
    ///
    /// Owners of scrollable selection hosts call this from their auto-scroll
    /// tick, after the frame that mounted newly scrolled-in participants: the
    /// endpoint hit-test then resolves against that fresh geometry even while
    /// the pointer is stationary. Unlike a pointer move, this never
    /// recomputes auto-scroll; the caller owns the scroll viewport and drives
    /// the scroll itself. Returns whether a gesture is still active.
    pub fn update_drag_at_pointer(window: &mut Window, cx: &mut App) -> bool {
        let Some(state) = live_text_selection_state(window, cx) else {
            return false;
        };
        let position = window.mouse_position();
        let is_selecting = state.update(cx, |state, cx| {
            if state.is_selecting && !cx.has_active_drag() {
                state.update_impl(position, Some(&*window), cx);
            }
            state.is_selecting
        });
        if is_selecting {
            WindowSelectionState::resolve_content_keys(&state, cx);
            window.refresh();
        }
        is_selecting
    }

    /// Subscribes to window-scoped drag auto-scroll commands.
    ///
    /// `Some(delta)` starts or retargets stationary-drag scrolling (positive
    /// delta scrolls down) and `None` stops it. Commands are deduplicated and
    /// survive participants being dropped and recreated by virtualization, so
    /// a host needs exactly one subscription per window, not one per leaf.
    /// Acquiring the command source here also makes it available before the
    /// selection layer's first paint.
    ///
    /// The returned [`AutoScrollLease`] retains both the window selection
    /// state entity and the GPUI subscription. The host must store it for the
    /// lifetime of its participation; dropping it stops receiving commands.
    #[must_use = "retain the AutoScrollLease or drag auto-scroll commands are dropped"]
    pub fn subscribe_auto_scroll(
        window: &mut Window,
        cx: &mut App,
        mut callback: impl FnMut(Option<Pixels>, &mut App) + 'static,
    ) -> AutoScrollLease {
        let window_id = window.window_handle().window_id();
        let state = WindowSelectionState::acquire(window_id, cx);
        let source = state.update(cx, |state, cx| state.ensure_auto_scroll_source(cx));
        let subscription =
            cx.subscribe(&source, move |_, event: &TextSelectionEvent, cx| {
                if let TextSelectionEvent::AutoScroll(delta) = event {
                    callback(*delta, cx);
                }
            });
        AutoScrollLease {
            state,
            _subscription: subscription,
        }
    }

    /// Activates the opaque selection scope for this window.
    pub fn activate_scope(scope: TextSelectionScopeId, window: &mut Window, cx: &mut App) {
        let Some(state) = WindowSelectionState::existing(window, cx) else {
            if !cx.has_global::<PendingTextSelectionScopes>() {
                cx.set_global(PendingTextSelectionScopes::default());
            }
            cx.global_mut::<PendingTextSelectionScopes>()
                .0
                .insert(window.window_handle().window_id(), scope);
            return;
        };
        let handlers = state.update(cx, |state, cx| state.set_active_scope_state(scope, cx));
        dispatch_clear_handlers(handlers, cx);
    }
}

/// A zero-sized root layer which enables text selection for a window.
///
/// Mount one as the root's first child. Its stable `"window-text-selection"`
/// element identity retains the window-local selection entity across frames.
pub struct TextSelectionLayer;

pub(crate) fn text_selection_scope(
    scope: TextSelectionScopeId,
    element: impl IntoElement,
) -> impl IntoElement {
    TextSelectionScopeMarker {
        scope,
        element: element.into_element(),
    }
}

struct TextSelectionScopeMarker<E> {
    scope: TextSelectionScopeId,
    element: E,
}

impl<E: Element> IntoElement for TextSelectionScopeMarker<E> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<E: Element> Element for TextSelectionScopeMarker<E> {
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = E::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        self.element.id()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.element.source_location()
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let window_id = window.window_handle().window_id();
        with_text_selection_scope(window_id, self.scope, cx, |cx| {
            self.element.request_layout(id, inspector_id, window, cx)
        })
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let window_id = window.window_handle().window_id();
        with_text_selection_scope(window_id, self.scope, cx, |cx| {
            self.element
                .prepaint(id, inspector_id, bounds, request_layout, window, cx)
        })
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let window_id = window.window_handle().window_id();
        with_text_selection_scope(window_id, self.scope, cx, |cx| {
            self.element.paint(
                id,
                inspector_id,
                bounds,
                request_layout,
                prepaint,
                window,
                cx,
            );
        });
    }
}

#[doc(hidden)]
pub struct TextSelectionLayerPrepaintState(Entity<WindowSelectionState>);

impl IntoElement for TextSelectionLayer {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextSelectionLayer {
    type RequestLayoutState = ();
    type PrepaintState = TextSelectionLayerPrepaintState;

    fn id(&self) -> Option<ElementId> {
        Some("window-text-selection".into())
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
        (window.request_layout(Style::default(), [], cx), ())
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // Automatic participant order is paint order within this frame. Keep
        // this lifecycle in base so base-only applications do not need a
        // separate root component to reset it. Otherwise, registering the
        // first of two selected TextViews temporarily reverses their order
        // against the previous frame and alternates coverage forever.
        GlobalState::init(cx);
        GlobalState::global_mut(cx).begin_selection_frame();
        TextSelectionLayerPrepaintState(retain_text_selection_state(global_id, window, cx))
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        state: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        paint_text_selection(&state.0, window, cx);
    }
}

fn retain_text_selection_state(
    global_id: Option<&GlobalElementId>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<WindowSelectionState> {
    let window_id = window.window_handle().window_id();
    let state = window.with_element_state::<Entity<WindowSelectionState>, _>(
        global_id.expect("TextSelection has a stable element id"),
        |retained, _| {
            let state = retained.unwrap_or_else(|| WindowSelectionState::acquire(window_id, cx));
            (state.clone(), state)
        },
    );
    if !cx.has_global::<SelectionStateRegistry>() {
        cx.set_global(SelectionStateRegistry::default());
    }
    cx.global_mut::<SelectionStateRegistry>()
        .0
        .insert(window_id, state.downgrade());
    state
}

fn paint_text_selection(state: &Entity<WindowSelectionState>, window: &mut Window, cx: &mut App) {
    if state.update(cx, |state, _| state.schedule_finish_frame()) {
        let state = state.downgrade();
        window.defer(cx, move |_, cx| {
            let Some(state) = state.upgrade() else {
                return;
            };
            let handlers = state.update(cx, |state, cx| state.finish_frame(cx));
            dispatch_clear_handlers(handlers, cx);
        });
    }

    let mouse_down_state = state.downgrade();
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
        if event.button != MouseButton::Left {
            return;
        }
        let Some(state) = mouse_down_state.upgrade() else {
            return;
        };
        if phase.capture() {
            GlobalState::init(cx);
            GlobalState::reset_text_selection_suppression(cx);
            let handlers = state.update(cx, |state, cx| {
                if state.mouse_down_prepared {
                    return Vec::new();
                }
                state.mouse_down_prepared = true;
                state.prepare_for_mouse_down(event.click_count == 1 && event.modifiers.shift, cx)
            });
            dispatch_clear_handlers(handlers, cx);
        } else if event.click_count == 1 {
            if GlobalState::is_text_selection_suppressed(cx) {
                state.update(cx, |state, _| state.pending_extension_anchor = None);
                return;
            }
            state.update(cx, |state, cx| {
                if !state.is_selecting {
                    state.begin_in_window(event.position, event.modifiers.shift, window, cx)
                }
            });
            WindowSelectionState::resolve_content_keys(&state, cx);
        } else if event.click_count >= 2 {
            if GlobalState::is_text_selection_suppressed(cx) {
                return;
            }
            state.update(cx, |state, cx| {
                state.select_at(event.position, event.click_count, window, cx)
            });
            WindowSelectionState::resolve_content_keys(&state, cx);
        }
    });

    let mouse_move_state = state.downgrade();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if !phase.bubble() {
            return;
        }
        let Some(state) = mouse_move_state.upgrade() else {
            return;
        };
        let is_selecting = state.update(cx, |state, cx| {
            state.update_in_window(event.position, window, cx);
            state.is_selecting
        });
        WindowSelectionState::resolve_content_keys(&state, cx);
        if is_selecting {
            window.refresh();
        }
    });

    let mouse_up_state = state.downgrade();
    window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
        if !phase.bubble() {
            return;
        }
        let Some(state) = mouse_up_state.upgrade() else {
            return;
        };
        state.update(cx, |state, cx| {
            state.mouse_down_prepared = false;
            state.end(cx)
        });
    });

    let scroll_state = state.downgrade();
    window.on_mouse_event(move |_: &ScrollWheelEvent, phase, window, cx| {
        if !phase.bubble() {
            return;
        }
        let Some(state) = scroll_state.upgrade() else {
            return;
        };
        let position = window.mouse_position();
        state.update(cx, |state, cx| state.update_in_window(position, window, cx));
        WindowSelectionState::resolve_content_keys(&state, cx);
    });
}

fn live_text_selection_state(
    window: &Window,
    cx: &mut App,
) -> Option<Entity<WindowSelectionState>> {
    WindowSelectionState::existing(window, cx)
}

pub(crate) fn clear_window_text_selection(window_id: gpui::WindowId, cx: &mut App) {
    if !cx.has_global::<SelectionStateRegistry>() {
        return;
    }
    let Some(state) = cx
        .global::<SelectionStateRegistry>()
        .0
        .get(&window_id)
        .and_then(WeakEntity::upgrade)
    else {
        return;
    };
    let handlers = state.update(cx, |state, cx| state.clear_state(cx));
    dispatch_clear_handlers(handlers, cx);
}

#[cfg(test)]
mod virtualization_copy_tests {
    use super::*;
    use gpui::{size, HitboxBehavior, Render, TestAppContext};
    use std::{cell::RefCell, rc::Rc};

    #[derive(Clone)]
    struct Registration {
        selection: TextSelectionHandle,
        y: f32,
        document_order: u64,
    }

    struct RegistrationView {
        selection_state: Rc<RefCell<WindowSelectionState>>,
        registrations: Rc<RefCell<Vec<Registration>>>,
    }

    struct RegistrationElement {
        selection_state: Rc<RefCell<WindowSelectionState>>,
        registrations: Rc<RefCell<Vec<Registration>>>,
    }

    impl IntoElement for RegistrationElement {
        type Element = Self;

        fn into_element(self) -> Self::Element {
            self
        }
    }

    impl Element for RegistrationElement {
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
            (window.request_layout(Style::default(), [], cx), ())
        }

        fn prepaint(
            &mut self,
            _: Option<&GlobalElementId>,
            _: Option<&InspectorElementId>,
            _: Bounds<Pixels>,
            _: &mut Self::RequestLayoutState,
            window: &mut Window,
            cx: &mut App,
        ) -> Self::PrepaintState {
            for registration in self.registrations.borrow().iter() {
                let bounds = Bounds::new(
                    point(px(0.), px(registration.y)),
                    size(px(100.), px(10.)),
                );
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                self.selection_state.borrow_mut().register_participant(
                    registration.selection.clone(),
                    TextSelectionRegistration::new(hitbox, bounds)
                        .with_document_order(registration.document_order)
                        .with_text_bounds(vec![bounds]),
                    cx,
                );
            }
        }

        fn paint(
            &mut self,
            _: Option<&GlobalElementId>,
            _: Option<&InspectorElementId>,
            _: Bounds<Pixels>,
            _: &mut Self::RequestLayoutState,
            _: &mut Self::PrepaintState,
            _: &mut Window,
            _: &mut App,
        ) {
        }
    }

    impl Render for RegistrationView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            RegistrationElement {
                selection_state: Rc::clone(&self.selection_state),
                registrations: Rc::clone(&self.registrations),
            }
        }
    }

    fn registration(selection: &TextSelectionHandle, y: f32, document_order: u64) -> Registration {
        Registration {
            selection: selection.clone(),
            y,
            document_order,
        }
    }

    #[gpui::test]
    fn virtual_participant_retains_copy_callback_after_element_owner_drops(
        cx: &mut TestAppContext,
    ) {
        let selection_state = Rc::new(RefCell::new(WindowSelectionState::default()));
        let registrations = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view({
            let selection_state = Rc::clone(&selection_state);
            let registrations = Rc::clone(&registrations);
            move |_, _| RegistrationView {
                selection_state,
                registrations,
            }
        });
        cx.update(|window, cx| {
            let participant = TextSelectionHandle::new("stale", cx);
            registrations
                .borrow_mut()
                .push(registration(&participant, 0., 0));
            _ = window.draw(cx);
            let mut state = selection_state.borrow_mut();
            state.begin(point(px(1.), px(1.)), false, cx);
            state.update(point(px(8.), px(1.)), cx);
            state.end(cx);
            state.finish_frame(cx);
            drop(state);

            registrations.borrow_mut().clear();
            _ = window.draw(cx);
            selection_state.borrow_mut().finish_frame(cx);
            let weak_participant = participant.downgrade();
            drop(participant);

            assert_eq!(selection_state.borrow().selected_text(cx), "stale");
            assert!(weak_participant.upgrade().is_some());
            selection_state.borrow_mut().clear(cx);
            assert!(selection_state.borrow().virtual_copy_participants.is_empty());
        });
    }

    #[gpui::test]
    fn visible_endpoint_projects_when_other_endpoint_is_virtual(cx: &mut TestAppContext) {
        let selection_state = Rc::new(RefCell::new(WindowSelectionState::default()));
        let registrations = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view({
            let selection_state = Rc::clone(&selection_state);
            let registrations = Rc::clone(&registrations);
            move |_, _| RegistrationView {
                selection_state,
                registrations,
            }
        });
        cx.update(|window, cx| {
            let first = TextSelectionHandle::new("first", cx);
            let second = TextSelectionHandle::new("second", cx);
            *registrations.borrow_mut() = vec![
                registration(&first, 0., 0),
                registration(&second, 20., 1),
            ];
            _ = window.draw(cx);
            let mut state = selection_state.borrow_mut();
            state.begin(point(px(1.), px(1.)), false, cx);
            state.update(point(px(5.), px(25.)), cx);
            state.finish_frame(cx);
            drop(state);

            *registrations.borrow_mut() = vec![registration(&second, 20., 1)];
            _ = window.draw(cx);
            selection_state.borrow_mut().finish_frame(cx);

            let points = selection_state
                .borrow()
                .snapshot()
                .and_then(|snapshot| snapshot.window_points())
                .expect("one visible endpoint should keep a rendering projection");
            assert!(points.anchor().y < points.cursor().y);
            assert!(first.snapshot(cx).is_some());
            assert!(second.snapshot(cx).is_some());
        });
    }

    #[gpui::test]
    fn unresolved_copy_item_refuses_other_resolved_output(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let items = vec![
                CopyItem {
                    document_order: 0,
                    callback: Some(Rc::new(|_| Some("first".to_string()))),
                    fallback: "first fallback".to_string(),
                    separator_before: String::new(),
                },
                CopyItem {
                    document_order: 1,
                    callback: Some(Rc::new(|_| None)),
                    fallback: "second fallback".to_string(),
                    separator_before: "\n\n".to_string(),
                },
            ];

            assert_eq!(resolve_copy_items(items, cx), "");
        });
    }
}

#[cfg(test)]
mod window_auto_scroll_tests {
    use super::*;
    use gpui::{
        HitboxBehavior, Modifiers, ParentElement, Render, Styled, TestAppContext, div, size,
    };
    use std::{cell::RefCell, rc::Rc, time::Duration};

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
            (window.request_layout(Style::default(), [], cx), ())
        }

        fn prepaint(
            &mut self,
            _: Option<&GlobalElementId>,
            _: Option<&InspectorElementId>,
            _: Bounds<Pixels>,
            _: &mut Self::RequestLayoutState,
            window: &mut Window,
            cx: &mut App,
        ) -> Self::PrepaintState {
            for registration in self.registrations.borrow().iter() {
                let bounds = Bounds::new(
                    point(px(0.), px(registration.y)),
                    size(px(100.), px(10.)),
                );
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                let entity = WindowSelectionState::existing(window, cx)
                    .expect("subscribe_auto_scroll acquires the state before the first draw");
                entity.update(cx, |state, cx| {
                    state.register_participant(
                        registration.selection.clone(),
                        TextSelectionRegistration::new(hitbox, bounds)
                            .with_document_order(registration.document_order)
                            .with_text_bounds(vec![bounds]),
                        cx,
                    )
                });
            }
        }

        fn paint(
            &mut self,
            _: Option<&GlobalElementId>,
            _: Option<&InspectorElementId>,
            _: Bounds<Pixels>,
            _: &mut Self::RequestLayoutState,
            _: &mut Self::PrepaintState,
            _: &mut Window,
            _: &mut App,
        ) {
        }
    }

    impl Render for AutoScrollView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            // The layer installs the window mouse handlers that drive
            // gesture state; the element registers participants.
            div().size_full().child(TextSelectionLayer).child(AutoScrollElement {
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

    fn auto_scroll_window<'a>(
        cx: &'a mut TestAppContext,
        registrations: &Rc<RefCell<Vec<Registration>>>,
    ) -> (&'a mut gpui::VisualTestContext, Rc<RefCell<Option<AutoScrollLease>>>) {
        let lease = Rc::new(RefCell::new(None));
        let (_, window_cx) = cx.add_window_view({
            let registrations = Rc::clone(registrations);
            move |_, _| AutoScrollView { registrations }
        });
        // The whole window is the scroll viewport, so the bottom trigger sits
        // 16px above its 40px bottom edge.
        window_cx.simulate_resize(size(px(100.), px(40.)));
        (window_cx, lease)
    }

    #[gpui::test]
    fn pointer_move_after_mouse_up_or_clear_does_not_restart_auto_scroll(
        cx: &mut TestAppContext,
    ) {
        let registrations = Rc::new(RefCell::new(Vec::new()));
        let commands = Rc::new(RefCell::new(Vec::<Option<Pixels>>::new()));
        let (window_cx, lease) = auto_scroll_window(cx, &registrations);
        window_cx.update(|window, cx| {
            let observed = commands.clone();
            *lease.borrow_mut() = Some(TextSelection::subscribe_auto_scroll(window, cx, move |delta, _| {
                observed.borrow_mut().push(delta);
            }));
            let participant = TextSelectionHandle::new("line", cx);
            registrations
                .borrow_mut()
                .push(registration(&participant, 0., 0));
            _ = window.draw(cx);
        });

        // Gesture one ends through mouse-up.
        window_cx.simulate_mouse_down(point(px(5.), px(5.)), MouseButton::Left, Modifiers::default());
        window_cx.simulate_mouse_move(
            point(px(5.), px(30.)),
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        window_cx.simulate_mouse_up(point(px(5.), px(30.)), MouseButton::Left, Modifiers::default());
        assert!(commands.borrow().iter().any(Option::is_some));
        assert_eq!(commands.borrow().last(), Some(&None));
        let commands_after_mouse_up = commands.borrow().len();
        window_cx.simulate_mouse_move(point(px(5.), px(32.)), None, Modifiers::default());
        assert_eq!(commands.borrow().len(), commands_after_mouse_up);

        // Gesture two ends through clear.
        window_cx.simulate_mouse_down(point(px(5.), px(5.)), MouseButton::Left, Modifiers::default());
        window_cx.simulate_mouse_move(
            point(px(5.), px(34.)),
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        window_cx.update(TextSelection::clear);
        let commands_after_clear = commands.borrow().len();
        window_cx.simulate_mouse_move(point(px(5.), px(34.)), None, Modifiers::default());
        assert_eq!(commands.borrow().len(), commands_after_clear);
        assert_eq!(
            commands.borrow().clone(),
            vec![
                AutoScroll::compute_delta(px(30.), Bounds::new(point(px(0.), px(0.)), size(px(100.), px(40.)))),
                None,
                AutoScroll::compute_delta(px(34.), Bounds::new(point(px(0.), px(0.)), size(px(100.), px(40.)))),
                None,
            ]
        );
    }

    #[gpui::test]
    fn window_command_stream_survives_participant_churn_and_dedupes(
        cx: &mut TestAppContext,
    ) {
        let registrations = Rc::new(RefCell::new(Vec::new()));
        let commands = Rc::new(RefCell::new(Vec::<Option<Pixels>>::new()));
        let (window_cx, lease) = auto_scroll_window(cx, &registrations);
        let anchor = window_cx.update(|window, cx| {
            let observed = commands.clone();
            *lease.borrow_mut() = Some(TextSelection::subscribe_auto_scroll(window, cx, move |delta, _| {
                observed.borrow_mut().push(delta);
            }));
            let anchor = TextSelectionHandle::new("anchor", cx);
            registrations.borrow_mut().push(registration(&anchor, 0., 0));
            _ = window.draw(cx);
            anchor
        });

        window_cx.simulate_mouse_down(point(px(5.), px(5.)), MouseButton::Left, Modifiers::default());
        window_cx.simulate_mouse_move(
            point(px(5.), px(25.)),
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        // Repeating the same pointer position must not re-emit the command.
        window_cx.simulate_mouse_move(
            point(px(5.), px(25.)),
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        assert_eq!(commands.borrow().len(), 1);
        assert!(commands.borrow()[0].is_some());

        // Replace the anchor's leaf entirely: the stream must neither stop
        // (no spurious None) nor restart while the drag continues.
        window_cx.update(|window, cx| {
            let replacement = TextSelectionHandle::new("replacement", cx);
            *registrations.borrow_mut() = vec![registration(&replacement, 0., 0)];
            _ = window.draw(cx);
        });
        window_cx.simulate_mouse_move(
            point(px(5.), px(25.)),
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        assert_eq!(commands.borrow().len(), 1);

        window_cx.simulate_mouse_up(point(px(5.), px(25.)), MouseButton::Left, Modifiers::default());
        assert_eq!(commands.borrow().clone(), vec![commands.borrow()[0], None]);
        let anchor_alive = window_cx.update(|_, cx| anchor.snapshot(cx).is_some());
        assert!(anchor_alive);
    }

    #[gpui::test]
    fn stationary_drag_tick_updates_the_endpoint_against_a_newly_mounted_participant(
        cx: &mut TestAppContext,
    ) {
        let registrations = Rc::new(RefCell::new(Vec::new()));
        let commands = Rc::new(RefCell::new(Vec::<Option<Pixels>>::new()));
        let (window_cx, lease) = auto_scroll_window(cx, &registrations);
        let first = window_cx.update(|window, cx| {
            let observed = commands.clone();
            *lease.borrow_mut() = Some(TextSelection::subscribe_auto_scroll(window, cx, move |delta, _| {
                observed.borrow_mut().push(delta);
            }));
            let first = TextSelectionHandle::new("first", cx);
            registrations.borrow_mut().push(registration(&first, 0., 0));
            _ = window.draw(cx);
            first
        });

        window_cx.simulate_mouse_down(point(px(5.), px(5.)), MouseButton::Left, Modifiers::default());
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

        let is_selecting = window_cx.update(TextSelection::update_drag_at_pointer);
        assert!(is_selecting);
        assert_eq!(commands.borrow().len(), 1);
        let cursor = window_cx.update(|window, cx| {
            second
                .snapshot(cx)
                .expect("newly mounted participant joins the selection")
                .cursor()
                .entity_id()
                .map(|id| (id == second.entity_id(), TextSelection::has_selection(window, cx)))
        });
        assert_eq!(cursor, Some((true, true)));
        let points = window_cx.update(|_, cx| {
            first
                .snapshot(cx)
                .and_then(|snapshot| snapshot.window_points())
        });
        let points = points.expect("window points still project");
        assert!(points.anchor().y < points.cursor().y);
    }

    #[gpui::test]
    fn subscribe_before_first_prepaint_then_drag_receives_some_then_none(
        cx: &mut TestAppContext,
    ) {
        let registrations = Rc::new(RefCell::new(Vec::new()));
        let commands = Rc::new(RefCell::new(Vec::<Option<Pixels>>::new()));
        let (window_cx, lease) = auto_scroll_window(cx, &registrations);

        // Subscribe BEFORE any draw/prepaint — the selection layer has not
        // been mounted yet. The lease must retain the state entity so it
        // survives until the first paint.
        window_cx.update(|window, cx| {
            let observed = commands.clone();
            *lease.borrow_mut() = Some(TextSelection::subscribe_auto_scroll(window, cx, move |delta, _| {
                observed.borrow_mut().push(delta);
            }));
        });
        assert!(
            lease.borrow().is_some(),
            "the lease is held before the first draw"
        );

        // Now mount the layer and register a participant.
        window_cx.update(|window, cx| {
            let participant = TextSelectionHandle::new("line", cx);
            registrations
                .borrow_mut()
                .push(registration(&participant, 0., 0));
            _ = window.draw(cx);
        });

        // Drag into the bottom edge zone: should publish Some.
        window_cx.simulate_mouse_down(point(px(5.), px(5.)), MouseButton::Left, Modifiers::default());
        window_cx.simulate_mouse_move(
            point(px(5.), px(30.)),
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        assert!(
            commands.borrow().iter().any(Option::is_some),
            "drag into the bottom edge publishes Some after subscribing before prepaint"
        );

        // Mouse up: should publish None.
        window_cx.simulate_mouse_up(point(px(5.), px(30.)), MouseButton::Left, Modifiers::default());
        assert_eq!(
            commands.borrow().last(),
            Some(&None),
            "mouse up publishes None"
        );
    }

    #[gpui::test]
    fn anchor_virtualized_dead_zone_publishes_none_then_edge_publishes_some_again(
        cx: &mut TestAppContext,
    ) {
        let registrations = Rc::new(RefCell::new(Vec::new()));
        let commands = Rc::new(RefCell::new(Vec::<Option<Pixels>>::new()));
        let (window_cx, lease) = auto_scroll_window(cx, &registrations);
        window_cx.update(|window, cx| {
            let observed = commands.clone();
            *lease.borrow_mut() = Some(TextSelection::subscribe_auto_scroll(window, cx, move |delta, _| {
                observed.borrow_mut().push(delta);
            }));
            let anchor = TextSelectionHandle::new("anchor", cx);
            registrations.borrow_mut().push(registration(&anchor, 0., 0));
            _ = window.draw(cx);
        });

        // Start a drag with the pointer in the bottom edge zone.
        window_cx.simulate_mouse_down(point(px(5.), px(5.)), MouseButton::Left, Modifiers::default());
        window_cx.simulate_mouse_move(
            point(px(5.), px(30.)),
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        assert!(commands.borrow().last().is_some_and(Option::is_some));

        // Virtualize the anchor: remove it and draw, simulating the scroll
        // host dropping the anchor's leaf. The cursor endpoint may still
        // be registered, but the anchor is gone.
        window_cx.update(|window, cx| {
            // Replace registrations with a cursor-only participant at a
            // different position (outside the edge zone).
            let cursor = TextSelectionHandle::new("cursor", cx);
            *registrations.borrow_mut() = vec![registration(&cursor, 0., 0)];
            _ = window.draw(cx);
        });

        // Move pointer to the dead zone (middle of the viewport, y=20).
        // With the anchor virtualized, the viewport falls back to the cursor
        // participant. y=20 is in the dead zone (top trigger is y<24, bottom
        // trigger is y>24 for a 40px viewport), so it should publish None.
        window_cx.simulate_mouse_move(
            point(px(5.), px(20.)),
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        assert_eq!(
            commands.borrow().last(),
            Some(&None),
            "dead zone after virtualization publishes None"
        );

        // Move pointer back to the bottom edge zone: should publish Some again.
        window_cx.simulate_mouse_move(
            point(px(5.), px(30.)),
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        assert!(
            commands.borrow().last().is_some_and(Option::is_some),
            "pointer edge after virtualization publishes Some again"
        );
    }

    #[gpui::test]
    fn reverse_direction_after_bound_while_held(cx: &mut TestAppContext) {
        let registrations = Rc::new(RefCell::new(Vec::new()));
        let commands = Rc::new(RefCell::new(Vec::<Option<Pixels>>::new()));
        let (window_cx, lease) = auto_scroll_window(cx, &registrations);
        window_cx.update(|window, cx| {
            let observed = commands.clone();
            *lease.borrow_mut() = Some(TextSelection::subscribe_auto_scroll(window, cx, move |delta, _| {
                observed.borrow_mut().push(delta);
            }));
            let participant = TextSelectionHandle::new("line", cx);
            registrations.borrow_mut().push(registration(&participant, 0., 0));
            _ = window.draw(cx);
        });

        // Drag to the bottom edge: Some(positive).
        window_cx.simulate_mouse_down(point(px(5.), px(5.)), MouseButton::Left, Modifiers::default());
        window_cx.simulate_mouse_move(
            point(px(5.), px(30.)),
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        let bottom_delta = *commands.borrow().last().unwrap();
        assert!(bottom_delta.is_some_and(|d| d > Pixels::ZERO));

        // Reverse to the top edge without releasing: Some(negative).
        window_cx.simulate_mouse_move(
            point(px(5.), px(10.)),
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        let top_delta = *commands.borrow().last().unwrap();
        assert!(
            top_delta.is_some_and(|d| d < Pixels::ZERO),
            "reversing direction while held publishes a negative delta"
        );

        // Back to dead zone: None.
        window_cx.simulate_mouse_move(
            point(px(5.), px(20.)),
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        assert_eq!(
            commands.borrow().last(),
            Some(&None),
            "returning to the dead zone publishes None"
        );
    }

    #[gpui::test]
    fn sustained_drag_advances_offset_twice_then_stops_on_mouse_up(
        cx: &mut TestAppContext,
    ) {
        // This test exercises the host-side repeat loop by simulating a
        // drag with active auto-scroll, advancing the clock through two
        // 16ms ticks (with draws/remounts between), verifying the offset
        // changes twice and the selection endpoint advances under a
        // stationary pointer, then confirming mouse-up/clear/None prevents
        // further ticks.
        //
        // Because the host loop lives in ChatView (not the vendor crate),
        // this test verifies the vendor-side primitives that the host loop
        // relies on: update_drag_at_pointer advances the endpoint, and the
        // command stream delivers Some/None correctly across ticks.
        let registrations = Rc::new(RefCell::new(Vec::new()));
        let commands = Rc::new(RefCell::new(Vec::<Option<Pixels>>::new()));
        let (window_cx, lease) = auto_scroll_window(cx, &registrations);
        let first = window_cx.update(|window, cx| {
            let observed = commands.clone();
            *lease.borrow_mut() = Some(TextSelection::subscribe_auto_scroll(window, cx, move |delta, _| {
                observed.borrow_mut().push(delta);
            }));
            let first = TextSelectionHandle::new("first", cx);
            registrations.borrow_mut().push(registration(&first, 0., 0));
            _ = window.draw(cx);
            first
        });

        // Start drag at bottom edge.
        window_cx.simulate_mouse_down(point(px(5.), px(5.)), MouseButton::Left, Modifiers::default());
        window_cx.simulate_mouse_move(
            point(px(5.), px(30.)),
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        assert!(commands.borrow().last().is_some_and(Option::is_some));

        // Tick 1: mount a new participant under the stationary pointer and
        // advance the drag. The endpoint should move to the new participant.
        let second = window_cx.update(|window, cx| {
            let second = TextSelectionHandle::new("second", cx);
            registrations.borrow_mut().push(registration(&second, 20., 1));
            _ = window.draw(cx);
            second
        });
        window_cx.run_until_parked();
        let tick1_selecting = window_cx.update(TextSelection::update_drag_at_pointer);
        assert!(tick1_selecting, "tick 1: drag still active");

        let cursor_after_tick1 = window_cx.update(|_, cx| {
            second
                .snapshot(cx)
                .and_then(|s| s.cursor().entity_id())
        });
        assert_eq!(
            cursor_after_tick1,
            Some(second.entity_id()),
            "tick 1: cursor endpoint advanced to the newly mounted participant"
        );

        // Tick 2: the previous tick scrolled the host, so the row under the
        // stationary pointer is replaced by a newly mounted one (the anchor
        // row stays registered). The endpoint must advance to that new row
        // on the second tick; an exact-overlap push could never win the
        // hit-test's document-order tie-break.
        let third = window_cx.update(|window, cx| {
            let third = TextSelectionHandle::new("third", cx);
            *registrations.borrow_mut() =
                vec![registration(&first, 0., 0), registration(&third, 20., 1)];
            _ = window.draw(cx);
            third
        });
        window_cx.run_until_parked();
        let tick2_selecting = window_cx.update(TextSelection::update_drag_at_pointer);
        assert!(tick2_selecting, "tick 2: drag still active");

        let cursor_after_tick2 = window_cx.update(|_, cx| {
            third
                .snapshot(cx)
                .and_then(|s| s.cursor().entity_id())
        });
        assert_eq!(
            cursor_after_tick2,
            Some(third.entity_id()),
            "tick 2: cursor endpoint advanced to the newly mounted participant"
        );

        // Mouse up: publishes None, and further ticks must not fire.
        window_cx.simulate_mouse_up(point(px(5.), px(30.)), MouseButton::Left, Modifiers::default());
        assert_eq!(
            commands.borrow().last(),
            Some(&None),
            "mouse up publishes None"
        );
        let commands_before = commands.borrow().len();

        // Advance the clock; no further commands should arrive.
        cx.executor().advance_clock(Duration::from_millis(48));
        cx.run_until_parked();
        assert_eq!(
            commands.borrow().len(),
            commands_before,
            "no further commands after mouse up/clear/None"
        );
    }
}

#[cfg(test)]
mod logical_select_all_tests {
    use super::*;
    use gpui::{
        div, point, px, size, HitboxBehavior, Modifiers, MouseButton, ParentElement, Render,
        SharedString, Styled, StyledText, TestAppContext,
    };
    use std::{cell::RefCell, rc::Rc};

    fn key(value: u64) -> TextSelectionContentKey {
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

    struct LogicalSelectionView {
        registrations: Rc<RefCell<Vec<KeyedRegistration>>>,
    }

    struct LogicalSelectionElement {
        registrations: Rc<RefCell<Vec<KeyedRegistration>>>,
    }

    impl IntoElement for LogicalSelectionElement {
        type Element = Self;

        fn into_element(self) -> Self::Element {
            self
        }
    }

    impl Element for LogicalSelectionElement {
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
            (window.request_layout(Style::default(), [], cx), ())
        }

        fn prepaint(
            &mut self,
            _: Option<&GlobalElementId>,
            _: Option<&InspectorElementId>,
            _: Bounds<Pixels>,
            _: &mut Self::RequestLayoutState,
            window: &mut Window,
            cx: &mut App,
        ) -> Self::PrepaintState {
            for registration in self.registrations.borrow().iter() {
                let bounds = Bounds::new(
                    point(px(0.), px(registration.y)),
                    size(px(100.), px(10.)),
                );
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                let entity = WindowSelectionState::existing(window, cx).unwrap_or_else(|| {
                    WindowSelectionState::acquire(window.window_handle().window_id(), cx)
                });
                entity.update(cx, |state, cx| {
                    state.register_participant(
                        registration.selection.clone(),
                        TextSelectionRegistration::new(hitbox, bounds)
                            .with_document_order(registration.document_order)
                            .with_text_bounds(vec![bounds])
                            .with_scope(registration.scope)
                            .with_content_key(registration.content_key),
                        cx,
                    )
                });
            }
        }

        fn paint(
            &mut self,
            _: Option<&GlobalElementId>,
            _: Option<&InspectorElementId>,
            _: Bounds<Pixels>,
            _: &mut Self::RequestLayoutState,
            _: &mut Self::PrepaintState,
            _: &mut Window,
            _: &mut App,
        ) {
        }
    }

    impl Render for LogicalSelectionView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(TextSelectionLayer).child(
                LogicalSelectionElement {
                    registrations: Rc::clone(&self.registrations),
                },
            )
        }
    }

    struct RunLayoutView {
        texts: Vec<SharedString>,
        layouts: Vec<TextLayout>,
    }

    impl Render for RunLayoutView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.layouts.clear();
            let children = self
                .texts
                .iter()
                .enumerate()
                .map(|(index, text)| {
                    let text = StyledText::new(text.clone());
                    self.layouts.push(text.layout().clone());
                    div().absolute().top(px(index as f32 * 40.)).child(text)
                })
                .collect::<Vec<_>>();
            div().size_full().children(children)
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

    fn keyed_window(
        cx: &mut TestAppContext,
    ) -> (Rc<RefCell<Vec<KeyedRegistration>>>, &mut gpui::VisualTestContext) {
        let registrations = Rc::new(RefCell::new(Vec::new()));
        let (_, window_cx) = cx.add_window_view({
            let registrations = Rc::clone(&registrations);
            move |_, _| LogicalSelectionView { registrations }
        });
        (registrations, window_cx)
    }

    #[gpui::test]
    fn logical_select_all_publishes_frozen_copy_and_all_keys(cx: &mut TestAppContext) {
        let (registrations, window_cx) = keyed_window(cx);
        window_cx.update(|window, cx| {
            let first = TextSelectionHandle::new("first text", cx);
            let second = TextSelectionHandle::new("second text", cx);
            *registrations.borrow_mut() = vec![
                keyed(&first, 0., 0, key(1)),
                keyed(&second, 20., 1, key(2)),
            ];
            _ = window.draw(cx);

            TextSelection::select_all(&[key(1), key(2)], "frozen whole content", window, cx);

            assert!(TextSelection::has_selection(window, cx));
            assert_eq!(
                TextSelection::selected_text(window, cx),
                "frozen whole content"
            );
            assert_eq!(
                TextSelection::selected_content_keys(window, cx),
                Some(vec![key(1), key(2)])
            );
            assert_eq!(
                first.snapshot(cx).unwrap().coverage(),
                TextSelectionCoverage::Full
            );
            assert_eq!(
                second.snapshot(cx).unwrap().coverage(),
                TextSelectionCoverage::Full
            );

            // Replacing an existing logical selection refreezes the text.
            TextSelection::select_all(&[key(1), key(2)], "replacement", window, cx);
            assert_eq!(TextSelection::selected_text(window, cx), "replacement");
        });
    }

    #[gpui::test]
    fn select_all_rejects_empty_keys_or_blank_text(cx: &mut TestAppContext) {
        let (registrations, window_cx) = keyed_window(cx);
        window_cx.update(|window, cx| {
            let participant = TextSelectionHandle::new("text", cx);
            *registrations.borrow_mut() = vec![keyed(&participant, 0., 0, key(1))];
            _ = window.draw(cx);

            TextSelection::select_all(&[], "frozen", window, cx);
            assert!(!TextSelection::has_selection(window, cx));
            TextSelection::select_all(&[key(1)], "", window, cx);
            assert!(!TextSelection::has_selection(window, cx));
            TextSelection::select_all(&[key(1)], "   ", window, cx);
            assert!(!TextSelection::has_selection(window, cx));
            assert!(participant.snapshot(cx).is_none());
        });
    }

    #[gpui::test]
    fn logical_selection_marks_only_frozen_keys_in_the_active_scope(cx: &mut TestAppContext) {
        let (registrations, window_cx) = keyed_window(cx);
        window_cx.update(|window, cx| {
            let first = TextSelectionHandle::new("first", cx);
            let second = TextSelectionHandle::new("second", cx);
            let third = TextSelectionHandle::new("third", cx);
            let mut modal = keyed(&third, 40., 2, key(3));
            modal.scope = TextSelectionScopeId::from_raw(5);
            *registrations.borrow_mut() = vec![
                keyed(&first, 0., 0, key(1)),
                keyed(&second, 20., 1, key(2)),
                modal,
            ];
            _ = window.draw(cx);

            TextSelection::select_all(&[key(1), key(3)], "frozen", window, cx);

            assert_eq!(
                first.snapshot(cx).unwrap().coverage(),
                TextSelectionCoverage::Full
            );
            assert!(
                second.snapshot(cx).is_none(),
                "a key outside the frozen set is not marked"
            );
            assert!(
                third.snapshot(cx).is_none(),
                "a frozen key outside the active scope is not marked"
            );
        });
    }

    #[gpui::test]
    fn virtualized_logical_participant_rejoins_by_content_key(cx: &mut TestAppContext) {
        let (registrations, window_cx) = keyed_window(cx);
        let participant = window_cx.update(|window, cx| {
            let participant = TextSelectionHandle::new("mountable", cx);
            *registrations.borrow_mut() = vec![keyed(&participant, 0., 0, key(5))];
            _ = window.draw(cx);
            participant
        });
        window_cx.update(|window, cx| {
            TextSelection::select_all(&[key(5)], "frozen", window, cx);
            assert!(participant.snapshot(cx).is_some());

            // Unmount the participant in its own update cycle so the
            // post-frame sweep observes it stale: it must clear the
            // participant's state instead of retaining it virtually.
            registrations.borrow_mut().clear();
            _ = window.draw(cx);
        });
        window_cx.update(|window, cx| {
            let state = WindowSelectionState::existing(window, cx).unwrap();
            state.update(cx, |state, _| {
                assert!(
                    state.virtual_copy_participants.is_empty(),
                    "logical participants are not retained virtually"
                );
            });
            assert!(participant.snapshot(cx).is_none());

            // The frozen copy survives without the participant mounted.
            assert!(TextSelection::has_selection(window, cx));
            assert_eq!(TextSelection::selected_text(window, cx), "frozen");
            assert_eq!(
                TextSelection::selected_content_keys(window, cx),
                Some(vec![key(5)])
            );
        });

        // Remount with the same content key: republished as Full.
        window_cx.update(|window, cx| {
            registrations
                .borrow_mut()
                .push(keyed(&participant, 0., 0, key(5)));
            _ = window.draw(cx);
            assert_eq!(
                participant.snapshot(cx).unwrap().coverage(),
                TextSelectionCoverage::Full
            );

            // A participant mounting with an unknown key stays unmarked.
            let outsider = TextSelectionHandle::new("outsider", cx);
            registrations
                .borrow_mut()
                .push(keyed(&outsider, 20., 1, key(6)));
            _ = window.draw(cx);
            assert!(outsider.snapshot(cx).is_none());
        });
    }

    #[gpui::test]
    fn clear_and_new_pointer_press_end_logical_selection(cx: &mut TestAppContext) {
        let (registrations, window_cx) = keyed_window(cx);
        window_cx.update(|window, cx| {
            let participant = TextSelectionHandle::new("pointer text", cx);
            participant.resolve_content_key_with(|_, _| Some(key(1)), cx);
            registrations
                .borrow_mut()
                .push(keyed(&participant, 0., 0, key(1)));
            _ = window.draw(cx);

            TextSelection::select_all(&[key(1)], "frozen", window, cx);
            assert_eq!(
                TextSelection::selected_content_keys(window, cx),
                Some(vec![key(1)])
            );
        });

        // A new pointer press replaces the logical selection with a gesture.
        window_cx.simulate_mouse_down(point(px(5.), px(5.)), MouseButton::Left, Modifiers::default());
        window_cx.simulate_mouse_move(
            point(px(50.), px(5.)),
            Some(MouseButton::Left),
            Modifiers::default(),
        );
        window_cx.simulate_mouse_up(point(px(50.), px(5.)), MouseButton::Left, Modifiers::default());
        window_cx.update(|window, cx| {
            assert!(TextSelection::has_selection(window, cx));
            assert_eq!(TextSelection::selected_text(window, cx), "pointer text");
            assert_eq!(
                TextSelection::selected_content_keys(window, cx),
                Some(vec![key(1), key(1)]),
                "a pointer selection reports its endpoint keys"
            );

            TextSelection::clear(window, cx);
            assert_eq!(TextSelection::selected_content_keys(window, cx), None);
            assert!(!TextSelection::has_selection(window, cx));
        });
    }

    #[gpui::test]
    fn full_projection_without_geometry_returns_all_utf8_bytes(cx: &mut TestAppContext) {
        let view = cx.add_window({
            let texts: Vec<SharedString> = vec!["aé🙂z".into(), "".into()];
            move |_, _| RunLayoutView {
                texts,
                layouts: Vec::new(),
            }
        });
        cx.update_window(*view, |_, window, cx| {
            let _ = window.draw(cx);
        })
        .unwrap();
        let runs = cx.update(|cx| {
            let view = view.read(cx).unwrap();
            view.layouts
                .iter()
                .enumerate()
                .map(|(index, layout)| {
                    TextSelectionRun::new(
                        view.texts[index].clone(),
                        layout.clone(),
                        layout.bounds(),
                    )
                    .with_document_order(index as u64)
                })
                .collect::<Vec<_>>()
        });
        let endpoint = TextSelectionEndpoint::new(None, Point::default());
        let full_snapshot = TextSelectionSnapshot::new(endpoint, endpoint)
            .with_coverage(TextSelectionCoverage::Full);
        let bounded_snapshot = TextSelectionSnapshot::new(endpoint, endpoint);

        let full = project_ranges(Some(full_snapshot), &runs);
        assert_eq!(full.ranges(), &[Some(0..8), Some(0..0)]);
        assert!(full.is_active());

        let bounded = project_ranges(Some(bounded_snapshot), &runs);
        assert_eq!(bounded.ranges(), &[None, None]);
        assert!(bounded.is_active());

        // The handle-level projection follows the same rule, matching the
        // SelectableText paint path.
        let handle_projection = cx.update(|cx| {
            let handle = TextSelectionHandle::new("aé🙂z", cx);
            handle.0.update(cx, |state, cx| {
                state.set_snapshot(Some(full_snapshot), cx);
            });
            handle.update_runs(&runs[..1], cx)
        });
        assert_eq!(handle_projection.ranges(), &[Some(0..8)]);
        assert!(handle_projection.is_active());
    }
}
#[cfg(test)]
mod selectable_text_bridge_tests {
    use super::*;
    use crate::SelectableText;
    use gpui::{
        div, hsla, App, ParentElement, Render, SharedString, Styled, TestAppContext, TextRun,
        Window,
    };
    use std::{cell::Cell, rc::Rc};

    const BRIDGE_TEXT: &str = "bridge text";

    fn key(value: u64) -> TextSelectionContentKey {
        TextSelectionContentKey::new(value)
    }

    fn bridge_runs() -> Vec<TextRun> {
        vec![TextRun {
            len: BRIDGE_TEXT.len(),
            color: hsla(0., 0., 0., 1.),
            ..Default::default()
        }]
    }

    /// Returns the only registered participant, wrapped as a public handle.
    ///
    /// The participant is registered by the real `SelectableText` element;
    /// reading the window state only observes what it published.
    fn sole_participant(window: &Window, cx: &App) -> Option<TextSelectionHandle> {
        let state = WindowSelectionState::existing(window, cx)?;
        let participant = state
            .read(cx)
            .participants
            .values()
            .next()?
            .participant
            .upgrade()?;
        Some(TextSelectionHandle(participant))
    }

    struct BridgeView {
        mounted: Rc<Cell<bool>>,
    }

    impl Render for BridgeView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let mut root = div().size_full().child(TextSelectionLayer);
            if self.mounted.get() {
                root = root.child(
                    SelectableText::new(
                        SharedString::from("bridge-leaf"),
                        BRIDGE_TEXT,
                        bridge_runs(),
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

    #[gpui::test]
    fn selectable_text_rejoins_logical_selection_after_remount(cx: &mut TestAppContext) {
        let mounted = Rc::new(Cell::new(true));
        let (_, window_cx) = cx.add_window_view({
            let mounted = mounted.clone();
            move |_, _| BridgeView { mounted }
        });

        // The first paint registers the real element as a participant.
        let first = window_cx.update(|window, cx| {
            _ = window.draw(cx);
            let handle =
                sole_participant(window, cx).expect("SelectableText registers itself on paint");
            assert!(handle.snapshot(cx).is_none());
            handle
        });

        // A logical whole-content selection marks the registered element
        // Full, and the next paint projects the highlight over every byte.
        window_cx.update(|window, cx| {
            TextSelection::select_all(&[key(7)], "frozen bridge copy", window, cx);
            assert_eq!(
                first.snapshot(cx).expect("logically selected").coverage(),
                TextSelectionCoverage::Full
            );
            _ = window.draw(cx);
            let projection = first.project_cached_runs(cx);
            assert!(projection.is_active());
            assert_eq!(projection.ranges(), &[Some(0..BRIDGE_TEXT.len())]);
        });

        // Unmounting sweeps the participant away without retaining it; the
        // frozen logical selection survives on its own.
        window_cx.update(|window, cx| {
            mounted.set(false);
            _ = window.draw(cx);
        });
        window_cx.update(|window, cx| {
            assert!(sole_participant(window, cx).is_none());
            assert!(first.snapshot(cx).is_none());
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

        // Remounting a fresh SelectableText with the same content key
        // rejoins the logical selection as Full.
        window_cx.update(|window, cx| {
            mounted.set(true);
            _ = window.draw(cx);
            let second = sole_participant(window, cx)
                .expect("the remounted SelectableText registers again");
            assert_ne!(
                second.entity_id(),
                first.entity_id(),
                "the remounted element owns a fresh participant"
            );
            assert_eq!(
                second
                    .snapshot(cx)
                    .expect("remounted content key rejoins")
                    .coverage(),
                TextSelectionCoverage::Full
            );
            _ = window.draw(cx);
            assert_eq!(
                second.project_cached_runs(cx).ranges(),
                &[Some(0..BRIDGE_TEXT.len())]
            );
        });
    }
}
#[cfg(all(test, any()))]
mod tests {
    use super::*;
    use crate::ElementExt as _;
    use gpui::{
        Bounds, ContentMask, Context, Hitbox, HitboxBehavior, HitboxId, InteractiveElement as _,
        IntoElement, ParentElement as _, Render, SharedString, Styled as _, StyledText,
        TestAppContext, TextLayout, Window, div, point, prelude::FluentBuilder as _, px, size,
    };
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    struct FakeParticipant {
        selection: TextSelectionHandle,
    }

    struct WindowSelectionView {
        selection: TextSelectionHandle,
    }

    struct SelectionElementOnlyView;
    struct ToggleSelectionElementView {
        enabled: bool,
        selection: TextSelectionHandle,
    }

    struct DoubleSelectionElementView {
        selection: TextSelectionHandle,
    }

    struct WindowOwnedSelectionView {
        selection: TextSelectionHandle,
    }

    struct FirstFrameScopedSelectionView {
        selection: TextSelectionHandle,
    }

    struct PlainRunLayoutView {
        texts: Vec<SharedString>,
        layouts: Vec<TextLayout>,
    }

    impl Render for WindowSelectionView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    impl Render for SelectionElementOnlyView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(TextSelectionLayer)
                .child(
                    div()
                        .size_full()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                            GlobalState::suppress_text_selection(cx);
                        }),
                )
        }
    }

    impl Render for ToggleSelectionElementView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selection = self.selection.clone();
            div().when(self.enabled, |this| {
                this.child(TextSelectionLayer)
                    .child(div().size_full().on_prepaint(move |bounds, window, cx| {
                        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                        selection.register(
                            TextSelectionRegistration::new(hitbox, bounds)
                                .with_text_bounds(vec![bounds]),
                            window,
                            cx,
                        );
                    }))
            })
        }
    }

    impl Render for DoubleSelectionElementView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selection = self.selection.clone();
            div()
                .size_full()
                .child(TextSelectionLayer)
                .child(TextSelectionLayer)
                .on_prepaint(move |bounds, window, cx| {
                    let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                    selection.register(
                        TextSelectionRegistration::new(hitbox, bounds)
                            .with_text_bounds(vec![bounds]),
                        window,
                        cx,
                    );
                })
        }
    }

    impl Render for WindowOwnedSelectionView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let selection = self.selection.clone();
            div()
                .size_full()
                .child(TextSelectionLayer)
                .child(div().size_full().on_prepaint(move |bounds, window, cx| {
                    let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                    selection.register(
                        TextSelectionRegistration::new(hitbox, bounds)
                            .with_text_bounds(vec![bounds]),
                        window,
                        cx,
                    );
                }))
        }
    }

    impl Render for FirstFrameScopedSelectionView {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let scope = TextSelectionScopeId::from_raw(23);
            TextSelection::activate_scope(scope, window, cx);
            let selection = self.selection.clone();

            div().child(TextSelectionLayer).child(
                div()
                    .size_full()
                    .on_prepaint(move |bounds, window, cx| {
                        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                        selection.register(
                            TextSelectionRegistration::new(hitbox, bounds)
                                .with_text_bounds(vec![bounds]),
                            window,
                            cx,
                        );
                    })
                    .text_selection_scope(scope),
            )
        }
    }

    impl Render for PlainRunLayoutView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.layouts.clear();
            let children = self
                .texts
                .iter()
                .enumerate()
                .map(|(index, text)| {
                    let text = StyledText::new(text.clone());
                    self.layouts.push(text.layout().clone());
                    div().absolute().top(px(index as f32 * 40.)).child(text)
                })
                .collect::<Vec<_>>();
            div().size_full().children(children)
        }
    }

    impl FakeParticipant {
        fn new(text: &str, cx: &mut gpui::App) -> Self {
            let selection = TextSelectionHandle::new(text, cx);
            Self { selection }
        }

        fn register(
            &self,
            selection_state: &mut WindowSelectionState,
            y: f32,
            scope: TextSelectionScopeId,
            document_order: u64,
            cx: &mut gpui::App,
        ) {
            let bounds = Bounds::new(point(px(0.), px(y)), size(px(100.), px(10.)));
            selection_state.register_participant(
                self.selection.clone(),
                TextSelectionRegistration::new(
                    Hitbox {
                        id: HitboxId::placeholder(),
                        bounds,
                        content_mask: ContentMask { bounds },
                        behavior: HitboxBehavior::Normal,
                    },
                    bounds,
                )
                .with_scope(scope)
                .with_document_order(document_order)
                .with_text_bounds(vec![bounds]),
                cx,
            );
        }
    }

    fn laid_out_runs(texts: &[&str], cx: &mut TestAppContext) -> Vec<(SharedString, TextLayout)> {
        let texts = texts
            .iter()
            .map(|text| SharedString::from(*text))
            .collect::<Vec<_>>();
        let view = cx.add_window({
            let texts = texts.clone();
            move |_, _| PlainRunLayoutView {
                texts,
                layouts: Vec::new(),
            }
        });
        cx.update_window(*view, |_, window, cx| {
            let _ = window.draw(cx);
        })
        .unwrap();
        let layouts = cx.update(|cx| view.read(cx).unwrap().layouts.clone());
        texts.into_iter().zip(layouts).collect()
    }

    fn plain_snapshot(anchor: Point<Pixels>, cursor: Point<Pixels>) -> TextSelectionSnapshot {
        TextSelectionSnapshot::new(
            TextSelectionEndpoint::new(None, anchor),
            TextSelectionEndpoint::new(None, cursor),
        )
        .with_window_points(Some(TextSelectionWindowPoints { anchor, cursor }))
    }

    #[gpui::test]
    fn scope_stack_is_cleaned_after_panicking_subtree(cx: &mut TestAppContext) {
        let window_id = {
            let (_, window_cx) = cx.add_window_view(|_, _| SelectionElementOnlyView);
            window_cx.update(|window, _| window.window_handle().window_id())
        };
        let scope = TextSelectionScopeId::from_raw(41);

        cx.update(|cx| {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_text_selection_scope(window_id, scope, cx, |_| panic!("subtree failed"));
            }));

            assert!(result.is_err());
            assert_eq!(current_text_selection_scope(window_id, cx), None);
        });
    }

    #[gpui::test]
    fn reentrant_scope_from_one_window_does_not_pollute_another(cx: &mut TestAppContext) {
        let first_window_id = {
            let (_, window_cx) = cx.add_window_view(|_, _| SelectionElementOnlyView);
            window_cx.update(|window, _| window.window_handle().window_id())
        };
        let second_window_id = {
            let (_, window_cx) = cx.add_window_view(|_, _| SelectionElementOnlyView);
            window_cx.update(|window, _| window.window_handle().window_id())
        };
        let scope = TextSelectionScopeId::from_raw(42);

        cx.update(|cx| {
            with_text_selection_scope(first_window_id, scope, cx, |cx| {
                assert_eq!(current_text_selection_scope(second_window_id, cx), None);
                assert_eq!(
                    current_text_selection_scope(first_window_id, cx),
                    Some(scope)
                );
            });
        });
    }

    #[gpui::test]
    fn selection_callback_can_reenter_its_selection_state(cx: &mut TestAppContext) {
        let called = Rc::new(Cell::new(false));
        let called_from_callback = called.clone();
        let (selection_state, participant) = cx.update(|cx| {
            let selection_state = cx.new(|_| WindowSelectionState::default());
            let selection_state_for_callback = selection_state.clone();
            let participant = FakeParticipant::new("participant", cx);
            participant
                .selection
                .subscribe(
                    move |event, cx| {
                        if matches!(event, TextSelectionEvent::SelectionChanged(Some(_))) {
                            selection_state_for_callback
                                .update(cx, |_, _| called_from_callback.set(true));
                        }
                    },
                    cx,
                )
                .detach();
            (selection_state, participant)
        });
        cx.run_until_parked();
        cx.update(|cx| {
            selection_state.update(cx, |selection_state, cx| {
                participant.register(selection_state, 0., TextSelectionScopeId::default(), 0, cx);
                selection_state.begin(point(px(1.), px(1.)), false, cx);
                selection_state.update(point(px(20.), px(1.)), cx);
            });
        });
        cx.run_until_parked();
        assert!(called.get());
    }

    #[gpui::test]
    fn selection_events_preserve_snapshot_then_clear_order(cx: &mut TestAppContext) {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let observed_for_callback = observed.clone();
        let selection = cx.update(|cx| {
            let selection = TextSelectionHandle::new("selection", cx);
            selection
                .subscribe(
                    move |event, _| {
                        if let TextSelectionEvent::SelectionChanged(snapshot) = event {
                            observed_for_callback.borrow_mut().push(snapshot.is_some());
                        }
                    },
                    cx,
                )
                .detach();
            selection
        });
        cx.run_until_parked();
        cx.update(|cx| {
            selection.0.update(cx, |state, cx| {
                state.set_snapshot(
                    Some(plain_snapshot(point(px(1.), px(1.)), point(px(8.), px(1.)))),
                    cx,
                );
                state.clear_state(cx);
            });
        });
        cx.run_until_parked();
        assert_eq!(&*observed.borrow(), &[true, false]);
    }

    fn text_run(order: u64, text: SharedString, layout: TextLayout) -> TextSelectionRun {
        let bounds = layout.bounds();
        TextSelectionRun::new(text, layout, bounds).with_document_order(order)
    }

    #[gpui::test]
    fn public_selection_data_uses_builders_and_readers(cx: &mut TestAppContext) {
        let bounds = Bounds::new(point(px(1.), px(2.)), size(px(30.), px(10.)));
        let hitbox = Hitbox {
            id: HitboxId::placeholder(),
            bounds,
            content_mask: ContentMask { bounds },
            behavior: HitboxBehavior::Normal,
        };
        let scope = TextSelectionScopeId::from_raw(7);
        let endpoint = TextSelectionEndpoint::new(None, bounds.origin)
            .with_content_key(TextSelectionContentKey::new(11));
        let snapshot = TextSelectionSnapshot::new(endpoint, endpoint)
            .with_selecting(true)
            .with_window_points(Some(TextSelectionWindowPoints {
                anchor: bounds.origin,
                cursor: bounds.bottom_right(),
            }))
            .with_coverage(TextSelectionCoverage::Full);
        let registration = TextSelectionRegistration::new(hitbox, bounds)
            .with_scroll_offset(point(px(3.), px(4.)))
            .with_scope(scope)
            .with_document_order(9)
            .with_text_bounds(vec![bounds]);

        assert_eq!(endpoint.entity_id(), None);
        assert_eq!(endpoint.content_point(), bounds.origin);
        assert_eq!(
            endpoint.content_key(),
            Some(TextSelectionContentKey::new(11))
        );
        assert_eq!(snapshot.anchor(), endpoint);
        assert_eq!(snapshot.cursor(), endpoint);
        assert!(snapshot.is_selecting());
        assert_eq!(snapshot.coverage(), TextSelectionCoverage::Full);
        assert_eq!(
            snapshot.window_points(),
            Some(TextSelectionWindowPoints {
                anchor: bounds.origin,
                cursor: bounds.bottom_right(),
            })
        );
        assert_eq!(registration.bounds(), bounds);
        assert_eq!(registration.scroll_offset(), point(px(3.), px(4.)));
        assert_eq!(registration.scope(), scope);
        assert_eq!(registration.document_order(), 9);
        assert_eq!(registration.text_bounds(), &[bounds]);

        let (text, layout) = laid_out_runs(&["aé"], cx).pop().unwrap();
        let text_run = TextSelectionRun::new(text.clone(), layout.clone(), layout.bounds())
            .with_document_order(3);
        assert_eq!(text_run.document_order(), 3);
        assert_eq!(text_run.text(), &text);
        assert_eq!(text_run.layout().len(), layout.len());
        assert_eq!(text_run.bounds(), layout.bounds());

        let projection = TextSelectionProjection {
            ranges: vec![Some(1..3)],
            is_active: true,
        };
        assert_eq!(projection.ranges(), &[Some(1..3)]);
        assert!(projection.is_active());
    }

    #[gpui::test]
    fn selection_handle_is_the_public_adapter_seam(cx: &mut TestAppContext) {
        let selected = Rc::new(Cell::new(false));
        let selected_from_callback = selected.clone();
        cx.update(|cx| {
            let selection = TextSelectionHandle::new("initial", cx);
            let entity_id = selection.entity_id();
            selection.set_fallback_copy_text("updated", cx);
            selection.set_local_selection(true, cx);
            selection
                .subscribe(
                    move |event, _| {
                        if let TextSelectionEvent::SelectionChanged(snapshot) = event {
                            selected_from_callback.set(snapshot.is_some());
                        }
                    },
                    cx,
                )
                .detach();
            selection.focus_with(|_, _| {}, cx);
            selection.copy_with(|_| Some("copied".to_string()), cx);
            selection.resolve_content_key_with(|_, _| Some(TextSelectionContentKey::new(3)), cx);

            assert_eq!(selection.entity_id(), entity_id);
            assert_eq!(selection.snapshot(cx), None);
            assert_eq!(
                selection.update_runs(&[], cx),
                TextSelectionProjection::default()
            );
        });
        assert!(!selected.get());
    }

    #[gpui::test]
    fn selection_handle_can_subscribe_its_window_to_refresh(cx: &mut TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, cx| WindowSelectionView {
            selection: TextSelectionHandle::new("refresh", cx),
        });
        cx.update(|window, cx| {
            let selection = TextSelectionHandle::new("refresh", cx);
            selection.refresh_window_on_change(window, cx).detach();
        });
    }

    #[gpui::test]
    fn plain_projection_preserves_forward_reversed_and_unicode_ranges(cx: &mut TestAppContext) {
        let (text, layout) = laid_out_runs(&["aé🙂z"], cx).pop().unwrap();
        let run = text_run(0, text, layout.clone());
        let start = layout.position_for_index(1).unwrap();
        let end = layout.position_for_index(7).unwrap();

        let forward = project_ranges(Some(plain_snapshot(start, end)), std::slice::from_ref(&run));
        let reversed = project_ranges(Some(plain_snapshot(end, start)), &[run]);

        assert_eq!(forward.ranges(), &[Some(1..7)]);
        assert_eq!(reversed.ranges(), &[Some(1..7)]);
        assert!(forward.is_active());
        assert!(reversed.is_active());
    }

    #[gpui::test]
    fn double_click_expands_a_plain_run_to_the_input_word_boundary(cx: &mut TestAppContext) {
        let (text, layout) = laid_out_runs(&["one café, three"], cx).pop().unwrap();
        let run = text_run(0, text, layout.clone());
        let click = layout.position_for_index(6).unwrap();

        let (anchor, cursor) =
            points_for_multi_click(std::slice::from_ref(&run), click, 2).unwrap();
        let states = project_ranges(Some(plain_snapshot(anchor, cursor)), &[run]);

        assert_eq!(states.ranges(), &[Some(4..9)]);
    }

    #[gpui::test]
    fn multi_click_uses_text_layout_window_coordinates_at_a_nonzero_origin(
        cx: &mut TestAppContext,
    ) {
        let mut runs = laid_out_runs(&["above", "alpha beta"], cx);
        let (text, layout) = runs.pop().unwrap();
        assert!(layout.bounds().origin.y > px(0.));
        let run = text_run(0, text, layout.clone());
        let click = layout.position_for_index(7).unwrap();

        let (anchor, cursor) =
            points_for_multi_click(std::slice::from_ref(&run), click, 2).unwrap();
        let projection = project_ranges(Some(plain_snapshot(anchor, cursor)), &[run]);

        assert_eq!(projection.ranges(), &[Some(6..10)]);
    }

    #[gpui::test]
    fn triple_click_expands_to_the_input_logical_line_not_the_visual_row(cx: &mut TestAppContext) {
        let (text, layout) = laid_out_runs(&["second line"], cx).pop().unwrap();
        let run = text_run(0, text, layout.clone());
        let click = layout.position_for_index(4).unwrap();

        let (anchor, cursor) =
            points_for_multi_click(std::slice::from_ref(&run), click, 4).unwrap();
        let states = project_ranges(Some(plain_snapshot(anchor, cursor)), &[run]);

        assert_eq!(states.ranges(), &[Some(0..11)]);
        assert_eq!(line_range_at("first line\nsecond line\nthird", 15), 11..22);
    }

    #[gpui::test]
    fn plain_projection_spans_multiple_runs_and_leaves_empty_gutters_unselected(
        cx: &mut TestAppContext,
    ) {
        let mut runs = laid_out_runs(&["first", "", "second"], cx);
        let (first_text, first_layout) = runs.remove(0);
        let (gutter_text, gutter_layout) = runs.remove(0);
        let (second_text, second_layout) = runs.remove(0);
        let start = first_layout.position_for_index(2).unwrap();
        let end = second_layout.position_for_index(3).unwrap();
        let states = project_ranges(
            Some(plain_snapshot(start, end)),
            &[
                text_run(2, second_text, second_layout),
                text_run(1, gutter_text, gutter_layout),
                text_run(0, first_text, first_layout),
            ],
        );

        assert_eq!(states.ranges(), &[Some(0..3), None, Some(2..5)]);
        assert!(states.is_active());
    }

    #[gpui::test]
    fn plain_projection_caches_multiple_participant_copies_in_document_order(
        cx: &mut TestAppContext,
    ) {
        let mut runs = laid_out_runs(&["one", "two"], cx);
        let (first_text, first_layout) = runs.remove(0);
        let (second_text, second_layout) = runs.remove(0);
        let snapshot = plain_snapshot(
            first_layout.position_for_index(1).unwrap(),
            second_layout.position_for_index(2).unwrap(),
        );
        cx.update(|cx| {
            let mut selection_state = WindowSelectionState::default();
            let first = FakeParticipant::new("", cx);
            let second = FakeParticipant::new("", cx);
            first.register(
                &mut selection_state,
                0.,
                TextSelectionScopeId::default(),
                1,
                cx,
            );
            second.register(
                &mut selection_state,
                20.,
                TextSelectionScopeId::default(),
                0,
                cx,
            );

            first
                .selection
                .0
                .update(cx, |state, cx| state.set_snapshot(Some(snapshot), cx));
            let projection = first
                .selection
                .update_runs(&[text_run(0, first_text, first_layout)], cx);
            assert_eq!(projection.ranges(), &[Some(1..3)]);
            assert!(projection.is_active());
            second
                .selection
                .0
                .update(cx, |state, cx| state.set_snapshot(Some(snapshot), cx));
            let projection = second
                .selection
                .update_runs(&[text_run(0, second_text, second_layout)], cx);
            assert_eq!(projection.ranges(), &[Some(0..2)]);
            assert!(projection.is_active());

            assert_eq!(selection_state.selected_text(cx), "tw\nne");
        });
    }

    #[gpui::test]
    fn plain_projection_invalidates_cached_copy_when_the_snapshot_changes(cx: &mut TestAppContext) {
        let (text, layout) = laid_out_runs(&["first"], cx).pop().unwrap();
        let first_snapshot = plain_snapshot(
            layout.position_for_index(1).unwrap(),
            layout.position_for_index(3).unwrap(),
        );
        let changed_snapshot = plain_snapshot(
            layout.position_for_index(3).unwrap(),
            layout.position_for_index(5).unwrap(),
        );
        let run = text_run(0, text, layout);
        cx.update(|cx| {
            let mut selection_state = WindowSelectionState::default();
            let participant = FakeParticipant::new("", cx);
            participant.register(
                &mut selection_state,
                0.,
                TextSelectionScopeId::default(),
                0,
                cx,
            );
            participant.selection.0.update(cx, |state, cx| {
                state.set_snapshot(Some(first_snapshot), cx);
                state.update_runs(std::slice::from_ref(&run));
            });
            assert_eq!(selection_state.selected_text(cx), "ir");

            participant.selection.0.update(cx, |state, cx| {
                state.set_snapshot(Some(changed_snapshot), cx);
            });
            assert_eq!(selection_state.selected_text(cx), "");

            participant.selection.update_runs(&[run], cx);
            assert_eq!(selection_state.selected_text(cx), "st");
            selection_state.clear(cx);
            participant.selection.set_local_selection(true, cx);
            assert_eq!(selection_state.selected_text(cx), "");
        });
    }

    #[gpui::test]
    fn plain_projection_orders_cached_runs_by_frame_order_not_input_order(cx: &mut TestAppContext) {
        let mut runs = laid_out_runs(&["one", "two"], cx);
        let (first_text, first_layout) = runs.remove(0);
        let (second_text, second_layout) = runs.remove(0);
        let snapshot = plain_snapshot(
            first_layout.position_for_index(1).unwrap(),
            second_layout.position_for_index(2).unwrap(),
        );
        cx.update(|cx| {
            let mut selection_state = WindowSelectionState::default();
            let participant = FakeParticipant::new("", cx);
            participant.register(
                &mut selection_state,
                0.,
                TextSelectionScopeId::default(),
                0,
                cx,
            );
            participant.selection.0.update(cx, |state, cx| {
                state.set_snapshot(Some(snapshot), cx);
                state.update_runs(&[
                    text_run(1, first_text, first_layout),
                    text_run(0, second_text, second_layout),
                ]);
            });

            assert_eq!(selection_state.selected_text(cx), "twne");
        });
    }

    #[gpui::test]
    fn plain_projection_safely_rejects_a_text_layout_length_mismatch(cx: &mut TestAppContext) {
        let (_, layout) = laid_out_runs(&["short"], cx).pop().unwrap();
        let start = layout.position_for_index(0).unwrap();
        let end = layout.position_for_index(5).unwrap();
        let states = project_ranges(
            Some(plain_snapshot(start, end)),
            &[text_run(0, SharedString::from("longer"), layout)],
        );

        assert_eq!(states.ranges(), &[None]);
        assert!(states.is_active());
    }

    #[gpui::test]
    fn begin_update_and_end_publish_a_cross_participant_selection(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut selection_state = WindowSelectionState::default();
            let first = FakeParticipant::new("first", cx);
            let second = FakeParticipant::new("second", cx);
            first.register(
                &mut selection_state,
                0.,
                TextSelectionScopeId::default(),
                0,
                cx,
            );
            second.register(
                &mut selection_state,
                20.,
                TextSelectionScopeId::default(),
                1,
                cx,
            );

            selection_state.begin(point(px(1.), px(1.)), false, cx);
            selection_state.update(point(px(1.), px(25.)), cx);
            assert!(selection_state.has_selection(cx));
            assert_eq!(selection_state.selected_text(cx), "first\nsecond");

            selection_state.end(cx);
            assert!(!selection_state.is_selecting());
        });
    }

    #[gpui::test]
    fn shift_extension_keeps_its_original_anchor_when_reversed(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut selection_state = WindowSelectionState::default();
            let participant = FakeParticipant::new("participant", cx);
            participant.register(
                &mut selection_state,
                0.,
                TextSelectionScopeId::default(),
                0,
                cx,
            );

            selection_state.begin(point(px(2.), px(2.)), false, cx);
            selection_state.end(cx);
            selection_state.begin(point(px(8.), px(2.)), true, cx);
            selection_state.end(cx);
            let first_anchor = selection_state.snapshot().unwrap().anchor();

            selection_state.begin(point(px(0.), px(2.)), true, cx);
            selection_state.end(cx);
            let reversed = selection_state.snapshot().unwrap();
            assert_eq!(reversed.anchor(), first_anchor);
            assert!(reversed.cursor().content_point().x < reversed.anchor().content_point().x);
        });
    }

    #[gpui::test]
    fn content_key_resolver_runs_outside_the_window_state_lease(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let state = cx.new(|_| WindowSelectionState::default());
            let participant = FakeParticipant::new("virtual", cx);
            let state_for_callback = state.clone();
            participant.selection.resolve_content_key_with(
                move |_, cx| {
                    let _ = state_for_callback.read(cx).snapshot();
                    Some(TextSelectionContentKey::new(7))
                },
                cx,
            );
            state.update(cx, |state, cx| {
                participant.register(state, 0., TextSelectionScopeId::default(), 0, cx);
                state.begin(point(px(1.), px(1.)), false, cx);
                state.update(point(px(8.), px(1.)), cx);
            });

            WindowSelectionState::resolve_content_keys(&state, cx);

            assert_eq!(
                state.read(cx).snapshot().unwrap().cursor().content_key(),
                Some(TextSelectionContentKey::new(7))
            );
        });
    }

    #[gpui::test]
    fn active_dnd_does_not_move_a_text_selection_cursor(cx: &mut TestAppContext) {
        let window = cx.add_window(|_, cx| WindowSelectionView {
            selection: TextSelectionHandle::new("unused", cx),
        });
        window
            .update(cx, |_, window, cx| {
                let mut state = WindowSelectionState::default();
                let participant = FakeParticipant::new("participant", cx);
                participant.register(&mut state, 0., TextSelectionScopeId::default(), 0, cx);
                state.begin(point(px(1.), px(1.)), false, cx);
                let before = state.cursor.as_ref().unwrap().point;
                state.update_in_window_with_active_drag(point(px(80.), px(1.)), true, window, cx);
                assert_eq!(state.cursor.as_ref().unwrap().point, before);
            })
            .unwrap();
    }

    #[gpui::test]
    fn shift_extension_falls_back_when_the_anchor_participant_was_swept(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut selection_state = WindowSelectionState::default();
            let first = FakeParticipant::new("first", cx);
            let second = FakeParticipant::new("second", cx);
            first.register(
                &mut selection_state,
                0.,
                TextSelectionScopeId::default(),
                0,
                cx,
            );
            selection_state.begin(point(px(1.), px(1.)), false, cx);
            selection_state.update(point(px(8.), px(1.)), cx);
            selection_state.end(cx);

            selection_state.finish_frame(cx);
            selection_state.finish_frame(cx);
            second.register(
                &mut selection_state,
                20.,
                TextSelectionScopeId::default(),
                1,
                cx,
            );
            selection_state.begin(point(px(1.), px(21.)), true, cx);
            selection_state.update(point(px(8.), px(21.)), cx);
            selection_state.end(cx);

            assert_eq!(selection_state.selected_text(cx), "second");
        });
    }

    #[gpui::test]
    fn scope_and_suppression_prevent_unrelated_participants_from_participating(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            let mut selection_state = WindowSelectionState::default();
            let base = FakeParticipant::new("base", cx);
            let modal = FakeParticipant::new("modal", cx);
            base.register(
                &mut selection_state,
                0.,
                TextSelectionScopeId::default(),
                0,
                cx,
            );
            modal.register(&mut selection_state, 20., TextSelectionScopeId(1), 1, cx);

            selection_state.set_active_scope(TextSelectionScopeId(1), cx);
            selection_state.begin(point(px(1.), px(21.)), false, cx);
            selection_state.update(point(px(8.), px(21.)), cx);
            selection_state.end(cx);
            assert_eq!(selection_state.selected_text(cx), "modal");

            selection_state.clear(cx);
            GlobalState::init(cx);
            GlobalState::suppress_text_selection(cx);
            selection_state.begin(point(px(1.), px(21.)), false, cx);
            selection_state.update(point(px(8.), px(21.)), cx);
            assert!(!selection_state.has_selection(cx));
        });
    }

    #[gpui::test]
    fn dead_participants_are_pruned_and_empty_selection_falls_back_safely(cx: &mut TestAppContext) {
        let selection_state = cx.update(|cx| {
            let selection_state = cx.new(|_| WindowSelectionState::default());
            let participant = FakeParticipant::new("gone", cx);
            selection_state.update(cx, |selection_state, cx| {
                participant.register(selection_state, 0., TextSelectionScopeId::default(), 0, cx)
            });
            selection_state
        });
        cx.update(|cx| {
            selection_state.update(cx, |selection_state, cx| {
                selection_state.begin(point(px(1.), px(1.)), false, cx);
                selection_state.update(point(px(8.), px(1.)), cx);
                selection_state.end(cx);

                assert_eq!(selection_state.selected_text(cx), "");
                assert!(!selection_state.has_selection(cx));
            });
        });
    }

    #[gpui::test]
    fn text_selection_namespace_reports_copies_ends_and_clears_selection(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, cx| WindowSelectionView {
            selection: TextSelectionHandle::new("copied", cx),
        });
        cx.update(|window, cx| {
            let selection = view.read(cx).selection.clone();
            let selection_state = WindowSelectionState::ensure(window, cx);
            selection_state.update(cx, |selection_state, cx| {
                FakeParticipant { selection }.register(
                    selection_state,
                    0.,
                    TextSelectionScopeId::default(),
                    0,
                    cx,
                );
                selection_state.begin(point(px(1.), px(1.)), false, cx);
                selection_state.update(point(px(8.), px(1.)), cx);
            });

            assert!(TextSelection::has_selection(window, cx));
            assert_eq!(TextSelection::selected_text(window, cx), "copied");
            TextSelection::end(window, cx);
            assert!(TextSelection::has_selection(window, cx));
            TextSelection::clear(window, cx);
            assert!(!TextSelection::has_selection(window, cx));
            assert_eq!(TextSelection::selected_text(window, cx), "");
        });
    }

    #[gpui::test]
    fn two_windows_isolate_selection_copy_clear_and_release_ownership(cx: &mut TestAppContext) {
        let first = cx.add_window(|_, cx| WindowOwnedSelectionView {
            selection: TextSelectionHandle::new("first", cx),
        });
        let second = cx.add_window(|_, cx| WindowOwnedSelectionView {
            selection: TextSelectionHandle::new("second", cx),
        });
        let first_selection = cx.update(|cx| first.read(cx).unwrap().selection.clone());
        let second_selection = cx.update(|cx| second.read(cx).unwrap().selection.clone());

        let first_state = cx
            .update_window(*first, |_, window, cx| {
                let _ = window.draw(cx);
                first_selection.set_local_selection(true, cx);
                assert_eq!(TextSelection::selected_text(window, cx), "first");
                WindowSelectionState::existing(window, cx)
                    .unwrap()
                    .downgrade()
            })
            .unwrap();
        cx.update_window(*second, |_, window, cx| {
            let _ = window.draw(cx);
            second_selection.set_local_selection(true, cx);
            assert_eq!(TextSelection::selected_text(window, cx), "second");
        })
        .unwrap();

        cx.update_window(*first, |_, window, cx| {
            TextSelection::clear(window, cx);
            assert_eq!(TextSelection::selected_text(window, cx), "");
        })
        .unwrap();
        cx.update_window(*second, |_, window, cx| {
            assert_eq!(TextSelection::selected_text(window, cx), "second");
        })
        .unwrap();

        cx.update_window(*first, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();

        assert!(first_state.upgrade().is_none());
        cx.update_window(*second, |_, window, cx| {
            assert_eq!(TextSelection::selected_text(window, cx), "second");
        })
        .unwrap();
        cx.update(|cx| {
            assert_eq!(cx.global::<SelectionStateRegistry>().0.len(), 1);
        });
    }

    #[gpui::test]
    fn copy_callback_can_reenter_window_and_handle_selection(cx: &mut TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| SelectionElementOnlyView);
        cx.update(|window, cx| {
            let _ = window.draw(cx);
            let state = WindowSelectionState::existing(window, cx).unwrap();
            let selection = TextSelectionHandle::new("fallback", cx);
            let state_for_copy = state.clone();
            let selection_for_copy = selection.clone();
            selection.copy_with(
                move |cx: &mut App| {
                    state_for_copy.update(cx, |state, _| {
                        assert!(state.snapshot().is_some());
                    });
                    assert!(selection_for_copy.snapshot(cx).is_some());
                    selection_for_copy.set_fallback_copy_text("reentered", cx);
                    Some("reentrant copy".to_string())
                },
                cx,
            );
            state.update(cx, |state, cx| {
                FakeParticipant {
                    selection: selection.clone(),
                }
                .register(state, 0., TextSelectionScopeId::default(), 0, cx);
                state.begin(point(px(1.), px(1.)), false, cx);
                state.update(point(px(8.), px(1.)), cx);
                state.end(cx);
            });

            assert_eq!(TextSelection::selected_text(window, cx), "reentrant copy");
        });
    }

    #[gpui::test]
    fn cross_participant_selection_excludes_participants_outside_its_document_interval(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            let mut selection_state = WindowSelectionState::default();
            let first = FakeParticipant::new("first", cx);
            let second = FakeParticipant::new("second", cx);
            let third = FakeParticipant::new("third", cx);
            first.register(
                &mut selection_state,
                0.,
                TextSelectionScopeId::default(),
                0,
                cx,
            );
            second.register(
                &mut selection_state,
                20.,
                TextSelectionScopeId::default(),
                1,
                cx,
            );
            third.register(
                &mut selection_state,
                40.,
                TextSelectionScopeId::default(),
                2,
                cx,
            );

            selection_state.begin(point(px(1.), px(1.)), false, cx);
            selection_state.update(point(px(1.), px(25.)), cx);
            selection_state.end(cx);

            assert_eq!(selection_state.selected_text(cx), "first\nsecond");
            assert!(third.selection.snapshot(cx).is_none());
        });
    }

    #[gpui::test]
    fn changing_scope_clears_the_previous_scope_selection(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut selection_state = WindowSelectionState::default();
            let base = FakeParticipant::new("base", cx);
            let modal = FakeParticipant::new("modal", cx);
            base.register(
                &mut selection_state,
                0.,
                TextSelectionScopeId::default(),
                0,
                cx,
            );
            modal.register(
                &mut selection_state,
                20.,
                TextSelectionScopeId::from_raw(1),
                1,
                cx,
            );

            selection_state.begin(point(px(1.), px(1.)), false, cx);
            selection_state.update(point(px(8.), px(1.)), cx);
            selection_state.end(cx);
            selection_state.set_active_scope(TextSelectionScopeId::from_raw(1), cx);

            assert!(!selection_state.has_selection(cx));
            assert!(base.selection.snapshot(cx).is_none());
        });
    }

    #[gpui::test]
    fn blank_only_drag_never_publishes_or_copies_selection(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut selection_state = WindowSelectionState::default();
            let participant = FakeParticipant::new("participant", cx);
            participant.register(
                &mut selection_state,
                0.,
                TextSelectionScopeId::default(),
                0,
                cx,
            );

            selection_state.begin(point(px(200.), px(1.)), false, cx);
            selection_state.update(point(px(200.), px(8.)), cx);
            selection_state.end(cx);

            assert!(!selection_state.has_selection(cx));
            assert_eq!(selection_state.selected_text(cx), "");
            assert!(participant.selection.snapshot(cx).is_none());
        });
    }

    #[gpui::test]
    fn stale_selected_participants_retain_copy_state_until_clear(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut selection_state = WindowSelectionState::default();
            let participant = FakeParticipant::new("stale", cx);
            participant.register(
                &mut selection_state,
                0.,
                TextSelectionScopeId::default(),
                0,
                cx,
            );
            selection_state.begin(point(px(1.), px(1.)), false, cx);
            selection_state.update(point(px(8.), px(1.)), cx);
            selection_state.end(cx);

            selection_state.finish_frame(cx);
            selection_state.finish_frame(cx);
            let participant = participant.selection.downgrade();
            assert_eq!(selection_state.selected_text(cx), "stale");
            assert!(participant.upgrade().is_some());

            selection_state.clear(cx);
            assert_eq!(selection_state.selected_text(cx), "");
            assert!(participant.upgrade().is_none());
        });
    }

    #[gpui::test]
    fn active_endpoint_keeps_projectable_window_points_when_other_endpoint_is_virtual(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            let mut selection_state = WindowSelectionState::default();
            let first = FakeParticipant::new("first", cx);
            let second = FakeParticipant::new("second", cx);
            first.register(
                &mut selection_state,
                0.,
                TextSelectionScopeId::default(),
                0,
                cx,
            );
            second.register(
                &mut selection_state,
                20.,
                TextSelectionScopeId::default(),
                1,
                cx,
            );
            selection_state.begin(point(px(1.), px(1.)), false, cx);
            selection_state.update(point(px(5.), px(25.)), cx);
            selection_state.finish_frame(cx);
            second.register(
                &mut selection_state,
                20.,
                TextSelectionScopeId::default(),
                1,
                cx,
            );
            selection_state.finish_frame(cx);

            let snapshot = selection_state.snapshot().unwrap();
            let points = snapshot.window_points().unwrap();
            assert!(points.anchor().y < points.cursor().y);
            assert!(first.selection.snapshot(cx).is_some());
            assert!(second.selection.snapshot(cx).is_some());
        });
    }

    #[gpui::test]
    fn unresolved_copy_callback_refuses_all_participant_output(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut selection_state = WindowSelectionState::default();
            let first = FakeParticipant::new("first fallback", cx);
            let second = FakeParticipant::new("second fallback", cx);
            first.selection.copy_with(|_| Some("first".to_string()), cx);
            second.selection.copy_with(|_| None, cx);
            first.register(
                &mut selection_state,
                0.,
                TextSelectionScopeId::default(),
                0,
                cx,
            );
            second.register(
                &mut selection_state,
                20.,
                TextSelectionScopeId::default(),
                1,
                cx,
            );
            selection_state.begin(point(px(1.), px(1.)), false, cx);
            selection_state.update(point(px(5.), px(25.)), cx);

            assert_eq!(selection_state.selected_text(cx), "");
        });
    }

    #[gpui::test]
    fn clear_stops_anchor_auto_scroll_before_discarding_the_anchor(cx: &mut TestAppContext) {
        let commands = Rc::new(RefCell::new(Vec::new()));
        let observed = commands.clone();
        let (mut selection_state, participant) = cx.update(|cx| {
            let selection_state = WindowSelectionState::default();
            let participant = FakeParticipant::new("scroll", cx);
            participant
                .selection
                .subscribe(
                    move |event, _| {
                        if let TextSelectionEvent::AutoScroll(delta) = event {
                            observed.borrow_mut().push(*delta);
                        }
                    },
                    cx,
                )
                .detach();
            (selection_state, participant)
        });
        cx.run_until_parked();
        cx.update(|cx| {
            participant.register(
                &mut selection_state,
                0.,
                TextSelectionScopeId::default(),
                0,
                cx,
            );

            selection_state.begin(point(px(1.), px(1.)), false, cx);
            selection_state.update(point(px(1.), px(25.)), cx);
            selection_state.clear(cx);
        });
        cx.run_until_parked();
        assert!(commands.borrow().iter().any(Option::is_some));
        assert_eq!(commands.borrow().last(), Some(&None));
    }

    #[gpui::test]
    fn proxy_endpoints_break_equal_position_ties_by_document_order(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut selection_state = WindowSelectionState::default();
            let later = FakeParticipant::new("later", cx);
            let earlier = FakeParticipant::new("earlier", cx);
            later.register(
                &mut selection_state,
                0.,
                TextSelectionScopeId::default(),
                2,
                cx,
            );
            earlier.register(
                &mut selection_state,
                0.,
                TextSelectionScopeId::default(),
                1,
                cx,
            );

            selection_state.begin(point(px(1.), px(1.)), false, cx);
            selection_state.update(point(px(200.), px(25.)), cx);
            let endpoint = selection_state.snapshot().unwrap().cursor();

            assert_eq!(endpoint.entity_id(), Some(earlier.selection.entity_id()));
        });
    }

    #[gpui::test]
    fn equal_area_hovered_participants_break_ties_by_document_order(cx: &mut TestAppContext) {
        cx.update(|cx| {
            for _ in 0..64 {
                let mut selection_state = WindowSelectionState::default();
                let later = FakeParticipant::new("later", cx);
                let earliest = FakeParticipant::new("earliest", cx);
                let middle = FakeParticipant::new("middle", cx);
                later.register(
                    &mut selection_state,
                    0.,
                    TextSelectionScopeId::default(),
                    30,
                    cx,
                );
                earliest.register(
                    &mut selection_state,
                    0.,
                    TextSelectionScopeId::default(),
                    10,
                    cx,
                );
                middle.register(
                    &mut selection_state,
                    0.,
                    TextSelectionScopeId::default(),
                    20,
                    cx,
                );

                selection_state.begin(point(px(1.), px(1.)), false, cx);
                selection_state.update(point(px(8.), px(1.)), cx);

                assert_eq!(
                    selection_state.snapshot().unwrap().anchor().entity_id(),
                    Some(earliest.selection.entity_id())
                );
            }
        });
    }

    #[gpui::test]
    fn text_selection_namespace_is_a_safe_no_op_until_the_element_is_rendered(
        cx: &mut TestAppContext,
    ) {
        let (_, cx) = cx.add_window_view(|_, cx| WindowSelectionView {
            selection: TextSelectionHandle::new("not enabled", cx),
        });
        cx.update(|window, cx| {
            assert!(!TextSelection::has_selection(window, cx));
            assert_eq!(TextSelection::selected_text(window, cx), "");
            TextSelection::clear(window, cx);
            TextSelection::end(window, cx);
            assert!(!TextSelection::has_selection(window, cx));
        });
    }

    #[gpui::test]
    fn unit_selection_element_supports_scope_and_registration_on_the_first_frame(
        cx: &mut TestAppContext,
    ) {
        let (view, cx) = cx.add_window_view(|_, cx| FirstFrameScopedSelectionView {
            selection: TextSelectionHandle::new("first frame", cx),
        });
        let selection = cx.update(|_, cx| view.read(cx).selection.clone());

        cx.update(|window, cx| {
            let _ = window.draw(cx);
            let state = WindowSelectionState::existing(window, cx).unwrap();
            assert_eq!(
                state.read(cx).active_scope,
                TextSelectionScopeId::from_raw(23)
            );
            assert!(
                state
                    .read(cx)
                    .participants
                    .contains_key(&selection.entity_id())
            );
        });
    }

    #[gpui::test]
    fn lazy_registration_does_not_enable_queries_without_the_element(cx: &mut TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, cx| WindowSelectionView {
            selection: TextSelectionHandle::new("registered", cx),
        });
        cx.update(|window, cx| {
            let selection = TextSelectionHandle::new("registered", cx);
            selection.set_local_selection(true, cx);
            let bounds = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(20.)));
            let hitbox = Hitbox {
                id: HitboxId::placeholder(),
                bounds,
                content_mask: ContentMask { bounds },
                behavior: HitboxBehavior::Normal,
            };
            selection.register(
                TextSelectionRegistration::new(hitbox, bounds).with_text_bounds(vec![bounds]),
                window,
                cx,
            );
            assert_eq!(TextSelection::selected_text(window, cx), "");
            assert!(!TextSelection::has_selection(window, cx));
            TextSelection::clear(window, cx);
            assert_eq!(TextSelection::selected_text(window, cx), "");
        });
    }

    #[gpui::test]
    fn retained_selection_state_releases_and_does_not_resurrect_selection(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, cx| ToggleSelectionElementView {
            enabled: true,
            selection: TextSelectionHandle::new("local", cx),
        });
        let selection = cx.update(|_, cx| view.read(cx).selection.clone());
        cx.update(|window, cx| {
            let _ = window.draw(cx);
            selection.set_local_selection(true, cx);
            assert!(TextSelection::has_selection(window, cx));

            window.simulate_next_frame(cx);
            assert!(TextSelection::has_selection(window, cx));
            let _ = window.draw(cx);
            assert!(TextSelection::has_selection(window, cx));
        });
        view.update(cx, |view, cx| {
            view.enabled = false;
            cx.notify();
        });
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        cx.update(|window, cx| {
            window.simulate_next_frame(cx);
        });
        cx.update(|window, cx| {
            window.simulate_next_frame(cx);
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            assert!(!TextSelection::has_selection(window, cx));
            assert_eq!(TextSelection::selected_text(window, cx), "");
            assert!(!selection.has_local_selection(cx));
            TextSelection::clear(window, cx);
        });

        view.update(cx, |view, cx| {
            view.enabled = true;
            cx.notify();
        });
        cx.update(|window, cx| {
            let _ = window.draw(cx);
            assert!(!TextSelection::has_selection(window, cx));
            assert_eq!(TextSelection::selected_text(window, cx), "");
        });
    }

    #[gpui::test]
    fn mounted_selection_element_does_not_keep_an_idle_frame_queue_alive(cx: &mut TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| SelectionElementOnlyView);
        cx.update(|window, cx| {
            let _ = window.draw(cx);
            assert_eq!(window.simulate_next_frame(cx), 0);
            assert_eq!(window.simulate_next_frame(cx), 0);
            assert!(live_text_selection_state(window, cx).is_some());
        });
    }

    #[gpui::test]
    fn selection_element_initializes_suppression_and_respects_bubble_suppression(
        cx: &mut TestAppContext,
    ) {
        let (_, cx) = cx.add_window_view(|_, _| SelectionElementOnlyView);
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        cx.simulate_mouse_down(
            point(px(1.), px(1.)),
            MouseButton::Left,
            gpui::Modifiers::default(),
        );
        cx.simulate_mouse_up(
            point(px(1.), px(1.)),
            MouseButton::Left,
            gpui::Modifiers::default(),
        );
        cx.update(|window, cx| {
            assert!(GlobalState::is_text_selection_suppressed(cx));
            assert!(!TextSelection::has_selection(window, cx));
        });
    }

    #[gpui::test]
    fn frame_sweep_keeps_a_participant_registered_before_the_selection_element_paints(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            let mut selection_state = WindowSelectionState::default();
            let participant = FakeParticipant::new("painted first", cx);
            participant.register(
                &mut selection_state,
                0.,
                TextSelectionScopeId::default(),
                0,
                cx,
            );
            selection_state.begin(point(px(1.), px(1.)), false, cx);
            selection_state.update(point(px(8.), px(1.)), cx);
            selection_state.end(cx);

            selection_state.finish_frame(cx);

            assert_eq!(selection_state.selected_text(cx), "painted first");
            assert!(participant.selection.snapshot(cx).is_some());
        });
    }

    #[gpui::test]
    fn two_selection_elements_schedule_only_one_post_frame_sweep(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, cx| DoubleSelectionElementView {
            selection: TextSelectionHandle::new("once", cx),
        });
        cx.update(|window, cx| {
            let selection_state = WindowSelectionState::ensure(window, cx);
            let selection = view.read(cx).selection.clone();
            selection_state.update(cx, |selection_state, cx| {
                FakeParticipant { selection }.register(
                    selection_state,
                    0.,
                    TextSelectionScopeId::default(),
                    0,
                    cx,
                );
                selection_state.begin(point(px(1.), px(1.)), false, cx);
                selection_state.update(point(px(8.), px(1.)), cx);
                selection_state.end(cx);
            });

            let _ = window.draw(cx);
            window.simulate_next_frame(cx);

            let items = selection_state.read(cx).copy_items(cx);
            assert_eq!(resolve_copy_items(items, cx), "once");
        });
    }

    #[gpui::test]
    fn duplicate_selection_elements_gate_real_pointer_gestures_and_reentrant_clear(
        cx: &mut TestAppContext,
    ) {
        let (view, cx) = cx.add_window_view(|_, cx| DoubleSelectionElementView {
            selection: TextSelectionHandle::new("once", cx),
        });
        let clear_count = Rc::new(Cell::new(0));
        cx.update(|window, cx| {
            let state = WindowSelectionState::ensure(window, cx);
            let state_for_clear = state.clone();
            let count = clear_count.clone();
            let selection = view.read(cx).selection.clone();
            selection
                .subscribe(
                    move |event, cx| {
                        if matches!(event, TextSelectionEvent::Cleared) {
                            count.set(count.get() + 1);
                            let _ = state_for_clear.read(cx).snapshot();
                        }
                    },
                    cx,
                )
                .detach();
            let _ = window.draw(cx);
        });

        cx.simulate_mouse_down(
            point(px(10.), px(10.)),
            MouseButton::Left,
            gpui::Modifiers::default(),
        );
        cx.simulate_mouse_up(
            point(px(10.), px(10.)),
            MouseButton::Left,
            gpui::Modifiers::default(),
        );
        cx.simulate_mouse_down(
            point(px(70.), px(10.)),
            MouseButton::Left,
            gpui::Modifiers {
                shift: true,
                ..Default::default()
            },
        );
        cx.simulate_mouse_up(
            point(px(70.), px(10.)),
            MouseButton::Left,
            gpui::Modifiers::default(),
        );
        cx.update(|window, cx| assert!(TextSelection::has_selection(window, cx)));

        cx.simulate_mouse_down(
            point(px(15.), px(10.)),
            MouseButton::Left,
            gpui::Modifiers::default(),
        );
        cx.simulate_mouse_move(
            point(px(85.), px(10.)),
            Some(MouseButton::Left),
            gpui::Modifiers::default(),
        );
        cx.simulate_mouse_up(
            point(px(85.), px(10.)),
            MouseButton::Left,
            gpui::Modifiers::default(),
        );
        cx.update(|window, cx| assert!(TextSelection::has_selection(window, cx)));
        assert_eq!(clear_count.get(), 3);
    }

    #[gpui::test]
    fn selection_layer_handles_real_double_and_triple_click_events(cx: &mut TestAppContext) {
        let (text, layout) = laid_out_runs(&["alpha beta"], cx).pop().unwrap();
        let (view, cx) = cx.add_window_view(|_, cx| DoubleSelectionElementView {
            selection: TextSelectionHandle::new("", cx),
        });
        cx.update(|window, cx| {
            let _ = window.draw(cx);
            let selection = view.read(cx).selection.clone();
            selection.resolve_content_key_with(|_, _| Some(TextSelectionContentKey::new(17)), cx);
            selection.update_runs(&[text_run(0, text.clone(), layout.clone())], cx);
        });

        let position = layout.position_for_index(7).unwrap();
        cx.simulate_event(MouseDownEvent {
            position,
            modifiers: gpui::Modifiers::default(),
            button: MouseButton::Left,
            click_count: 2,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            position,
            modifiers: gpui::Modifiers::default(),
            button: MouseButton::Left,
            click_count: 2,
        });
        cx.update(|window, cx| {
            let selection = view.read(cx).selection.clone();
            selection.update_runs(&[text_run(0, text.clone(), layout.clone())], cx);
            assert_eq!(TextSelection::selected_text(window, cx), "beta");
            let snapshot = selection.snapshot(cx).unwrap();
            assert_eq!(
                snapshot.anchor().content_key(),
                Some(TextSelectionContentKey::new(17))
            );
            assert_eq!(
                snapshot.cursor().content_key(),
                Some(TextSelectionContentKey::new(17))
            );
            assert_eq!(
                TextSelection::content_keys(window, cx),
                Some([
                    TextSelectionContentKey::new(17),
                    TextSelectionContentKey::new(17),
                ])
            );
        });

        cx.simulate_event(MouseDownEvent {
            position,
            modifiers: gpui::Modifiers::default(),
            button: MouseButton::Left,
            click_count: 3,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            position,
            modifiers: gpui::Modifiers::default(),
            button: MouseButton::Left,
            click_count: 3,
        });
        cx.update(|window, cx| {
            let selection = view.read(cx).selection.clone();
            selection.update_runs(&[text_run(0, text, layout)], cx);
            assert_eq!(TextSelection::selected_text(window, cx), "alpha beta");
        });
    }
}

#[cfg(test)]
mod word_drag_tests {
    use super::*;
    use gpui::{
        div, size, HitboxBehavior, Modifiers, ParentElement, Render, Styled, StyledText,
        TestAppContext,
    };
    use std::{cell::RefCell, rc::Rc};

    fn key(value: u64) -> TextSelectionContentKey {
        TextSelectionContentKey::new(value)
    }

    /// One painted participant: a single laid-out text run.
    #[derive(Clone)]
    struct WordParticipant {
        selection: TextSelectionHandle,
        text: SharedString,
        layout: TextLayout,
        document_order: u64,
        content_key: TextSelectionContentKey,
    }

    impl WordParticipant {
        fn run(&self) -> TextSelectionRun {
            TextSelectionRun::new(self.text.clone(), self.layout.clone(), self.layout.bounds())
                .with_document_order(self.document_order)
        }

        /// The participant-relative point for a byte index.
        fn content_point(&self, index: usize) -> Point<Pixels> {
            self.layout.position_for_index(index).unwrap() - self.layout.bounds().origin
        }

        /// A window point inside the glyph at a byte index.
        fn glyph_center(&self, index: usize) -> Point<Pixels> {
            let start = self.layout.position_for_index(index).unwrap();
            let end = self.layout.position_for_index(index + 1).unwrap();
            point(px((f32::from(start.x) + f32::from(end.x)) / 2.), start.y)
        }
    }

    struct RunsLayoutView {
        runs: Vec<(SharedString, f32)>,
        layouts: Rc<RefCell<Vec<TextLayout>>>,
    }

    impl Render for RunsLayoutView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
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
    ) -> Vec<(SharedString, TextLayout)> {
        let runs = texts
            .iter()
            .map(|(text, y)| (SharedString::from(*text), *y))
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
        participants: Rc<RefCell<Vec<WordParticipant>>>,
    }

    struct WordDragElement {
        participants: Rc<RefCell<Vec<WordParticipant>>>,
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
            (window.request_layout(Style::default(), [], cx), ())
        }

        fn prepaint(
            &mut self,
            _: Option<&GlobalElementId>,
            _: Option<&InspectorElementId>,
            _: Bounds<Pixels>,
            _: &mut Self::RequestLayoutState,
            window: &mut Window,
            cx: &mut App,
        ) -> Self::PrepaintState {
            for participant in self.participants.borrow().iter() {
                let bounds = participant.layout.bounds();
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                let entity = WindowSelectionState::existing(window, cx).unwrap_or_else(|| {
                    WindowSelectionState::acquire(window.window_handle().window_id(), cx)
                });
                entity.update(cx, |state, cx| {
                    state.register_participant(
                        participant.selection.clone(),
                        TextSelectionRegistration::new(hitbox, bounds)
                            .with_document_order(participant.document_order)
                            .with_text_bounds(vec![bounds])
                            .with_content_key(participant.content_key),
                        cx,
                    )
                });
            }
        }

        fn paint(
            &mut self,
            _: Option<&GlobalElementId>,
            _: Option<&InspectorElementId>,
            _: Bounds<Pixels>,
            _: &mut Self::RequestLayoutState,
            _: &mut Self::PrepaintState,
            _: &mut Window,
            cx: &mut App,
        ) {
            for participant in self.participants.borrow().iter() {
                participant.selection.update_runs(&[participant.run()], cx);
            }
        }
    }

    impl Render for WordDragView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(TextSelectionLayer).child(WordDragElement {
                participants: Rc::clone(&self.participants),
            })
        }
    }

    /// A word-drag event window over `(text, y, document_order, content_key)`
    /// specs; the first `mounted_count` participants start registered.
    #[allow(clippy::type_complexity)]
    fn word_drag_window<'a>(
        cx: &'a mut TestAppContext,
        specs: &[(&'static str, f32, u64, u64)],
        mounted_count: usize,
    ) -> (
        Rc<RefCell<Vec<WordParticipant>>>,
        Vec<WordParticipant>,
        &'a mut gpui::VisualTestContext,
    ) {
        word_drag_window_with_resolver(cx, specs, mounted_count, None)
    }

    /// [`word_drag_window`] with an optional shared content-key resolver
    /// installed on every participant instead of the default always-resolved
    /// per-participant key.
    #[allow(clippy::type_complexity)]
    fn word_drag_window_with_resolver<'a>(
        cx: &'a mut TestAppContext,
        specs: &[(&'static str, f32, u64, u64)],
        mounted_count: usize,
        shared_resolver: Option<ContentKeyResolver>,
    ) -> (
        Rc<RefCell<Vec<WordParticipant>>>,
        Vec<WordParticipant>,
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
            let participants = participants.clone();
            move |_, _| WordDragView { participants }
        });
        let spares = window_cx.update(|window, cx| {
            let entries = layouts
                .into_iter()
                .zip(specs.iter())
                .map(|((text, layout), (_, _, document_order, content_key))| WordParticipant {
                    selection: TextSelectionHandle::new(text.to_string(), cx),
                    text,
                    layout,
                    document_order: *document_order,
                    content_key: key(*content_key),
                })
                .collect::<Vec<_>>();
            for entry in &entries {
                if let Some(resolver) = shared_resolver.clone() {
                    entry.selection.resolve_content_key_with(
                        move |point, cx| resolver(point, cx),
                        cx,
                    );
                } else {
                    let content_key = entry.content_key;
                    entry
                        .selection
                        .resolve_content_key_with(move |_, _| Some(content_key), cx);
                }
            }
            *participants.borrow_mut() = entries.iter().take(mounted_count).cloned().collect();
            _ = window.draw(cx);
            entries.into_iter().skip(mounted_count).collect::<Vec<_>>()
        });
        (participants, spares, window_cx)
    }

    fn double_click(window_cx: &mut gpui::VisualTestContext, position: Point<Pixels>) {
        window_cx.simulate_event(MouseDownEvent {
            position,
            modifiers: Modifiers::default(),
            button: MouseButton::Left,
            click_count: 2,
            first_mouse: false,
        });
    }

    fn drag_to(window_cx: &mut gpui::VisualTestContext, position: Point<Pixels>) {
        window_cx.simulate_event(MouseMoveEvent {
            position,
            modifiers: Modifiers::default(),
            pressed_button: Some(MouseButton::Left),
        });
    }

    fn release_at(window_cx: &mut gpui::VisualTestContext, position: Point<Pixels>) {
        window_cx.simulate_event(MouseUpEvent {
            position,
            modifiers: Modifiers::default(),
            button: MouseButton::Left,
            click_count: 2,
        });
    }

    fn selected_text_after_repaint(window_cx: &mut gpui::VisualTestContext) -> String {
        window_cx.update(|window, cx| {
            _ = window.draw(cx);
            TextSelection::selected_text(window, cx)
        })
    }

    #[gpui::test]
    fn held_double_click_extends_by_whole_words_within_a_run(cx: &mut TestAppContext) {
        let text = "alpha words beta";
        let (participants, _spares, window_cx) = word_drag_window(cx, &[(text, 0., 0, 1)], 1);
        let first = participants.borrow()[0].clone();
        let words = text.find("words").unwrap();
        let beta = text.find("beta").unwrap();

        double_click(window_cx, first.glyph_center(words + 1));
        window_cx.update(|_, cx| {
            let snapshot = first.selection.snapshot(cx).unwrap();
            assert!(snapshot.is_selecting());
            assert_eq!(snapshot.anchor().content_point(), first.content_point(words));
            assert_eq!(
                snapshot.cursor().content_point(),
                first.content_point(words + 5)
            );
        });
        assert_eq!(selected_text_after_repaint(window_cx), "words");

        // Dragging forward extends to the target word's end.
        drag_to(window_cx, first.glyph_center(beta + 1));
        window_cx.update(|_, cx| {
            let snapshot = first.selection.snapshot(cx).unwrap();
            assert_eq!(snapshot.anchor().content_point(), first.content_point(words));
            assert_eq!(
                snapshot.cursor().content_point(),
                first.content_point(beta + 4)
            );
        });
        assert_eq!(selected_text_after_repaint(window_cx), "words beta");

        // Dragging back inside the original word restores the whole word.
        drag_to(window_cx, first.glyph_center(words + 1));
        assert_eq!(selected_text_after_repaint(window_cx), "words");

        // Dragging before the original word reverses by whole words and
        // anchors on the original word's end.
        drag_to(window_cx, first.glyph_center(1));
        window_cx.update(|_, cx| {
            let snapshot = first.selection.snapshot(cx).unwrap();
            assert_eq!(snapshot.cursor().content_point(), first.content_point(0));
            assert_eq!(
                snapshot.anchor().content_point(),
                first.content_point(words + 5)
            );
        });
        assert_eq!(selected_text_after_repaint(window_cx), "alpha words");

        // Releasing keeps the selection, and later moves do not extend it.
        release_at(window_cx, first.glyph_center(1));
        window_cx.update(|_, cx| {
            assert!(!first.selection.snapshot(cx).unwrap().is_selecting());
        });
        assert_eq!(selected_text_after_repaint(window_cx), "alpha words");
        window_cx.simulate_mouse_move(first.glyph_center(beta + 1), None, Modifiers::default());
        assert_eq!(selected_text_after_repaint(window_cx), "alpha words");
    }

    #[gpui::test]
    fn word_drag_spans_participants_in_both_directions(cx: &mut TestAppContext) {
        let first_text = "first words tail";
        let second_text = "second words tail";
        let (participants, _spares, window_cx) = word_drag_window(
            cx,
            &[(first_text, 0., 0, 1), (second_text, 40., 1, 2)],
            2,
        );
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
            let snapshot = second.selection.snapshot(cx).unwrap();
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
            assert_eq!(TextSelection::content_keys(window, cx), Some([key(1), key(2)]));
        });

        // Dragging back into the first participant reverses by whole words.
        drag_to(window_cx, first.glyph_center(1));
        assert_eq!(selected_text_after_repaint(window_cx), "first words");
        window_cx.update(|window, cx| {
            let snapshot = first.selection.snapshot(cx).unwrap();
            assert_eq!(snapshot.cursor().content_point(), first.content_point(0));
            assert_eq!(
                snapshot.anchor().content_point(),
                first.content_point(first_words + 5)
            );
            assert_eq!(TextSelection::content_keys(window, cx), Some([key(1), key(1)]));
        });
    }

    #[gpui::test]
    fn word_drag_survives_virtualizing_the_original_participant(cx: &mut TestAppContext) {
        let first_text = "first words tail";
        let second_text = "second words tail";
        let (participants, _spares, window_cx) = word_drag_window(
            cx,
            &[(first_text, 0., 0, 1), (second_text, 40., 1, 2)],
            2,
        );
        let first = participants.borrow()[0].clone();
        let second = participants.borrow()[1].clone();
        let first_words = first_text.find("words").unwrap();
        let second_words = second_text.find("words").unwrap();

        double_click(window_cx, first.glyph_center(first_words + 1));
        drag_to(window_cx, second.glyph_center(second_words + 1));
        window_cx.update(|window, cx| {
            participants.borrow_mut().remove(0);
            _ = window.draw(cx);
        });
        window_cx.run_until_parked();

        // Dragging back toward the original word keeps the original word
        // endpoints even though the original participant is virtualized: the
        // anchor still names it, and the cursor snaps to a whole word.
        drag_to(window_cx, second.glyph_center(1));
        window_cx.update(|window, cx| {
            let snapshot = second.selection.snapshot(cx).unwrap();
            assert!(snapshot.is_selecting());
            assert_eq!(
                snapshot.anchor().entity_id(),
                Some(first.selection.entity_id())
            );
            assert_eq!(
                snapshot.anchor().content_point(),
                first.content_point(first_words)
            );
            assert_eq!(snapshot.cursor().content_point(), second.content_point(6));
            assert_eq!(TextSelection::content_keys(window, cx), Some([key(1), key(2)]));
        });

        // The drag keeps resolving after the original participant remounts.
        window_cx.update(|window, cx| {
            participants.borrow_mut().insert(0, first.clone());
            _ = window.draw(cx);
        });
        window_cx.run_until_parked();
        drag_to(window_cx, second.glyph_center(second_words + 1));
        assert_eq!(
            selected_text_after_repaint(window_cx),
            "words tail\nsecond words"
        );
    }

    #[gpui::test]
    fn reverse_drag_retains_resolved_originals_without_resolver_replay(
        cx: &mut TestAppContext,
    ) {
        let text = "alpha words beta";
        // Scripted resolver responses by phase: 0 -> None, 1 -> Some(key),
        // 2 -> None. Every invocation records its phase.
        let phase = Rc::new(RefCell::new(0usize));
        let calls = Rc::new(RefCell::new(Vec::<usize>::new()));
        let resolver_phase = phase.clone();
        let resolver_calls = calls.clone();
        let resolver: ContentKeyResolver = Rc::new(move |_, _| {
            let current = *resolver_phase.borrow();
            resolver_calls.borrow_mut().push(current);
            (current == 1).then_some(key(7))
        });
        let (participants, _spares, window_cx) =
            word_drag_window_with_resolver(cx, &[(text, 0., 0, 7)], 1, Some(resolver));
        let first = participants.borrow()[0].clone();
        let words = text.find("words").unwrap();
        let beta = text.find("beta").unwrap();
        let calls_in_phase =
            |phase: usize| calls.borrow().iter().filter(|call| **call == phase).count();

        // Phase 0: the resolver declines every lookup, so nothing resolves.
        double_click(window_cx, first.glyph_center(words + 1));
        drag_to(window_cx, first.glyph_center(1));
        window_cx.update(|window, cx| {
            assert_eq!(TextSelection::content_keys(window, cx), None);
            assert_eq!(calls_in_phase(0), 4);
        });

        // Phase 1: a reverse update anchors on the original word's end and
        // resolves it; the resolved key must reach the retained original.
        *phase.borrow_mut() = 1;
        drag_to(window_cx, first.glyph_center(1));
        window_cx.update(|window, cx| {
            assert_eq!(
                TextSelection::content_keys(window, cx),
                Some([key(7), key(7)])
            );
            assert_eq!(calls_in_phase(1), 2);
        });
        // A forward update anchors on the original word's start and resolves
        // it too, so both retained originals now carry resolved keys.
        drag_to(window_cx, first.glyph_center(beta + 1));
        window_cx.update(|_, cx| {
            assert_eq!(
                first.selection.snapshot(cx).unwrap().anchor().content_key(),
                Some(key(7))
            );
        });

        // Phase 2: the resolver declines again. Another reverse update keeps
        // the retained original end resolved: only the fresh cursor endpoint
        // consults the resolver, and the anchor endpoint carries its key.
        *phase.borrow_mut() = 2;
        drag_to(window_cx, first.glyph_center(1));
        window_cx.update(|_, cx| {
            assert_eq!(
                first.selection.snapshot(cx).unwrap().anchor().content_key(),
                Some(key(7))
            );
            assert_eq!(
                calls_in_phase(2),
                1,
                "only the fresh cursor endpoint consults the resolver"
            );
        });

        // Dragging back inside the original word restores both retained
        // originals, whose resolved keys need no resolver replay at all.
        drag_to(window_cx, first.glyph_center(words + 1));
        window_cx.update(|window, cx| {
            assert_eq!(
                TextSelection::content_keys(window, cx),
                Some([key(7), key(7)])
            );
            assert_eq!(
                calls_in_phase(2),
                1,
                "restoring both retained originals needs no resolver replay"
            );
        });
        assert_eq!(selected_text_after_repaint(window_cx), "words");
    }

    #[gpui::test]
    fn word_drag_segments_rebuild_when_run_text_changes(cx: &mut TestAppContext) {
        let first_text = "alpha words beta";
        let replacement = "say can't stop";
        let (replacement_text, replacement_layout) =
            laid_out_runs(&[(replacement, 0.)], cx).pop().unwrap();
        let (participants, _spares, window_cx) = word_drag_window(cx, &[(first_text, 0., 0, 1)], 1);
        let first = participants.borrow()[0].clone();
        let words = first_text.find("words").unwrap();
        let beta = first_text.find("beta").unwrap();

        // The first gesture populates the cached word segments for the
        // painted run text.
        double_click(window_cx, first.glyph_center(words + 1));
        drag_to(window_cx, first.glyph_center(beta + 1));
        assert_eq!(selected_text_after_repaint(window_cx), "words beta");
        release_at(window_cx, first.glyph_center(beta + 1));

        // Repaint the same participant with different run text: the cached
        // segments must rebuild, so a new gesture snaps to the new whole
        // words (the apostrophe joins `can't` into one UAX #29 word).
        window_cx.update(|window, cx| {
            {
                let mut participants = participants.borrow_mut();
                participants[0].text = replacement_text;
                participants[0].layout = replacement_layout;
            }
            _ = window.draw(cx);
        });
        window_cx.run_until_parked();
        let renewed = participants.borrow()[0].clone();
        let cant = replacement.find("can't").unwrap();
        let stop = replacement.find("stop").unwrap();

        double_click(window_cx, renewed.glyph_center(cant + 1));
        assert_eq!(selected_text_after_repaint(window_cx), "can't");
        drag_to(window_cx, renewed.glyph_center(stop + 1));
        assert_eq!(selected_text_after_repaint(window_cx), "can't stop");
        drag_to(window_cx, renewed.glyph_center(1));
        assert_eq!(selected_text_after_repaint(window_cx), "say can't");
    }

    #[gpui::test]
    fn stationary_word_drag_tick_snaps_the_target_word_and_stops_on_mouse_up(
        cx: &mut TestAppContext,
    ) {
        let (participants, spares, window_cx) = word_drag_window(
            cx,
            &[("alpha words tail", 0., 0, 1), ("second words tail", 40., 1, 2)],
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

        let first_words = "alpha words tail".find("words").unwrap();
        let second_words = "second words tail".find("words").unwrap();
        double_click(window_cx, first.glyph_center(first_words + 1));

        // Hold the pointer in the bottom auto-scroll zone below the mounted
        // text: the drag extends over the gap to the terminal run end.
        let pointer = point(second.glyph_center(second_words + 1).x, px(50.));
        drag_to(window_cx, pointer);
        assert!(commands.borrow().last().is_some_and(Option::is_some));
        assert_eq!(selected_text_after_repaint(window_cx), "words tail");

        // The scroll host mounts a new row under the stationary pointer; the
        // stationary tick snaps the cursor to the target word's outer edge
        // without recomputing the auto-scroll command.
        window_cx.update(|window, cx| {
            participants.borrow_mut().push(second.clone());
            _ = window.draw(cx);
        });
        window_cx.run_until_parked();
        assert!(window_cx.update(TextSelection::update_drag_at_pointer));
        assert_eq!(
            commands.borrow().len(),
            1,
            "the stationary tick never recomputes auto-scroll"
        );
        window_cx.update(|_, cx| {
            let snapshot = second.selection.snapshot(cx).unwrap();
            assert_eq!(
                snapshot.cursor().content_point(),
                second.content_point(second_words + 5)
            );
        });
        assert_eq!(
            selected_text_after_repaint(window_cx),
            "words tail\nsecond words"
        );

        release_at(window_cx, pointer);
        assert_eq!(commands.borrow().last(), Some(&None));
        assert_eq!(
            selected_text_after_repaint(window_cx),
            "words tail\nsecond words"
        );
    }

    #[gpui::test]
    fn triple_and_quadruple_clicks_stay_static_line_selections(cx: &mut TestAppContext) {
        let text = "alpha words beta";
        let (participants, _spares, window_cx) = word_drag_window(cx, &[(text, 0., 0, 1)], 1);
        let first = participants.borrow()[0].clone();
        let words = text.find("words").unwrap();
        let beta = text.find("beta").unwrap();

        for click_count in [3usize, 4usize] {
            window_cx.simulate_event(MouseDownEvent {
                position: first.glyph_center(words + 1),
                modifiers: Modifiers::default(),
                button: MouseButton::Left,
                click_count,
                first_mouse: false,
            });
            // Moving while the button is still held does not extend the
            // line: triple and further clicks stay static selections.
            drag_to(window_cx, first.glyph_center(beta + 1));
            assert_eq!(selected_text_after_repaint(window_cx), text);
            window_cx.simulate_event(MouseUpEvent {
                position: first.glyph_center(beta + 1),
                modifiers: Modifiers::default(),
                button: MouseButton::Left,
                click_count,
            });
            assert_eq!(selected_text_after_repaint(window_cx), text);
            window_cx.update(|_, cx| {
                assert!(!first.selection.snapshot(cx).unwrap().is_selecting());
            });

            // A move after release does not extend the line either.
            drag_to(window_cx, first.glyph_center(words + 1));
            assert_eq!(selected_text_after_repaint(window_cx), text);
        }
    }

    #[gpui::test]
    fn mapping_failure_keeps_the_last_valid_word_range(cx: &mut TestAppContext) {
        let text = "alpha words beta";
        let (participants, _spares, window_cx) = word_drag_window(cx, &[(text, 0., 0, 1)], 1);
        let first = participants.borrow()[0].clone();
        let words = text.find("words").unwrap();
        let beta = text.find("beta").unwrap();

        double_click(window_cx, first.glyph_center(words + 1));
        drag_to(window_cx, first.glyph_center(beta + 1));
        assert_eq!(selected_text_after_repaint(window_cx), "words beta");
        let cursor = window_cx.update(|_, cx| {
            first.selection.snapshot(cx).unwrap().cursor().content_point()
        });

        // Unmount every participant mid-drag: the next update cannot map the
        // pointer to any word and must keep the last valid word range.
        window_cx.update(|window, cx| {
            participants.borrow_mut().clear();
            _ = window.draw(cx);
        });
        window_cx.run_until_parked();
        drag_to(window_cx, first.glyph_center(1));
        window_cx.update(|window, cx| {
            let snapshot = first.selection.snapshot(cx).unwrap();
            assert_eq!(snapshot.cursor().content_point(), cursor);
            assert!(snapshot.is_selecting());
            assert_eq!(TextSelection::content_keys(window, cx), Some([key(1), key(1)]));
        });
        assert_eq!(selected_text_after_repaint(window_cx), "words beta");
    }

    #[gpui::test]
    fn word_drag_outside_text_above_reverses_to_the_whole_run_start(cx: &mut TestAppContext) {
        let text = "alpha words tail";
        let (participants, _spares, window_cx) = word_drag_window(cx, &[(text, 40., 0, 1)], 1);
        let first = participants.borrow()[0].clone();
        let words = text.find("words").unwrap();
        let tail = text.find("tail").unwrap();

        double_click(window_cx, first.glyph_center(words + 1));
        assert_eq!(selected_text_after_repaint(window_cx), "words");

        // Dragging above the participant's text is outside-text and before
        // the original word: the selection reverses to cover the whole
        // terminal run start through the original word's end, never a
        // partial character.
        let above = point(first.glyph_center(tail + 1).x, px(10.));
        drag_to(window_cx, above);
        window_cx.update(|_, cx| {
            let snapshot = first.selection.snapshot(cx).unwrap();
            assert_eq!(snapshot.cursor().content_point(), first.content_point(0));
            assert_eq!(
                snapshot.anchor().content_point(),
                first.content_point(words + 5)
            );
        });
        assert_eq!(selected_text_after_repaint(window_cx), "alpha words");

        // Dragging back inside the original word restores it whole.
        drag_to(window_cx, first.glyph_center(words + 1));
        assert_eq!(selected_text_after_repaint(window_cx), "words");
    }
}
