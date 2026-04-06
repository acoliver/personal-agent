//! Presenter regression tests for conversation selection and settings profile actions.
//!
//! These tests cover wiring that is hard to verify from script greps alone:
//! - `SelectConversation` should emit `ConversationActivated` and replay stored messages
//! - `EditProfile` should emit `ProfileEditorLoad`
//! - `DeleteProfile` should emit `ProfileDeleted`

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use personal_agent::events::{bus::EventBus, types::UserEvent, AppEvent};
use personal_agent::models::{
    AuthConfig, ContextState, Conversation, ConversationMetadata, Message,
    MessageRole as DomainMessageRole, ModelParameters, ModelProfile, SearchResult,
};
use personal_agent::presentation::{
    chat_presenter::ChatPresenter,
    settings_presenter::SettingsPresenter,
    view_command::{MessageRole, ViewCommand, ViewId},
};
use personal_agent::services::{
    AppSettingsService, ChatService, ConversationService, ProfileService, ServiceError,
};

// ─────────────────────────────────────────────────────────────────────────────
// ChatPresenter conversation selection test doubles
// ─────────────────────────────────────────────────────────────────────────────

struct SelectConversationService {
    id: Uuid,
    messages: Vec<Message>,
}

#[async_trait]
impl ConversationService for SelectConversationService {
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

    async fn list_metadata(
        &self,
        _limit: Option<usize>,
        _offset: Option<usize>,
    ) -> Result<Vec<ConversationMetadata>, ServiceError> {
        Ok(vec![])
    }

    async fn add_message(
        &self,
        _conversation_id: Uuid,
        _message: Message,
    ) -> Result<Message, ServiceError> {
        Err(ServiceError::NotFound("not implemented".to_string()))
    }

    async fn search(
        &self,
        _query: &str,
        _limit: Option<usize>,
        _offset: Option<usize>,
    ) -> Result<Vec<SearchResult>, ServiceError> {
        Ok(vec![])
    }

    async fn message_count(&self, _conversation_id: Uuid) -> Result<usize, ServiceError> {
        Ok(0)
    }

    async fn update_context_state(
        &self,
        _id: Uuid,
        _state: &ContextState,
    ) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn get_context_state(&self, _id: Uuid) -> Result<Option<ContextState>, ServiceError> {
        Ok(None)
    }

    async fn rename(&self, _id: Uuid, _new_title: String) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn delete(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn set_active(&self, id: Uuid) -> Result<(), ServiceError> {
        if id == self.id {
            Ok(())
        } else {
            Err(ServiceError::NotFound("conversation not found".to_string()))
        }
    }

    async fn get_active(&self) -> Result<Option<Uuid>, ServiceError> {
        Ok(Some(self.id))
    }

    async fn get_messages(&self, conversation_id: Uuid) -> Result<Vec<Message>, ServiceError> {
        if conversation_id == self.id {
            Ok(self.messages.clone())
        } else {
            Err(ServiceError::NotFound("conversation not found".to_string()))
        }
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

struct MockChatService;

#[async_trait]
impl ChatService for MockChatService {
    async fn send_message(
        &self,
        _conversation_id: Uuid,
        _content: String,
    ) -> Result<
        Box<dyn futures::Stream<Item = personal_agent::services::ChatStreamEvent> + Send + Unpin>,
        ServiceError,
    > {
        Ok(Box::new(futures::stream::empty::<
            personal_agent::services::ChatStreamEvent,
        >()))
    }

    fn cancel(&self) {}

    fn is_streaming(&self) -> bool {
        false
    }

    async fn resolve_tool_approval(
        &self,
        _request_id: String,
        _decision: personal_agent::events::types::ToolApprovalResponseAction,
    ) -> Result<(), ServiceError> {
        Ok(())
    }
}

struct EmptyProfileService;

#[async_trait]
impl ProfileService for EmptyProfileService {
    async fn list(&self) -> Result<Vec<ModelProfile>, ServiceError> {
        Ok(vec![])
    }

    async fn get(&self, id: Uuid) -> Result<ModelProfile, ServiceError> {
        Err(ServiceError::NotFound(format!("profile {id} not found")))
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum Phase03SelectionObservation {
    Activated {
        id: Uuid,
    },
    ReplayLoaded {
        conversation_id: Uuid,
        message_count: usize,
    },
    Cleared,
    Other,
}

struct SelectionTracker {
    selected_id: Uuid,
    observations: Vec<Phase03SelectionObservation>,
}

impl SelectionTracker {
    const fn new(selected_id: Uuid) -> Self {
        Self {
            selected_id,
            observations: Vec::new(),
        }
    }

    fn record(&mut self, cmd: &ViewCommand) {
        let observation = match cmd {
            ViewCommand::ConversationActivated {
                id,
                selection_generation: _,
            } if *id == self.selected_id => Phase03SelectionObservation::Activated { id: *id },
            ViewCommand::ConversationMessagesLoaded {
                conversation_id,
                selection_generation: _,
                messages,
            } if *conversation_id == self.selected_id => {
                Phase03SelectionObservation::ReplayLoaded {
                    conversation_id: *conversation_id,
                    message_count: messages.len(),
                }
            }
            ViewCommand::ConversationCleared => Phase03SelectionObservation::Cleared,
            _ => Phase03SelectionObservation::Other,
        };

        self.observations.push(observation);
    }

    fn saw_activation(&self) -> bool {
        self.observations.iter().any(|observation| {
            matches!(
                observation,
                Phase03SelectionObservation::Activated { id } if *id == self.selected_id
            )
        })
    }

    fn replay_message_count(&self) -> Option<usize> {
        self.observations
            .iter()
            .find_map(|observation| match observation {
                Phase03SelectionObservation::ReplayLoaded {
                    conversation_id,
                    message_count,
                } if *conversation_id == self.selected_id => Some(*message_count),
                _ => None,
            })
    }

    fn saw_cleared(&self) -> bool {
        self.observations
            .iter()
            .any(|observation| matches!(observation, Phase03SelectionObservation::Cleared))
    }
}

fn spawn_mpsc_to_flume_view_command_bridge_for_test(
    mut rx: tokio::sync::mpsc::Receiver<ViewCommand>,
    tx: flume::Sender<ViewCommand>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            if tx.send(cmd).is_err() {
                break;
            }
        }
    })
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P05
/// @requirement REQ-INT-001.2
/// @requirement REQ-ARCH-003.4
/// @requirement REQ-ARCH-003.6
/// @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:001-063
fn assert_selection_generation_protocol(commands: &[ViewCommand]) {
    let mut activated_generation: Option<u64> = None;
    let mut loaded_generation: Option<u64> = None;

    for cmd in commands {
        match cmd {
            ViewCommand::ConversationActivated {
                selection_generation,
                ..
            } => {
                activated_generation = Some(*selection_generation);
            }
            ViewCommand::ConversationMessagesLoaded {
                selection_generation,
                ..
            } => {
                loaded_generation = Some(*selection_generation);
            }
            _ => {}
        }
    }

    assert!(
        activated_generation.is_some(),
        "selection_generation protocol: ConversationActivated must carry selection_generation"
    );
    assert!(
        loaded_generation.is_some(),
        "selection_generation protocol: ConversationMessagesLoaded must carry selection_generation"
    );
    assert_eq!(
        activated_generation, loaded_generation,
        "selection_generation protocol: generation token must be consistent across ConversationActivated and ConversationMessagesLoaded"
    );
}

#[tokio::test]
async fn select_conversation_emits_activation_and_replays_messages() {
    let selected_id = Uuid::new_v4();
    let conversation_service = Arc::new(SelectConversationService {
        id: selected_id,
        messages: vec![
            Message {
                role: DomainMessageRole::User,
                content: "first".to_string(),
                thinking_content: None,
                timestamp: Utc::now(),
                model_id: None,
                tool_calls: None,
                tool_results: None,
            },
            Message {
                role: DomainMessageRole::Assistant,
                content: "second".to_string(),
                thinking_content: None,
                timestamp: Utc::now(),
                model_id: None,
                tool_calls: None,
                tool_results: None,
            },
        ],
    }) as Arc<dyn ConversationService>;

    let chat_service = Arc::new(MockChatService) as Arc<dyn ChatService>;
    let profile_service = Arc::new(EmptyProfileService) as Arc<dyn ProfileService>;
    let app_settings = Arc::new(MockAppSettings) as Arc<dyn AppSettingsService>;

    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(64);

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service,
        profile_service,
        app_settings,
        view_tx,
    );
    presenter.start().await.expect("start chat presenter");

    tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;
    while view_rx.try_recv().is_ok() {}

    event_bus
        .publish(AppEvent::User(UserEvent::SelectConversation {
            id: selected_id,
            selection_generation: 0,
        }))
        .ok();

    tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;

    let mut saw_activation = false;
    let mut replayed = None;

    while let Ok(cmd) = view_rx.try_recv() {
        match cmd {
            ViewCommand::ConversationActivated {
                id,
                selection_generation: _,
            } => {
                if id == selected_id {
                    saw_activation = true;
                }
            }
            ViewCommand::ConversationMessagesLoaded {
                conversation_id,
                selection_generation: _,
                messages,
            } if conversation_id == selected_id => {
                replayed = Some(messages);
            }
            _ => {}
        }
    }

    let replayed = replayed.expect("Should replay stored messages via ConversationMessagesLoaded");
    assert!(
        saw_activation,
        "SelectConversation should emit ConversationActivated"
    );
    assert_eq!(
        replayed.len(),
        2,
        "Should replay all stored messages for selected conversation"
    );
    assert!(matches!(replayed[0].role, MessageRole::User));
    assert_eq!(replayed[0].content, "first");
    assert!(matches!(replayed[1].role, MessageRole::Assistant));
    assert_eq!(replayed[1].content, "second");
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P06
/// @requirement REQ-INT-001.2
/// @requirement REQ-ARCH-002.4
/// @requirement REQ-ARCH-004.3
/// @pseudocode analysis/pseudocode/03-main-panel-integration.md:009-013
#[tokio::test]
async fn startup_and_manual_selection_converge_on_one_authoritative_delivery_path() {
    use personal_agent::presentation::view_command::{
        ConversationMessagePayload, ConversationSummary,
    };
    use personal_agent::ui_gpui::app_store::ConversationLoadState;
    use personal_agent::ui_gpui::app_store::{
        BeginSelectionMode, GpuiAppStore, StartupInputs, StartupMode, StartupSelectedConversation,
        StartupTranscriptResult,
    };

    let conv_id = Uuid::new_v4();
    let messages = vec![
        ConversationMessagePayload {
            role: MessageRole::User,
            content: "hello".to_string(),
            thinking_content: None,
            timestamp: Some(1000),
        },
        ConversationMessagePayload {
            role: MessageRole::Assistant,
            content: "world".to_string(),
            thinking_content: None,
            timestamp: Some(2000),
        },
    ];

    // ── Startup path: build inputs and hydrate store ────────────────────
    let startup_inputs = StartupInputs {
        profiles: vec![],
        selected_profile_id: None,
        conversations: vec![ConversationSummary {
            id: conv_id,
            title: "Test Conversation".to_string(),
            updated_at: Utc::now(),
            message_count: 2,
            preview: None,
        }],
        selected_conversation: Some(StartupSelectedConversation {
            conversation_id: conv_id,
            mode: StartupMode::ModeA {
                transcript_result: StartupTranscriptResult::Success(messages.clone()),
            },
        }),
    };

    let store = std::sync::Arc::new(GpuiAppStore::from_startup_inputs(startup_inputs));
    let startup_snap = store.current_snapshot();

    assert_eq!(startup_snap.chat.selected_conversation_id, Some(conv_id));
    assert_eq!(startup_snap.chat.transcript, messages);
    assert!(
        matches!(startup_snap.chat.load_state, ConversationLoadState::Ready { conversation_id, generation } if conversation_id == conv_id && generation == 1),
        "startup hydration should produce Ready with generation 1"
    );
    let startup_generation = startup_snap.chat.selection_generation;
    assert_eq!(startup_generation, 1, "startup generation should be 1");

    // ── Runtime path: manual selection through the same store ────────────
    let other_id = Uuid::new_v4();
    let other_messages = vec![ConversationMessagePayload {
        role: MessageRole::User,
        content: "runtime message".to_string(),
        thinking_content: None,
        timestamp: Some(3000),
    }];

    let result = store.begin_selection(other_id, BeginSelectionMode::PublishImmediately);
    assert!(
        matches!(result, personal_agent::ui_gpui::app_store::BeginSelectionResult::BeganSelection { generation } if generation == 2),
        "runtime begin_selection should mint generation 2"
    );

    let runtime_snap_loading = store.current_snapshot();
    assert_eq!(
        runtime_snap_loading.chat.selected_conversation_id,
        Some(other_id)
    );
    assert!(
        matches!(runtime_snap_loading.chat.load_state, ConversationLoadState::Loading { conversation_id, generation } if conversation_id == other_id && generation == 2),
        "runtime selection should enter Loading"
    );

    store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
        conversation_id: other_id,
        selection_generation: 2,
        messages: other_messages.clone(),
    }]);

    let runtime_snap_ready = store.current_snapshot();
    assert_eq!(
        runtime_snap_ready.chat.selected_conversation_id,
        Some(other_id)
    );
    assert_eq!(runtime_snap_ready.chat.transcript, other_messages);
    assert!(
        matches!(runtime_snap_ready.chat.load_state, ConversationLoadState::Ready { conversation_id, generation } if conversation_id == other_id && generation == 2),
        "runtime selection should reach Ready with generation 2"
    );

    // ── Convergence proof: both paths use the same store reducer ─────────
    // Startup produced Ready(gen=1), runtime produced Ready(gen=2).
    // Both used begin_selection + ConversationMessagesLoaded through the same
    // authoritative store. No legacy build_startup_view_commands or
    // apply_startup_commands replay was needed for chat state.
    let source = include_str!("../src/main_gpui.rs");
    assert!(
        !source.contains("build_startup_view_commands"),
        "convergence: legacy build_startup_view_commands should no longer exist"
    );
    assert!(
        source.contains("build_startup_inputs"),
        "convergence: startup should use structured inputs"
    );
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P05
/// @requirement REQ-INT-001.2
/// @requirement REQ-ARCH-003.4
/// @requirement REQ-ARCH-003.6
/// @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:001-063
#[tokio::test]
async fn selection_generation_protocol_is_present() {
    let selected_id = Uuid::new_v4();
    let conversation_service = Arc::new(SelectConversationService {
        id: selected_id,
        messages: vec![Message {
            role: DomainMessageRole::User,
            content: "first".to_string(),
            thinking_content: None,
            timestamp: Utc::now(),
            model_id: None,
            tool_calls: None,
            tool_results: None,
        }],
    }) as Arc<dyn ConversationService>;

    let chat_service = Arc::new(MockChatService) as Arc<dyn ChatService>;
    let profile_service = Arc::new(EmptyProfileService) as Arc<dyn ProfileService>;
    let app_settings = Arc::new(MockAppSettings) as Arc<dyn AppSettingsService>;

    let event_bus = Arc::new(EventBus::new(64));
    let (user_tx, user_rx) = flume::bounded::<UserEvent>(16);
    let (flume_view_tx, flume_view_rx) = flume::bounded::<ViewCommand>(16);
    let (presenter_view_tx, presenter_view_rx) = mpsc::channel::<ViewCommand>(16);
    let bridge = personal_agent::ui_gpui::bridge::GpuiBridge::new(user_tx, flume_view_rx);
    let _forwarder =
        personal_agent::ui_gpui::bridge::spawn_user_event_forwarder(event_bus.clone(), user_rx);
    let _view_bridge =
        spawn_mpsc_to_flume_view_command_bridge_for_test(presenter_view_rx, flume_view_tx);

    let mut presenter = ChatPresenter::new(
        event_bus,
        conversation_service,
        chat_service,
        profile_service,
        app_settings,
        presenter_view_tx,
    );
    presenter.start().await.expect("start chat presenter");

    tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;
    let _ = bridge.drain_commands();

    assert!(
        bridge.emit(UserEvent::SelectConversation {
            id: selected_id,
            selection_generation: 0
        }),
        "precondition: bridge should still accept SelectConversation for runtime delivery"
    );

    tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;

    let commands = bridge.drain_commands();
    let mut tracker = SelectionTracker::new(selected_id);
    for command in &commands {
        tracker.record(command);
    }

    assert!(
        tracker.saw_activation(),
        "precondition: current presenter/bridge harness should still emit ConversationActivated for the selected conversation"
    );
    assert_eq!(
        tracker.replay_message_count(),
        Some(1),
        "precondition: current presenter/bridge harness should still bulk replay the selected conversation"
    );
    assert!(
        !tracker.saw_cleared(),
        "precondition: current presenter/bridge harness should not emit ConversationCleared"
    );

    assert_selection_generation_protocol(&commands);
}

// ─────────────────────────────────────────────────────────────────────────────
// SettingsPresenter profile edit/delete test doubles
// ─────────────────────────────────────────────────────────────────────────────

struct MockProfileServiceForSettings {
    profile: ModelProfile,
}

#[async_trait]
impl ProfileService for MockProfileServiceForSettings {
    async fn list(&self) -> Result<Vec<ModelProfile>, ServiceError> {
        Ok(vec![self.profile.clone()])
    }

    async fn get(&self, id: Uuid) -> Result<ModelProfile, ServiceError> {
        if id == self.profile.id {
            Ok(self.profile.clone())
        } else {
            Err(ServiceError::NotFound("profile not found".to_string()))
        }
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

    async fn delete(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn test_connection(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn get_default(&self) -> Result<Option<ModelProfile>, ServiceError> {
        Ok(Some(self.profile.clone()))
    }

    async fn set_default(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }
}

struct MockAppSettings;

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

#[tokio::test]
async fn edit_profile_emits_profile_editor_load_with_existing_data() {
    let profile_id = Uuid::new_v4();
    let profile = ModelProfile {
        id: profile_id,
        name: "Existing Profile".to_string(),
        provider_id: "anthropic".to_string(),
        model_id: "claude-sonnet-4-20250514".to_string(),
        base_url: "https://api.anthropic.com/v1".to_string(),
        auth: AuthConfig::Keychain {
            label: "sk-test".to_string(),
        },
        parameters: ModelParameters::default(),
        system_prompt: "system".to_string(),
        context_window_size: 128_000,
    };

    let profile_service =
        Arc::new(MockProfileServiceForSettings { profile }) as Arc<dyn ProfileService>;
    let app_settings = Arc::new(MockAppSettings) as Arc<dyn AppSettingsService>;

    let (event_tx, _) = broadcast::channel::<AppEvent>(64);
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(64);

    let skills_service = Arc::new(
        personal_agent::services::SkillsServiceImpl::new_for_tests(
            app_settings.clone(),
            std::path::PathBuf::from("/tmp/nonexistent-bundled-skills"),
            std::env::temp_dir().join(format!(
                "presenter-selection-settings-skills-{}",
                uuid::Uuid::new_v4()
            )),
        )
        .expect("skills service should initialize for tests"),
    ) as Arc<dyn personal_agent::services::SkillsService>;
    let mut presenter = SettingsPresenter::new(
        profile_service,
        app_settings,
        skills_service,
        &event_tx,
        view_tx,
    );
    presenter.start().await.expect("start settings presenter");

    event_tx
        .send(AppEvent::User(UserEvent::EditProfile { id: profile_id }))
        .ok();

    tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;

    let mut saw_prefill = false;
    let mut saw_nav = false;
    while let Ok(cmd) = view_rx.try_recv() {
        match cmd {
            ViewCommand::ProfileEditorLoad {
                id,
                name,
                provider_id,
                model_id,
                ..
            } => {
                if id == profile_id {
                    assert_eq!(name, "Existing Profile");
                    assert_eq!(provider_id, "anthropic");
                    assert_eq!(model_id, "claude-sonnet-4-20250514");
                    saw_prefill = true;
                }
            }
            ViewCommand::NavigateTo { view } => {
                if matches!(view, ViewId::ProfileEditor) {
                    saw_nav = true;
                }
            }
            _ => {}
        }
    }

    assert!(
        saw_prefill,
        "EditProfile should emit ProfileEditorLoad prefill command"
    );
    assert!(saw_nav, "EditProfile should navigate to ProfileEditor");
}

#[tokio::test]
async fn delete_profile_emits_profile_deleted_command() {
    let profile_id = Uuid::new_v4();
    let profile = ModelProfile {
        id: profile_id,
        name: "DeleteMe".to_string(),
        provider_id: "openai".to_string(),
        model_id: "gpt-4o".to_string(),
        base_url: "https://api.openai.com/v1".to_string(),
        auth: AuthConfig::Keychain {
            label: "sk-test".to_string(),
        },
        parameters: ModelParameters::default(),
        system_prompt: "system".to_string(),
        context_window_size: 128_000,
    };

    let profile_service =
        Arc::new(MockProfileServiceForSettings { profile }) as Arc<dyn ProfileService>;
    let app_settings = Arc::new(MockAppSettings) as Arc<dyn AppSettingsService>;

    let (event_tx, _) = broadcast::channel::<AppEvent>(64);
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(64);

    let skills_service = Arc::new(
        personal_agent::services::SkillsServiceImpl::new_for_tests(
            app_settings.clone(),
            std::path::PathBuf::from("/tmp/nonexistent-bundled-skills"),
            std::env::temp_dir().join(format!(
                "presenter-selection-settings-skills-{}",
                uuid::Uuid::new_v4()
            )),
        )
        .expect("skills service should initialize for tests"),
    ) as Arc<dyn personal_agent::services::SkillsService>;
    let mut presenter = SettingsPresenter::new(
        profile_service,
        app_settings,
        skills_service,
        &event_tx,
        view_tx,
    );
    presenter.start().await.expect("start settings presenter");

    event_tx
        .send(AppEvent::User(UserEvent::DeleteProfile { id: profile_id }))
        .ok();

    tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;

    let mut saw_deleted = false;
    while let Ok(cmd) = view_rx.try_recv() {
        if let ViewCommand::ProfileDeleted { id } = cmd {
            if id == profile_id {
                saw_deleted = true;
            }
        }
    }

    assert!(
        saw_deleted,
        "DeleteProfile should emit ProfileDeleted for settings view refresh"
    );
}
