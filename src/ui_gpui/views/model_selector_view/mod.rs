//! Model Selector View implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P07
//! @requirement REQ-UI-MS

mod command;
mod ime;
mod render;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_scale;

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
    ///
    /// Uses a single pass through models to bucket by provider (O(M + P)),
    /// avoiding the O(P × M) cost of iterating all models per provider.
    pub fn rebuild_display_rows(&mut self) {
        let query_lower = self.search_query.to_lowercase();

        // Single pass: bucket matching model indices by provider.
        // Key: provider index in cached_providers. Value: sorted model indices.
        let provider_count = self.cached_providers.len();
        let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); provider_count];

        // Build a provider-name → bucket-index map for O(1) lookup
        let provider_idx: std::collections::HashMap<&str, usize> = self
            .cached_providers
            .iter()
            .enumerate()
            .map(|(i, p)| (p.as_str(), i))
            .collect();

        for (model_ix, s) in self.searchable_models.iter().enumerate() {
            // Provider filter
            if let Some(ref selected) = self.selected_provider {
                if &s.info.provider_id != selected {
                    continue;
                }
            }
            // Search filter
            if !query_lower.is_empty()
                && !s.id_lower.contains(&query_lower)
                && !s.provider_lower.contains(&query_lower)
            {
                continue;
            }
            // Capability filters
            if self.filter_reasoning && !s.info.reasoning {
                continue;
            }
            if self.filter_vision && !s.info.vision {
                continue;
            }

            if let Some(&bucket_ix) = provider_idx.get(s.info.provider_id.as_str()) {
                buckets[bucket_ix].push(model_ix);
            }
        }

        // Pre-allocate: worst case is all models + all providers
        let total_models: usize = buckets.iter().map(Vec::len).sum();
        let non_empty = buckets.iter().filter(|b| !b.is_empty()).count();
        self.cached_display_rows.clear();
        self.cached_display_rows.reserve(total_models + non_empty);

        let mut model_count = 0;
        let mut header_count = 0;

        for (prov_ix, mut bucket) in buckets.into_iter().enumerate() {
            if bucket.is_empty() {
                continue;
            }
            // Sort models within provider by model ID (alphabetical)
            bucket.sort_by(|&a, &b| {
                self.searchable_models[a]
                    .info
                    .id
                    .cmp(&self.searchable_models[b].info.id)
            });

            self.cached_display_rows.push(DisplayRow::ProviderHeader(
                self.cached_providers[prov_ix].clone(),
            ));
            header_count += 1;

            for idx in bucket {
                self.cached_display_rows.push(DisplayRow::Model(idx));
                model_count += 1;
            }
        }

        self.cached_model_count = model_count;
        self.cached_provider_count = header_count;
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
