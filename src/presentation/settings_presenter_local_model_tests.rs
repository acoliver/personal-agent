//! Presenter-level tests for the Local Model settings panel (REQ-LM-006).
//!
//! Drives `handle_local_model_user_event` directly against a scripted
//! `AppSettingsService`, asserting the emitted `ViewCommand`s: settings load
//! (and its defaults fallback), save echo + notification, save-failure error,
//! unload status push, and the poll-generation re-arm.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::broadcast;

use crate::events::types::UserEvent;
use crate::presentation::view_command::{ErrorSeverity, ViewCommand};
use crate::services::local_model_settings::{
    LocalModelSettings, ENV_LOCK, LOCAL_MODEL_SETTINGS_KEY,
};
use crate::services::{AppSettingsService, ServiceError, ServiceResult};

use super::SettingsPresenter;

/// The panel re-arms polling on every entry; tests use the same counter the
/// presenter does so they can terminate the spawned poll task.
type PollGeneration = Arc<std::sync::atomic::AtomicU64>;

/// Scripted app-settings store: the local-model blob travels through
/// `get_setting`/`set_setting`; everything else is inert. `fail_setting_reads`
/// and `fail_setting_writes` script the storage failure paths.
#[derive(Default)]
struct MockAppSettings {
    values: std::sync::Mutex<HashMap<String, String>>,
    fail_setting_reads: bool,
    fail_setting_writes: bool,
}

impl MockAppSettings {
    fn with_saved_blob(json: String) -> Self {
        let mock = Self::default();
        mock.values
            .lock()
            .expect("values")
            .insert(LOCAL_MODEL_SETTINGS_KEY.to_string(), json);
        mock
    }

    fn stored_blob(&self, key: &str) -> Option<String> {
        self.values.lock().expect("values").get(key).cloned()
    }
}

#[async_trait::async_trait]
impl AppSettingsService for MockAppSettings {
    async fn get_default_profile_id(&self) -> ServiceResult<Option<uuid::Uuid>> {
        Ok(None)
    }
    async fn set_default_profile_id(&self, _id: uuid::Uuid) -> ServiceResult<()> {
        Ok(())
    }
    async fn clear_default_profile_id(&self) -> ServiceResult<()> {
        Ok(())
    }
    async fn get_current_conversation_id(&self) -> ServiceResult<Option<uuid::Uuid>> {
        Ok(None)
    }
    async fn set_current_conversation_id(&self, _id: uuid::Uuid) -> ServiceResult<()> {
        Ok(())
    }
    async fn get_hotkey(&self) -> ServiceResult<Option<String>> {
        Ok(None)
    }
    async fn set_hotkey(&self, _hotkey: String) -> ServiceResult<()> {
        Ok(())
    }
    async fn get_theme(&self) -> ServiceResult<Option<String>> {
        Ok(None)
    }
    async fn set_theme(&self, _theme: String) -> ServiceResult<()> {
        Ok(())
    }
    async fn get_filter_emoji(&self) -> ServiceResult<Option<bool>> {
        Ok(None)
    }
    async fn set_filter_emoji(&self, _enabled: bool) -> ServiceResult<()> {
        Ok(())
    }
    async fn get_launch_at_login(&self) -> ServiceResult<Option<bool>> {
        Ok(None)
    }
    async fn set_launch_at_login(&self, _enabled: bool) -> ServiceResult<()> {
        Ok(())
    }
    async fn get_setting(&self, key: &str) -> ServiceResult<Option<String>> {
        if self.fail_setting_reads {
            return Err(ServiceError::Storage("simulated read failure".into()));
        }
        Ok(self.stored_blob(key))
    }
    async fn set_setting(&self, key: &str, value: String) -> ServiceResult<()> {
        if self.fail_setting_writes {
            return Err(ServiceError::Storage("simulated write failure".into()));
        }
        self.values
            .lock()
            .expect("values")
            .insert(key.to_string(), value);
        Ok(())
    }
    async fn reset_to_defaults(&self) -> ServiceResult<()> {
        Ok(())
    }
}

fn channel() -> (
    Arc<dyn AppSettingsService>,
    broadcast::Sender<ViewCommand>,
    broadcast::Receiver<ViewCommand>,
) {
    let mock: Arc<dyn AppSettingsService> = Arc::new(MockAppSettings::default());
    let (view_tx, view_rx) = broadcast::channel(32);
    (mock, view_tx, view_rx)
}

fn poll_generation() -> PollGeneration {
    Arc::new(std::sync::atomic::AtomicU64::new(0))
}

/// Receives commands until one matches the predicate, with a bounded wait so
/// a missing command fails the test instead of hanging it.
async fn recv_matching(
    rx: &mut broadcast::Receiver<ViewCommand>,
    what: &str,
    matches: impl Fn(&ViewCommand) -> bool + Send,
) -> ViewCommand {
    let deadline = tokio::time::Duration::from_secs(5);
    tokio::time::timeout(deadline, async move {
        loop {
            match rx.recv().await {
                Ok(command) => {
                    if matches(&command) {
                        return command;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => {
                    panic!("view channel closed while waiting for {what}");
                }
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
}

/// Stops the poll task a handler spawned by invalidating its generation and
/// giving the task one tick to observe it.
async fn retire_poll_tasks(poll_generation: &PollGeneration) {
    poll_generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
}

#[tokio::test]
async fn load_pushes_saved_settings_and_arms_the_status_poll() {
    let saved = LocalModelSettings {
        model_path: "/models/panel.gguf".into(),
        n_ctx: 2048,
        gpu_layers: 33,
        idle_unload: false,
        idle_timeout_minutes: 3,
    };
    let mock: Arc<dyn AppSettingsService> = Arc::new(MockAppSettings::with_saved_blob(
        serde_json::to_string(&saved).expect("serialize"),
    ));
    let (view_tx, mut view_rx) = broadcast::channel(32);
    let poll = poll_generation();

    let handled = SettingsPresenter::handle_local_model_user_event(
        &mock,
        &view_tx,
        &UserEvent::LoadLocalModelSettings,
        &poll,
    )
    .await;
    assert!(handled, "the load event must be handled");

    let loaded = recv_matching(&mut view_rx, "LocalModelSettingsLoaded", |command| {
        matches!(command, ViewCommand::LocalModelSettingsLoaded { .. })
    })
    .await;
    match loaded {
        ViewCommand::LocalModelSettingsLoaded { settings } => assert_eq!(settings, saved),
        other => panic!("unexpected command: {other:?}"),
    }

    // The re-armed poll pushes one status snapshot per tick, starting with an
    // immediate first tick after the settings snapshot.
    let status = recv_matching(&mut view_rx, "LocalModelStatusUpdated", |command| {
        matches!(command, ViewCommand::LocalModelStatusUpdated { .. })
    })
    .await;
    match status {
        ViewCommand::LocalModelStatusUpdated { status } => {
            // The engine singleton is shared with other tests in this
            // binary; both a fresh (NotLoaded) and a lost-init (Error)
            // actor produce a valid card payload.
            assert!(matches!(
                status,
                crate::llm::local::engine::EngineStatus::NotLoaded
                    | crate::llm::local::engine::EngineStatus::Error { .. }
            ));
        }
        other => panic!("unexpected command: {other:?}"),
    }
    assert!(
        poll.load(std::sync::atomic::Ordering::Relaxed) >= 1,
        "panel entry must re-arm the status poll"
    );

    retire_poll_tasks(&poll).await;
}

#[tokio::test]
async fn load_with_failing_storage_falls_back_to_defaults() {
    let mock: Arc<dyn AppSettingsService> = Arc::new(MockAppSettings {
        fail_setting_reads: true,
        ..MockAppSettings::default()
    });
    let (view_tx, mut view_rx) = broadcast::channel(32);
    let poll = poll_generation();

    // The defaults fallback resolves model_path from `PA_LOCAL_GGUF`; hold
    // the env lock so a sibling test's override cannot interleave.
    let guard = ENV_LOCK.lock().await;
    let handled = SettingsPresenter::handle_local_model_user_event(
        &mock,
        &view_tx,
        &UserEvent::LoadLocalModelSettings,
        &poll,
    )
    .await;
    assert!(handled);

    let loaded = recv_matching(&mut view_rx, "LocalModelSettingsLoaded", |command| {
        matches!(command, ViewCommand::LocalModelSettingsLoaded { .. })
    })
    .await;
    match loaded {
        ViewCommand::LocalModelSettingsLoaded { settings } => {
            // model_path depends on `PA_LOCAL_GGUF`/env state, so the
            // defaults fallback is pinned on the env-independent knobs.
            assert_eq!(settings.n_ctx, 32_768);
            assert_eq!(settings.gpu_layers, 999);
            assert!(settings.idle_unload);
            assert_eq!(settings.idle_timeout_minutes, 5);
        }
        other => panic!("unexpected command: {other:?}"),
    }

    drop(guard);
    retire_poll_tasks(&poll).await;
}

#[tokio::test]
async fn save_persists_echoes_the_snapshot_and_notifies() {
    let (mock, view_tx, mut view_rx) = channel();
    let poll = poll_generation();
    let settings = LocalModelSettings {
        model_path: "/models/saved.gguf".into(),
        n_ctx: 8192,
        gpu_layers: 0,
        idle_unload: true,
        idle_timeout_minutes: 10,
    };

    let handled = SettingsPresenter::handle_local_model_user_event(
        &mock,
        &view_tx,
        &UserEvent::SaveLocalModelSettings {
            settings: settings.clone(),
        },
        &poll,
    )
    .await;
    assert!(handled);

    let echoed = recv_matching(&mut view_rx, "LocalModelSettingsLoaded", |command| {
        matches!(command, ViewCommand::LocalModelSettingsLoaded { .. })
    })
    .await;
    match echoed {
        ViewCommand::LocalModelSettingsLoaded { settings: echoed } => {
            assert_eq!(echoed, settings);
        }
        other => panic!("unexpected command: {other:?}"),
    }
    let notification = recv_matching(&mut view_rx, "ShowNotification", |command| {
        matches!(command, ViewCommand::ShowNotification { .. })
    })
    .await;
    match notification {
        ViewCommand::ShowNotification { message } => {
            assert_eq!(message, "Local model settings saved");
        }
        other => panic!("unexpected command: {other:?}"),
    }

    let stored = mock
        .get_setting(LOCAL_MODEL_SETTINGS_KEY)
        .await
        .expect("stored")
        .expect("blob persisted under the local model key");
    let persisted: LocalModelSettings = serde_json::from_str(&stored).expect("valid json");
    assert_eq!(persisted, settings);
}

#[tokio::test]
async fn save_failure_surfaces_a_warning_and_no_echo() {
    let mock: Arc<dyn AppSettingsService> = Arc::new(MockAppSettings {
        fail_setting_writes: true,
        ..MockAppSettings::default()
    });
    let (view_tx, mut view_rx) = broadcast::channel(32);
    let poll = poll_generation();

    let guard = ENV_LOCK.lock().await;
    let handled = SettingsPresenter::handle_local_model_user_event(
        &mock,
        &view_tx,
        &UserEvent::SaveLocalModelSettings {
            settings: LocalModelSettings::default(),
        },
        &poll,
    )
    .await;
    assert!(handled);
    drop(guard);

    let error = recv_matching(&mut view_rx, "ShowError", |command| {
        matches!(command, ViewCommand::ShowError { .. })
    })
    .await;
    match error {
        ViewCommand::ShowError {
            title,
            message,
            severity,
        } => {
            assert_eq!(title, "Local Model");
            assert!(
                message.contains("Failed to save"),
                "error should name the save failure, got: {message}"
            );
            assert_eq!(severity, ErrorSeverity::Warning);
        }
        other => panic!("unexpected command: {other:?}"),
    }
    assert!(
        mock.get_setting(LOCAL_MODEL_SETTINGS_KEY)
            .await
            .expect("read")
            .is_none(),
        "a failed save must not persist a blob"
    );
}

#[tokio::test]
async fn unload_pushes_a_status_snapshot() {
    let (mock, view_tx, mut view_rx) = channel();
    let poll = poll_generation();

    let handled = SettingsPresenter::handle_local_model_user_event(
        &mock,
        &view_tx,
        &UserEvent::UnloadLocalModel,
        &poll,
    )
    .await;
    assert!(handled, "the unload event must be handled");

    let status = recv_matching(&mut view_rx, "LocalModelStatusUpdated", |command| {
        matches!(command, ViewCommand::LocalModelStatusUpdated { .. })
    })
    .await;
    match status {
        ViewCommand::LocalModelStatusUpdated { status } => {
            assert!(matches!(
                status,
                crate::llm::local::engine::EngineStatus::NotLoaded
                    | crate::llm::local::engine::EngineStatus::Error { .. }
            ));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[tokio::test]
async fn unrelated_events_are_left_unhandled() {
    let (mock, view_tx, mut view_rx) = channel();
    let poll = poll_generation();

    let handled = SettingsPresenter::handle_local_model_user_event(
        &mock,
        &view_tx,
        &UserEvent::RefreshProfiles,
        &poll,
    )
    .await;
    assert!(
        !handled,
        "the local-model handler must not swallow other events"
    );

    let drained = view_rx.try_recv().is_err();
    assert!(drained, "an unhandled event must emit no commands");
}
