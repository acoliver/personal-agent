use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use async_trait::async_trait;
use chrono::Utc;
use futures::stream;
use std::sync::LazyLock;
use tokio::sync::{mpsc, Mutex};

use uuid::Uuid;

use personal_agent::events::{
    bus::EventBus,
    types::{ChatEvent, ConversationEvent, UserEvent},
    AppEvent,
};
use personal_agent::models::{
    AuthConfig, ContextState, Conversation, ConversationExportFormat, ConversationMetadata,
    Message, MessageRole as DomainMessageRole, ModelParameters, ModelProfile, SearchResult,
};
use personal_agent::presentation::{
    chat_presenter::ChatPresenter,
    view_command::{ErrorSeverity, MessageRole, ViewCommand},
};
use personal_agent::services::{
    AppSettingsService, ChatService, ChatStreamEvent, ConversationService, ProfileService,
    ServiceError,
};

static ERROR_LOG_TEST_MUTEX: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));

struct MockConversationService {
    conversations: Mutex<Vec<Conversation>>,
    active_id: Mutex<Option<Uuid>>,
    fail_get_active: AtomicBool,
    fail_create: Mutex<Option<String>>,
    fail_rename: Mutex<Option<String>>,
    fail_set_active: Mutex<Option<String>>,
    fail_get_messages: Mutex<Option<String>>,
}

impl MockConversationService {
    fn new(conversations: Vec<Conversation>, active_id: Option<Uuid>) -> Self {
        Self {
            conversations: Mutex::new(conversations),
            active_id: Mutex::new(active_id),
            fail_get_active: AtomicBool::new(false),
            fail_create: Mutex::new(None),
            fail_rename: Mutex::new(None),
            fail_set_active: Mutex::new(None),
            fail_get_messages: Mutex::new(None),
        }
    }

    async fn set_fail_create(&self, message: &str) {
        *self.fail_create.lock().await = Some(message.to_string());
    }

    async fn set_fail_rename(&self, message: &str) {
        *self.fail_rename.lock().await = Some(message.to_string());
    }

    async fn set_fail_set_active(&self, message: &str) {
        *self.fail_set_active.lock().await = Some(message.to_string());
    }

    async fn set_fail_get_messages(&self, message: &str) {
        *self.fail_get_messages.lock().await = Some(message.to_string());
    }
}

#[async_trait]
impl ConversationService for MockConversationService {
    async fn create(
        &self,
        title: Option<String>,
        model_profile_id: Uuid,
    ) -> Result<Conversation, ServiceError> {
        let fail_create = self.fail_create.lock().await.clone();
        if let Some(message) = fail_create {
            return Err(ServiceError::Internal(message));
        }

        let mut conversation = Conversation::new(model_profile_id);
        if let Some(title) = title {
            conversation.set_title(title);
        }

        self.conversations.lock().await.push(conversation.clone());
        Ok(conversation)
    }

    async fn load(&self, id: Uuid) -> Result<Conversation, ServiceError> {
        self.conversations
            .lock()
            .await
            .iter()
            .find(|conversation| conversation.id == id)
            .cloned()
            .ok_or_else(|| ServiceError::NotFound(format!("conversation {id} not found")))
    }

    async fn list_metadata(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ConversationMetadata>, ServiceError> {
        let convs = self.conversations.lock().await;
        let o = offset.unwrap_or(0);
        let l = limit.unwrap_or(convs.len());
        let end = std::cmp::min(o + l, convs.len());
        if o >= convs.len() {
            return Ok(Vec::new());
        }
        Ok(convs[o..end]
            .iter()
            .map(|c| ConversationMetadata {
                id: c.id,
                title: c.title.clone(),
                profile_id: Some(c.profile_id),
                created_at: c.created_at,
                updated_at: c.updated_at,
                message_count: c.messages.len(),
                last_message_preview: c
                    .messages
                    .last()
                    .map(|m| m.content.chars().take(100).collect()),
            })
            .collect())
    }

    #[allow(clippy::significant_drop_tightening)]
    async fn add_message(
        &self,
        conversation_id: Uuid,
        message: Message,
    ) -> Result<Message, ServiceError> {
        let mut conversations = self.conversations.lock().await;
        let conversation = conversations
            .iter_mut()
            .find(|conversation| conversation.id == conversation_id)
            .ok_or_else(|| ServiceError::NotFound("missing conversation".to_string()))?;
        conversation.add_message(message.clone());
        Ok(message)
    }

    async fn search(
        &self,
        _query: &str,
        _limit: Option<usize>,
        _offset: Option<usize>,
    ) -> Result<Vec<SearchResult>, ServiceError> {
        Ok(vec![])
    }

    async fn message_count(&self, conversation_id: Uuid) -> Result<usize, ServiceError> {
        let count = {
            let convs = self.conversations.lock().await;
            convs
                .iter()
                .find(|c| c.id == conversation_id)
                .ok_or_else(|| ServiceError::NotFound("missing conversation".to_string()))?
                .messages
                .len()
        };
        Ok(count)
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

    #[allow(clippy::significant_drop_tightening)]
    async fn rename(&self, id: Uuid, new_title: String) -> Result<(), ServiceError> {
        let fail_rename = self.fail_rename.lock().await.clone();
        if let Some(message) = fail_rename {
            return Err(ServiceError::Internal(message));
        }

        {
            let mut conversations = self.conversations.lock().await;
            let conversation = conversations
                .iter_mut()
                .find(|conversation| conversation.id == id)
                .ok_or_else(|| ServiceError::NotFound("missing conversation".to_string()))?;
            conversation.set_title(new_title);
        }
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<(), ServiceError> {
        self.conversations
            .lock()
            .await
            .retain(|conversation| conversation.id != id);
        Ok(())
    }

    async fn set_active(&self, id: Uuid) -> Result<(), ServiceError> {
        let fail_set_active = self.fail_set_active.lock().await.clone();
        if let Some(message) = fail_set_active {
            return Err(ServiceError::Internal(message));
        }

        let conversations = self.conversations.lock().await;
        if !conversations
            .iter()
            .any(|conversation| conversation.id == id)
        {
            return Err(ServiceError::NotFound(format!(
                "conversation {id} not found"
            )));
        }
        drop(conversations);

        *self.active_id.lock().await = Some(id);
        Ok(())
    }

    async fn get_active(&self) -> Result<Option<Uuid>, ServiceError> {
        if self.fail_get_active.load(Ordering::SeqCst) {
            return Err(ServiceError::Internal("active lookup failed".to_string()));
        }
        Ok(*self.active_id.lock().await)
    }

    #[allow(clippy::significant_drop_tightening)]
    async fn get_messages(&self, conversation_id: Uuid) -> Result<Vec<Message>, ServiceError> {
        let fail_get_messages = self.fail_get_messages.lock().await.clone();
        if let Some(message) = fail_get_messages {
            return Err(ServiceError::Internal(message));
        }

        let messages = {
            let conversations = self.conversations.lock().await;
            let conversation = conversations
                .iter()
                .find(|conversation| conversation.id == conversation_id)
                .ok_or_else(|| ServiceError::NotFound("missing conversation".to_string()))?;
            conversation.messages.clone()
        };
        Ok(messages)
    }

    #[allow(clippy::significant_drop_tightening)]
    async fn update(
        &self,
        id: Uuid,
        title: Option<String>,
        model_profile_id: Option<Uuid>,
    ) -> Result<Conversation, ServiceError> {
        let mut conversations = self.conversations.lock().await;
        let conversation = conversations
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or_else(|| ServiceError::NotFound(format!("conversation {id} not found")))?;
        if let Some(t) = title {
            conversation.set_title(t);
        }
        if let Some(pid) = model_profile_id {
            conversation.profile_id = pid;
        }
        Ok(conversation.clone())
    }
}

struct MockChatService {
    fail_send: Mutex<Option<String>>,
    cancelled: AtomicBool,
}

impl MockChatService {
    fn new() -> Self {
        Self {
            fail_send: Mutex::new(None),
            cancelled: AtomicBool::new(false),
        }
    }

    async fn set_fail_send(&self, message: &str) {
        *self.fail_send.lock().await = Some(message.to_string());
    }
}

#[async_trait]
impl ChatService for MockChatService {
    async fn send_message(
        &self,
        _conversation_id: Uuid,
        _content: String,
    ) -> Result<Box<dyn futures::Stream<Item = ChatStreamEvent> + Send + Unpin>, ServiceError> {
        let fail_send = self.fail_send.lock().await.clone();
        if let Some(message) = fail_send {
            return Err(ServiceError::Internal(message));
        }

        Ok(Box::new(stream::empty::<ChatStreamEvent>()))
    }

    fn cancel(&self, _conversation_id: Uuid) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    fn is_streaming(&self) -> bool {
        false
    }

    fn is_streaming_for(&self, _conversation_id: Uuid) -> bool {
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

struct MockProfileService {
    default_profile: Mutex<Option<ModelProfile>>,
    fail_get_default: Mutex<Option<String>>,
}

impl MockProfileService {
    fn new(default_profile: Option<ModelProfile>) -> Self {
        Self {
            default_profile: Mutex::new(default_profile),
            fail_get_default: Mutex::new(None),
        }
    }

    async fn set_fail_get_default(&self, message: &str) {
        *self.fail_get_default.lock().await = Some(message.to_string());
    }
}

#[async_trait]
impl ProfileService for MockProfileService {
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
        let fail_get_default = self.fail_get_default.lock().await.clone();
        if let Some(message) = fail_get_default {
            return Err(ServiceError::Internal(message));
        }
        let default_profile = self.default_profile.lock().await.clone();
        Ok(default_profile)
    }

    async fn set_default(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }
}

fn profile() -> ModelProfile {
    ModelProfile::new(
        "Default".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: "chat-presenter-test".to_string(),
        },
    )
}

struct MockAppSettingsService {
    export_format: Mutex<Option<String>>,
    export_dir: Mutex<Option<String>>,
    fail_set_setting: AtomicBool,
}

impl MockAppSettingsService {
    fn new() -> Self {
        Self {
            export_format: Mutex::new(None),
            export_dir: Mutex::new(None),
            fail_set_setting: AtomicBool::new(false),
        }
    }

    async fn set_export_dir(&self, value: Option<PathBuf>) {
        *self.export_dir.lock().await = value.map(|path| path.to_string_lossy().to_string());
    }

    fn set_fail_set_setting(&self, value: bool) {
        self.fail_set_setting.store(value, Ordering::Relaxed);
    }
}

#[async_trait]
impl AppSettingsService for MockAppSettingsService {
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
        Ok(None)
    }

    async fn set_launch_at_login(&self, _enabled: bool) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn get_setting(&self, key: &str) -> Result<Option<String>, ServiceError> {
        if key == "chat.export.format" {
            return Ok(self.export_format.lock().await.clone());
        }
        if key == "chat.export.dir" {
            return Ok(self.export_dir.lock().await.clone());
        }
        Ok(None)
    }

    async fn set_setting(&self, key: &str, value: String) -> Result<(), ServiceError> {
        if key == "chat.export.format" {
            if self.fail_set_setting.load(Ordering::Relaxed) {
                return Err(ServiceError::Internal("persist failed".to_string()));
            }
            *self.export_format.lock().await = Some(value);
        }
        Ok(())
    }

    async fn reset_to_defaults(&self) -> Result<(), ServiceError> {
        Ok(())
    }
}

fn conversation_with_messages(profile_id: Uuid, messages: Vec<Message>) -> Conversation {
    let mut conversation = Conversation::new(profile_id);
    conversation.messages = messages;
    conversation.updated_at = Utc::now();
    conversation
}

async fn collect_commands(view_rx: &mut mpsc::Receiver<ViewCommand>) -> Vec<ViewCommand> {
    tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;
    let mut commands = Vec::new();
    while let Ok(command) = view_rx.try_recv() {
        commands.push(command);
    }
    commands
}

#[tokio::test]
async fn send_message_creates_conversation_and_appends_user_message() {
    let default_profile = profile();
    let profile_id = default_profile.id;
    let conversation_service = Arc::new(MockConversationService::new(vec![], None));
    let chat_service = Arc::new(MockChatService::new());
    let profile_service = Arc::new(MockProfileService::new(Some(default_profile)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel(64);
    let app_settings_service =
        Arc::new(MockAppSettingsService::new()) as Arc<dyn AppSettingsService>;

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service,
        profile_service,
        app_settings_service,
        view_tx,
    );
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::SendMessage {
            text: "hello world".to_string(),
        }))
        .expect("publish send event");

    let commands = collect_commands(&mut view_rx).await;

    let created = commands.iter().find_map(|command| match command {
        ViewCommand::ConversationCreated {
            id,
            profile_id: seen_profile_id,
        } => Some((*id, *seen_profile_id)),
        _ => None,
    });
    let (conversation_id, seen_profile_id) = created.expect("conversation created command");
    assert_eq!(seen_profile_id, profile_id);

    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ConversationActivated {
            id,
            selection_generation: 1
        } if *id == conversation_id
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::MessageAppended {
            conversation_id: seen_id,
            role: MessageRole::User,
            content,
            ..
        } if *seen_id == conversation_id && content == "hello world"
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowThinking { conversation_id: seen_id, .. } if *seen_id == conversation_id
    )));
}

#[tokio::test]
async fn send_message_reports_chat_service_errors_and_hides_thinking() {
    let default_profile = profile();
    let profile_id = default_profile.id;
    let conversation = conversation_with_messages(profile_id, vec![]);
    let conversation_id = conversation.id;
    let conversation_service = Arc::new(MockConversationService::new(
        vec![conversation],
        Some(conversation_id),
    ));
    let chat_service = Arc::new(MockChatService::new());
    chat_service.set_fail_send("send failed").await;
    let profile_service = Arc::new(MockProfileService::new(Some(default_profile)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel(64);
    let app_settings_service =
        Arc::new(MockAppSettingsService::new()) as Arc<dyn AppSettingsService>;

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service,
        profile_service,
        app_settings_service,
        view_tx,
    );
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::SendMessage {
            text: "boom".to_string(),
        }))
        .expect("publish send event");

    let commands = collect_commands(&mut view_rx).await;

    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::StreamError {
            conversation_id: seen_id,
            error,
            recoverable: false,
        } if *seen_id == conversation_id && error.contains("send failed")
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::HideThinking { conversation_id: seen_id } if *seen_id == conversation_id
    )));
}

#[tokio::test]
async fn send_message_reports_profile_resolution_errors() {
    let conversation_service = Arc::new(MockConversationService::new(vec![], None));
    let chat_service = Arc::new(MockChatService::new());
    let profile_service = Arc::new(MockProfileService::new(None));
    profile_service
        .set_fail_get_default("default profile missing")
        .await;
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel(64);
    let app_settings_service =
        Arc::new(MockAppSettingsService::new()) as Arc<dyn AppSettingsService>;

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service,
        profile_service,
        app_settings_service,
        view_tx,
    );
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::SendMessage {
            text: "hello".to_string(),
        }))
        .expect("publish send event");

    let commands = collect_commands(&mut view_rx).await;
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Error,
        } if title == "Conversation Error" && message.contains("default profile missing")
    )));
}

#[tokio::test]
async fn stop_streaming_invokes_chat_service_cancel() {
    let default_profile = profile();
    let profile_id = default_profile.id;
    let conversation = conversation_with_messages(profile_id, vec![]);
    let conversation_id = conversation.id;
    let conversation_service = Arc::new(MockConversationService::new(
        vec![conversation],
        Some(conversation_id),
    ));
    let chat_service = Arc::new(MockChatService::new());
    let profile_service = Arc::new(MockProfileService::new(Some(default_profile)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel(64);
    let app_settings_service =
        Arc::new(MockAppSettingsService::new()) as Arc<dyn AppSettingsService>;

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service.clone(),
        profile_service,
        app_settings_service,
        view_tx,
    );
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::StopStreaming { conversation_id }))
        .expect("publish stop event");
    let _ = collect_commands(&mut view_rx).await;

    assert!(chat_service.cancelled.load(Ordering::SeqCst));
}

#[tokio::test]
async fn new_conversation_creates_and_activates_conversation() {
    let default_profile = profile();
    let profile_id = default_profile.id;
    let conversation_service = Arc::new(MockConversationService::new(vec![], None));
    let chat_service = Arc::new(MockChatService::new());
    let profile_service = Arc::new(MockProfileService::new(Some(default_profile)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel(64);
    let app_settings_service =
        Arc::new(MockAppSettingsService::new()) as Arc<dyn AppSettingsService>;

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service,
        profile_service,
        app_settings_service,
        view_tx,
    );
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::NewConversation))
        .expect("publish new conversation event");

    let commands = collect_commands(&mut view_rx).await;

    let created_id = commands.iter().find_map(|command| match command {
        ViewCommand::ConversationCreated {
            id,
            profile_id: seen_profile_id,
        } => {
            assert_eq!(*seen_profile_id, profile_id);
            Some(*id)
        }
        _ => None,
    });
    let created_id = created_id.expect("conversation created");

    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ConversationActivated {
            id,
            selection_generation: 1
        } if *id == created_id
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ConversationListRefreshed { conversations } if conversations.iter().any(|c| c.id == created_id)
    )));
}

#[tokio::test]
async fn new_conversation_reports_creation_errors() {
    let default_profile = profile();
    let conversation_service = Arc::new(MockConversationService::new(vec![], None));
    conversation_service.set_fail_create("create failed").await;
    let chat_service = Arc::new(MockChatService::new());
    let profile_service = Arc::new(MockProfileService::new(Some(default_profile)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel(64);
    let app_settings_service =
        Arc::new(MockAppSettingsService::new()) as Arc<dyn AppSettingsService>;

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service,
        profile_service,
        app_settings_service,
        view_tx,
    );
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::NewConversation))
        .expect("publish new conversation event");

    let commands = collect_commands(&mut view_rx).await;
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Error,
        } if title == "Error" && message.contains("create failed")
    )));
}

#[tokio::test]
async fn rename_conversation_refreshes_history_after_success() {
    let default_profile = profile();
    let profile_id = default_profile.id;
    let conversation = conversation_with_messages(profile_id, vec![]);
    let conversation_id = conversation.id;
    let conversation_service = Arc::new(MockConversationService::new(
        vec![conversation],
        Some(conversation_id),
    ));
    let chat_service = Arc::new(MockChatService::new());
    let profile_service = Arc::new(MockProfileService::new(Some(default_profile)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel(64);
    let app_settings_service =
        Arc::new(MockAppSettingsService::new()) as Arc<dyn AppSettingsService>;

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service,
        profile_service,
        app_settings_service,
        view_tx,
    );
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::ConfirmRenameConversation {
            id: conversation_id,
            title: "Renamed Title".to_string(),
        }))
        .expect("publish rename event");

    let commands = collect_commands(&mut view_rx).await;
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ConversationRenamed { id, new_title }
            if *id == conversation_id && new_title == "Renamed Title"
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ConversationListRefreshed { conversations }
            if conversations.iter().any(|conversation| conversation.id == conversation_id && conversation.title == "Renamed Title")
    )));
}

#[tokio::test]
async fn rename_conversation_reports_errors() {
    let default_profile = profile();
    let profile_id = default_profile.id;
    let conversation = conversation_with_messages(profile_id, vec![]);
    let conversation_id = conversation.id;
    let conversation_service = Arc::new(MockConversationService::new(
        vec![conversation],
        Some(conversation_id),
    ));
    conversation_service.set_fail_rename("rename failed").await;
    let chat_service = Arc::new(MockChatService::new());
    let profile_service = Arc::new(MockProfileService::new(Some(default_profile)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel(64);
    let app_settings_service =
        Arc::new(MockAppSettingsService::new()) as Arc<dyn AppSettingsService>;

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service,
        profile_service,
        app_settings_service,
        view_tx,
    );
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::ConfirmRenameConversation {
            id: conversation_id,
            title: "Renamed Title".to_string(),
        }))
        .expect("publish rename event");

    let commands = collect_commands(&mut view_rx).await;
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Error,
        } if title == "Error" && message.contains("rename failed")
    )));
}

#[tokio::test]
async fn select_conversation_replays_messages_and_filters_system_messages() {
    let default_profile = profile();
    let profile_id = default_profile.id;
    let conversation = conversation_with_messages(
        profile_id,
        vec![
            Message::system("system note".to_string()),
            Message {
                role: DomainMessageRole::User,
                content: "hi".to_string(),
                thinking_content: None,
                timestamp: Utc::now(),
                model_id: None,
                tool_calls: None,
                tool_results: None,
            },
            Message {
                role: DomainMessageRole::Assistant,
                content: "hello".to_string(),
                thinking_content: Some("reasoning".to_string()),
                timestamp: Utc::now(),
                model_id: None,
                tool_calls: None,
                tool_results: None,
            },
        ],
    );
    let conversation_id = conversation.id;
    let conversation_service = Arc::new(MockConversationService::new(
        vec![conversation],
        Some(conversation_id),
    ));
    let chat_service = Arc::new(MockChatService::new());
    let profile_service = Arc::new(MockProfileService::new(Some(default_profile)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel(64);
    let app_settings_service =
        Arc::new(MockAppSettingsService::new()) as Arc<dyn AppSettingsService>;

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service,
        profile_service,
        app_settings_service,
        view_tx,
    );
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::SelectConversation {
            id: conversation_id,
            selection_generation: 7,
        }))
        .expect("publish select event");

    let commands = collect_commands(&mut view_rx).await;

    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ConversationActivated {
            id,
            selection_generation: 7
        } if *id == conversation_id
    )));

    let loaded_messages = commands.iter().find_map(|command| match command {
        ViewCommand::ConversationMessagesLoaded {
            conversation_id: seen_id,
            selection_generation: 7,
            messages,
        } if *seen_id == conversation_id => Some(messages.clone()),
        _ => None,
    });
    let loaded_messages = loaded_messages.expect("messages loaded");
    assert_eq!(loaded_messages.len(), 2);
    assert!(matches!(loaded_messages[0].role, MessageRole::User));
    assert_eq!(loaded_messages[0].content, "hi");
    assert!(matches!(loaded_messages[1].role, MessageRole::Assistant));
    assert_eq!(
        loaded_messages[1].thinking_content.as_deref(),
        Some("reasoning")
    );
}

#[tokio::test]
async fn select_conversation_reports_message_replay_failures() {
    let default_profile = profile();
    let profile_id = default_profile.id;
    let conversation = conversation_with_messages(profile_id, vec![]);
    let conversation_id = conversation.id;
    let conversation_service = Arc::new(MockConversationService::new(
        vec![conversation],
        Some(conversation_id),
    ));
    conversation_service
        .set_fail_get_messages("history failed")
        .await;
    let chat_service = Arc::new(MockChatService::new());
    let profile_service = Arc::new(MockProfileService::new(Some(default_profile)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel(64);
    let app_settings_service =
        Arc::new(MockAppSettingsService::new()) as Arc<dyn AppSettingsService>;

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service,
        profile_service,
        app_settings_service,
        view_tx,
    );
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::SelectConversation {
            id: conversation_id,
            selection_generation: 3,
        }))
        .expect("publish select event");

    let commands = collect_commands(&mut view_rx).await;

    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ConversationLoadFailed {
            conversation_id: seen_id,
            selection_generation: 3,
            message,
        } if *seen_id == conversation_id && message.contains("history failed")
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Error,
        } if title == "Error" && message.contains("history failed")
    )));
}

#[tokio::test]
async fn select_conversation_reports_activation_failures() {
    let default_profile = profile();
    let profile_id = default_profile.id;
    let conversation = conversation_with_messages(profile_id, vec![]);
    let conversation_id = conversation.id;
    let conversation_service = Arc::new(MockConversationService::new(
        vec![conversation],
        Some(conversation_id),
    ));
    conversation_service
        .set_fail_set_active("activation failed")
        .await;
    let chat_service = Arc::new(MockChatService::new());
    let profile_service = Arc::new(MockProfileService::new(Some(default_profile)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel(64);
    let app_settings_service =
        Arc::new(MockAppSettingsService::new()) as Arc<dyn AppSettingsService>;

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service,
        profile_service,
        app_settings_service,
        view_tx,
    );
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::SelectConversation {
            id: conversation_id,
            selection_generation: 9,
        }))
        .expect("publish select event");

    let commands = collect_commands(&mut view_rx).await;

    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ConversationLoadFailed {
            conversation_id: seen_id,
            selection_generation: 9,
            message,
        } if *seen_id == conversation_id && message.contains("activation failed")
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Error,
        } if title == "Error" && message.contains("activation failed")
    )));
}

#[tokio::test]
async fn conversation_events_map_to_expected_view_commands() {
    let default_profile = profile();
    let profile_id = default_profile.id;
    let conversation = conversation_with_messages(profile_id, vec![]);
    let conversation_id = conversation.id;
    let conversation_service = Arc::new(MockConversationService::new(
        vec![conversation],
        Some(conversation_id),
    ));
    let chat_service = Arc::new(MockChatService::new());
    let profile_service = Arc::new(MockProfileService::new(Some(default_profile)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel(128);
    let app_settings_service =
        Arc::new(MockAppSettingsService::new()) as Arc<dyn AppSettingsService>;

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service,
        profile_service,
        app_settings_service,
        view_tx,
    );
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    for event in [
        AppEvent::Conversation(ConversationEvent::Created {
            id: conversation_id,
            title: "Created".to_string(),
        }),
        AppEvent::Conversation(ConversationEvent::TitleUpdated {
            id: conversation_id,
            title: "Updated".to_string(),
        }),
        AppEvent::Conversation(ConversationEvent::Deleted {
            id: conversation_id,
        }),
        AppEvent::Conversation(ConversationEvent::Activated {
            id: conversation_id,
        }),
        AppEvent::Conversation(ConversationEvent::Loaded {
            id: conversation_id,
        }),
        AppEvent::Conversation(ConversationEvent::Deactivated),
        AppEvent::Conversation(ConversationEvent::ListRefreshed { count: 4 }),
    ] {
        event_bus
            .publish(event)
            .expect("publish conversation event");
    }

    let commands = collect_commands(&mut view_rx).await;

    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ConversationCreated { id, profile_id } if *id == conversation_id && *profile_id == conversation_id
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ConversationRenamed { id, new_title }
            if *id == conversation_id && new_title == "Updated"
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ConversationDeleted { id } if *id == conversation_id
    )));
    assert!(
        commands
            .iter()
            .filter(|command| matches!(
                command,
                ViewCommand::ConversationActivated {
                    id,
                    selection_generation: 0
                } if *id == conversation_id
            ))
            .count()
            >= 2
    );
    assert!(commands
        .iter()
        .any(|command| matches!(command, ViewCommand::ConversationCleared)));
    assert!(commands
        .iter()
        .any(|command| matches!(command, ViewCommand::HistoryUpdated { count: Some(4) })));
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn chat_events_surface_errors_and_completion_commands() {
    let default_profile = profile();
    let profile_id = default_profile.id;
    let conversation = conversation_with_messages(profile_id, vec![]);
    let conversation_id = conversation.id;
    let conversation_service = Arc::new(MockConversationService::new(
        vec![conversation],
        Some(conversation_id),
    ));
    let chat_service = Arc::new(MockChatService::new());
    let profile_service = Arc::new(MockProfileService::new(Some(default_profile)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel(128);
    let app_settings_service =
        Arc::new(MockAppSettingsService::new()) as Arc<dyn AppSettingsService>;

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service,
        profile_service,
        app_settings_service,
        view_tx,
    );
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    for event in [
        AppEvent::Chat(ChatEvent::StreamStarted {
            conversation_id,
            message_id: Uuid::new_v4(),
            model_id: "gpt-4o".to_string(),
        }),
        AppEvent::Chat(ChatEvent::TextDelta {
            conversation_id,
            text: "chunk".to_string(),
        }),
        AppEvent::Chat(ChatEvent::ThinkingDelta {
            conversation_id,
            text: "thought".to_string(),
        }),
        AppEvent::Chat(ChatEvent::ToolCallStarted {
            conversation_id,
            tool_call_id: "tool-1".to_string(),
            tool_name: "search".to_string(),
        }),
        AppEvent::Chat(ChatEvent::ToolCallCompleted {
            conversation_id,
            tool_call_id: "tool-1".to_string(),
            tool_name: "search".to_string(),
            success: false,
            result: "bad".to_string(),
            duration_ms: 25,
        }),
        AppEvent::Chat(ChatEvent::StreamCompleted {
            conversation_id,
            message_id: Uuid::new_v4(),
            total_tokens: Some(42),
        }),
        AppEvent::Chat(ChatEvent::StreamCancelled {
            conversation_id,
            message_id: Uuid::new_v4(),
            partial_content: "partial".to_string(),
        }),
        AppEvent::Chat(ChatEvent::StreamError {
            conversation_id,
            error: "recoverable".to_string(),
            recoverable: true,
            diagnostics: None,
        }),
        AppEvent::Chat(ChatEvent::MessageSaved {
            conversation_id,
            message_id: Uuid::new_v4(),
        }),
    ] {
        event_bus.publish(event).expect("publish chat event");
    }

    let commands = collect_commands(&mut view_rx).await;

    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowThinking { conversation_id: seen_id, .. } if *seen_id == conversation_id
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::AppendStream {
            conversation_id: seen_id,
            chunk,
        } if *seen_id == conversation_id && chunk == "chunk"
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::AppendThinking {
            conversation_id: seen_id,
            content,
        } if *seen_id == conversation_id && content == "thought"
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowToolCall {
            conversation_id: seen_id,
            tool_name,
            status,
        } if *seen_id == conversation_id && tool_name == "search" && status == "running"
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::UpdateToolCall {
            conversation_id: seen_id,
            tool_name,
            status,
            result,
            duration,
        } if *seen_id == conversation_id
            && tool_name == "search"
            && status == "failed"
            && result.as_deref() == Some("bad")
            && *duration == Some(25)
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::FinalizeStream {
            conversation_id: seen_id,
            tokens: 42,
        } if *seen_id == conversation_id
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::StreamCancelled {
            conversation_id: seen_id,
            partial_content,
        } if *seen_id == conversation_id && partial_content == "partial"
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::StreamError {
            conversation_id: seen_id,
            error,
            recoverable: true,
        } if *seen_id == conversation_id && error == "recoverable"
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Warning,
        } if title == "Stream Error" && message == "recoverable"
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::MessageSaved { conversation_id: seen_id } if *seen_id == conversation_id
    )));
}

#[tokio::test]
async fn save_conversation_uses_selected_format_when_setting_persist_fails() {
    let default_profile = profile();
    let profile_id = default_profile.id;

    let mut conversation = conversation_with_messages(
        profile_id,
        vec![Message::assistant(
            "Persist failure still exports markdown".to_string(),
        )],
    );
    conversation.title = Some("Sprint 2".to_string());
    let conversation_id = conversation.id;

    let conversation_service = Arc::new(MockConversationService::new(
        vec![conversation],
        Some(conversation_id),
    ));
    let chat_service = Arc::new(MockChatService::new());
    let profile_service = Arc::new(MockProfileService::new(Some(default_profile)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel(128);

    let app_settings = Arc::new(MockAppSettingsService::new());
    let export_dir = tempfile::tempdir().expect("temp export dir");
    app_settings
        .set_export_dir(Some(export_dir.path().to_path_buf()))
        .await;
    app_settings.set_fail_set_setting(true);
    let app_settings_service = app_settings.clone() as Arc<dyn AppSettingsService>;

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service,
        profile_service,
        app_settings_service,
        view_tx,
    );
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::SelectConversationExportFormat {
            format: ConversationExportFormat::Md,
        }))
        .expect("publish export format selection");
    let select_commands = collect_commands(&mut view_rx).await;

    assert!(select_commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Warning,
        } if title == "Export Format" && message == "Failed to persist export format preference"
    )));
    assert!(select_commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowConversationExportFormat { format }
            if *format == ConversationExportFormat::Md
    )));

    event_bus
        .publish(AppEvent::User(UserEvent::SaveConversation))
        .expect("publish save conversation");
    let save_commands = collect_commands(&mut view_rx).await;

    let export_completed = save_commands
        .iter()
        .find(|command| matches!(command, ViewCommand::ExportCompleted { .. }));
    let export_completed = export_completed.expect("save notification");
    match export_completed {
        ViewCommand::ExportCompleted { format_label, .. } => {
            assert_eq!(format_label, "MD");
        }
        _ => unreachable!(),
    }

    let export_path = export_dir
        .path()
        .read_dir()
        .expect("read export dir")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("md"))
        .expect("expected markdown export file");

    let body = std::fs::read_to_string(&export_path).expect("read export body");
    assert!(body.contains("# Sprint 2"));
    assert!(body.contains("Persist failure still exports markdown"));
}

// ── SelectChatProfile tests ──────────────────────────────────────────────────

#[tokio::test]
async fn select_chat_profile_updates_active_conversation_profile_id() {
    let default = profile();
    let kimi_profile_id = Uuid::new_v4();

    // Create a conversation bound to the default profile
    let conversation = Conversation::new(default.id);
    let conversation_id = conversation.id;

    let conv_service = Arc::new(MockConversationService::new(
        vec![conversation],
        Some(conversation_id),
    ));
    let chat_service = Arc::new(MockChatService::new());
    let profile_service = Arc::new(MockProfileService::new(Some(default)));
    let app_settings_service = Arc::new(MockAppSettingsService::new());
    let event_bus = Arc::new(EventBus::new(64));

    let (view_tx, mut _view_rx) = mpsc::channel::<ViewCommand>(100);
    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conv_service.clone(),
        chat_service,
        profile_service,
        app_settings_service,
        view_tx,
    );

    presenter.start().await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Simulate user selecting a different chat profile (e.g. Kimi)
    let _ = event_bus.publish(AppEvent::User(UserEvent::SelectChatProfile {
        id: kimi_profile_id,
    }));

    tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;

    // Verify the active conversation's profile_id was updated
    let updated = conv_service.load(conversation_id).await.unwrap();
    assert_eq!(
        updated.profile_id, kimi_profile_id,
        "active conversation should have its profile_id updated to the selected chat profile"
    );
}

#[tokio::test]
async fn select_chat_profile_without_active_conversation_is_harmless() {
    let default = profile();
    let kimi_profile_id = Uuid::new_v4();

    // No conversations, no active
    let conv_service = Arc::new(MockConversationService::new(vec![], None));
    let chat_service = Arc::new(MockChatService::new());
    let profile_service = Arc::new(MockProfileService::new(Some(default)));
    let app_settings_service = Arc::new(MockAppSettingsService::new());
    let event_bus = Arc::new(EventBus::new(64));

    let (view_tx, mut _view_rx) = mpsc::channel::<ViewCommand>(100);
    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conv_service.clone(),
        chat_service,
        profile_service,
        app_settings_service,
        view_tx,
    );

    presenter.start().await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Should not panic or error — just a no-op
    let _ = event_bus.publish(AppEvent::User(UserEvent::SelectChatProfile {
        id: kimi_profile_id,
    }));

    tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;

    // No crash, no active conversation = success
    assert!(conv_service.get_active().await.unwrap().is_none());
}

#[tokio::test]
async fn save_error_log_exports_txt_and_emits_completion() {
    let _error_log_lock = ERROR_LOG_TEST_MUTEX.lock().await;

    let default_profile = profile();
    let profile_id = default_profile.id;
    let conversation =
        conversation_with_messages(profile_id, vec![Message::assistant("context".to_string())]);

    let conversation_service = Arc::new(MockConversationService::new(vec![conversation], None));
    let chat_service = Arc::new(MockChatService::new());
    let profile_service = Arc::new(MockProfileService::new(Some(default_profile)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel(128);

    let app_settings = Arc::new(MockAppSettingsService::new());
    let export_dir = tempfile::tempdir().expect("temp export dir");
    app_settings
        .set_export_dir(Some(export_dir.path().to_path_buf()))
        .await;
    let app_settings_service = app_settings.clone() as Arc<dyn AppSettingsService>;

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service,
        profile_service,
        app_settings_service,
        view_tx,
    );
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    personal_agent::ui_gpui::error_log::ErrorLogStore::global().clear();
    personal_agent::ui_gpui::error_log::ErrorLogStore::global().push(|id| {
        personal_agent::ui_gpui::error_log::ErrorLogEntry {
            id,
            timestamp: Utc::now(),
            severity: personal_agent::ui_gpui::error_log::ErrorSeverityTag::Auth,
            source: "anthropic / claude".to_string(),
            message: "401 unauthorized".to_string(),
            raw_detail: Some("invalid_api_key".to_string()),
            conversation_title: Some("Bug report".to_string()),
            conversation_id: None,
            diagnostics: None,
        }
    });

    event_bus
        .publish(AppEvent::User(UserEvent::SaveErrorLog {
            format: personal_agent::models::ConversationExportFormat::Txt,
        }))
        .expect("publish save error log");
    let commands = collect_commands(&mut view_rx).await;

    let export_completed = commands
        .iter()
        .find(|command| matches!(command, ViewCommand::ErrorLogExportCompleted { .. }))
        .expect("error log export completion command");

    let export_path = match export_completed {
        ViewCommand::ErrorLogExportCompleted { path } => std::path::PathBuf::from(path),
        _ => unreachable!(),
    };

    assert!(
        export_path.exists(),
        "exported error log path should exist: {}",
        export_path.display()
    );

    let body = std::fs::read_to_string(&export_path).expect("read exported error log");
    assert!(body.contains("AUTH"));
    assert!(body.contains("Source: anthropic / claude"));
    assert!(body.contains("Message:"));
    assert!(body.contains("401 unauthorized"));

    personal_agent::ui_gpui::error_log::ErrorLogStore::global().clear();
}

#[tokio::test]
async fn save_error_log_with_empty_store_emits_notification() {
    let _error_log_lock = ERROR_LOG_TEST_MUTEX.lock().await;

    let default_profile = profile();
    let profile_id = default_profile.id;
    let conversation =
        conversation_with_messages(profile_id, vec![Message::assistant("context".to_string())]);

    let conversation_service = Arc::new(MockConversationService::new(vec![conversation], None));
    let chat_service = Arc::new(MockChatService::new());
    let profile_service = Arc::new(MockProfileService::new(Some(default_profile)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel(128);

    let app_settings = Arc::new(MockAppSettingsService::new());
    let export_dir = tempfile::tempdir().expect("temp export dir");
    app_settings
        .set_export_dir(Some(export_dir.path().to_path_buf()))
        .await;
    let app_settings_service = app_settings.clone() as Arc<dyn AppSettingsService>;

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service,
        profile_service,
        app_settings_service,
        view_tx,
    );
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    personal_agent::ui_gpui::error_log::ErrorLogStore::global().clear();

    event_bus
        .publish(AppEvent::User(UserEvent::SaveErrorLog {
            format: personal_agent::models::ConversationExportFormat::Txt,
        }))
        .expect("publish save error log");
    let commands = collect_commands(&mut view_rx).await;

    assert!(commands.iter().any(|command| {
        matches!(
            command,
            ViewCommand::ShowNotification { message }
                if message == "No errors recorded"
        )
    }));

    personal_agent::ui_gpui::error_log::ErrorLogStore::global().clear();
}
