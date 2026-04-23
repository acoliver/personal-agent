//! Presenter-level tests for the macOS "Launch at Login" feature (Issue #177).
//!
//! These tests pin the contract between the Settings UI and the presenter:
//!
//! - `UserEvent::SetLaunchAtLogin { enabled: true }` must persist the
//!   preference via `AppSettingsService` AND call `LoginItemService::register`.
//! - A `RequiresApproval` status must surface as `enabled: true` with an
//!   informative error message so the UI can tell the user to approve in
//!   System Settings.
//! - An `SMAppService` failure must roll the persisted preference back to its
//!   previous value so the toggle does not "stick" on after a transient
//!   failure.
//! - Startup must emit an initial `SetLaunchAtLoginState` so the toggle
//!   matches OS state even if the user flipped the login item off while the
//!   app was closed.

use std::path::Path;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::sync::broadcast;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

use personal_agent::backup::{BackupInfo, BackupResult, DatabaseBackupSettings, RestoreResult};
use personal_agent::events::types::{AppEvent, UserEvent};
use personal_agent::models::{AuthConfig, ModelParameters, ModelProfile};
use personal_agent::presentation::settings_presenter::SettingsPresenter;
use personal_agent::presentation::view_command::ViewCommand;
use personal_agent::services::login_item::{LoginItemError, LoginItemService, LoginItemStatus};
use personal_agent::services::{AppSettingsService, BackupService, ProfileService, ServiceError};

const RECV_TIMEOUT: Duration = Duration::from_millis(750);

// ---------------------------------------------------------------------------
// Fakes
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct MockProfile;

#[async_trait]
impl ProfileService for MockProfile {
    async fn list(&self) -> Result<Vec<ModelProfile>, ServiceError> {
        Ok(Vec::new())
    }
    async fn get(&self, _id: Uuid) -> Result<ModelProfile, ServiceError> {
        Err(ServiceError::NotFound("unused".into()))
    }
    async fn create(
        &self,
        _name: String,
        _provider: String,
        _model: String,
        _base_url: Option<String>,
        _auth: AuthConfig,
        _parameters: ModelParameters,
        _system_prompt: Option<String>,
    ) -> Result<ModelProfile, ServiceError> {
        Err(ServiceError::NotFound("unused".into()))
    }
    async fn update(
        &self,
        _id: Uuid,
        _name: Option<String>,
        _provider: Option<String>,
        _model: Option<String>,
        _base_url: Option<String>,
        _auth: Option<AuthConfig>,
        _parameters: Option<ModelParameters>,
        _system_prompt: Option<String>,
    ) -> Result<ModelProfile, ServiceError> {
        Err(ServiceError::NotFound("unused".into()))
    }
    async fn delete(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }
    async fn test_connection(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }
    async fn get_default(&self) -> Result<Option<ModelProfile>, ServiceError> {
        Ok(None)
    }
    async fn set_default(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }
}

#[derive(Default)]
struct AppSettingsState {
    launch_at_login: Option<bool>,
}

#[derive(Clone, Default)]
struct MockAppSettings {
    state: Arc<Mutex<AppSettingsState>>,
    set_failures: Arc<AtomicUsize>,
}

impl MockAppSettings {
    fn current_launch_at_login(&self) -> Option<bool> {
        self.state.lock().unwrap().launch_at_login
    }

    fn fail_next_set_launch(&self) {
        self.set_failures.fetch_add(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl AppSettingsService for MockAppSettings {
    async fn get_default_profile_id(&self) -> Result<Option<Uuid>, ServiceError> {
        Ok(None)
    }
    async fn set_default_profile_id(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }
    async fn clear_default_profile_id(&self) -> Result<(), ServiceError> {
        Ok(())
    }
    async fn get_current_conversation_id(&self) -> Result<Option<Uuid>, ServiceError> {
        Ok(None)
    }
    async fn set_current_conversation_id(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }
    async fn get_hotkey(&self) -> Result<Option<String>, ServiceError> {
        Ok(None)
    }
    async fn set_hotkey(&self, _hotkey: String) -> Result<(), ServiceError> {
        Ok(())
    }
    async fn get_theme(&self) -> Result<Option<String>, ServiceError> {
        Ok(None)
    }
    async fn set_theme(&self, _theme: String) -> Result<(), ServiceError> {
        Ok(())
    }
    async fn get_filter_emoji(&self) -> Result<Option<bool>, ServiceError> {
        Ok(None)
    }
    async fn set_filter_emoji(&self, _enabled: bool) -> Result<(), ServiceError> {
        Ok(())
    }
    async fn get_launch_at_login(&self) -> Result<Option<bool>, ServiceError> {
        Ok(self.state.lock().unwrap().launch_at_login)
    }
    async fn set_launch_at_login(&self, enabled: bool) -> Result<(), ServiceError> {
        if self.set_failures.load(Ordering::SeqCst) > 0 {
            self.set_failures.fetch_sub(1, Ordering::SeqCst);
            return Err(ServiceError::Storage("simulated disk failure".into()));
        }
        self.state.lock().unwrap().launch_at_login = Some(enabled);
        Ok(())
    }
    async fn get_setting(&self, _key: &str) -> Result<Option<String>, ServiceError> {
        Ok(None)
    }
    async fn set_setting(&self, _key: &str, _value: String) -> Result<(), ServiceError> {
        Ok(())
    }
    async fn reset_to_defaults(&self) -> Result<(), ServiceError> {
        Ok(())
    }
}

#[derive(Clone, Default)]
struct MockBackupService;

#[async_trait]
impl BackupService for MockBackupService {
    async fn create_backup(&self) -> Result<BackupResult, ServiceError> {
        Ok(BackupResult::Skipped {
            reason: "test".to_string(),
        })
    }
    async fn list_backups(&self) -> Result<Vec<BackupInfo>, ServiceError> {
        Ok(Vec::new())
    }
    async fn restore_backup(&self, _path: &Path) -> Result<RestoreResult, ServiceError> {
        Ok(RestoreResult::Success)
    }
    async fn get_settings(&self) -> Result<DatabaseBackupSettings, ServiceError> {
        Ok(DatabaseBackupSettings::default())
    }
    async fn update_settings(&self, _settings: DatabaseBackupSettings) -> Result<(), ServiceError> {
        Ok(())
    }
    async fn get_last_backup_time(&self) -> Result<Option<DateTime<Utc>>, ServiceError> {
        Ok(None)
    }
    async fn should_backup(&self) -> Result<bool, ServiceError> {
        Ok(false)
    }
}

/// Login-item fake that lets each test script the next register/unregister
/// outcome. Tracks call counts so we can assert the presenter actually
/// invokes the OS API and doesn't just update the preference.
struct FakeLoginItemState {
    status: LoginItemStatus,
    register_script: Option<Result<LoginItemStatus, String>>,
    unregister_script: Option<Result<LoginItemStatus, String>>,
    register_calls: usize,
    unregister_calls: usize,
}

struct FakeLoginItem {
    state: Mutex<FakeLoginItemState>,
}

impl FakeLoginItem {
    fn new(initial: LoginItemStatus) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(FakeLoginItemState {
                status: initial,
                register_script: None,
                unregister_script: None,
                register_calls: 0,
                unregister_calls: 0,
            }),
        })
    }

    fn script_register(&self, outcome: Result<LoginItemStatus, String>) {
        self.state.lock().unwrap().register_script = Some(outcome);
    }

    fn register_calls(&self) -> usize {
        self.state.lock().unwrap().register_calls
    }

    fn unregister_calls(&self) -> usize {
        self.state.lock().unwrap().unregister_calls
    }
}

impl LoginItemService for FakeLoginItem {
    fn status(&self) -> Result<LoginItemStatus, LoginItemError> {
        Ok(self.state.lock().unwrap().status)
    }

    fn register(&self) -> Result<LoginItemStatus, LoginItemError> {
        let outcome = {
            let mut state = self.state.lock().unwrap();
            state.register_calls += 1;
            state.register_script.take()
        };
        match outcome {
            Some(Ok(new_status)) => {
                self.state.lock().unwrap().status = new_status;
                Ok(new_status)
            }
            Some(Err(msg)) => Err(LoginItemError(msg)),
            None => {
                self.state.lock().unwrap().status = LoginItemStatus::Enabled;
                Ok(LoginItemStatus::Enabled)
            }
        }
    }

    fn unregister(&self) -> Result<LoginItemStatus, LoginItemError> {
        let outcome = {
            let mut state = self.state.lock().unwrap();
            state.unregister_calls += 1;
            state.unregister_script.take()
        };
        match outcome {
            Some(Ok(new_status)) => {
                self.state.lock().unwrap().status = new_status;
                Ok(new_status)
            }
            Some(Err(msg)) => Err(LoginItemError(msg)),
            None => {
                self.state.lock().unwrap().status = LoginItemStatus::NotRegistered;
                Ok(LoginItemStatus::NotRegistered)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_presenter(
    settings: MockAppSettings,
    login_item: Arc<FakeLoginItem>,
) -> (
    SettingsPresenter,
    broadcast::Sender<AppEvent>,
    broadcast::Receiver<ViewCommand>,
    tempfile::TempDir,
) {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let (event_tx, _) = broadcast::channel::<AppEvent>(64);
    let (view_tx, view_rx) = broadcast::channel::<ViewCommand>(128);

    let app_settings: Arc<dyn AppSettingsService> = Arc::new(settings);
    let skills_service = Arc::new(
        personal_agent::services::SkillsServiceImpl::new_for_tests(
            app_settings.clone(),
            temp_dir.path().join("bundled-skills"),
            temp_dir.path().join("user-skills"),
        )
        .expect("skills service should initialize for tests"),
    );

    let presenter = SettingsPresenter::new(
        Arc::new(MockProfile),
        app_settings,
        Arc::new(MockBackupService),
        skills_service,
        &event_tx,
        view_tx,
    )
    .with_login_item_service(login_item);

    (presenter, event_tx, view_rx, temp_dir)
}

async fn recv_launch_at_login(rx: &mut broadcast::Receiver<ViewCommand>) -> (bool, Option<String>) {
    let deadline = tokio::time::Instant::now() + RECV_TIMEOUT * 4;
    loop {
        assert!(
            tokio::time::Instant::now() <= deadline,
            "timed out waiting for SetLaunchAtLoginState"
        );
        let cmd = timeout(RECV_TIMEOUT, rx.recv())
            .await
            .expect("recv timed out")
            .expect("presenter channel closed");
        if let ViewCommand::SetLaunchAtLoginState { enabled, error } = cmd {
            return (enabled, error);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn enabling_registers_and_persists() {
    let settings = MockAppSettings::default();
    let login_item = FakeLoginItem::new(LoginItemStatus::NotRegistered);
    let (mut presenter, event_tx, mut view_rx, _td) =
        build_presenter(settings.clone(), login_item.clone());
    presenter.start().await.expect("start presenter");

    let (enabled, error) = recv_launch_at_login(&mut view_rx).await;
    assert!(!enabled);
    assert!(error.is_none());

    event_tx
        .send(AppEvent::User(UserEvent::SetLaunchAtLogin {
            enabled: true,
        }))
        .unwrap();

    let (enabled, error) = recv_launch_at_login(&mut view_rx).await;
    assert!(
        enabled,
        "toggle should report enabled after successful register"
    );
    assert!(error.is_none(), "no error expected on clean success");
    assert_eq!(login_item.register_calls(), 1);
    assert_eq!(login_item.unregister_calls(), 0);
    assert_eq!(settings.current_launch_at_login(), Some(true));

    presenter.stop().await.expect("stop");
}

#[tokio::test]
async fn requires_approval_reports_enabled_with_actionable_error() {
    let settings = MockAppSettings::default();
    let login_item = FakeLoginItem::new(LoginItemStatus::NotRegistered);
    login_item.script_register(Ok(LoginItemStatus::RequiresApproval));
    let (mut presenter, event_tx, mut view_rx, _td) =
        build_presenter(settings.clone(), login_item.clone());
    presenter.start().await.expect("start presenter");

    let _ = recv_launch_at_login(&mut view_rx).await;

    event_tx
        .send(AppEvent::User(UserEvent::SetLaunchAtLogin {
            enabled: true,
        }))
        .unwrap();

    let (enabled, error) = recv_launch_at_login(&mut view_rx).await;
    assert!(enabled);
    let msg = error.expect("RequiresApproval should surface an error message");
    assert!(
        msg.to_lowercase().contains("login items"),
        "message should point the user to Login Items; got: {msg}"
    );
    assert_eq!(settings.current_launch_at_login(), Some(true));

    presenter.stop().await.expect("stop");
}

#[tokio::test]
async fn register_failure_rolls_back_persisted_preference() {
    let settings = MockAppSettings::default();
    let login_item = FakeLoginItem::new(LoginItemStatus::NotRegistered);
    login_item.script_register(Err("codesign check failed".to_string()));
    let (mut presenter, event_tx, mut view_rx, _td) =
        build_presenter(settings.clone(), login_item.clone());
    presenter.start().await.expect("start presenter");

    let _ = recv_launch_at_login(&mut view_rx).await;

    event_tx
        .send(AppEvent::User(UserEvent::SetLaunchAtLogin {
            enabled: true,
        }))
        .unwrap();

    let (enabled, error) = recv_launch_at_login(&mut view_rx).await;
    assert!(!enabled, "failed register must leave toggle off");
    assert_eq!(error.as_deref(), Some("codesign check failed"));
    assert_eq!(
        settings.current_launch_at_login(),
        Some(false),
        "preference should be rolled back on failure"
    );

    presenter.stop().await.expect("stop");
}

#[tokio::test]
async fn disabling_unregisters_and_persists() {
    let settings = MockAppSettings::default();
    settings
        .set_launch_at_login(true)
        .await
        .expect("seed setting");
    let login_item = FakeLoginItem::new(LoginItemStatus::Enabled);
    let (mut presenter, event_tx, mut view_rx, _td) =
        build_presenter(settings.clone(), login_item.clone());
    presenter.start().await.expect("start presenter");

    let (enabled, _error) = recv_launch_at_login(&mut view_rx).await;
    assert!(enabled);

    event_tx
        .send(AppEvent::User(UserEvent::SetLaunchAtLogin {
            enabled: false,
        }))
        .unwrap();

    let (enabled, error) = recv_launch_at_login(&mut view_rx).await;
    assert!(!enabled);
    assert!(error.is_none());
    assert_eq!(login_item.unregister_calls(), 1);
    assert_eq!(settings.current_launch_at_login(), Some(false));

    presenter.stop().await.expect("stop");
}

#[tokio::test]
async fn persist_failure_does_not_call_os() {
    let settings = MockAppSettings::default();
    settings.fail_next_set_launch();
    let login_item = FakeLoginItem::new(LoginItemStatus::NotRegistered);
    let (mut presenter, event_tx, mut view_rx, _td) =
        build_presenter(settings.clone(), login_item.clone());
    presenter.start().await.expect("start presenter");

    let _ = recv_launch_at_login(&mut view_rx).await;

    event_tx
        .send(AppEvent::User(UserEvent::SetLaunchAtLogin {
            enabled: true,
        }))
        .unwrap();

    let (enabled, error) = recv_launch_at_login(&mut view_rx).await;
    assert!(!enabled);
    let msg = error.expect("persist failure should surface an error");
    assert!(
        msg.to_lowercase().contains("launch-at-login") || msg.to_lowercase().contains("preference"),
        "message should explain we failed to save the preference; got: {msg}"
    );
    assert_eq!(
        login_item.register_calls(),
        0,
        "we should not touch the OS if we could not even persist the pref"
    );

    presenter.stop().await.expect("stop");
}

#[tokio::test]
async fn startup_snapshot_reflects_os_state() {
    let settings = MockAppSettings::default();
    // Stored preference says "off" but the OS reports "enabled" — the
    // snapshot must trust the OS, not the stored pref.
    let login_item = FakeLoginItem::new(LoginItemStatus::Enabled);
    let (mut presenter, _event_tx, mut view_rx, _td) = build_presenter(settings, login_item);
    presenter.start().await.expect("start presenter");

    let (enabled, error) = recv_launch_at_login(&mut view_rx).await;
    assert!(enabled);
    assert!(error.is_none());

    presenter.stop().await.expect("stop");
}
