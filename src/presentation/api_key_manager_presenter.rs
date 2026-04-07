//! `ApiKeyManagerPresenter` — handles API key CRUD via the OS keychain.
//!
//! Listens for `StoreApiKey`, `DeleteApiKey`, and `RefreshApiKeys` user events
//! and emits `ApiKeysListed`, `ApiKeyStored`, `ApiKeyDeleted` view commands.

use std::sync::Arc;
use tokio::sync::broadcast;

use super::view_command::{ApiKeyInfo, ErrorSeverity};
use super::{PresenterError, ViewCommand};
use crate::events::{types::UserEvent, AppEvent, EventBus};
use crate::models::profile::AuthConfig;
use crate::services::{secure_store, ProfileService};

pub struct ApiKeyManagerPresenter {
    rx: broadcast::Receiver<AppEvent>,
    profile_service: Arc<dyn ProfileService>,
    view_tx: broadcast::Sender<ViewCommand>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl ApiKeyManagerPresenter {
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

    #[allow(dead_code)]
    pub fn new_with_event_bus(
        profile_service: Arc<dyn ProfileService>,
        event_bus: &Arc<EventBus>,
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

    /// # Errors
    ///
    /// Returns `PresenterError` if presenter startup becomes fallible in the future.
    pub async fn start(&mut self) -> Result<(), PresenterError> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }
        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        Self::emit_keys_list(&self.profile_service, &self.view_tx).await;

        let mut rx = self.rx.resubscribe();
        let running = self.running.clone();
        let view_tx = self.view_tx.clone();
        let profile_service = self.profile_service.clone();

        tokio::spawn(async move {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(event) => {
                        Self::handle_event(&profile_service, &view_tx, event).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("ApiKeyManagerPresenter lagged: {n} events missed");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("ApiKeyManagerPresenter event stream closed");
                        break;
                    }
                }
            }
            tracing::info!("ApiKeyManagerPresenter event loop ended");
        });

        Ok(())
    }

    /// # Errors
    ///
    /// Returns `PresenterError` if presenter shutdown becomes fallible in the future.
    pub async fn stop(&mut self) -> Result<(), PresenterError> {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    async fn handle_event(
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: AppEvent,
    ) {
        match event {
            AppEvent::User(UserEvent::StoreApiKey { label, value }) => {
                Self::handle_store_key(profile_service, view_tx, label, value).await;
            }
            AppEvent::User(UserEvent::DeleteApiKey { label }) => {
                Self::handle_delete_key(profile_service, view_tx, label).await;
            }
            AppEvent::User(UserEvent::RefreshApiKeys) => {
                Self::emit_keys_list(profile_service, view_tx).await;
            }
            _ => {}
        }
    }

    async fn handle_store_key(
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        label: String,
        value: String,
    ) {
        match secure_store::api_keys::store(&label, &value) {
            Ok(()) => {
                tracing::info!(label = %label, "API key stored in keychain");
                let _ = view_tx.send(ViewCommand::ApiKeyStored {
                    label: label.clone(),
                });
                Self::emit_keys_list(profile_service, view_tx).await;
            }
            Err(e) => {
                tracing::error!(label = %label, error = %e, "Failed to store API key");
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Keychain Error".to_string(),
                    message: format!("Failed to store API key \"{label}\": {e}"),
                    severity: ErrorSeverity::Error,
                });
            }
        }
    }

    async fn handle_delete_key(
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        label: String,
    ) {
        match secure_store::api_keys::delete(&label) {
            Ok(()) => {
                tracing::info!(label = %label, "API key deleted from keychain");
                let _ = view_tx.send(ViewCommand::ApiKeyDeleted {
                    label: label.clone(),
                });
                Self::emit_keys_list(profile_service, view_tx).await;
            }
            Err(e) => {
                tracing::error!(label = %label, error = %e, "Failed to delete API key");
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Keychain Error".to_string(),
                    message: format!("Failed to delete API key \"{label}\": {e}"),
                    severity: ErrorSeverity::Error,
                });
            }
        }
    }

    /// Build the full key info list (with masked values and "used by" cross-refs)
    /// and send it to the view.
    async fn emit_keys_list(
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &broadcast::Sender<ViewCommand>,
    ) {
        let mut labels = secure_store::api_keys::list();
        tracing::info!(label_count = labels.len(), labels = ?labels, "ApiKeyManagerPresenter: loaded key labels from secure store");

        // Build profile cross-reference map: label → [profile names]
        let profiles = profile_service.list().await.unwrap_or_default();
        tracing::info!(
            profile_count = profiles.len(),
            "ApiKeyManagerPresenter: loaded profiles for key usage cross-reference"
        );
        let mut used_by_map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for p in &profiles {
            if let AuthConfig::Keychain { ref label } = p.auth {
                if !label.is_empty() {
                    used_by_map
                        .entry(label.clone())
                        .or_default()
                        .push(p.name.clone());
                }
            }
            // InProcess profiles don't have API keys, skip them
        }

        for label in used_by_map.keys() {
            if !labels.iter().any(|existing| existing == label) {
                labels.push(label.clone());
            }
        }
        labels.sort();
        labels.dedup();

        let keys: Vec<ApiKeyInfo> = labels
            .into_iter()
            .map(|label| {
                let used_by = used_by_map.remove(&label).unwrap_or_default();
                ApiKeyInfo {
                    label,
                    masked_value: "••••••••".to_string(),
                    used_by,
                }
            })
            .collect();

        tracing::info!(
            key_count = keys.len(),
            "ApiKeyManagerPresenter: sending ApiKeysListed"
        );
        let _ = view_tx.send(ViewCommand::ApiKeysListed { keys });
    }
}
