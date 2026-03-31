//! Model Selector View implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P07
//! @requirement REQ-UI-MS

mod command;
mod ime;
mod render;

use gpui::{FocusHandle, UniformListScrollHandle};
use std::sync::Arc;

use crate::events::types::UserEvent;
use crate::ui_gpui::bridge::GpuiBridge;

/// Model information for display
/// @plan PLAN-20250130-GPUIREDUX.P07
#[derive(Clone, Debug, PartialEq)]
pub struct ModelInfo {
    pub id: String,
    pub provider_id: String,
    pub context: u64,
    pub reasoning: bool,
    pub vision: bool,
    pub cost_input: f64,
    pub cost_output: f64,
}

impl ModelInfo {
    pub fn new(id: impl Into<String>, provider_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            provider_id: provider_id.into(),
            context: 128_000,
            reasoning: false,
            vision: false,
            cost_input: 0.0,
            cost_output: 0.0,
        }
    }

    #[must_use]
    pub const fn with_context(mut self, context: u64) -> Self {
        self.context = context;
        self
    }

    #[must_use]
    pub const fn with_capabilities(mut self, reasoning: bool, vision: bool) -> Self {
        self.reasoning = reasoning;
        self.vision = vision;
        self
    }

    #[must_use]
    pub const fn with_costs(mut self, input: f64, output: f64) -> Self {
        self.cost_input = input;
        self.cost_output = output;
        self
    }

    /// Format context for display (e.g., "128K", "1M")
    #[must_use]
    pub fn context_display(&self) -> String {
        if self.context >= 1_000_000 {
            format!("{}M", self.context / 1_000_000)
        } else if self.context >= 1_000 {
            format!("{}K", self.context / 1_000)
        } else {
            self.context.to_string()
        }
    }

    /// Format cost for display (e.g., "$3", "$0.25", "free")
    #[must_use]
    pub fn cost_display(cost: f64) -> String {
        #[allow(clippy::float_cmp)]
        if cost == 0.0 {
            "free".to_string()
        } else if (cost - cost.floor()).abs() < f64::EPSILON {
            #[allow(clippy::cast_possible_truncation)]
            let whole = cost as i64;
            format!("${whole}")
        } else {
            format!("${cost:.2}")
        }
    }
}

/// Provider information
/// @plan PLAN-20250130-GPUIREDUX.P07
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
}

impl ProviderInfo {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

/// Row in the pre-computed display list for the model selector.
///
/// `Model(usize)` indices are valid only for the current `cached_display_rows`.
/// They are rebuilt atomically in `rebuild_display_rows()`. The render callback
/// uses `.get(ix)` to guard against stale reads.
#[derive(Clone, Debug)]
pub(super) enum DisplayRow {
    ProviderHeader(#[allow(dead_code)] String),
    Model(usize),
}

/// Pre-lowercased wrapper for fast case-insensitive search.
/// Invariant: `searchable_models[i]` corresponds to `models[i]`.
/// Both are built atomically in `load_models()`.
#[derive(Clone, Debug)]
pub(super) struct SearchableModelInfo {
    pub info: ModelInfo,
    pub id_lower: String,
    pub provider_lower: String,
}

/// Model Selector view state
/// @plan PLAN-20250130-GPUIREDUX.P07
#[derive(Clone, Default)]
pub struct ModelSelectorState {
    // NOTE: unused for rendering — see cached_providers
    pub providers: Vec<ProviderInfo>,
    pub models: Vec<ModelInfo>,
    pub search_query: String,
    pub selected_provider: Option<String>,
    pub filter_reasoning: bool,
    pub filter_vision: bool,
    pub show_provider_dropdown: bool,
    pub(super) searchable_models: Vec<SearchableModelInfo>,
    pub(super) cached_providers: Vec<String>,
    pub(super) cached_display_rows: Vec<DisplayRow>,
    pub(super) cached_model_count: usize,
    pub(super) cached_provider_count: usize,
}

impl ModelSelectorState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Load models and providers, rebuilding all cached display state.
    pub fn load_models(&mut self, providers: Vec<ProviderInfo>, models: Vec<ModelInfo>) {
        self.providers = providers;
        self.models.clone_from(&models);

        // Build searchable models with pre-lowered fields
        self.searchable_models = models
            .into_iter()
            .map(|m| {
                let id_lower = m.id.to_lowercase();
                let provider_lower = m.provider_id.to_lowercase();
                SearchableModelInfo {
                    info: m,
                    id_lower,
                    provider_lower,
                }
            })
            .collect();

        // Build sorted, deduped provider list from model data
        let mut provider_ids: Vec<String> = self
            .searchable_models
            .iter()
            .map(|s| s.info.provider_id.clone())
            .collect();
        provider_ids.sort_unstable();
        provider_ids.dedup();
        self.cached_providers = provider_ids;

        // Clear selected_provider if it's no longer valid
        if let Some(ref provider) = self.selected_provider {
            if !self.cached_providers.contains(provider) {
                self.selected_provider = None;
            }
        }

        self.rebuild_display_rows();
    }

    /// Rebuild the cached display rows from current filters.
    pub fn rebuild_display_rows(&mut self) {
        let query_lower = self.search_query.to_lowercase();
        self.cached_display_rows.clear();

        for provider in &self.cached_providers {
            // Skip providers that don't match the selected provider filter
            if let Some(ref selected) = self.selected_provider {
                if provider != selected {
                    continue;
                }
            }

            // Collect matching model indices for this provider
            let mut matching: Vec<usize> = self
                .searchable_models
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    // Provider must match
                    if &s.info.provider_id != provider {
                        return false;
                    }
                    // Search filter
                    if !query_lower.is_empty()
                        && !s.id_lower.contains(&query_lower)
                        && !s.provider_lower.contains(&query_lower)
                    {
                        return false;
                    }
                    // Capability filters
                    if self.filter_reasoning && !s.info.reasoning {
                        return false;
                    }
                    if self.filter_vision && !s.info.vision {
                        return false;
                    }
                    true
                })
                .map(|(i, _)| i)
                .collect();

            // Sort matching models within provider by model ID (alphabetical)
            matching.sort_by(|&a, &b| {
                self.searchable_models[a]
                    .info
                    .id
                    .cmp(&self.searchable_models[b].info.id)
            });

            if !matching.is_empty() {
                self.cached_display_rows
                    .push(DisplayRow::ProviderHeader(provider.clone()));
                for idx in matching {
                    self.cached_display_rows.push(DisplayRow::Model(idx));
                }
            }
        }

        // Update counts from built rows
        self.cached_model_count = self
            .cached_display_rows
            .iter()
            .filter(|r| matches!(r, DisplayRow::Model(_)))
            .count();
        self.cached_provider_count = self
            .cached_display_rows
            .iter()
            .filter(|r| matches!(r, DisplayRow::ProviderHeader(_)))
            .count();
    }

    /// Get filtered models based on current filters (delegates to cache).
    #[must_use]
    pub fn filtered_models(&self) -> Vec<&ModelInfo> {
        self.cached_display_rows
            .iter()
            .filter_map(|row| match row {
                DisplayRow::Model(idx) => self.models.get(*idx),
                DisplayRow::ProviderHeader(_) => None,
            })
            .collect()
    }

    /// Get unique providers from all models (delegates to cache).
    #[must_use]
    pub fn all_providers(&self) -> Vec<&str> {
        self.cached_providers.iter().map(String::as_str).collect()
    }

    /// Number of models in the current filtered view.
    pub(super) const fn cached_filtered_model_count(&self) -> usize {
        self.cached_model_count
    }

    /// Number of providers visible in the current filtered view.
    pub(super) const fn cached_visible_provider_count(&self) -> usize {
        self.cached_provider_count
    }
}

/// Model Selector view component
/// @plan PLAN-20250130-GPUIREDUX.P07
pub struct ModelSelectorView {
    pub(super) state: ModelSelectorState,
    pub(super) bridge: Option<Arc<GpuiBridge>>,
    pub(super) focus_handle: FocusHandle,
    pub(super) ime_marked_byte_count: usize,
    pub(super) scroll_handle: UniformListScrollHandle,
}

impl ModelSelectorView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state: ModelSelectorState::new(),
            bridge: None,
            focus_handle: cx.focus_handle(),
            ime_marked_byte_count: 0,
            scroll_handle: UniformListScrollHandle::default(),
        }
    }

    /// Set the event bridge
    /// @plan PLAN-20250130-GPUIREDUX.P07
    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
    }

    /// Set models from presenter
    pub fn set_models(&mut self, providers: Vec<ProviderInfo>, models: Vec<ModelInfo>) {
        self.state.load_models(providers, models);
        self.scroll_handle
            .scroll_to_item(0, gpui::ScrollStrategy::Top);
    }

    /// Set search query programmatically (for testing)
    pub fn set_search_query(&mut self, query: String) {
        self.state.search_query = query;
    }

    /// Set selected provider programmatically (for testing)
    pub fn set_selected_provider(&mut self, provider: Option<String>) {
        self.state.selected_provider = provider;
    }

    /// Get current state for testing
    #[must_use]
    pub const fn get_state(&self) -> &ModelSelectorState {
        &self.state
    }

    /// Emit a `UserEvent` through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P07
    fn emit(&self, event: &UserEvent) {
        if let Some(bridge) = &self.bridge {
            if !bridge.emit(event.clone()) {
                tracing::error!("Failed to emit event {:?}", event);
            }
        } else {
            tracing::warn!("No bridge set - event not emitted: {:?}", event);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::future_not_send)]

    use super::*;
    use crate::events::types::UserEvent;
    use crate::presentation::view_command::ViewCommand;
    use flume;
    use gpui::{AppContext, EntityInputHandler, TestAppContext};

    fn remote_model(
        provider_id: &str,
        model_id: &str,
        context_length: Option<u32>,
    ) -> crate::presentation::view_command::ModelInfo {
        crate::presentation::view_command::ModelInfo {
            provider_id: provider_id.to_string(),
            model_id: model_id.to_string(),
            name: model_id.to_string(),
            context_length,
        }
    }

    #[test]
    fn model_info_formatting_and_state_filters_work() {
        let free = ModelInfo::new("claude", "anthropic")
            .with_context(200_000)
            .with_capabilities(true, false)
            .with_costs(0.0, 3.5);
        assert_eq!(free.context_display(), "200K");
        assert_eq!(ModelInfo::cost_display(0.0), "free");
        assert_eq!(ModelInfo::cost_display(3.0), "$3");
        assert_eq!(ModelInfo::cost_display(0.25), "$0.25");

        let vision = ModelInfo::new("gpt-4o", "openai")
            .with_context(1_000_000)
            .with_capabilities(false, true);
        assert_eq!(vision.context_display(), "1M");

        let mut state = ModelSelectorState::new();
        state.load_models(vec![], vec![free, vision]);
        assert_eq!(state.filtered_models().len(), 2);

        state.selected_provider = Some("anthropic".to_string());
        state.rebuild_display_rows();
        let filtered = state.filtered_models();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].provider_id, "anthropic");

        state.selected_provider = None;
        state.search_query = "4o".to_string();
        state.rebuild_display_rows();
        let filtered = state.filtered_models();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "gpt-4o");

        state.search_query.clear();
        state.filter_reasoning = true;
        state.rebuild_display_rows();
        let filtered = state.filtered_models();
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].reasoning);

        state.filter_reasoning = false;
        state.filter_vision = true;
        state.rebuild_display_rows();
        let filtered = state.filtered_models();
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].vision);

        assert_eq!(state.all_providers(), vec!["anthropic", "openai"]);
    }

    #[gpui::test]
    async fn handle_command_maps_models_and_request_emits_open_selector(cx: &mut TestAppContext) {
        let (user_tx, user_rx) = flume::bounded(16);
        let (_view_tx, view_rx) = flume::bounded(16);
        let bridge = Arc::new(GpuiBridge::new(user_tx, view_rx));
        let view = cx.new(ModelSelectorView::new);

        view.update(cx, |view: &mut ModelSelectorView, cx| {
            view.set_bridge(Arc::clone(&bridge));
            view.handle_command(
                ViewCommand::ModelSearchResults {
                    models: vec![
                        remote_model("anthropic", "claude-3-5-sonnet", Some(200_000)),
                        remote_model("openai", "gpt-4o", Some(128_000)),
                        remote_model("anthropic", "claude-haiku", None),
                    ],
                },
                cx,
            );

            assert_eq!(view.state.models.len(), 3);
            assert_eq!(view.state.providers.len(), 2);
            assert_eq!(view.state.models[0].id, "claude-3-5-sonnet");
            assert_eq!(view.state.models[0].context, 200_000);
            assert_eq!(view.state.models[2].context, 128_000);

            // Filtering is now local-only; no SearchModels / FilterModelsByProvider
            // events are emitted.  Verify the state-level filter logic still works.
            view.state.search_query = "claude".to_string();
            view.state.rebuild_display_rows();
            let filtered = view.state.filtered_models();
            assert_eq!(filtered.len(), 2);

            view.state.selected_provider = Some("anthropic".to_string());
            view.state.rebuild_display_rows();
            let filtered = view.state.filtered_models();
            assert_eq!(filtered.len(), 2);

            view.request_models();
        });

        assert_eq!(
            user_rx.recv().expect("open selector event"),
            UserEvent::OpenModelSelector
        );
        assert!(
            user_rx.try_recv().is_err(),
            "no filter/search events should be emitted"
        );
    }

    #[gpui::test]
    async fn input_handler_mutates_search_query_and_marks_composition(cx: &mut TestAppContext) {
        let view = cx.new(ModelSelectorView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut ModelSelectorView, cx| {
                view.replace_text_in_range(None, "cla", window, cx);
                assert_eq!(view.state.search_query, "cla");
                assert_eq!(
                    view.text_for_range(0..2, &mut None, window, cx),
                    Some("cl".to_string())
                );

                view.replace_and_mark_text_in_range(None, "u", None, window, cx);
                assert_eq!(view.state.search_query, "clau");
                assert!(view.marked_text_range(window, cx).is_some());

                view.replace_text_in_range(None, "de", window, cx);
                assert_eq!(view.state.search_query, "clade");
                assert_eq!(view.marked_text_range(window, cx), None);

                let selected = view
                    .selected_text_range(false, window, cx)
                    .expect("selection range");
                let len = "clade".encode_utf16().count();
                assert_eq!(selected.range, len..len);

                view.unmark_text(window, cx);
                assert_eq!(view.marked_text_range(window, cx), None);
            });
        });
    }

    #[gpui::test]
    async fn provider_dropdown_selection_and_model_emission_follow_real_filter_rules(
        cx: &mut TestAppContext,
    ) {
        let (user_tx, user_rx) = flume::bounded(16);
        let (_view_tx, view_rx) = flume::bounded(16);
        let bridge = Arc::new(GpuiBridge::new(user_tx, view_rx));
        let view = cx.new(ModelSelectorView::new);

        view.update(cx, |view: &mut ModelSelectorView, cx| {
            view.set_bridge(Arc::clone(&bridge));
            view.set_models(
                vec![
                    ProviderInfo::new("anthropic", "anthropic"),
                    ProviderInfo::new("openai", "openai"),
                ],
                vec![
                    ModelInfo::new("claude-3-7-sonnet", "anthropic")
                        .with_context(200_000)
                        .with_capabilities(true, false),
                    ModelInfo::new("gpt-4o", "openai")
                        .with_context(128_000)
                        .with_capabilities(false, true),
                ],
            );

            view.toggle_provider_dropdown(cx);
            assert!(view.get_state().show_provider_dropdown);

            view.select_provider_filter("anthropic".to_string(), cx);
            assert_eq!(
                view.get_state().selected_provider.as_deref(),
                Some("anthropic")
            );
            assert!(!view.get_state().show_provider_dropdown);
            assert_eq!(view.get_state().filtered_models().len(), 1);
            assert_eq!(
                view.get_state().filtered_models()[0].id,
                "claude-3-7-sonnet"
            );

            view.toggle_reasoning_filter(cx);
            assert!(view.get_state().filter_reasoning);
            assert_eq!(view.get_state().filtered_models().len(), 1);

            view.toggle_vision_filter(cx);
            assert!(view.get_state().filter_vision);
            assert!(view.get_state().filtered_models().is_empty());

            view.clear_provider_filter(cx);
            assert_eq!(view.get_state().selected_provider, None);
            assert_eq!(view.get_state().filtered_models().len(), 0);

            view.toggle_vision_filter(cx);
            assert!(!view.get_state().filter_vision);
            assert_eq!(view.get_state().filtered_models().len(), 1);
            assert_eq!(
                view.get_state().filtered_models()[0].id,
                "claude-3-7-sonnet"
            );

            view.select_model("anthropic".to_string(), "claude-3-7-sonnet".to_string());
            assert!(!view.get_state().show_provider_dropdown);
        });

        // Only SelectModel should be emitted — filter changes are local-only now.
        assert_eq!(
            user_rx.recv().expect("select model event"),
            UserEvent::SelectModel {
                provider_id: "anthropic".to_string(),
                model_id: "claude-3-7-sonnet".to_string(),
            }
        );
        assert!(
            user_rx.try_recv().is_err(),
            "unexpected additional selector events"
        );
    }

    #[gpui::test]
    async fn key_handling_closes_dropdown_navigates_and_backspaces_search(cx: &mut TestAppContext) {
        while crate::ui_gpui::navigation_channel()
            .take_pending()
            .is_some()
        {}
        let view = cx.new(ModelSelectorView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut ModelSelectorView, cx| {
                view.state.show_provider_dropdown = true;
                view.state.search_query = "claude".to_string();

                view.handle_key_down(
                    &gpui::KeyDownEvent {
                        keystroke: gpui::Keystroke::parse("escape").expect("escape keystroke"),
                        is_held: false,
                        prefer_character_input: false,
                    },
                    cx,
                );
                assert!(!view.state.show_provider_dropdown);
                assert_eq!(crate::ui_gpui::navigation_channel().take_pending(), None);

                view.handle_key_down(
                    &gpui::KeyDownEvent {
                        keystroke: gpui::Keystroke::parse("backspace")
                            .expect("backspace keystroke"),
                        is_held: false,
                        prefer_character_input: false,
                    },
                    cx,
                );
                assert_eq!(view.state.search_query, "claud");

                view.handle_key_down(
                    &gpui::KeyDownEvent {
                        keystroke: gpui::Keystroke::parse("cmd-w").expect("cmd-w keystroke"),
                        is_held: false,
                        prefer_character_input: false,
                    },
                    cx,
                );
                assert_eq!(
                    crate::ui_gpui::navigation_channel().take_pending(),
                    Some(crate::presentation::view_command::ViewId::Settings)
                );

                view.replace_and_mark_text_in_range(None, "e", None, window, cx);
                assert_eq!(view.state.search_query, "claude");
                assert_eq!(view.marked_text_range(window, cx), Some(5..6));
                view.replace_text_in_range(None, "e-3", window, cx);
                assert_eq!(view.state.search_query, "claude-3");
                assert_eq!(view.marked_text_range(window, cx), None);
            });
        });
        // No SearchModels events emitted — filtering is local-only.
    }

    fn test_models() -> (Vec<ProviderInfo>, Vec<ModelInfo>) {
        let providers = vec![
            ProviderInfo::new("anthropic", "Anthropic"),
            ProviderInfo::new("openai", "OpenAI"),
            ProviderInfo::new("google", "Google"),
        ];
        let models = vec![
            ModelInfo::new("claude-3-5-sonnet", "anthropic").with_capabilities(true, false),
            ModelInfo::new("claude-haiku", "anthropic"),
            ModelInfo::new("gpt-4o", "openai").with_capabilities(false, true),
            ModelInfo::new("gpt-4-mini", "openai"),
            ModelInfo::new("gemini-pro", "google").with_capabilities(true, true),
            ModelInfo::new("gemini-flash", "google"),
        ];
        (providers, models)
    }

    // --- Test 1: load_models builds searchable_models and cached_providers ---
    #[test]
    fn test_load_models_builds_searchable_models_and_cached_providers() {
        let (providers, models) = test_models();
        let mut state = ModelSelectorState::new();
        state.load_models(providers, models);

        assert_eq!(state.models.len(), 6);
        assert_eq!(state.searchable_models.len(), 6);
        // cached_providers sorted, deduped
        assert_eq!(
            state.cached_providers,
            vec!["anthropic", "google", "openai"]
        );
    }

    // --- Test 2: load_models clears stale selected_provider ---
    #[test]
    fn test_load_models_clears_stale_selected_provider() {
        let (providers, models) = test_models();
        let mut state = ModelSelectorState::new();
        state.selected_provider = Some("nonexistent".to_string());
        state.load_models(providers, models);
        assert_eq!(state.selected_provider, None);
    }

    // --- Test 3: load_models preserves valid selected_provider ---
    #[test]
    fn test_load_models_preserves_valid_selected_provider() {
        let (providers, models) = test_models();
        let mut state = ModelSelectorState::new();
        state.selected_provider = Some("openai".to_string());
        state.load_models(providers, models);
        assert_eq!(state.selected_provider.as_deref(), Some("openai"));
    }

    // --- Test 4: rebuild_display_rows empty query returns all ---
    #[test]
    fn test_rebuild_display_rows_empty_query_returns_all() {
        let (providers, models) = test_models();
        let mut state = ModelSelectorState::new();
        state.load_models(providers, models);

        assert_eq!(state.cached_model_count, 6);
        assert_eq!(state.cached_provider_count, 3);
        assert_eq!(state.filtered_models().len(), 6);
    }

    // --- Test 5: rebuild_display_rows search query filters ---
    #[test]
    fn test_rebuild_display_rows_search_query_filters() {
        let (providers, models) = test_models();
        let mut state = ModelSelectorState::new();
        state.load_models(providers, models);

        state.search_query = "claude".to_string();
        state.rebuild_display_rows();
        assert_eq!(state.cached_model_count, 2);
        let filtered = state.filtered_models();
        assert!(filtered.iter().all(|m| m.id.contains("claude")));
    }

    // --- Test 6: rebuild_display_rows provider filter ---
    #[test]
    fn test_rebuild_display_rows_provider_filter() {
        let (providers, models) = test_models();
        let mut state = ModelSelectorState::new();
        state.load_models(providers, models);

        state.selected_provider = Some("openai".to_string());
        state.rebuild_display_rows();
        assert_eq!(state.cached_model_count, 2);
        let filtered = state.filtered_models();
        assert!(filtered.iter().all(|m| m.provider_id == "openai"));
    }

    // --- Test 7: rebuild_display_rows reasoning filter ---
    #[test]
    fn test_rebuild_display_rows_reasoning_filter() {
        let (providers, models) = test_models();
        let mut state = ModelSelectorState::new();
        state.load_models(providers, models);

        state.filter_reasoning = true;
        state.rebuild_display_rows();
        assert_eq!(state.cached_model_count, 2);
        let filtered = state.filtered_models();
        assert!(filtered.iter().all(|m| m.reasoning));
    }

    // --- Test 8: rebuild_display_rows vision filter ---
    #[test]
    fn test_rebuild_display_rows_vision_filter() {
        let (providers, models) = test_models();
        let mut state = ModelSelectorState::new();
        state.load_models(providers, models);

        state.filter_vision = true;
        state.rebuild_display_rows();
        assert_eq!(state.cached_model_count, 2);
        let filtered = state.filtered_models();
        assert!(filtered.iter().all(|m| m.vision));
    }

    // --- Test 9: rebuild_display_rows combined filters ---
    #[test]
    fn test_rebuild_display_rows_combined_filters() {
        let (providers, models) = test_models();
        let mut state = ModelSelectorState::new();
        state.load_models(providers, models);

        state.selected_provider = Some("google".to_string());
        state.filter_reasoning = true;
        state.rebuild_display_rows();
        // Only gemini-pro has reasoning=true in google
        assert_eq!(state.cached_model_count, 1);
        assert_eq!(state.filtered_models()[0].id, "gemini-pro");
    }

    // --- Test 10: rebuild_display_rows no match returns empty ---
    #[test]
    fn test_rebuild_display_rows_no_match_returns_empty() {
        let (providers, models) = test_models();
        let mut state = ModelSelectorState::new();
        state.load_models(providers, models);

        state.search_query = "zzzzz".to_string();
        state.rebuild_display_rows();
        assert_eq!(state.cached_model_count, 0);
        assert_eq!(state.cached_provider_count, 0);
        assert!(state.filtered_models().is_empty());
    }

    // --- Test 11: display_rows_ordering ---
    #[test]
    fn test_display_rows_ordering_by_provider_then_model() {
        let (providers, models) = test_models();
        let mut state = ModelSelectorState::new();
        state.load_models(providers, models);

        // Verify rows: provider headers interleaved with model rows,
        // providers in alphabetical order, models within provider alphabetical
        let row_kinds: Vec<&str> = state
            .cached_display_rows
            .iter()
            .map(|r| match r {
                DisplayRow::ProviderHeader(p) => p.as_str(),
                DisplayRow::Model(_) => "model",
            })
            .collect();

        assert_eq!(
            row_kinds,
            vec![
                "anthropic",
                "model", // claude-3-5-sonnet
                "model", // claude-haiku
                "google",
                "model", // gemini-flash
                "model", // gemini-pro
                "openai",
                "model", // gpt-4-mini
                "model", // gpt-4o
            ]
        );
    }

    // --- Test 12: all_providers returns sorted deduped ---
    #[test]
    fn test_all_providers_returns_sorted_deduped() {
        let (providers, models) = test_models();
        let mut state = ModelSelectorState::new();
        state.load_models(providers, models);

        let providers = state.all_providers();
        assert_eq!(providers, vec!["anthropic", "google", "openai"]);
    }

    // --- Test 13: searchable_models_pre_lowercase ---
    #[test]
    fn test_searchable_models_pre_lowercase() {
        let (providers, models) = test_models();
        let mut state = ModelSelectorState::new();
        state.load_models(providers, models);

        for s in &state.searchable_models {
            assert_eq!(s.id_lower, s.info.id.to_lowercase());
            assert_eq!(s.provider_lower, s.info.provider_id.to_lowercase());
        }
    }

    // --- Test 14: case_insensitive_search ---
    #[test]
    fn test_case_insensitive_search() {
        let (providers, models) = test_models();
        let mut state = ModelSelectorState::new();
        state.load_models(providers, models);

        state.search_query = "GPT".to_string();
        state.rebuild_display_rows();
        assert_eq!(state.cached_model_count, 2);

        state.search_query = "Claude".to_string();
        state.rebuild_display_rows();
        assert_eq!(state.cached_model_count, 2);
    }

    // --- Test 15: cached_counts_match_filtered ---
    #[test]
    fn test_cached_counts_match_filtered() {
        let (providers, models) = test_models();
        let mut state = ModelSelectorState::new();
        state.load_models(providers, models);

        state.search_query = "gemini".to_string();
        state.rebuild_display_rows();

        assert_eq!(
            state.cached_filtered_model_count(),
            state.filtered_models().len()
        );
        assert_eq!(state.cached_visible_provider_count(), 1);
    }

    // --- Test 16: load_models_empty ---
    #[test]
    fn test_load_models_empty() {
        let mut state = ModelSelectorState::new();
        state.load_models(vec![], vec![]);

        assert_eq!(state.cached_model_count, 0);
        assert_eq!(state.cached_provider_count, 0);
        assert!(state.filtered_models().is_empty());
        assert!(state.all_providers().is_empty());
    }

    // --- Test 17: rebuild_after_query_change ---
    #[test]
    fn test_rebuild_after_query_change() {
        let (providers, models) = test_models();
        let mut state = ModelSelectorState::new();
        state.load_models(providers, models);

        state.search_query = "flash".to_string();
        state.rebuild_display_rows();

        // "flash" only matches gemini-flash
        let filtered = state.filtered_models();
        let names: Vec<&str> = filtered.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(names, vec!["gemini-flash"]);
        assert_eq!(state.cached_model_count, 1);
        assert_eq!(state.cached_provider_count, 1);
    }

    // --- Test 18: display_row_indices_valid ---
    #[test]
    fn test_display_row_indices_valid() {
        let (providers, models) = test_models();
        let mut state = ModelSelectorState::new();
        state.load_models(providers, models);

        // Every Model(idx) must be a valid index into state.models
        for row in &state.cached_display_rows {
            if let DisplayRow::Model(idx) = row {
                assert!(
                    state.models.get(*idx).is_some(),
                    "Invalid model index {idx}"
                );
            }
        }
    }

    // --- Test 19 (GPUI): set_models_populates_cache ---
    #[gpui::test]
    async fn test_set_models_populates_cache(cx: &mut TestAppContext) {
        let view = cx.new(ModelSelectorView::new);
        let (providers, models) = test_models();

        view.update(cx, |view: &mut ModelSelectorView, _cx| {
            view.set_models(providers, models);
            assert_eq!(view.state.models.len(), 6);
            assert_eq!(view.state.cached_providers.len(), 3);
            assert_eq!(view.state.cached_model_count, 6);
        });
    }

    // --- Test 20 (GPUI): toggle_reasoning_rebuilds_cache ---
    #[gpui::test]
    async fn test_toggle_reasoning_rebuilds_cache(cx: &mut TestAppContext) {
        let view = cx.new(ModelSelectorView::new);
        let (providers, models) = test_models();

        view.update(cx, |view: &mut ModelSelectorView, cx| {
            view.set_models(providers, models);
            assert_eq!(view.state.cached_model_count, 6);

            view.toggle_reasoning_filter(cx);
            assert_eq!(view.state.cached_model_count, 2);
        });
    }

    // --- Test 21 (GPUI): toggle_vision_rebuilds_cache ---
    #[gpui::test]
    async fn test_toggle_vision_rebuilds_cache(cx: &mut TestAppContext) {
        let view = cx.new(ModelSelectorView::new);
        let (providers, models) = test_models();

        view.update(cx, |view: &mut ModelSelectorView, cx| {
            view.set_models(providers, models);
            view.toggle_vision_filter(cx);
            assert_eq!(view.state.cached_model_count, 2);
        });
    }

    // --- Test 22 (GPUI): select_provider_rebuilds_cache ---
    #[gpui::test]
    async fn test_select_provider_rebuilds_cache(cx: &mut TestAppContext) {
        let view = cx.new(ModelSelectorView::new);
        let (providers, models) = test_models();

        view.update(cx, |view: &mut ModelSelectorView, cx| {
            view.set_models(providers, models);
            view.select_provider_filter("google".to_string(), cx);
            assert_eq!(view.state.cached_model_count, 2);
            assert_eq!(view.state.cached_provider_count, 1);
        });
    }

    // --- Test 23 (GPUI): clear_provider_rebuilds_cache ---
    #[gpui::test]
    async fn test_clear_provider_rebuilds_cache(cx: &mut TestAppContext) {
        let view = cx.new(ModelSelectorView::new);
        let (providers, models) = test_models();

        view.update(cx, |view: &mut ModelSelectorView, cx| {
            view.set_models(providers, models);
            view.select_provider_filter("google".to_string(), cx);
            assert_eq!(view.state.cached_model_count, 2);

            view.clear_provider_filter(cx);
            assert_eq!(view.state.cached_model_count, 6);
        });
    }

    // --- Test 24 (GPUI): backspace_rebuilds_cache ---
    #[gpui::test]
    async fn test_backspace_rebuilds_cache(cx: &mut TestAppContext) {
        let view = cx.new(ModelSelectorView::new);
        let (providers, models) = test_models();
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|_window, app| {
            view.update(app, |view: &mut ModelSelectorView, cx| {
                view.set_models(providers, models);
                view.state.search_query = "claude".to_string();
                view.state.rebuild_display_rows();
                assert_eq!(view.state.cached_model_count, 2);

                view.handle_key_down(
                    &gpui::KeyDownEvent {
                        keystroke: gpui::Keystroke::parse("backspace")
                            .expect("backspace keystroke"),
                        is_held: false,
                        prefer_character_input: false,
                    },
                    cx,
                );
                assert_eq!(view.state.search_query, "claud");
                // After backspace, "claud" still only matches claude models
                assert_eq!(view.state.cached_model_count, 2);
            });
        });
    }

    // --- Test 25 (GPUI): ime_replace_text_rebuilds_cache ---
    #[gpui::test]
    async fn test_ime_replace_text_rebuilds_cache(cx: &mut TestAppContext) {
        let view = cx.new(ModelSelectorView::new);
        let (providers, models) = test_models();
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut ModelSelectorView, cx| {
                view.set_models(providers, models);
                assert_eq!(view.state.cached_model_count, 6);

                view.replace_text_in_range(None, "gemini", window, cx);
                assert_eq!(view.state.search_query, "gemini");
                assert_eq!(view.state.cached_model_count, 2);
            });
        });
    }

    // --- Test 26 (GPUI): ime_replace_and_mark_rebuilds_cache ---
    #[gpui::test]
    async fn test_ime_replace_and_mark_rebuilds_cache(cx: &mut TestAppContext) {
        let view = cx.new(ModelSelectorView::new);
        let (providers, models) = test_models();
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut ModelSelectorView, cx| {
                view.set_models(providers, models);

                view.replace_and_mark_text_in_range(None, "gpt", None, window, cx);
                assert_eq!(view.state.search_query, "gpt");
                assert_eq!(view.state.cached_model_count, 2);
            });
        });
    }
}
