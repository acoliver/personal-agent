//! ProfileEditorPresenter - handles profile creation/editing UI
//!
//! ProfileEditorPresenter subscribes to profile editor events,
//! coordinates with profile service, and emits view commands.
//!
//! @plan PLAN-20250125-REFACTOR.P10
//! @requirement REQ-025.1

use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::events::{AppEvent, types::{ProfileEvent, UserEvent}};
use crate::services::ProfileService;
use super::{Presenter, PresenterError, ViewCommand};

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
        let profile_service = self.profile_service.clone();
        let view_tx = self.view_tx.clone();

        tokio::spawn(async move {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(event) => {
                        Self::handle_event(&profile_service, &view_tx, event).await;
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
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: AppEvent,
    ) {
        match event {
            AppEvent::User(user_evt) => {
                Self::handle_user_event(profile_service, view_tx, user_evt).await;
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
    async fn handle_user_event(
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: UserEvent,
    ) {
        match event {
            UserEvent::SaveProfile { profile } => {
                Self::on_save_profile(profile_service, view_tx, profile).await;
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
        _profile_service: &Arc<dyn ProfileService>,
        _view_tx: &broadcast::Sender<ViewCommand>,
        profile: crate::events::types::ModelProfile,
    ) {
        // Placeholder - profile creation/update requires individual fields
        tracing::info!("Saving profile: {}", profile.name);
    }

    /// Handle test connection event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn on_test_connection(
        _profile_service: &Arc<dyn ProfileService>,
        _view_tx: &broadcast::Sender<ViewCommand>,
        profile_id: Uuid,
    ) {
        // Placeholder service call - test_connection not fully implemented
        tracing::info!("Testing connection for profile: {}", profile_id);
    }

    /// Handle profile domain events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn handle_profile_event(
        _view_tx: &broadcast::Sender<ViewCommand>,
        event: ProfileEvent,
    ) {
        match event {
            ProfileEvent::TestStarted { id: _id } => {
                tracing::info!("Profile connection test started");
            }
            ProfileEvent::TestCompleted { id: _id, success, response_time_ms, error } => {
                if success {
                    tracing::info!("Profile connection test successful ({}ms)", response_time_ms.unwrap_or(0));
                } else {
                    let _ = _view_tx.send(ViewCommand::ShowError {
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
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }
}
