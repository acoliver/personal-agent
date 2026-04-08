//! Emoji filter toggle handling for `ChatPresenter`.

use std::sync::Arc;

use super::view_command::ErrorSeverity;
use super::{ChatPresenter, ViewCommand};
use crate::services::AppSettingsService;
use tokio::sync::mpsc;

impl ChatPresenter {
    /// Handle `ToggleEmojiFilter` user event.
    pub(crate) async fn handle_toggle_emoji_filter(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
    ) {
        tracing::info!("handle_toggle_emoji_filter called");

        let current = Self::get_current_emoji_filter(app_settings_service, view_tx).await;
        let new_value = !current;

        tracing::info!("Setting filter_emoji to: {}", new_value);
        if let Err(error) = app_settings_service.set_filter_emoji(new_value).await {
            tracing::warn!("Failed to persist emoji filter setting: {error}");
            let _ = view_tx
                .send(ViewCommand::ShowError {
                    title: "Emoji Filter".to_string(),
                    message: "Failed to persist emoji filter preference".to_string(),
                    severity: ErrorSeverity::Warning,
                })
                .await;
            return;
        }

        tracing::info!(
            "Sending SetEmojiFilterVisibility with enabled={}",
            new_value
        );
        let _ = view_tx
            .send(ViewCommand::SetEmojiFilterVisibility { enabled: new_value })
            .await;
    }

    /// Get current emoji filter setting from storage.
    pub(crate) async fn get_current_emoji_filter(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
    ) -> bool {
        match app_settings_service.get_filter_emoji().await {
            Ok(Some(value)) => {
                tracing::info!("Current filter_emoji value from storage: {}", value);
                value
            }
            Ok(None) => {
                tracing::info!("No filter_emoji value in storage, defaulting to false");
                false
            }
            Err(error) => {
                tracing::warn!("Failed to read emoji filter setting: {error}");
                let _ = view_tx
                    .send(ViewCommand::ShowError {
                        title: "Emoji Filter".to_string(),
                        message: "Failed to read emoji filter preference".to_string(),
                        severity: ErrorSeverity::Warning,
                    })
                    .await;
                false
            }
        }
    }

    /// Emit initial emoji filter state at startup.
    pub(crate) async fn emit_initial_emoji_filter(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &mpsc::Sender<ViewCommand>,
    ) {
        let view_tx = view_tx.clone();
        let enabled = match app_settings_service.get_filter_emoji().await {
            Ok(Some(value)) => {
                tracing::info!("emit_initial_emoji_filter: loaded from storage: {}", value);
                value
            }
            Ok(None) => {
                tracing::info!(
                    "emit_initial_emoji_filter: no value in storage, defaulting to false"
                );
                false
            }
            Err(error) => {
                tracing::warn!("Failed to read emoji filter setting at startup: {error}");
                false
            }
        };
        tracing::info!(
            "emit_initial_emoji_filter: sending SetEmojiFilterVisibility with enabled={}",
            enabled
        );
        let _ = view_tx
            .send(ViewCommand::SetEmojiFilterVisibility { enabled })
            .await;
    }
}
