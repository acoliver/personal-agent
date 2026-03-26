//! Model Selector View implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P07
//! @requirement REQ-UI-MS

mod command;
mod ime;
mod render;

use gpui::FocusHandle;
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

/// Model Selector view state
/// @plan PLAN-20250130-GPUIREDUX.P07
#[derive(Clone, Default)]
pub struct ModelSelectorState {
    pub providers: Vec<ProviderInfo>,
    pub models: Vec<ModelInfo>,
    pub search_query: String,
    pub selected_provider: Option<String>,
    pub filter_reasoning: bool,
    pub filter_vision: bool,
    pub show_provider_dropdown: bool,

    /// Last search query emitted to presenter, used to avoid redundant events.
    pub last_emitted_search_query: String,
    /// Last provider filter emitted to presenter, used to avoid redundant events.
    pub last_emitted_provider: Option<String>,
}

impl ModelSelectorState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get filtered models based on current filters
    #[must_use]
    pub fn filtered_models(&self) -> Vec<&ModelInfo> {
        self.models
            .iter()
            .filter(|m| {
                // Provider filter
                if let Some(ref provider) = self.selected_provider {
                    if &m.provider_id != provider {
                        return false;
                    }
                }
                // Search filter
                if !self.search_query.is_empty() {
                    let query = self.search_query.to_lowercase();
                    if !m.id.to_lowercase().contains(&query)
                        && !m.provider_id.to_lowercase().contains(&query)
                    {
                        return false;
                    }
                }
                // Capability filters
                if self.filter_reasoning && !m.reasoning {
                    return false;
                }
                if self.filter_vision && !m.vision {
                    return false;
                }
                true
            })
            .collect()
    }

    /// Get unique providers from all models (not just filtered)
    #[must_use]
    pub fn all_providers(&self) -> Vec<&str> {
        let mut providers: Vec<&str> = self.models.iter().map(|m| m.provider_id.as_str()).collect();
        providers.sort_unstable();
        providers.dedup();
        providers
    }
}

/// Model Selector view component
/// @plan PLAN-20250130-GPUIREDUX.P07
pub struct ModelSelectorView {
    pub(super) state: ModelSelectorState,
    pub(super) bridge: Option<Arc<GpuiBridge>>,
    pub(super) focus_handle: FocusHandle,
    pub(super) ime_marked_byte_count: usize,
}

impl ModelSelectorView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state: ModelSelectorState::new(),
            bridge: None,
            focus_handle: cx.focus_handle(),
            ime_marked_byte_count: 0,
        }
    }

    /// Set the event bridge
    /// @plan PLAN-20250130-GPUIREDUX.P07
    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
    }

    /// Set models from presenter
    pub fn set_models(&mut self, providers: Vec<ProviderInfo>, models: Vec<ModelInfo>) {
        self.state.providers = providers;
        self.state.models = models;
    }

    /// Set search query programmatically (for testing)
    pub fn set_search_query(&mut self, query: String) {
        self.state.search_query = query;
    }

    /// Set selected provider programmatically (for testing)
    pub fn set_selected_provider(&mut self, provider: Option<String>) {
        self.state.selected_provider = provider;
    }

    /// Emit `SearchModels` event for current query.
    pub fn emit_search_models(&self) {
        self.emit(&UserEvent::SearchModels {
            query: self.state.search_query.clone(),
        });
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
        state.models = vec![free, vision];
        assert_eq!(state.filtered_models().len(), 2);

        state.selected_provider = Some("anthropic".to_string());
        let filtered = state.filtered_models();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].provider_id, "anthropic");

        state.selected_provider = None;
        state.search_query = "4o".to_string();
        let filtered = state.filtered_models();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "gpt-4o");

        state.search_query.clear();
        state.filter_reasoning = true;
        let filtered = state.filtered_models();
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].reasoning);

        state.filter_reasoning = false;
        state.filter_vision = true;
        let filtered = state.filtered_models();
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].vision);

        assert_eq!(state.all_providers(), vec!["anthropic", "openai"]);
    }

    #[gpui::test]
    async fn handle_command_maps_models_and_filter_events_emit_only_on_changes(
        cx: &mut TestAppContext,
    ) {
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

            view.state.search_query = "claude".to_string();
            view.emit_filter_events_if_changed();
            view.emit_filter_events_if_changed();

            view.state.selected_provider = Some("anthropic".to_string());
            view.emit_filter_events_if_changed();
            view.emit_filter_events_if_changed();

            view.request_models();
            view.emit_search_models();
        });

        assert_eq!(
            user_rx.recv().expect("search event"),
            UserEvent::SearchModels {
                query: "claude".to_string(),
            }
        );
        assert_eq!(
            user_rx.recv().expect("provider filter event"),
            UserEvent::FilterModelsByProvider {
                provider_id: Some("anthropic".to_string()),
            }
        );
        assert_eq!(
            user_rx.recv().expect("open selector event"),
            UserEvent::OpenModelSelector
        );
        assert_eq!(
            user_rx.recv().expect("manual search emit"),
            UserEvent::SearchModels {
                query: "claude".to_string(),
            }
        );
        assert!(
            user_rx.try_recv().is_err(),
            "duplicate filter events should not be emitted"
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

        assert_eq!(
            user_rx.recv().expect("provider filter event"),
            UserEvent::FilterModelsByProvider {
                provider_id: Some("anthropic".to_string()),
            }
        );
        assert_eq!(
            user_rx.recv().expect("clear provider filter event"),
            UserEvent::FilterModelsByProvider { provider_id: None }
        );
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
    async fn key_handling_closes_dropdown_navigates_and_backspaces_search_once(
        cx: &mut TestAppContext,
    ) {
        while crate::ui_gpui::navigation_channel()
            .take_pending()
            .is_some()
        {}
        let (user_tx, user_rx) = flume::bounded(16);
        let (_view_tx, view_rx) = flume::bounded(16);
        let bridge = Arc::new(GpuiBridge::new(user_tx, view_rx));
        let view = cx.new(ModelSelectorView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut ModelSelectorView, cx| {
                view.set_bridge(Arc::clone(&bridge));
                view.state.show_provider_dropdown = true;
                view.state.search_query = "claude".to_string();
                view.state.last_emitted_search_query = "claude".to_string();

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

        assert_eq!(
            user_rx.recv().expect("backspace search event"),
            UserEvent::SearchModels {
                query: "claud".to_string(),
            }
        );
        assert_eq!(
            user_rx.recv().expect("composition search event"),
            UserEvent::SearchModels {
                query: "claude".to_string(),
            }
        );
        assert_eq!(
            user_rx.recv().expect("composition replace search event"),
            UserEvent::SearchModels {
                query: "claude-3".to_string(),
            }
        );
        assert!(
            user_rx.try_recv().is_err(),
            "unexpected additional key-handling events"
        );
    }
}
