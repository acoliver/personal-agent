//! Command handling for `ModelSelectorView`.

use super::{ModelInfo, ModelSelectorView, ProviderInfo};
use crate::events::types::UserEvent;
use crate::presentation::view_command::ViewCommand;

impl ModelSelectorView {
    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        if let ViewCommand::ModelSearchResults { models } = command {
            println!(
                ">>> ModelSelectorView::handle_command received {} models <<<",
                models.len()
            );
            tracing::info!("ModelSelectorView received {} models", models.len());

            // Extract unique providers from the models
            let mut provider_set = std::collections::HashSet::new();
            for m in &models {
                provider_set.insert(m.provider_id.clone());
            }
            let providers: Vec<ProviderInfo> = provider_set
                .into_iter()
                .map(|id| ProviderInfo {
                    id: id.clone(),
                    name: id,
                })
                .collect();

            println!(">>> Providers extracted: {} <<<", providers.len());

            let local_models: Vec<ModelInfo> = models
                .into_iter()
                .map(|m| ModelInfo {
                    id: m.model_id,
                    provider_id: m.provider_id,
                    context: u64::from(m.context_length.unwrap_or(128_000)),
                    reasoning: false,
                    vision: false,
                    cost_input: 0.0,
                    cost_output: 0.0,
                })
                .collect();

            println!(">>> Setting {} models on view <<<", local_models.len());
            self.set_models(providers, local_models);
            println!(
                ">>> Models set, state.models.len() = {} <<<",
                self.state.models.len()
            );
        }
        cx.notify();
    }

    /// Request models from presenter on view open
    pub fn request_models(&self) {
        tracing::info!("ModelSelectorView requesting models");
        self.emit(&UserEvent::OpenModelSelector);
    }

    /// Emit search/filter events when local filter state changes.
    pub(super) fn emit_filter_events_if_changed(&mut self) {
        if self.state.search_query != self.state.last_emitted_search_query {
            self.emit(&UserEvent::SearchModels {
                query: self.state.search_query.clone(),
            });
            self.state.last_emitted_search_query = self.state.search_query.clone();
        }

        if self.state.selected_provider != self.state.last_emitted_provider {
            self.emit(&UserEvent::FilterModelsByProvider {
                provider_id: self.state.selected_provider.clone(),
            });
            self.state.last_emitted_provider = self.state.selected_provider.clone();
        }
    }

    pub(super) fn toggle_provider_dropdown(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.show_provider_dropdown = !self.state.show_provider_dropdown;
        cx.notify();
    }

    pub(super) fn toggle_reasoning_filter(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.filter_reasoning = !self.state.filter_reasoning;
        cx.notify();
    }

    pub(super) fn toggle_vision_filter(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.filter_vision = !self.state.filter_vision;
        cx.notify();
    }

    pub(super) fn clear_provider_filter(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.selected_provider = None;
        self.state.show_provider_dropdown = false;
        self.emit_filter_events_if_changed();
        cx.notify();
    }

    pub(super) fn select_provider_filter(
        &mut self,
        provider_id: String,
        cx: &mut gpui::Context<Self>,
    ) {
        self.state.selected_provider = Some(provider_id);
        self.state.show_provider_dropdown = false;
        self.emit_filter_events_if_changed();
        cx.notify();
    }

    pub(super) fn select_model(&mut self, provider_id: String, model_id: String) {
        println!(">>> Model selected: {model_id} from {provider_id} <<<");
        self.emit(&UserEvent::SelectModel {
            provider_id,
            model_id,
        });
        self.state.show_provider_dropdown = false;
    }

    pub(super) fn handle_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        cx: &mut gpui::Context<Self>,
    ) {
        let key = &event.keystroke.key;
        let modifiers = &event.keystroke.modifiers;

        if key == "escape" {
            if self.state.show_provider_dropdown {
                self.state.show_provider_dropdown = false;
                cx.notify();
            } else {
                crate::ui_gpui::navigation_channel()
                    .request_navigate(crate::presentation::view_command::ViewId::Settings);
            }
            return;
        }

        if modifiers.platform && key == "w" {
            crate::ui_gpui::navigation_channel()
                .request_navigate(crate::presentation::view_command::ViewId::Settings);
            return;
        }

        if !modifiers.platform && !modifiers.control && key == "backspace" {
            self.state.search_query.pop();
            self.emit_filter_events_if_changed();
            cx.notify();
        }
    }
}
