//! ModelSelectorPresenter - handles model selection UI
//!
//! ModelSelectorPresenter subscribes to model selector events,
//! coordinates with models registry service, and emits view commands.
//!
//! @plan PLAN-20250125-REFACTOR.P10
//! @requirement REQ-025.1

use std::sync::Arc;
use tokio::sync::broadcast;

use crate::events::{AppEvent, types::UserEvent};
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
            UserEvent::SelectModel { provider_id, model_id } => {
                Self::on_select_model(models_registry_service, view_tx, provider_id, model_id).await;
            }
            _ => {} // Ignore other user events
        }
    }

    /// Handle open model selector event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn on_open_selector(
        _models_registry_service: &Arc<dyn ModelsRegistryService>,
        _view_tx: &broadcast::Sender<ViewCommand>,
    ) {
        // Placeholder service call - would load available models
        tracing::info!("Opening model selector");
    }

    /// Handle search models event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn on_search_models(
        _models_registry_service: &Arc<dyn ModelsRegistryService>,
        _view_tx: &broadcast::Sender<ViewCommand>,
        query: String,
    ) {
        // Placeholder service call
        tracing::info!("Searching models for: {}", query);
    }

    /// Handle filter by provider event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn on_filter_by_provider(
        _models_registry_service: &Arc<dyn ModelsRegistryService>,
        _view_tx: &broadcast::Sender<ViewCommand>,
        provider_id: Option<String>,
    ) {
        // Placeholder service call
        tracing::info!("Filtering models by provider: {:?}", provider_id);
    }

    /// Handle select model event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn on_select_model(
        _models_registry_service: &Arc<dyn ModelsRegistryService>,
        _view_tx: &broadcast::Sender<ViewCommand>,
        _provider_id: String,
        _model_id: String,
    ) {
        // Placeholder service call
        tracing::info!("Model selected");
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
