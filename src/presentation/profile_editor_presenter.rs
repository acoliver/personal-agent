//! ProfileEditorPresenter - handles profile creation/editing UI
//!
//! ProfileEditorPresenter subscribes to profile editor events,
//! coordinates with profile service, and emits view commands.
//!
//! @plan PLAN-20250125-REFACTOR.P10
//! @requirement REQ-025.1

use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use uuid::Uuid;

use super::{Presenter, PresenterError, ViewCommand};
use crate::events::{
    types::{ModelProfileAuth, ProfileEvent, UserEvent},
    AppEvent, EventBus,
};
use crate::models::{AuthConfig, ModelParameters};
use crate::services::{ProfileService, ServiceError};

/// ProfileEditorPresenter - handles profile creation/editing UI
///
/// @plan PLAN-20250125-REFACTOR.P10
/// @requirement REQ-025.1
pub struct ProfileEditorPresenter {
    /// Event receiver from EventBus
    rx: broadcast::Receiver<AppEvent>,

    /// Reference to profile service
    profile_service: Arc<dyn ProfileService>,

    /// View command sender
    view_tx: broadcast::Sender<ViewCommand>,

    /// Last model selected via ModelSelector (provider_id, model_id)
    /// used for lightweight SaveProfileEditor flow.
    pending_selected_model: Arc<Mutex<Option<(String, String)>>>,

    /// Event bus sender for emitting domain events consumed by other presenters.
    event_bus_tx: broadcast::Sender<AppEvent>,

    /// Running flag for event loop
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl ProfileEditorPresenter {
    /// Create a new ProfileEditorPresenter
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    pub fn new(
        profile_service: Arc<dyn ProfileService>,
        event_bus: &broadcast::Sender<AppEvent>,
        view_tx: broadcast::Sender<ViewCommand>,
    ) -> Self {
        let rx = event_bus.subscribe();
        Self {
            rx,
            profile_service,
            view_tx,
            pending_selected_model: Arc::new(Mutex::new(None)),
            event_bus_tx: event_bus.clone(),
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
        profile_service: Arc<dyn ProfileService>,
        event_bus: Arc<EventBus>,
        view_tx: broadcast::Sender<ViewCommand>,
    ) -> Self {
        let event_bus_tx = event_bus.sender().clone();
        let rx = event_bus_tx.subscribe();
        Self {
            rx,
            profile_service,
            view_tx,
            pending_selected_model: Arc::new(Mutex::new(None)),
            event_bus_tx,
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

        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let mut rx = self.rx.resubscribe();
        let running = self.running.clone();
        let profile_service = self.profile_service.clone();
        let view_tx = self.view_tx.clone();
        let pending_selected_model = Arc::clone(&self.pending_selected_model);
        let event_bus_tx = self.event_bus_tx.clone();

        tokio::spawn(async move {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(event) => {
                        Self::handle_event(
                            &profile_service,
                            &view_tx,
                            &pending_selected_model,
                            &event_bus_tx,
                            event,
                        )
                        .await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("ProfileEditorPresenter lagged: {} events missed", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("ProfileEditorPresenter event stream closed");
                        break;
                    }
                }
            }
            tracing::info!("ProfileEditorPresenter event loop ended");
        });

        Ok(())
    }

    /// Stop the presenter event loop
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    pub async fn stop(&mut self) -> Result<(), PresenterError> {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
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
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        pending_selected_model: &Arc<Mutex<Option<(String, String)>>>,
        event_bus_tx: &broadcast::Sender<AppEvent>,
        event: AppEvent,
    ) {
        match event {
            AppEvent::User(user_evt) => {
                Self::handle_user_event(
                    profile_service,
                    view_tx,
                    pending_selected_model,
                    event_bus_tx,
                    user_evt,
                )
                .await;
            }
            AppEvent::Profile(profile_evt) => {
                Self::handle_profile_event(view_tx, profile_evt).await;
            }
            _ => {} // Ignore other events
        }
    }

    /// Handle user events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
    /// @requirement REQ-WIRE-001
    async fn handle_user_event(
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        pending_selected_model: &Arc<Mutex<Option<(String, String)>>>,
        event_bus_tx: &broadcast::Sender<AppEvent>,
        event: UserEvent,
    ) {
        match event {
            UserEvent::SaveProfile { profile } => {
                Self::on_save_profile(profile_service, event_bus_tx, view_tx, profile).await;
            }
            UserEvent::SaveProfileEditor => {
                Self::on_save_profile_editor(
                    profile_service,
                    event_bus_tx,
                    view_tx,
                    pending_selected_model,
                )
                .await;
            }
            UserEvent::SelectModel {
                provider_id,
                model_id,
            } => {
                Self::on_select_model(pending_selected_model, provider_id, model_id);
            }
            UserEvent::TestProfileConnection { id } => {
                Self::on_test_connection(profile_service, view_tx, id).await;
            }
            _ => {} // Ignore other user events
        }
    }

    /// Handle save profile event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn on_save_profile(
        profile_service: &Arc<dyn ProfileService>,
        event_bus_tx: &broadcast::Sender<AppEvent>,
        view_tx: &broadcast::Sender<ViewCommand>,
        profile: crate::events::types::ModelProfile,
    ) {
        tracing::info!("Saving profile: {}", profile.name);

        let auth = match profile.auth.clone().unwrap_or(ModelProfileAuth::ApiKey {
            value: String::new(),
        }) {
            ModelProfileAuth::None => AuthConfig::Key {
                value: String::new(),
            },
            ModelProfileAuth::ApiKey { value } => AuthConfig::Key { value },
            ModelProfileAuth::Keyfile { path } => AuthConfig::Keyfile { path },
        };

        let mut parameters = ModelParameters::default();
        if let Some(payload_parameters) = profile.parameters.clone() {
            if let Some(temperature) = payload_parameters.temperature {
                parameters.temperature = temperature;
            }
            if let Some(max_tokens) = payload_parameters.max_tokens {
                parameters.max_tokens = max_tokens;
            }
            if let Some(show_thinking) = payload_parameters.show_thinking {
                parameters.show_thinking = show_thinking;
            }
            if let Some(enable_thinking) = payload_parameters.enable_thinking {
                parameters.enable_thinking = enable_thinking;
            }
            if let Some(thinking_budget) = payload_parameters.thinking_budget {
                parameters.thinking_budget = Some(thinking_budget);
            }
        }

        let update_name = Some(profile.name.clone());
        let update_provider = profile.provider_id.clone();
        let update_model = profile.model_id.clone();
        let update_base_url = profile.base_url.clone();
        let update_system_prompt = profile.system_prompt.clone();

        let updated = profile_service
            .update(
                profile.id,
                update_name,
                update_provider,
                update_model.clone(),
                update_base_url.clone(),
                Some(auth.clone()),
                Some(parameters.clone()),
                update_system_prompt.clone(),
            )
            .await;

        let fallback_provider = profile.provider_id.unwrap_or_else(|| "openai".to_string());
        let fallback_model = profile
            .model_id
            .clone()
            .unwrap_or_else(|| "gpt-4o".to_string());

        let persisted = match updated {
            Ok(saved) => Ok(saved),
            Err(ServiceError::NotFound(_)) => {
                profile_service
                    .create(
                        profile.name.clone(),
                        fallback_provider,
                        fallback_model,
                        update_base_url,
                        auth,
                        parameters,
                        update_system_prompt,
                    )
                    .await
            }
            Err(e) => Err(e),
        };

        match persisted {
            Ok(saved) => {
                let _ = event_bus_tx.send(AppEvent::Profile(ProfileEvent::Updated {
                    id: saved.id,
                    name: saved.name.clone(),
                }));
                let _ = view_tx.send(ViewCommand::ProfileUpdated {
                    id: saved.id,
                    name: saved.name,
                });
                let _ = view_tx.send(ViewCommand::NavigateBack);
            }
            Err(e) => {
                tracing::error!("Failed to persist SaveProfile payload: {}", e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Save Failed".to_string(),
                    message: e.to_string(),
                    severity: super::view_command::ErrorSeverity::Error,
                });
            }
        }
    }

    /// Handle SaveProfileEditor event (lightweight save without full profile payload)
    ///
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
    /// @requirement REQ-WIRE-001
    /// @pseudocode component-003-profile-flow.md lines 140-173
    async fn on_save_profile_editor(
        profile_service: &Arc<dyn ProfileService>,
        event_bus_tx: &broadcast::Sender<AppEvent>,
        view_tx: &broadcast::Sender<ViewCommand>,
        pending_selected_model: &Arc<Mutex<Option<(String, String)>>>,
    ) {
        tracing::info!("ProfileEditorPresenter: handling SaveProfileEditor");

        // Lightweight persistence path until full editable-field event payload lands.
        // We persist a minimal profile using the last selected model context.
        let (provider_id, model_id) = {
            let guard = pending_selected_model
                .lock()
                .expect("pending_selected_model lock poisoned");
            guard
                .clone()
                .unwrap_or_else(|| ("openai".to_string(), "gpt-4o".to_string()))
        };

        let mut parameters = ModelParameters::default();
        parameters.show_thinking = true;

        let auth = AuthConfig::Key {
            value: std::env::var("OPENAI_API_KEY")
                .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
                .unwrap_or_default()
                .trim()
                .to_string(),
        };

        let created = profile_service
            .create(
                model_id.clone(),
                provider_id,
                model_id,
                None,
                auth,
                parameters,
                None,
            )
            .await;

        match created {
            Ok(profile) => {
                let _ = event_bus_tx.send(AppEvent::Profile(ProfileEvent::Created {
                    id: profile.id,
                    name: profile.name.clone(),
                }));
                let _ = event_bus_tx.send(AppEvent::Profile(ProfileEvent::DefaultChanged {
                    profile_id: Some(profile.id),
                }));
                let _ = view_tx.send(ViewCommand::ProfileCreated {
                    id: profile.id,
                    name: profile.name,
                });
                let _ = view_tx.send(ViewCommand::NavigateBack);
                let _ = view_tx.send(ViewCommand::DefaultProfileChanged {
                    profile_id: Some(profile.id),
                });
            }
            Err(e) => {
                tracing::error!("Failed to persist profile from SaveProfileEditor: {}", e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Save Failed".to_string(),
                    message: e.to_string(),
                    severity: super::view_command::ErrorSeverity::Error,
                });
            }
        }
    }

    /// Track latest model selection from ModelSelector flow.
    fn on_select_model(
        pending_selected_model: &Arc<Mutex<Option<(String, String)>>>,
        provider_id: String,
        model_id: String,
    ) {
        let mut guard = pending_selected_model
            .lock()
            .expect("pending_selected_model lock poisoned");
        *guard = Some((provider_id, model_id));
    }

    /// Handle test connection event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn on_test_connection(
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        profile_id: Uuid,
    ) {
        tracing::info!("Testing connection for profile: {}", profile_id);
        let _ = view_tx.send(ViewCommand::ProfileTestStarted { id: profile_id });
        match profile_service.test_connection(profile_id).await {
            Ok(()) => {
                let _ = view_tx.send(ViewCommand::ProfileTestCompleted {
                    id: profile_id,
                    success: true,
                    response_time_ms: None,
                    error: None,
                });
            }
            Err(e) => {
                let _ = view_tx.send(ViewCommand::ProfileTestCompleted {
                    id: profile_id,
                    success: false,
                    response_time_ms: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    /// Handle profile domain events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn handle_profile_event(view_tx: &broadcast::Sender<ViewCommand>, event: ProfileEvent) {
        match event {
            ProfileEvent::TestStarted { id: _id } => {
                tracing::info!("Profile connection test started");
            }
            ProfileEvent::TestCompleted {
                id: _id,
                success,
                response_time_ms,
                error,
            } => {
                if success {
                    tracing::info!(
                        "Profile connection test successful ({}ms)",
                        response_time_ms.unwrap_or(0)
                    );
                } else {
                    let _ = view_tx.send(ViewCommand::ShowError {
                        title: "Connection Failed".to_string(),
                        message: error.unwrap_or_else(|| "Unknown error".to_string()),
                        severity: super::view_command::ErrorSeverity::Error,
                    });
                }
            }
            _ => {} // Ignore other profile events
        }
    }
}

// Implement Presenter trait
//
// @plan PLAN-20250125-REFACTOR.P10
impl Presenter for ProfileEditorPresenter {
    fn start(&mut self) -> Result<(), PresenterError> {
        // Note: This is a sync wrapper - in real usage, call async start() directly
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PresenterError> {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }
}
