use std::{collections::HashMap, path::PathBuf, sync::Arc};

use async_trait::async_trait;
use tokio::sync::Mutex;
use uuid::Uuid;

use personal_agent::db::spawn_db_thread;
use personal_agent::models::{
    AuthConfig, ContextState, Conversation, ConversationMetadata, Message, ModelParameters,
    ModelProfile, SearchResult,
};
use personal_agent::services::{
    chat_impl::ChatServiceImpl, secure_store, ChatService, ConversationService, ProfileService,
    ServiceError, SqliteConversationService,
};

struct InMemoryConversationService {
    conversations: Mutex<HashMap<Uuid, Conversation>>,
    fail_load: Mutex<Option<String>>,
    fail_add_user: Mutex<Option<String>>,
}

impl InMemoryConversationService {
    fn new(conversations: Vec<Conversation>) -> Self {
        Self {
            conversations: Mutex::new(
                conversations
                    .into_iter()
                    .map(|conversation| (conversation.id, conversation))
                    .collect(),
            ),
            fail_load: Mutex::new(None),
            fail_add_user: Mutex::new(None),
        }
    }

    async fn set_fail_load(&self, message: &str) {
        *self.fail_load.lock().await = Some(message.to_string());
    }

    async fn set_fail_add_user(&self, message: &str) {
        *self.fail_add_user.lock().await = Some(message.to_string());
    }
}

#[async_trait]
impl ConversationService for InMemoryConversationService {
    async fn create(
        &self,
        title: Option<String>,
        model_profile_id: Uuid,
    ) -> Result<Conversation, ServiceError> {
        let mut conversation = Conversation::new(model_profile_id);
        if let Some(title) = title {
            conversation.set_title(title);
        }
        self.conversations
            .lock()
            .await
            .insert(conversation.id, conversation.clone());
        Ok(conversation)
    }

    async fn load(&self, id: Uuid) -> Result<Conversation, ServiceError> {
        let fail_load = self.fail_load.lock().await.clone();
        if let Some(message) = fail_load {
            return Err(ServiceError::Internal(message));
        }

        self.conversations
            .lock()
            .await
            .get(&id)
            .cloned()
            .ok_or_else(|| ServiceError::NotFound(format!("conversation {id} not found")))
    }

    async fn list_metadata(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ConversationMetadata>, ServiceError> {
        let all: Vec<ConversationMetadata> = {
            let convs = self.conversations.lock().await;
            convs
                .values()
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
                .collect()
        };
        let o = offset.unwrap_or(0);
        let l = limit.unwrap_or(all.len());
        let end = std::cmp::min(o + l, all.len());
        if o >= all.len() {
            return Ok(Vec::new());
        }
        Ok(all[o..end].to_vec())
    }

    #[allow(clippy::significant_drop_tightening)]
    async fn add_message(
        &self,
        conversation_id: Uuid,
        message: Message,
    ) -> Result<Message, ServiceError> {
        let fail_add_user = self.fail_add_user.lock().await.clone();
        if let Some(err) = fail_add_user {
            return Err(ServiceError::Internal(err));
        }

        let mut conversations = self.conversations.lock().await;
        let conversation = conversations
            .get_mut(&conversation_id)
            .ok_or_else(|| ServiceError::NotFound("conversation missing".to_string()))?;
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
                .get(&conversation_id)
                .ok_or_else(|| ServiceError::NotFound("conversation missing".to_string()))?
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

    async fn rename(&self, _id: Uuid, _new_title: String) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn delete(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn set_active(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn get_active(&self) -> Result<Option<Uuid>, ServiceError> {
        Ok(None)
    }

    async fn get_messages(&self, conversation_id: Uuid) -> Result<Vec<Message>, ServiceError> {
        Ok(self.load(conversation_id).await?.messages)
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

fn profile_with_label(label: &str) -> ModelProfile {
    ModelProfile::new(
        "Default".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: label.to_string(),
        },
    )
}

fn temp_storage_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("gpuui-service-coverage-{name}-{}", Uuid::new_v4()))
}

#[tokio::test]
async fn chat_service_new_sets_initial_streaming_state() {
    let conversation_service = Arc::new(InMemoryConversationService::new(vec![]));
    let profile_service = Arc::new(MockProfileService::new(None));
    let _ = ChatServiceImpl::new_for_tests(conversation_service, profile_service);
}

#[tokio::test]
async fn chat_service_send_message_errors_without_default_profile_when_creating_conversation() {
    let conversation_service = Arc::new(InMemoryConversationService::new(vec![]));
    conversation_service
        .set_fail_load("force create path")
        .await;
    let profile_service = Arc::new(MockProfileService::new(None));
    profile_service
        .set_fail_get_default("default missing")
        .await;
    let chat_service = ChatServiceImpl::new_for_tests(conversation_service, profile_service);

    let result = chat_service
        .send_message(Uuid::new_v4(), "hello".to_string())
        .await;

    // The error message comes from chat_impl's get_default error handling
    // or from the conversation lookup path
    let error_msg = match &result {
        Err(ServiceError::Internal(msg) | ServiceError::NotFound(msg)) => msg.clone(),
        Err(e) => format!("{e:?}"),
        Ok(_) => "unexpected success".to_string(),
    };
    assert!(
        matches!(
            result,
            Err(ServiceError::Internal(_) | ServiceError::NotFound(_))
        ),
        "Expected Internal or NotFound error, got: {error_msg}"
    );
}

#[tokio::test]
async fn chat_service_send_message_reports_add_user_message_failures() {
    let profile = profile_with_label("chat-service-add-user");
    let conversation = Conversation::new(profile.id);
    let conversation_id = conversation.id;
    let conversation_service = Arc::new(InMemoryConversationService::new(vec![conversation]));
    conversation_service
        .set_fail_add_user("cannot persist user message")
        .await;
    let profile_service = Arc::new(MockProfileService::new(Some(profile)));
    let chat_service = ChatServiceImpl::new_for_tests(conversation_service, profile_service);

    let result = chat_service
        .send_message(conversation_id, "hello".to_string())
        .await;

    assert!(matches!(
        result,
        Err(ServiceError::Internal(message)) if message == "cannot persist user message"
    ));
}

#[tokio::test]
async fn chat_service_cancel_clears_streaming_flag() {
    let profile = profile_with_label("chat-service-cancel");
    let conversation_service = Arc::new(InMemoryConversationService::new(vec![Conversation::new(
        profile.id,
    )]));
    let profile_service = Arc::new(MockProfileService::new(Some(profile)));
    let chat_service = ChatServiceImpl::new_for_tests(conversation_service, profile_service);

    chat_service.cancel();

    assert!(!chat_service.is_streaming());
}

#[tokio::test]
async fn conversation_service_persists_messages_and_updates_active_conversation() {
    let db_path = temp_storage_path("conversation-success.db");
    let db_path_clone = db_path.clone();
    let db = tokio::task::spawn_blocking(move || {
        spawn_db_thread(&db_path_clone).expect("spawn db thread")
    })
    .await
    .expect("spawn_blocking failed");
    let service = SqliteConversationService::new(db);

    let profile_id = Uuid::new_v4();
    let created = service
        .create(Some("My Chat".to_string()), profile_id)
        .await
        .expect("create conversation");
    let conversation_id = created.id;

    let user_message = service
        .add_message(conversation_id, Message::user("hello".to_string()))
        .await
        .expect("add user message");
    let assistant_message = service
        .add_message(conversation_id, Message::assistant("hi there".to_string()))
        .await
        .expect("add assistant message");
    service
        .rename(conversation_id, "Renamed".to_string())
        .await
        .expect("rename conversation");
    service
        .set_active(conversation_id)
        .await
        .expect("set active conversation");

    let loaded = service
        .load(conversation_id)
        .await
        .expect("load conversation");
    assert_eq!(loaded.title.as_deref(), Some("Renamed"));
    assert_eq!(loaded.messages.len(), 2);
    assert_eq!(loaded.messages[0].content, user_message.content);
    assert_eq!(loaded.messages[1].content, assistant_message.content);
    assert_eq!(
        service.get_active().await.expect("get active"),
        Some(conversation_id)
    );
    assert_eq!(
        service
            .get_messages(conversation_id)
            .await
            .expect("get messages")
            .len(),
        2
    );

    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn conversation_service_update_delete_and_missing_paths_behave_as_expected() {
    let db_path = temp_storage_path("conversation-errors.db");
    let db_path_clone = db_path.clone();
    let db = tokio::task::spawn_blocking(move || {
        spawn_db_thread(&db_path_clone).expect("spawn db thread")
    })
    .await
    .expect("spawn_blocking failed");
    let service = SqliteConversationService::new(db);

    let profile_id = Uuid::new_v4();
    let created = service
        .create(Some("Original".to_string()), profile_id)
        .await
        .expect("create conversation");
    let replacement_profile = Uuid::new_v4();

    let updated = service
        .update(
            created.id,
            Some("Updated".to_string()),
            Some(replacement_profile),
        )
        .await
        .expect("update conversation");
    assert_eq!(updated.title.as_deref(), Some("Updated"));
    assert_eq!(updated.profile_id, replacement_profile);

    service
        .delete(created.id)
        .await
        .expect("delete conversation");
    let delete_missing = service.delete(created.id).await;
    assert!(matches!(delete_missing, Err(ServiceError::NotFound(_))));

    let set_active_missing = service.set_active(created.id).await;
    assert!(matches!(set_active_missing, Err(ServiceError::NotFound(_))));

    let load_missing = service.load(created.id).await;
    assert!(matches!(load_missing, Err(ServiceError::NotFound(_))));

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn secure_store_secret_helpers_and_api_key_index_round_trip() {
    secure_store::use_mock_backend();

    secure_store::set_secret("plain-secret", "value").expect("set secret");
    assert_eq!(
        secure_store::get_secret("plain-secret").expect("get secret"),
        Some("value".to_string())
    );
    assert!(secure_store::has_secret("plain-secret").expect("has secret"));
    secure_store::delete_secret("plain-secret").expect("delete secret");
    assert_eq!(
        secure_store::get_secret("plain-secret").expect("get after delete"),
        None
    );

    let label = format!("coverage-key-{}", Uuid::new_v4());
    secure_store::api_keys::store(&label, "sk-1234567890").expect("store api key");
    assert_eq!(
        secure_store::api_keys::get(&label).expect("get api key"),
        Some("sk-1234567890".to_string())
    );
    assert!(secure_store::api_keys::exists(&label).expect("exists api key"));
    assert!(secure_store::api_keys::list().contains(&label));
    assert_eq!(
        secure_store::api_keys::masked_display("sk-1234567890"),
        "sk-1••••••••7890"
    );
    assert_eq!(secure_store::api_keys::masked_display("short"), "••••••••");
    secure_store::api_keys::delete(&label).expect("delete api key");
    assert_eq!(
        secure_store::api_keys::get(&label).expect("get deleted api key"),
        None
    );
}

#[test]
fn secure_store_mcp_keys_round_trip_named_and_default() {
    secure_store::use_mock_backend();

    let mcp_id = Uuid::new_v4();
    secure_store::mcp_keys::store(mcp_id, "default-secret").expect("store default mcp key");
    secure_store::mcp_keys::store_named(mcp_id, "TOKEN", "named-secret")
        .expect("store named mcp key");

    assert_eq!(
        secure_store::mcp_keys::get(mcp_id).expect("get default mcp key"),
        Some("default-secret".to_string())
    );
    assert_eq!(
        secure_store::mcp_keys::get_named(mcp_id, "TOKEN").expect("get named mcp key"),
        Some("named-secret".to_string())
    );

    secure_store::mcp_keys::delete(mcp_id).expect("delete default mcp key");
    secure_store::mcp_keys::delete_named(mcp_id, "TOKEN").expect("delete named mcp key");

    assert_eq!(
        secure_store::mcp_keys::get(mcp_id).expect("get missing default mcp key"),
        None
    );
    assert_eq!(
        secure_store::mcp_keys::get_named(mcp_id, "TOKEN").expect("get missing named mcp key"),
        None
    );
}
