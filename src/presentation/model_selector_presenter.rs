//! ModelSelectorPresenter - handles model selection UI
//!
//! ModelSelectorPresenter subscribes to model selector events,
//! coordinates with models registry service, and emits view commands.
//!
//! @plan PLAN-20250125-REFACTOR.P10
//! @requirement REQ-025.1
//! @plan PLAN-20260219-NEXTGPUIREMEDIATE.P03
//! @requirement REQ-WIRE-006

use std::sync::Arc;
use tokio::sync::broadcast;

use crate::events::{types::UserEvent, AppEvent, EventBus};
use crate::registry::ModelInfo as RegistryModelInfo;
use crate::services::ModelsRegistryService;
use super::{Presenter, PresenterError, ViewCommand};

/// ModelSelectorPresenter - handles model selection UI
///
/// @plan PLAN-20250125-REFACTOR.P10
/// @requirement REQ-025.1
pub struct ModelSelectorPresenter {
    /// Event receiver from EventBus
    rx: broadcast::Receiver<AppEvent>,

    /// Reference to models registry service
    models_registry_service: Arc<dyn ModelsRegistryService>,

    /// View command sender
    view_tx: broadcast::Sender<ViewCommand>,

    /// Running flag for event loop
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl ModelSelectorPresenter {
    /// Create a new ModelSelectorPresenter
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    pub fn new(
        models_registry_service: Arc<dyn ModelsRegistryService>,
        event_bus: &broadcast::Sender<AppEvent>,
        view_tx: broadcast::Sender<ViewCommand>,
    ) -> Self {
        let rx = event_bus.subscribe();
        Self {
            rx,
            models_registry_service,
            view_tx,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Stub constructor using unified global EventBus (REQ-WIRE-006 unification path).
    ///
    /// This constructor accepts Arc<EventBus> directly, subscribing to the global event
    /// bus rather than a caller-supplied broadcast::Sender. Full wiring deferred to
    /// later implementation phases.
    ///
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P03
    /// @requirement REQ-WIRE-006
    /// @pseudocode component-001-event-pipeline.md lines 090-136
    #[allow(dead_code)]
    pub fn new_with_event_bus(
        models_registry_service: Arc<dyn ModelsRegistryService>,
        event_bus: Arc<EventBus>,
        view_tx: broadcast::Sender<ViewCommand>,
    ) -> Self {
        let rx = event_bus.sender().subscribe();
        Self {
            rx,
            models_registry_service,
            view_tx,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Start the presenter event loop
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    pub async fn start(&mut self) -> Result<(), PresenterError> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        self.running.store(true, std::sync::atomic::Ordering::Relaxed);

        let mut rx = self.rx.resubscribe();
        let running = self.running.clone();
        let models_registry_service = self.models_registry_service.clone();
        let view_tx = self.view_tx.clone();

        tokio::spawn(async move {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(event) => {
                        Self::handle_event(&models_registry_service, &view_tx, event).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("ModelSelectorPresenter lagged: {} events missed", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("ModelSelectorPresenter event stream closed");
                        break;
                    }
                }
            }
            tracing::info!("ModelSelectorPresenter event loop ended");
        });

        Ok(())
    }

    /// Stop the presenter event loop
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    pub async fn stop(&mut self) -> Result<(), PresenterError> {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Check if presenter is running
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Handle incoming events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn handle_event(
        models_registry_service: &Arc<dyn ModelsRegistryService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: AppEvent,
    ) {
        match event {
            AppEvent::User(user_evt) => {
                Self::handle_user_event(models_registry_service, view_tx, user_evt).await;
            }
            _ => {} // Ignore other events
        }
    }

    /// Handle user events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn handle_user_event(
        models_registry_service: &Arc<dyn ModelsRegistryService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: UserEvent,
    ) {
        match event {
            UserEvent::OpenModelSelector => {
                Self::on_open_selector(models_registry_service, view_tx).await;
            }
            UserEvent::SearchModels { query } => {
                Self::on_search_models(models_registry_service, view_tx, query).await;
            }
            UserEvent::FilterModelsByProvider { provider_id } => {
                Self::on_filter_by_provider(models_registry_service, view_tx, provider_id).await;
            }
            UserEvent::SelectModel {
                provider_id,
                model_id,
            } => {
                Self::on_select_model(models_registry_service, view_tx, provider_id, model_id).await;
            }
            _ => {} // Ignore other user events
        }
    }

    fn map_models_to_view(models: Vec<RegistryModelInfo>) -> Vec<super::view_command::ModelInfo> {
        models
            .into_iter()
            .map(|m| super::view_command::ModelInfo {
                model_id: m.id.clone(),
                name: m.name.clone(),
                provider_id: m.provider.as_deref().unwrap_or("unknown").to_string(),
                context_length: m.limit.as_ref().map(|l| l.context as u32),
            })
            .collect()
    }

    /// Handle open model selector event - load models from registry
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn on_open_selector(
        models_registry_service: &Arc<dyn ModelsRegistryService>,
        view_tx: &broadcast::Sender<ViewCommand>,
    ) {
        tracing::info!("Opening model selector - loading models from registry");

        // First try to get cached models, then refresh if needed
        let models = match models_registry_service.list_all().await {
            Ok(models) if !models.is_empty() => {
                tracing::info!("Loaded {} models from cache", models.len());
                models
            }
            _ => {
                // Try to refresh
                tracing::info!("Cache empty or failed, refreshing from models.dev...");
                if let Err(e) = models_registry_service.refresh().await {
                    tracing::warn!("Failed to refresh models registry: {:?}", e);
                }
                // Try again after refresh
                match models_registry_service.list_all().await {
                    Ok(models) => {
                        tracing::info!("Loaded {} models after refresh", models.len());
                        models
                    }
                    Err(e) => {
                        tracing::error!("Failed to load models: {:?}", e);
                        // Send error to view
                        let _ = view_tx.send(ViewCommand::ShowError {
                            title: "Failed to load models".to_string(),
                            message: format!("Could not load models from registry: {:?}", e),
                            severity: super::view_command::ErrorSeverity::Warning,
                        });
                        return;
                    }
                }
            }
        };

        let model_infos = Self::map_models_to_view(models);

        tracing::info!("Sending {} models to view", model_infos.len());
        let _ = view_tx.send(ViewCommand::ModelSearchResults { models: model_infos });
    }

    /// Search models by query and emit ModelSearchResults.
    async fn on_search_models(
        models_registry_service: &Arc<dyn ModelsRegistryService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        query: String,
    ) {
        tracing::info!("Searching models for: {}", query);

        match models_registry_service.search(&query).await {
            Ok(models) => {
                let mapped = Self::map_models_to_view(models);
                let _ = view_tx.send(ViewCommand::ModelSearchResults { models: mapped });
            }
            Err(e) => {
                tracing::error!("Model search failed: {:?}", e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Model Search Failed".to_string(),
                    message: e.to_string(),
                    severity: super::view_command::ErrorSeverity::Warning,
                });
            }
        }
    }

    /// Filter models by provider and emit ModelSearchResults.
    async fn on_filter_by_provider(
        models_registry_service: &Arc<dyn ModelsRegistryService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        provider_id: Option<String>,
    ) {
        tracing::info!("Filtering models by provider: {:?}", provider_id);

        let result = match provider_id {
            Some(provider) => models_registry_service.get_provider(&provider).await,
            None => models_registry_service.list_all().await,
        };

        match result {
            Ok(models) => {
                let mapped = Self::map_models_to_view(models);
                let _ = view_tx.send(ViewCommand::ModelSearchResults { models: mapped });
            }
            Err(e) => {
                tracing::error!("Model provider filter failed: {:?}", e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Model Filter Failed".to_string(),
                    message: e.to_string(),
                    severity: super::view_command::ErrorSeverity::Warning,
                });
            }
        }
    }

    /// Handle select model event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
    /// @requirement REQ-WIRE-001
    async fn on_select_model(
        models_registry_service: &Arc<dyn ModelsRegistryService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        provider_id: String,
        model_id: String,
    ) {
        tracing::info!("Model selected: provider={} model={}", provider_id, model_id);

        let context_length = models_registry_service
            .list_all()
            .await
            .ok()
            .and_then(|models| {
                models
                    .into_iter()
                    .find(|m| m.id == model_id)
                    .and_then(|m| m.limit.map(|l| l.context as u32))
            });

        let _ = view_tx.send(ViewCommand::ModelSelected {
            provider_id,
            model_id,
            context_length,
        });
        let _ = view_tx.send(ViewCommand::NavigateTo {
            view: super::view_command::ViewId::ProfileEditor,
        });
    }
}

// Implement Presenter trait
//
// @plan PLAN-20250125-REFACTOR.P10
impl Presenter for ModelSelectorPresenter {
    fn start(&mut self) -> Result<(), PresenterError> {
        // Note: This is a sync wrapper - in real usage, call async start() directly
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PresenterError> {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }
}
