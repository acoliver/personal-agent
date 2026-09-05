//! Local-model panel handlers for `SettingsPresenter`.
//!
//! @plan:PLAN-20260903-LOCALMODEL.P05

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tokio::sync::broadcast;

use super::settings_presenter::SettingsPresenter;
use super::view_command::ViewCommand;
use crate::events::types::UserEvent;
use crate::services::AppSettingsService;

impl SettingsPresenter {
    /// Handle Local Model panel events (REQ-LM-006): settings load + status
    /// poll on entry, save, and unload.
    pub(super) async fn handle_local_model_user_event(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: &UserEvent,
        poll_generation: &Arc<std::sync::atomic::AtomicU64>,
    ) -> bool {
        match event {
            UserEvent::LoadLocalModelSettings => {
                Self::on_load_local_model_settings(app_settings_service, view_tx, poll_generation)
                    .await;
                true
            }
            UserEvent::SaveLocalModelSettings { settings } => {
                Self::on_save_local_model_settings(app_settings_service, view_tx, settings.clone())
                    .await;
                true
            }
            UserEvent::UnloadLocalModel => {
                crate::llm::local::unload_local();
                let _ = view_tx.send(ViewCommand::LocalModelStatusUpdated {
                    status: crate::llm::local::status(),
                });
                true
            }
            _ => false,
        }
    }

    /// Load the persisted local-model settings, push them to the panel, and
    /// (re)arm the engine-status poll. Each panel entry bumps the poll
    /// generation, terminating the previous poll task, so at most one poll
    /// loop runs at a time.
    async fn on_load_local_model_settings(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        poll_generation: &Arc<std::sync::atomic::AtomicU64>,
    ) {
        let settings = crate::services::local_model_settings::LocalModelSettings::load(
            app_settings_service.as_ref(),
        )
        .await
        .unwrap_or_else(|error| {
            tracing::warn!("Failed to load local model settings: {error}; using defaults");
            crate::services::local_model_settings::LocalModelSettings::default()
        });
        let _ = view_tx.send(ViewCommand::LocalModelSettingsLoaded { settings });

        let generation = poll_generation.fetch_add(1, Ordering::Relaxed) + 1;
        let poll_generation = Arc::clone(poll_generation);
        let view_tx = view_tx.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_millis(500));
            loop {
                // The first tick fires immediately, which covers the initial
                // status push for the freshly-opened panel.
                ticker.tick().await;
                if poll_generation.load(Ordering::Relaxed) != generation {
                    break;
                }
                let _ = view_tx.send(ViewCommand::LocalModelStatusUpdated {
                    status: crate::llm::local::status(),
                });
            }
        });
    }

    /// Persist edited local-model settings and echo the snapshot back so the
    /// panel re-syncs; failures surface through `ShowError`.
    async fn on_save_local_model_settings(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        settings: crate::services::local_model_settings::LocalModelSettings,
    ) {
        match settings.save(app_settings_service.as_ref()).await {
            Ok(()) => {
                let _ = view_tx.send(ViewCommand::LocalModelSettingsLoaded { settings });
                let _ = view_tx.send(ViewCommand::ShowNotification {
                    message: "Local model settings saved".to_string(),
                });
            }
            Err(error) => {
                tracing::warn!("Failed to save local model settings: {error}");
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Local Model".to_string(),
                    message: format!("Failed to save settings: {error}"),
                    severity: super::view_command::ErrorSeverity::Warning,
                });
            }
        }
    }
}

#[cfg(test)]
#[path = "settings_presenter_local_model_tests.rs"]
mod tests;
