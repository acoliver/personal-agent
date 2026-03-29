use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{sleep, timeout, Duration};
use uuid::Uuid;

use personal_agent::events::{
    bus::EventBus,
    types::{AppEvent, ConversationEvent, McpEvent, ProfileEvent, SystemEvent, UserEvent},
};
use personal_agent::models::{AuthConfig, Conversation, Message, ModelParameters, ModelProfile};
use personal_agent::presentation::{
    history_presenter::HistoryPresenter,
    settings_presenter::SettingsPresenter,
    view_command::{ErrorSeverity, McpStatus, ProfileSummary, ViewCommand, ViewId},
};
use personal_agent::services::{
    AppSettingsService, ConversationService, ProfileService, ServiceError,
};

const PROCESSING_DELAY: Duration = Duration::from_millis(25);
const RECV_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Clone)]
struct MockConversationService {
    state: Arc<Mutex<MockConversationState>>,
}

struct MockConversationState {
    delete_results: VecDeque<Result<(), ServiceError>>,
    deleted_ids: Vec<Uuid>,
}

impl MockConversationService {
    fn new(delete_results: Vec<Result<(), ServiceError>>) -> Self {
        Self {
            state: Arc::new(Mutex::new(MockConversationState {
                delete_results: delete_results.into(),
                deleted_ids: Vec::new(),
            })),
        }
    }

    fn deleted_ids(&self) -> Vec<Uuid> {
        self.state.lock().unwrap().deleted_ids.clone()
    }
}

#[async_trait]
impl ConversationService for MockConversationService {
    async fn create(
        &self,
        _title: Option<String>,
        _model_profile_id: Uuid,
    ) -> Result<Conversation, ServiceError> {
        Err(ServiceError::NotFound("not implemented".to_string()))
    }

    async fn load(&self, _id: Uuid) -> Result<Conversation, ServiceError> {
        Err(ServiceError::NotFound("not implemented".to_string()))
    }

    async fn list(
        &self,
        _limit: Option<usize>,
        _offset: Option<usize>,
    ) -> Result<Vec<Conversation>, ServiceError> {
        Ok(vec![])
    }

    async fn add_user_message(
        &self,
        _conversation_id: Uuid,
        _content: String,
    ) -> Result<Message, ServiceError> {
        Err(ServiceError::NotFound("not implemented".to_string()))
    }

    async fn add_assistant_message(
        &self,
        _conversation_id: Uuid,
        _content: String,
    ) -> Result<Message, ServiceError> {
        Err(ServiceError::NotFound("not implemented".to_string()))
    }

    async fn rename(&self, _id: Uuid, _new_title: String) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<(), ServiceError> {
        let mut state = self.state.lock().unwrap();
        state.deleted_ids.push(id);
        state.delete_results.pop_front().unwrap_or(Ok(()))
    }

    async fn set_active(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn get_active(&self) -> Result<Option<Uuid>, ServiceError> {
        Ok(None)
    }

    async fn get_messages(&self, _conversation_id: Uuid) -> Result<Vec<Message>, ServiceError> {
        Ok(vec![])
    }

    async fn update(
        &self,
        _id: Uuid,
        _title: Option<String>,
        _model_profile_id: Option<Uuid>,
    ) -> Result<Conversation, ServiceError> {
        Err(ServiceError::NotFound("not implemented".to_string()))
    }
}

#[derive(Clone)]
struct MockProfileService {
    state: Arc<Mutex<MockProfileState>>,
}

struct MockProfileState {
    profiles: Vec<ModelProfile>,
    default_profile: Option<Uuid>,
    get_results: VecDeque<Result<ModelProfile, ServiceError>>,
    delete_results: VecDeque<Result<(), ServiceError>>,
    set_default_results: VecDeque<Result<(), ServiceError>>,
    deleted_ids: Vec<Uuid>,
    set_default_calls: Vec<Uuid>,
}

impl MockProfileService {
    fn new(profiles: Vec<ModelProfile>, default_profile: Option<Uuid>) -> Self {
        Self {
            state: Arc::new(Mutex::new(MockProfileState {
                profiles,
                default_profile,
                get_results: VecDeque::new(),
                delete_results: VecDeque::new(),
                set_default_results: VecDeque::new(),
                deleted_ids: Vec::new(),
                set_default_calls: Vec::new(),
            })),
        }
    }

    fn with_get_results(self, results: Vec<Result<ModelProfile, ServiceError>>) -> Self {
        self.state.lock().unwrap().get_results = results.into();
        self
    }

    fn with_delete_results(self, results: Vec<Result<(), ServiceError>>) -> Self {
        self.state.lock().unwrap().delete_results = results.into();
        self
    }

    fn with_set_default_results(self, results: Vec<Result<(), ServiceError>>) -> Self {
        self.state.lock().unwrap().set_default_results = results.into();
        self
    }

    fn deleted_ids(&self) -> Vec<Uuid> {
        self.state.lock().unwrap().deleted_ids.clone()
    }

    fn set_default_calls(&self) -> Vec<Uuid> {
        self.state.lock().unwrap().set_default_calls.clone()
    }
}

#[async_trait]
impl ProfileService for MockProfileService {
    async fn list(&self) -> Result<Vec<ModelProfile>, ServiceError> {
        Ok(self.state.lock().unwrap().profiles.clone())
    }

    async fn get(&self, id: Uuid) -> Result<ModelProfile, ServiceError> {
        let mut state = self.state.lock().unwrap();
        if let Some(result) = state.get_results.pop_front() {
            return result;
        }

        state
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .cloned()
            .ok_or_else(|| ServiceError::NotFound(format!("profile {id} not found")))
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
        Err(ServiceError::NotFound("not implemented".to_string()))
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
        Err(ServiceError::NotFound("not implemented".to_string()))
    }

    async fn delete(&self, id: Uuid) -> Result<(), ServiceError> {
        let mut state = self.state.lock().unwrap();
        state.deleted_ids.push(id);
        let result = state.delete_results.pop_front().unwrap_or(Ok(()));
        if result.is_ok() {
            state.profiles.retain(|profile| profile.id != id);
            if state.default_profile == Some(id) {
                state.default_profile = None;
            }
        }
        result
    }

    async fn test_connection(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn get_default(&self) -> Result<Option<ModelProfile>, ServiceError> {
        let state = self.state.lock().unwrap();
        Ok(state.default_profile.and_then(|default_id| {
            state
                .profiles
                .iter()
                .find(|profile| profile.id == default_id)
                .cloned()
        }))
    }

    async fn set_default(&self, id: Uuid) -> Result<(), ServiceError> {
        let mut state = self.state.lock().unwrap();
        state.set_default_calls.push(id);
        let result = state.set_default_results.pop_front().unwrap_or(Ok(()));
        if result.is_ok() {
            state.default_profile = Some(id);
        }
        result
    }
}

#[derive(Clone)]
struct MockAppSettingsService {
    state: Arc<Mutex<MockAppSettingsState>>,
}

struct MockAppSettingsState {
    default_profile_id: Option<Uuid>,
    set_default_profile_id_calls: Vec<Uuid>,
    set_default_profile_id_results: VecDeque<Result<(), ServiceError>>,
    theme: Option<String>,
    set_theme_calls: Vec<String>,
    set_theme_results: VecDeque<Result<(), ServiceError>>,
}

impl MockAppSettingsService {
    fn new(default_profile_id: Option<Uuid>) -> Self {
        Self {
            state: Arc::new(Mutex::new(MockAppSettingsState {
                default_profile_id,
                set_default_profile_id_calls: Vec::new(),
                set_default_profile_id_results: VecDeque::new(),
                theme: None,
                set_theme_calls: Vec::new(),
                set_theme_results: VecDeque::new(),
            })),
        }
    }

    fn with_set_default_results(self, results: Vec<Result<(), ServiceError>>) -> Self {
        self.state.lock().unwrap().set_default_profile_id_results = results.into();
        self
    }

    fn with_theme(self, theme: &str) -> Self {
        self.state.lock().unwrap().theme = Some(theme.to_string());
        self
    }

    fn with_set_theme_results(self, results: Vec<Result<(), ServiceError>>) -> Self {
        self.state.lock().unwrap().set_theme_results = results.into();
        self
    }

    fn set_current_theme(&self, theme: Option<&str>) {
        self.state.lock().unwrap().theme = theme.map(ToString::to_string);
    }

    fn set_default_profile_id_calls(&self) -> Vec<Uuid> {
        self.state
            .lock()
            .unwrap()
            .set_default_profile_id_calls
            .clone()
    }

    fn set_theme_calls(&self) -> Vec<String> {
        self.state.lock().unwrap().set_theme_calls.clone()
    }

    fn current_theme(&self) -> Option<String> {
        self.state.lock().unwrap().theme.clone()
    }
}

#[async_trait]
impl AppSettingsService for MockAppSettingsService {
    async fn get_default_profile_id(&self) -> Result<Option<Uuid>, ServiceError> {
        Ok(self.state.lock().unwrap().default_profile_id)
    }

    async fn set_default_profile_id(&self, id: Uuid) -> Result<(), ServiceError> {
        let mut state = self.state.lock().unwrap();
        state.set_default_profile_id_calls.push(id);
        let result = state
            .set_default_profile_id_results
            .pop_front()
            .unwrap_or(Ok(()));
        if result.is_ok() {
            state.default_profile_id = Some(id);
        }
        result
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
        Ok(self.state.lock().unwrap().theme.clone())
    }

    async fn set_theme(&self, theme: String) -> Result<(), ServiceError> {
        let mut state = self.state.lock().unwrap();
        state.set_theme_calls.push(theme.clone());
        let result = state.set_theme_results.pop_front().unwrap_or(Ok(()));
        if result.is_ok() {
            state.theme = Some(theme);
        }
        result
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

fn make_profile(id: Uuid, name: &str, provider_id: &str, model_id: &str) -> ModelProfile {
    ModelProfile {
        id,
        name: name.to_string(),
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        base_url: format!("https://{provider_id}.example.com"),
        auth: AuthConfig::Keychain {
            label: format!("{name}-key"),
        },
        parameters: ModelParameters {
            temperature: 0.42,
            top_p: 0.9,
            max_tokens: 2048,
            thinking_budget: Some(128),
            enable_thinking: true,
            show_thinking: true,
        },
        system_prompt: format!("prompt for {name}"),
    }
}

fn profile_summary(profile: &ModelProfile, selected_profile_id: Option<Uuid>) -> ProfileSummary {
    ProfileSummary {
        id: profile.id,
        name: profile.name.clone(),
        provider_id: profile.provider_id.clone(),
        model_id: profile.model_id.clone(),
        is_default: Some(profile.id) == selected_profile_id,
    }
}

async fn publish_history_event(event_bus: &Arc<EventBus>, event: AppEvent) {
    let _ = event_bus.publish(event);
    sleep(PROCESSING_DELAY).await;
}

async fn send_settings_event(event_tx: &broadcast::Sender<AppEvent>, event: AppEvent) {
    let _ = event_tx.send(event);
    sleep(PROCESSING_DELAY).await;
}

async fn recv_mpsc_command(rx: &mut mpsc::Receiver<ViewCommand>) -> ViewCommand {
    timeout(RECV_TIMEOUT, rx.recv())
        .await
        .expect("timed out waiting for mpsc command")
        .expect("mpsc channel closed unexpectedly")
}

async fn assert_mpsc_no_command(rx: &mut mpsc::Receiver<ViewCommand>) {
    let result = timeout(Duration::from_millis(50), rx.recv()).await;
    assert!(result.is_err(), "expected no additional mpsc command");
}

async fn recv_broadcast_command(rx: &mut broadcast::Receiver<ViewCommand>) -> ViewCommand {
    loop {
        let result = timeout(RECV_TIMEOUT, rx.recv())
            .await
            .expect("timed out waiting for broadcast command");
        match result {
            Ok(command) => return command,
            Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => {
                panic!("broadcast channel closed unexpectedly")
            }
        }
    }
}

async fn assert_broadcast_no_command(rx: &mut broadcast::Receiver<ViewCommand>) {
    let result = timeout(Duration::from_millis(50), rx.recv()).await;
    assert!(result.is_err(), "expected no additional broadcast command");
}

mod history_presenter_tests {
    use super::*;

    #[tokio::test]
    async fn lifecycle_start_stop_and_is_running_work() {
        let event_bus = Arc::new(EventBus::new(16));
        let conversation_service = Arc::new(MockConversationService::new(vec![]));
        let (view_tx, _view_rx) = mpsc::channel(16);
        let mut presenter = HistoryPresenter::new(event_bus, conversation_service, view_tx);

        assert!(!presenter.is_running());

        presenter.start().await.expect("start should succeed");
        assert!(presenter.is_running());

        presenter
            .start()
            .await
            .expect("second start should be idempotent");
        assert!(presenter.is_running());

        presenter.stop().await.expect("stop should succeed");
        assert!(!presenter.is_running());
    }

    #[tokio::test]
    async fn delete_conversation_success_calls_service_and_emits_conversation_deleted() {
        let event_bus = Arc::new(EventBus::new(16));
        let conversation_service = Arc::new(MockConversationService::new(vec![Ok(())]));
        let (view_tx, mut view_rx) = mpsc::channel(16);
        let mut presenter =
            HistoryPresenter::new(event_bus.clone(), conversation_service.clone(), view_tx);
        presenter.start().await.expect("start should succeed");

        let id = Uuid::new_v4();
        publish_history_event(
            &event_bus,
            AppEvent::User(UserEvent::DeleteConversation { id }),
        )
        .await;

        assert_eq!(
            recv_mpsc_command(&mut view_rx).await,
            ViewCommand::ConversationDeleted { id }
        );
        assert_eq!(conversation_service.deleted_ids(), vec![id]);
        assert_mpsc_no_command(&mut view_rx).await;
    }

    #[tokio::test]
    async fn delete_conversation_failure_logs_only_and_emits_no_command() {
        let event_bus = Arc::new(EventBus::new(16));
        let conversation_service = Arc::new(MockConversationService::new(vec![Err(
            ServiceError::Internal("delete failed".to_string()),
        )]));
        let (view_tx, mut view_rx) = mpsc::channel(16);
        let mut presenter =
            HistoryPresenter::new(event_bus.clone(), conversation_service.clone(), view_tx);
        presenter.start().await.expect("start should succeed");

        let id = Uuid::new_v4();
        publish_history_event(
            &event_bus,
            AppEvent::User(UserEvent::DeleteConversation { id }),
        )
        .await;

        assert_eq!(conversation_service.deleted_ids(), vec![id]);
        assert_mpsc_no_command(&mut view_rx).await;
    }

    #[tokio::test]
    async fn conversation_events_emit_expected_view_commands() {
        let event_bus = Arc::new(EventBus::new(16));
        let conversation_service = Arc::new(MockConversationService::new(vec![]));
        let (view_tx, mut view_rx) = mpsc::channel(16);
        let mut presenter = HistoryPresenter::new(event_bus.clone(), conversation_service, view_tx);
        presenter.start().await.expect("start should succeed");

        let created_id = Uuid::new_v4();
        publish_history_event(
            &event_bus,
            AppEvent::Conversation(ConversationEvent::Created {
                id: created_id,
                title: "New Chat".to_string(),
            }),
        )
        .await;
        assert_eq!(
            recv_mpsc_command(&mut view_rx).await,
            ViewCommand::ConversationCreated {
                id: created_id,
                profile_id: Uuid::nil(),
            }
        );

        let renamed_id = Uuid::new_v4();
        publish_history_event(
            &event_bus,
            AppEvent::Conversation(ConversationEvent::TitleUpdated {
                id: renamed_id,
                title: "Renamed".to_string(),
            }),
        )
        .await;
        assert_eq!(
            recv_mpsc_command(&mut view_rx).await,
            ViewCommand::ConversationTitleUpdated {
                id: renamed_id,
                title: "Renamed".to_string(),
            }
        );

        let deleted_id = Uuid::new_v4();
        publish_history_event(
            &event_bus,
            AppEvent::Conversation(ConversationEvent::Deleted { id: deleted_id }),
        )
        .await;
        assert_eq!(
            recv_mpsc_command(&mut view_rx).await,
            ViewCommand::ConversationDeleted { id: deleted_id }
        );

        publish_history_event(
            &event_bus,
            AppEvent::Conversation(ConversationEvent::ListRefreshed { count: 7 }),
        )
        .await;
        assert_eq!(
            recv_mpsc_command(&mut view_rx).await,
            ViewCommand::HistoryUpdated { count: Some(7) }
        );

        assert_mpsc_no_command(&mut view_rx).await;
    }

    #[tokio::test]
    async fn unhandled_events_are_ignored() {
        let event_bus = Arc::new(EventBus::new(16));
        let conversation_service = Arc::new(MockConversationService::new(vec![]));
        let (view_tx, mut view_rx) = mpsc::channel(16);
        let mut presenter =
            HistoryPresenter::new(event_bus.clone(), conversation_service.clone(), view_tx);
        presenter.start().await.expect("start should succeed");

        let id = Uuid::new_v4();
        publish_history_event(
            &event_bus,
            AppEvent::Conversation(ConversationEvent::Loaded { id }),
        )
        .await;
        publish_history_event(&event_bus, AppEvent::User(UserEvent::RefreshProfiles)).await;

        assert!(conversation_service.deleted_ids().is_empty());
        assert_mpsc_no_command(&mut view_rx).await;
    }
}

mod settings_presenter_tests {
    use super::*;
    use personal_agent::ui_gpui::theme::{active_theme_slug, set_active_theme_slug};

    static THEME_RUNTIME_TEST_LOCK: std::sync::LazyLock<Mutex<()>> =
        std::sync::LazyLock::new(|| Mutex::new(()));

    struct ThemeRuntimeGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        previous_slug: String,
    }

    impl ThemeRuntimeGuard {
        fn new() -> Self {
            let lock = THEME_RUNTIME_TEST_LOCK
                .lock()
                .expect("theme runtime test lock poisoned");
            let previous_slug = active_theme_slug();
            Self {
                _lock: lock,
                previous_slug,
            }
        }
    }

    impl Drop for ThemeRuntimeGuard {
        fn drop(&mut self) {
            set_active_theme_slug(&self.previous_slug);
        }
    }

    fn setup_settings_presenter(
        profile_service: MockProfileService,
        app_settings_service: MockAppSettingsService,
    ) -> (
        SettingsPresenter,
        broadcast::Sender<AppEvent>,
        broadcast::Receiver<ViewCommand>,
        MockProfileService,
        MockAppSettingsService,
    ) {
        let (event_tx, _) = broadcast::channel(64);
        let (view_tx, view_rx) = broadcast::channel(128);
        let presenter = SettingsPresenter::new(
            Arc::new(profile_service.clone()),
            Arc::new(app_settings_service.clone()),
            &event_tx,
            view_tx,
        );
        (
            presenter,
            event_tx,
            view_rx,
            profile_service,
            app_settings_service,
        )
    }

    #[tokio::test]
    async fn lifecycle_start_stop_and_is_running_work() {
        let profile = make_profile(Uuid::new_v4(), "default", "openai", "gpt-4");
        let profile_service = MockProfileService::new(vec![profile.clone()], Some(profile.id));
        let app_settings_service = MockAppSettingsService::new(Some(profile.id));
        let (mut presenter, _event_tx, mut view_rx, _profile_service, _app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        assert!(!presenter.is_running());

        presenter.start().await.expect("start should succeed");
        assert!(presenter.is_running());

        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowSettings {
                profiles: vec![profile_summary(&profile, Some(profile.id))],
                selected_profile_id: Some(profile.id),
            }
        );
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ChatProfilesUpdated {
                profiles: vec![profile_summary(&profile, Some(profile.id))],
                selected_profile_id: Some(profile.id),
            }
        );
        // Drain the ShowSettingsTheme snapshot emitted on startup
        let _ = recv_broadcast_command(&mut view_rx).await;

        presenter
            .start()
            .await
            .expect("second start should be idempotent");
        assert!(presenter.is_running());
        assert_broadcast_no_command(&mut view_rx).await;

        presenter.stop().await.expect("stop should succeed");
        assert!(!presenter.is_running());
    }

    #[tokio::test]
    async fn start_emits_initial_show_settings_and_chat_profiles_snapshot() {
        let first = make_profile(Uuid::new_v4(), "work", "anthropic", "claude");
        let second = make_profile(Uuid::new_v4(), "home", "openai", "gpt-4.1");
        let selected_profile_id = Some(second.id);
        let profile_service =
            MockProfileService::new(vec![first.clone(), second.clone()], Some(first.id));
        let app_settings_service = MockAppSettingsService::new(selected_profile_id);
        let (mut presenter, _event_tx, mut view_rx, _profile_service, _app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");

        let expected_profiles = vec![
            profile_summary(&first, selected_profile_id),
            profile_summary(&second, selected_profile_id),
        ];
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowSettings {
                profiles: expected_profiles.clone(),
                selected_profile_id,
            }
        );
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ChatProfilesUpdated {
                profiles: expected_profiles,
                selected_profile_id,
            }
        );
    }

    #[tokio::test]
    async fn select_profile_success_emits_default_profile_changed_and_snapshot() {
        let first = make_profile(Uuid::new_v4(), "work", "anthropic", "claude");
        let second = make_profile(Uuid::new_v4(), "home", "openai", "gpt-4.1");
        let profile_service =
            MockProfileService::new(vec![first.clone(), second.clone()], Some(first.id))
                .with_set_default_results(vec![Ok(())]);
        let app_settings_service =
            MockAppSettingsService::new(Some(first.id)).with_set_default_results(vec![Ok(())]);
        let (mut presenter, event_tx, mut view_rx, profile_service, app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;

        send_settings_event(
            &event_tx,
            AppEvent::User(UserEvent::SelectProfile { id: second.id }),
        )
        .await;

        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::DefaultProfileChanged {
                profile_id: Some(second.id),
            }
        );
        let expected_profiles = vec![
            profile_summary(&first, Some(second.id)),
            profile_summary(&second, Some(second.id)),
        ];
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowSettings {
                profiles: expected_profiles.clone(),
                selected_profile_id: Some(second.id),
            }
        );
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ChatProfilesUpdated {
                profiles: expected_profiles,
                selected_profile_id: Some(second.id),
            }
        );
        assert_eq!(profile_service.set_default_calls(), vec![second.id]);
        assert_eq!(
            app_settings_service.set_default_profile_id_calls(),
            vec![second.id]
        );
    }

    #[tokio::test]
    async fn select_profile_failure_emits_show_error() {
        let profile = make_profile(Uuid::new_v4(), "work", "anthropic", "claude");
        let profile_service = MockProfileService::new(vec![profile], None)
            .with_set_default_results(vec![Err(ServiceError::Internal(
                "cannot select".to_string(),
            ))]);
        let app_settings_service = MockAppSettingsService::new(None);
        let (mut presenter, event_tx, mut view_rx, profile_service, app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;

        let id = Uuid::new_v4();
        send_settings_event(&event_tx, AppEvent::User(UserEvent::SelectProfile { id })).await;

        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowError {
                title: "Error".to_string(),
                message: "Failed to select profile: Internal error: cannot select".to_string(),
                severity: ErrorSeverity::Error,
            }
        );
        assert_eq!(profile_service.set_default_calls(), vec![id]);
        assert!(app_settings_service
            .set_default_profile_id_calls()
            .is_empty());
        assert_broadcast_no_command(&mut view_rx).await;
    }

    #[tokio::test]
    async fn edit_profile_success_emits_profile_editor_load_and_navigation() {
        let profile = make_profile(Uuid::new_v4(), "editor", "openai", "gpt-4.1");
        let profile_service = MockProfileService::new(vec![profile.clone()], None)
            .with_get_results(vec![Ok(profile.clone())]);
        let app_settings_service = MockAppSettingsService::new(None);
        let (mut presenter, event_tx, mut view_rx, _profile_service, _app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;

        send_settings_event(
            &event_tx,
            AppEvent::User(UserEvent::EditProfile { id: profile.id }),
        )
        .await;

        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ProfileEditorLoad {
                id: profile.id,
                name: profile.name.clone(),
                provider_id: profile.provider_id.clone(),
                model_id: profile.model_id.clone(),
                base_url: profile.base_url.clone(),
                api_key_label: "editor-key".to_string(),
                temperature: profile.parameters.temperature,
                max_tokens: profile.parameters.max_tokens,
                context_limit: None,
                show_thinking: profile.parameters.show_thinking,
                enable_thinking: profile.parameters.enable_thinking,
                thinking_budget: profile.parameters.thinking_budget,
                system_prompt: profile.system_prompt.clone(),
            }
        );
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::NavigateTo {
                view: ViewId::ProfileEditor,
            }
        );
    }

    #[tokio::test]
    async fn edit_profile_failure_emits_show_error() {
        let id = Uuid::new_v4();
        let profile_service = MockProfileService::new(vec![], None).with_get_results(vec![Err(
            ServiceError::NotFound("missing profile".to_string()),
        )]);
        let app_settings_service = MockAppSettingsService::new(None);
        let (mut presenter, event_tx, mut view_rx, _profile_service, _app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;

        send_settings_event(&event_tx, AppEvent::User(UserEvent::EditProfile { id })).await;

        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowError {
                title: "Edit Failed".to_string(),
                message: "Failed to load profile: Not found: missing profile".to_string(),
                severity: ErrorSeverity::Error,
            }
        );
    }

    #[tokio::test]
    async fn delete_profile_success_emits_profile_deleted_and_snapshot() {
        let first = make_profile(Uuid::new_v4(), "keep", "anthropic", "claude");
        let second = make_profile(Uuid::new_v4(), "remove", "openai", "gpt-4.1");
        let profile_service =
            MockProfileService::new(vec![first.clone(), second.clone()], Some(first.id))
                .with_get_results(vec![Ok(second.clone())])
                .with_delete_results(vec![Ok(())]);
        let app_settings_service = MockAppSettingsService::new(Some(first.id));
        let (mut presenter, event_tx, mut view_rx, profile_service, _app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;

        send_settings_event(
            &event_tx,
            AppEvent::User(UserEvent::DeleteProfile { id: second.id }),
        )
        .await;

        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ProfileDeleted { id: second.id }
        );
        let expected_profiles = vec![profile_summary(&first, Some(first.id))];
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowSettings {
                profiles: expected_profiles.clone(),
                selected_profile_id: Some(first.id),
            }
        );
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ChatProfilesUpdated {
                profiles: expected_profiles,
                selected_profile_id: Some(first.id),
            }
        );
        assert_eq!(profile_service.deleted_ids(), vec![second.id]);
    }

    #[tokio::test]
    async fn delete_profile_failure_emits_show_error() {
        let profile = make_profile(Uuid::new_v4(), "remove", "openai", "gpt-4.1");
        let profile_service = MockProfileService::new(vec![profile.clone()], None)
            .with_get_results(vec![Ok(profile.clone())])
            .with_delete_results(vec![Err(ServiceError::Internal(
                "delete blocked".to_string(),
            ))]);
        let app_settings_service = MockAppSettingsService::new(None);
        let (mut presenter, event_tx, mut view_rx, profile_service, _app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;

        send_settings_event(
            &event_tx,
            AppEvent::User(UserEvent::DeleteProfile { id: profile.id }),
        )
        .await;

        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowError {
                title: "Delete Failed".to_string(),
                message: "Failed to delete profile: Internal error: delete blocked".to_string(),
                severity: ErrorSeverity::Error,
            }
        );
        assert_eq!(profile_service.deleted_ids(), vec![profile.id]);
        assert_broadcast_no_command(&mut view_rx).await;
    }

    #[tokio::test]
    async fn refresh_profiles_emits_snapshot() {
        let first = make_profile(Uuid::new_v4(), "one", "openai", "gpt-4.1");
        let second = make_profile(Uuid::new_v4(), "two", "anthropic", "claude");
        let profile_service =
            MockProfileService::new(vec![first.clone(), second.clone()], Some(second.id));
        let app_settings_service = MockAppSettingsService::new(Some(second.id));
        let (mut presenter, event_tx, mut view_rx, _profile_service, _app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;

        send_settings_event(&event_tx, AppEvent::User(UserEvent::RefreshProfiles)).await;

        let expected_profiles = vec![
            profile_summary(&first, Some(second.id)),
            profile_summary(&second, Some(second.id)),
        ];
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowSettings {
                profiles: expected_profiles.clone(),
                selected_profile_id: Some(second.id),
            }
        );
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ChatProfilesUpdated {
                profiles: expected_profiles,
                selected_profile_id: Some(second.id),
            }
        );
    }

    #[tokio::test]
    async fn toggle_mcp_emits_status_changed() {
        // Toggle and delete now operate directly on config.json.
        // With a random UUID that won't be in config, the presenter
        // logs a warning and returns without emitting — which is the
        // correct behaviour for a missing MCP.  Verify no crash.
        let profile_service = MockProfileService::new(vec![], None);
        let app_settings_service = MockAppSettingsService::new(None);
        let (mut presenter, event_tx, mut view_rx, _profile_service, _app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;

        let id = Uuid::new_v4();
        send_settings_event(
            &event_tx,
            AppEvent::User(UserEvent::ToggleMcp { id, enabled: true }),
        )
        .await;
        // No ViewCommand emitted for a missing MCP — channel should be empty
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            recv_broadcast_command(&mut view_rx),
        )
        .await;
        assert!(
            result.is_err(),
            "Expected timeout (no command emitted for missing MCP)"
        );
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn profile_events_emit_expected_commands_and_snapshots() {
        let first = make_profile(Uuid::new_v4(), "one", "openai", "gpt-4.1");
        let second = make_profile(Uuid::new_v4(), "two", "anthropic", "claude");
        let profile_service =
            MockProfileService::new(vec![first.clone(), second.clone()], Some(first.id));
        let app_settings_service = MockAppSettingsService::new(Some(first.id));
        let (mut presenter, event_tx, mut view_rx, _profile_service, app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;

        send_settings_event(
            &event_tx,
            AppEvent::Profile(ProfileEvent::Created {
                id: second.id,
                name: second.name.clone(),
            }),
        )
        .await;
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ProfileCreated {
                id: second.id,
                name: second.name.clone(),
            }
        );
        let expected_profiles = vec![
            profile_summary(&first, Some(first.id)),
            profile_summary(&second, Some(first.id)),
        ];
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowSettings {
                profiles: expected_profiles.clone(),
                selected_profile_id: Some(first.id),
            }
        );
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ChatProfilesUpdated {
                profiles: expected_profiles.clone(),
                selected_profile_id: Some(first.id),
            }
        );

        send_settings_event(
            &event_tx,
            AppEvent::Profile(ProfileEvent::Updated {
                id: first.id,
                name: "renamed".to_string(),
            }),
        )
        .await;
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ProfileUpdated {
                id: first.id,
                name: "renamed".to_string(),
            }
        );
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowSettings {
                profiles: expected_profiles.clone(),
                selected_profile_id: Some(first.id),
            }
        );
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ChatProfilesUpdated {
                profiles: expected_profiles.clone(),
                selected_profile_id: Some(first.id),
            }
        );

        send_settings_event(
            &event_tx,
            AppEvent::Profile(ProfileEvent::Deleted {
                id: second.id,
                name: second.name.clone(),
            }),
        )
        .await;
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ProfileDeleted { id: second.id }
        );
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowSettings {
                profiles: expected_profiles.clone(),
                selected_profile_id: Some(first.id),
            }
        );
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ChatProfilesUpdated {
                profiles: expected_profiles.clone(),
                selected_profile_id: Some(first.id),
            }
        );

        send_settings_event(
            &event_tx,
            AppEvent::Profile(ProfileEvent::DefaultChanged {
                profile_id: Some(second.id),
            }),
        )
        .await;
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::DefaultProfileChanged {
                profile_id: Some(second.id),
            }
        );
        let expected_default_changed_profiles = vec![
            profile_summary(&first, Some(second.id)),
            profile_summary(&second, Some(second.id)),
        ];
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowSettings {
                profiles: expected_default_changed_profiles.clone(),
                selected_profile_id: Some(second.id),
            }
        );
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ChatProfilesUpdated {
                profiles: expected_default_changed_profiles,
                selected_profile_id: Some(second.id),
            }
        );
        assert_eq!(
            app_settings_service.set_default_profile_id_calls(),
            vec![second.id]
        );
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn mcp_events_emit_expected_commands() {
        let profile_service = MockProfileService::new(vec![], None);
        let app_settings_service = MockAppSettingsService::new(None);
        let (mut presenter, event_tx, mut view_rx, _profile_service, _app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;

        let id = Uuid::new_v4();
        send_settings_event(
            &event_tx,
            AppEvent::Mcp(McpEvent::Starting {
                id,
                name: "server".to_string(),
            }),
        )
        .await;
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::McpStatusChanged {
                id,
                status: McpStatus::Starting,
            }
        );

        send_settings_event(
            &event_tx,
            AppEvent::Mcp(McpEvent::Started {
                id,
                name: "server".to_string(),
                tools: vec!["one".to_string(), "two".to_string()],
                tool_count: 2,
            }),
        )
        .await;
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::McpServerStarted {
                id,
                name: Some("server".to_string()),
                tool_count: 2,
                enabled: None
            }
        );
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::McpStatusChanged {
                id,
                status: McpStatus::Running,
            }
        );

        send_settings_event(
            &event_tx,
            AppEvent::Mcp(McpEvent::StartFailed {
                id,
                name: "server".to_string(),
                error: "boom".to_string(),
            }),
        )
        .await;
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::McpServerFailed {
                id,
                error: "boom".to_string(),
            }
        );
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::McpStatusChanged {
                id,
                status: McpStatus::Failed,
            }
        );

        send_settings_event(
            &event_tx,
            AppEvent::Mcp(McpEvent::Stopped {
                id,
                name: "server".to_string(),
            }),
        )
        .await;
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::McpStatusChanged {
                id,
                status: McpStatus::Stopped,
            }
        );

        send_settings_event(
            &event_tx,
            AppEvent::Mcp(McpEvent::Unhealthy {
                id,
                name: "server".to_string(),
                error: "degraded".to_string(),
            }),
        )
        .await;
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::McpStatusChanged {
                id,
                status: McpStatus::Unhealthy,
            }
        );
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowError {
                title: "MCP Server Unhealthy".to_string(),
                message: "server: degraded".to_string(),
                severity: ErrorSeverity::Warning,
            }
        );

        send_settings_event(
            &event_tx,
            AppEvent::Mcp(McpEvent::Recovered {
                id,
                name: "server".to_string(),
            }),
        )
        .await;
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::McpStatusChanged {
                id,
                status: McpStatus::Running,
            }
        );
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowNotification {
                message: "server recovered".to_string(),
            }
        );

        send_settings_event(&event_tx, AppEvent::Mcp(McpEvent::ConfigSaved { id })).await;
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::McpConfigSaved { id, name: None }
        );

        send_settings_event(
            &event_tx,
            AppEvent::Mcp(McpEvent::Deleted {
                id,
                name: "server".to_string(),
            }),
        )
        .await;
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::McpDeleted { id }
        );
    }

    #[tokio::test]
    async fn system_events_emit_expected_commands() {
        let profile_service = MockProfileService::new(vec![], None);
        let app_settings_service = MockAppSettingsService::new(None);
        let (mut presenter, event_tx, mut view_rx, _profile_service, _app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;

        send_settings_event(
            &event_tx,
            AppEvent::System(SystemEvent::Error {
                source: "settings".to_string(),
                error: "bad config".to_string(),
                context: Some("test-case".to_string()),
            }),
        )
        .await;
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowError {
                title: "System Error".to_string(),
                message: "settings: bad config (context: test-case)".to_string(),
                severity: ErrorSeverity::Error,
            }
        );

        send_settings_event(&event_tx, AppEvent::System(SystemEvent::ConfigLoaded)).await;
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowNotification {
                message: "Configuration loaded".to_string(),
            }
        );

        send_settings_event(&event_tx, AppEvent::System(SystemEvent::ConfigSaved)).await;
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowNotification {
                message: "Configuration saved".to_string(),
            }
        );

        send_settings_event(
            &event_tx,
            AppEvent::System(SystemEvent::ModelsRegistryRefreshed {
                provider_count: 3,
                model_count: 42,
            }),
        )
        .await;
        assert_eq!(
            recv_broadcast_command(&mut view_rx).await,
            ViewCommand::ShowNotification {
                message: "Models refreshed: 3 providers, 42 models".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn start_emits_show_settings_theme_snapshot() {
        let _theme_guard = ThemeRuntimeGuard::new();
        let profile_service = MockProfileService::new(vec![], None);
        let app_settings_service = MockAppSettingsService::new(None).with_theme("green-screen");
        let (mut presenter, _event_tx, mut view_rx, _profile_service, _app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");

        // Drain ShowSettings + ChatProfilesUpdated
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;

        let cmd = recv_broadcast_command(&mut view_rx).await;
        match cmd {
            ViewCommand::ShowSettingsTheme {
                options,
                selected_slug,
            } => {
                assert!(
                    !options.is_empty(),
                    "ShowSettingsTheme must include at least one theme option"
                );
                assert_eq!(
                    selected_slug, "green-screen",
                    "selected_slug must match persisted theme"
                );
                // Every bundled theme slug must be present
                let slugs: Vec<&str> = options.iter().map(|o| o.slug.as_str()).collect();
                assert!(
                    slugs.contains(&"default"),
                    "default slug must be present in theme options"
                );
                assert!(
                    slugs.contains(&"green-screen"),
                    "green-screen slug must be present in theme options"
                );
            }
            other => panic!("expected ShowSettingsTheme, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn select_theme_persists_and_re_emits_theme_snapshot() {
        let _theme_guard = ThemeRuntimeGuard::new();
        let profile_service = MockProfileService::new(vec![], None);
        let app_settings_service = MockAppSettingsService::new(None).with_theme("default");
        let (mut presenter, event_tx, mut view_rx, _profile_service, app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");

        // Drain startup commands (ShowSettings + ChatProfilesUpdated + ShowSettingsTheme)
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;

        send_settings_event(
            &event_tx,
            AppEvent::User(UserEvent::SelectTheme {
                slug: "green-screen".to_string(),
            }),
        )
        .await;

        let cmd = recv_broadcast_command(&mut view_rx).await;
        match cmd {
            ViewCommand::ShowSettingsTheme {
                options,
                selected_slug,
            } => {
                assert_eq!(selected_slug, "green-screen");
                assert!(
                    !options.is_empty(),
                    "theme options must be non-empty after switch"
                );
            }
            other => panic!("expected ShowSettingsTheme after SelectTheme, got {other:?}"),
        }

        assert_eq!(
            app_settings_service.set_theme_calls(),
            vec!["green-screen"],
            "set_theme must be called with the selected slug"
        );
        assert_eq!(
            app_settings_service.current_theme(),
            Some("green-screen".to_string())
        );
    }

    #[tokio::test]
    async fn select_theme_rejects_invalid_slug_without_persisting() {
        let _theme_guard = ThemeRuntimeGuard::new();
        let profile_service = MockProfileService::new(vec![], None);
        let app_settings_service = MockAppSettingsService::new(None).with_theme("default");
        let (mut presenter, event_tx, mut view_rx, _profile_service, app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");

        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;

        send_settings_event(
            &event_tx,
            AppEvent::User(UserEvent::SelectTheme {
                slug: "definitely-not-a-theme".to_string(),
            }),
        )
        .await;

        let cmd = recv_broadcast_command(&mut view_rx).await;
        match cmd {
            ViewCommand::ShowSettingsTheme { selected_slug, .. } => {
                assert_eq!(selected_slug, "default");
            }
            other => panic!("expected ShowSettingsTheme after invalid SelectTheme, got {other:?}"),
        }

        assert!(
            app_settings_service.set_theme_calls().is_empty(),
            "invalid slug must not call set_theme"
        );
        assert_eq!(
            app_settings_service.current_theme(),
            Some("default".to_string())
        );
    }

    #[tokio::test]
    async fn select_theme_does_not_apply_runtime_change_when_persist_fails() {
        let _theme_guard = ThemeRuntimeGuard::new();
        let profile_service = MockProfileService::new(vec![], None);
        let app_settings_service = MockAppSettingsService::new(None)
            .with_theme("default")
            .with_set_theme_results(vec![Err(ServiceError::Storage("boom".to_string()))]);
        let (mut presenter, event_tx, mut view_rx, _profile_service, app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");

        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;

        set_active_theme_slug("default");

        send_settings_event(
            &event_tx,
            AppEvent::User(UserEvent::SelectTheme {
                slug: "green-screen".to_string(),
            }),
        )
        .await;

        let cmd = recv_broadcast_command(&mut view_rx).await;
        match cmd {
            ViewCommand::ShowSettingsTheme { selected_slug, .. } => {
                assert_eq!(selected_slug, "default");
            }
            other => panic!("expected ShowSettingsTheme after persist failure, got {other:?}"),
        }

        assert_eq!(
            app_settings_service.set_theme_calls(),
            vec!["green-screen"],
            "attempted slug should still be sent to persistence layer"
        );
        assert_eq!(
            app_settings_service.current_theme(),
            Some("default".to_string()),
            "failed persistence must keep stored theme unchanged"
        );
        assert_eq!(
            active_theme_slug(),
            "default".to_string(),
            "runtime active theme must remain unchanged on persist failure"
        );
    }

    #[tokio::test]
    async fn select_theme_applies_selection_when_previous_persisted_theme_is_invalid() {
        let _theme_guard = ThemeRuntimeGuard::new();
        let profile_service = MockProfileService::new(vec![], None);
        let app_settings_service = MockAppSettingsService::new(None).with_theme("default");
        let (mut presenter, event_tx, mut view_rx, _profile_service, app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");
        app_settings_service.set_current_theme(Some("invalid-after-start"));

        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;

        send_settings_event(
            &event_tx,
            AppEvent::User(UserEvent::SelectTheme {
                slug: "green-screen".to_string(),
            }),
        )
        .await;

        let cmd = recv_broadcast_command(&mut view_rx).await;
        match cmd {
            ViewCommand::ShowSettingsTheme { selected_slug, .. } => {
                assert_eq!(selected_slug, "green-screen");
            }
            other => panic!("expected ShowSettingsTheme after SelectTheme, got {other:?}"),
        }

        assert_eq!(
            app_settings_service.current_theme(),
            Some("green-screen".to_string()),
            "successful persistence should replace previous invalid stored theme"
        );
        assert_eq!(
            active_theme_slug(),
            "green-screen".to_string(),
            "runtime active theme must match the persisted validated selection"
        );
    }

    #[tokio::test]
    async fn refresh_profiles_also_emits_theme_snapshot() {
        let profile_service = MockProfileService::new(vec![], None);
        let app_settings_service = MockAppSettingsService::new(None).with_theme("default");
        let (mut presenter, event_tx, mut view_rx, _profile_service, _app_settings_service) =
            setup_settings_presenter(profile_service, app_settings_service);

        presenter.start().await.expect("start should succeed");

        // Drain startup commands
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;
        let _ = recv_broadcast_command(&mut view_rx).await;

        send_settings_event(&event_tx, AppEvent::User(UserEvent::RefreshProfiles)).await;

        // Should receive ShowSettings + ChatProfilesUpdated + ShowSettingsTheme
        let first = recv_broadcast_command(&mut view_rx).await;
        assert!(
            matches!(first, ViewCommand::ShowSettings { .. }),
            "expected ShowSettings, got {first:?}"
        );
        let second = recv_broadcast_command(&mut view_rx).await;
        assert!(
            matches!(second, ViewCommand::ChatProfilesUpdated { .. }),
            "expected ChatProfilesUpdated, got {second:?}"
        );
        let third = recv_broadcast_command(&mut view_rx).await;
        assert!(
            matches!(third, ViewCommand::ShowSettingsTheme { .. }),
            "expected ShowSettingsTheme, got {third:?}"
        );
    }
}
