//! Presenter regression tests for conversation selection and settings profile actions.
//!
//! These tests cover wiring that is hard to verify from script greps alone:
//! - SelectConversation should emit ConversationActivated and replay stored messages
//! - EditProfile should emit ProfileEditorLoad
//! - DeleteProfile should emit ProfileDeleted

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use personal_agent::events::{bus::EventBus, types::UserEvent, AppEvent};
use personal_agent::models::{
    AuthConfig, Conversation, Message, MessageRole as DomainMessageRole, ModelParameters,
    ModelProfile,
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
    ) -> Result<Box<dyn futures::Stream<Item = personal_agent::services::ChatStreamEvent> + Send + Unpin>, ServiceError> {
        Ok(Box::new(futures::stream::empty::<personal_agent::services::ChatStreamEvent>()))
    }

    fn cancel(&self) {}

    fn is_streaming(&self) -> bool {
        false
    }
}

struct EmptyProfileService;

#[async_trait]
impl ProfileService for EmptyProfileService {
    async fn list(&self) -> Result<Vec<ModelProfile>, ServiceError> {
        Ok(vec![])
    }

    async fn get(&self, id: Uuid) -> Result<ModelProfile, ServiceError> {
        Err(ServiceError::NotFound(format!("profile {} not found", id)))
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
            },
            Message {
                role: DomainMessageRole::Assistant,
                content: "second".to_string(),
                thinking_content: None,
                timestamp: Utc::now(),
            },
        ],
    }) as Arc<dyn ConversationService>;

    let chat_service = Arc::new(MockChatService) as Arc<dyn ChatService>;
    let profile_service = Arc::new(EmptyProfileService) as Arc<dyn ProfileService>;

    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(64);

    let mut presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service,
        chat_service,
        profile_service,
        view_tx,
    );
    presenter.start().await.expect("start chat presenter");

    event_bus
        .publish(AppEvent::User(UserEvent::SelectConversation { id: selected_id }))
        .ok();

    tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;

    let mut saw_activation = false;
    let mut replayed = Vec::new();

    while let Ok(cmd) = view_rx.try_recv() {
        match cmd {
            ViewCommand::ConversationActivated { id } => {
                if id == selected_id {
                    saw_activation = true;
                }
            }
            ViewCommand::MessageAppended { role, content, .. } => {
                replayed.push((role, content));
            }
            _ => {}
        }
    }

    assert!(saw_activation, "SelectConversation should emit ConversationActivated");
    assert_eq!(replayed.len(), 2, "Should replay all stored messages for selected conversation");
    assert!(matches!(replayed[0].0, MessageRole::User));
    assert_eq!(replayed[0].1, "first");
    assert!(matches!(replayed[1].0, MessageRole::Assistant));
    assert_eq!(replayed[1].1, "second");
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
        auth: AuthConfig::Key {
            value: "sk-test".to_string(),
        },
        parameters: ModelParameters::default(),
        system_prompt: "system".to_string(),
    };

    let profile_service = Arc::new(MockProfileServiceForSettings { profile }) as Arc<dyn ProfileService>;
    let app_settings = Arc::new(MockAppSettings) as Arc<dyn AppSettingsService>;

    let (event_tx, _) = broadcast::channel::<AppEvent>(64);
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(64);

    let mut presenter = SettingsPresenter::new(profile_service, app_settings, &event_tx, view_tx);
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

    assert!(saw_prefill, "EditProfile should emit ProfileEditorLoad prefill command");
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
        auth: AuthConfig::Key {
            value: "sk-test".to_string(),
        },
        parameters: ModelParameters::default(),
        system_prompt: "system".to_string(),
    };

    let profile_service = Arc::new(MockProfileServiceForSettings { profile }) as Arc<dyn ProfileService>;
    let app_settings = Arc::new(MockAppSettings) as Arc<dyn AppSettingsService>;

    let (event_tx, _) = broadcast::channel::<AppEvent>(64);
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(64);

    let mut presenter = SettingsPresenter::new(profile_service, app_settings, &event_tx, view_tx);
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

    assert!(saw_deleted, "DeleteProfile should emit ProfileDeleted for settings view refresh");
}
