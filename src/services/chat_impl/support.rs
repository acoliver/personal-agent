use super::*;
use crate::agent::McpApprovalMode;
use crate::models::{AuthConfig, Message, ModelParameters};
use crate::services::{AppSettingsService, ServiceError, ServiceResult};
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub(super) struct MockConversationService {
    pub(super) profile_id: Uuid,
    pub(super) messages: Arc<RwLock<Vec<Message>>>,
    pub(super) context_state: Arc<RwLock<Option<crate::models::ContextState>>>,
}

pub(super) struct InMemoryAppSettingsService {
    settings: RwLock<HashMap<String, String>>,
}

impl InMemoryAppSettingsService {
    pub(super) fn new() -> Self {
        Self {
            settings: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl AppSettingsService for InMemoryAppSettingsService {
    async fn get_default_profile_id(&self) -> ServiceResult<Option<Uuid>> {
        Ok(None)
    }

    async fn set_default_profile_id(&self, _id: Uuid) -> ServiceResult<()> {
        Ok(())
    }

    async fn clear_default_profile_id(&self) -> ServiceResult<()> {
        Ok(())
    }

    async fn get_current_conversation_id(&self) -> ServiceResult<Option<Uuid>> {
        Ok(None)
    }

    async fn set_current_conversation_id(&self, _id: Uuid) -> ServiceResult<()> {
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

    async fn get_setting(&self, key: &str) -> ServiceResult<Option<String>> {
        Ok(self.settings.read().await.get(key).cloned())
    }

    async fn set_setting(&self, key: &str, value: String) -> ServiceResult<()> {
        self.settings.write().await.insert(key.to_string(), value);
        Ok(())
    }

    async fn reset_to_defaults(&self) -> ServiceResult<()> {
        self.settings.write().await.clear();
        Ok(())
    }
}

pub(super) struct FailingAppSettingsService;

#[async_trait::async_trait]
impl AppSettingsService for FailingAppSettingsService {
    async fn get_default_profile_id(&self) -> ServiceResult<Option<Uuid>> {
        Ok(None)
    }

    async fn set_default_profile_id(&self, _id: Uuid) -> ServiceResult<()> {
        Ok(())
    }

    async fn clear_default_profile_id(&self) -> ServiceResult<()> {
        Ok(())
    }

    async fn get_current_conversation_id(&self) -> ServiceResult<Option<Uuid>> {
        Ok(None)
    }

    async fn set_current_conversation_id(&self, _id: Uuid) -> ServiceResult<()> {
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

    async fn get_setting(&self, _key: &str) -> ServiceResult<Option<String>> {
        Ok(None)
    }

    async fn set_setting(&self, _key: &str, _value: String) -> ServiceResult<()> {
        Err(ServiceError::Storage(
            "simulated settings persistence failure".to_string(),
        ))
    }

    async fn reset_to_defaults(&self) -> ServiceResult<()> {
        Ok(())
    }
}

pub(super) fn make_approval_test_chat_service(
    app_settings_service: Arc<dyn AppSettingsService>,
) -> (
    ChatServiceImpl,
    tokio::sync::mpsc::Receiver<ViewCommand>,
    Arc<ApprovalGate>,
) {
    let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()))
        as Arc<dyn super::super::ConversationService>;
    let profile_service =
        Arc::new(MockProfileService::new()) as Arc<dyn crate::services::ProfileService>;
    let (view_tx, view_rx) = tokio::sync::mpsc::channel(8);
    let approval_gate = Arc::new(ApprovalGate::new());
    let policy = Arc::new(AsyncMutex::new(ToolApprovalPolicy {
        yolo_mode: false,
        auto_approve_reads: false,
        skills_auto_approve: false,
        mcp_approval_mode: McpApprovalMode::PerTool,
        persistent_allowlist: Vec::new(),
        persistent_denylist: Vec::new(),
        session_allowlist: std::collections::HashSet::new(),
    }));

    let skills_service = Arc::new(
        crate::services::SkillsServiceImpl::new(app_settings_service.clone())
            .expect("skills service should initialize"),
    ) as Arc<dyn crate::services::SkillsService>;
    let service = ChatServiceImpl::new(
        conversation_service,
        profile_service,
        app_settings_service,
        skills_service,
        view_tx,
        approval_gate.clone(),
        policy,
    );

    (service, view_rx, approval_gate)
}

impl MockConversationService {
    pub(super) fn new(profile_id: Uuid) -> Self {
        Self {
            profile_id,
            messages: Arc::new(RwLock::new(Vec::new())),
            context_state: Arc::new(RwLock::new(None)),
        }
    }
}

#[async_trait::async_trait]
impl super::super::ConversationService for MockConversationService {
    async fn create(
        &self,
        _title: Option<String>,
        model_profile_id: Uuid,
    ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
        Ok(crate::models::Conversation::new(model_profile_id))
    }

    async fn load(
        &self,
        _id: Uuid,
    ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
        let mut conversation = crate::models::Conversation::new(self.profile_id);
        conversation.messages = self.messages.read().await.clone();
        Ok(conversation)
    }

    async fn list_metadata(
        &self,
        _limit: Option<usize>,
        _offset: Option<usize>,
    ) -> Result<Vec<crate::models::ConversationMetadata>, crate::services::ServiceError> {
        Ok(vec![])
    }

    async fn add_message(
        &self,
        _conversation_id: Uuid,
        message: Message,
    ) -> Result<Message, crate::services::ServiceError> {
        self.messages.write().await.push(message.clone());
        Ok(message)
    }

    async fn search(
        &self,
        _query: &str,
        _limit: Option<usize>,
        _offset: Option<usize>,
    ) -> Result<Vec<crate::models::SearchResult>, crate::services::ServiceError> {
        Ok(vec![])
    }

    async fn message_count(
        &self,
        _conversation_id: Uuid,
    ) -> Result<usize, crate::services::ServiceError> {
        Ok(0)
    }

    async fn update_context_state(
        &self,
        _id: Uuid,
        state: &crate::models::ContextState,
    ) -> Result<(), crate::services::ServiceError> {
        *self.context_state.write().await = Some(state.clone());
        Ok(())
    }

    async fn get_context_state(
        &self,
        _id: Uuid,
    ) -> Result<Option<crate::models::ContextState>, crate::services::ServiceError> {
        Ok(self.context_state.read().await.clone())
    }

    async fn rename(
        &self,
        _id: Uuid,
        _new_title: String,
    ) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn delete(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn set_active(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn get_active(&self) -> Result<Option<Uuid>, crate::services::ServiceError> {
        Ok(None)
    }

    async fn get_messages(
        &self,
        _conversation_id: Uuid,
    ) -> Result<Vec<Message>, crate::services::ServiceError> {
        Ok(vec![])
    }

    async fn update(
        &self,
        _id: Uuid,
        _title: Option<String>,
        _model_profile_id: Option<Uuid>,
    ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
        Err(crate::services::ServiceError::NotFound("test".to_string()))
    }
}

pub(super) struct MockProfileService {
    profile: Arc<RwLock<Option<crate::models::ModelProfile>>>,
    profiles_by_id: Arc<RwLock<std::collections::HashMap<Uuid, crate::models::ModelProfile>>>,
}

impl MockProfileService {
    pub(super) fn new() -> Self {
        Self {
            profile: Arc::new(RwLock::new(None)),
            profiles_by_id: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub(super) async fn set_default_profile(&self, profile: crate::models::ModelProfile) {
        *self.profile.write().await = Some(profile);
    }

    pub(super) async fn add_profile(&self, profile: crate::models::ModelProfile) {
        self.profiles_by_id
            .write()
            .await
            .insert(profile.id, profile);
    }
}

#[async_trait::async_trait]
impl crate::services::ProfileService for MockProfileService {
    async fn list(
        &self,
    ) -> Result<Vec<crate::models::ModelProfile>, crate::services::ServiceError> {
        Ok(vec![])
    }

    async fn get(
        &self,
        id: Uuid,
    ) -> Result<crate::models::ModelProfile, crate::services::ServiceError> {
        self.profiles_by_id
            .read()
            .await
            .get(&id)
            .cloned()
            .ok_or_else(|| {
                crate::services::ServiceError::NotFound(format!("profile {id} not found"))
            })
    }

    async fn create(
        &self,
        name: String,
        provider: String,
        model: String,
        base_url: Option<String>,
        auth: AuthConfig,
        _parameters: ModelParameters,
        _system_prompt: Option<String>,
    ) -> Result<crate::models::ModelProfile, crate::services::ServiceError> {
        Ok(crate::models::ModelProfile::new(
            name,
            provider,
            model,
            base_url.unwrap_or_else(|| "https://api.test.com/v1".to_string()),
            auth,
        ))
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
    ) -> Result<crate::models::ModelProfile, crate::services::ServiceError> {
        Err(crate::services::ServiceError::NotFound("test".to_string()))
    }

    async fn delete(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
        Err(crate::services::ServiceError::NotFound("test".to_string()))
    }

    async fn test_connection(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn get_default(
        &self,
    ) -> Result<Option<crate::models::ModelProfile>, crate::services::ServiceError> {
        Ok(self.profile.read().await.clone())
    }

    async fn set_default(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }
}

pub(super) async fn setup_send_message_test() -> (Arc<MockConversationService>, bool) {
    crate::services::secure_store::use_mock_backend();
    crate::services::secure_store::api_keys::store("_test_send_msg", "fake-key-for-test")
        .expect("store test key");

    let profile = crate::models::ModelProfile::new(
        "Test Profile".to_string(),
        "openai".to_string(),
        "gpt-4".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: "_test_send_msg".to_string(),
        },
    );
    let profile_id = profile.id;

    let conversation_service_impl = Arc::new(MockConversationService::new(profile_id));
    let conversation_service =
        conversation_service_impl.clone() as Arc<dyn super::super::ConversationService>;
    let mock_profile_service = Arc::new(MockProfileService::new());
    mock_profile_service.set_default_profile(profile).await;
    let profile_service: Arc<dyn crate::services::ProfileService> = mock_profile_service;

    let chat_service = ChatServiceImpl::new_for_tests(conversation_service, profile_service);
    let conversation_id = Uuid::new_v4();
    let mut stream = chat_service
        .send_message(conversation_id, "Hello, world!".to_string())
        .await
        .expect("send_message should return Ok with a stream");

    let completed = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while let Some(event) = stream.next().await {
            match event {
                ChatStreamEvent::Complete { .. } => return true,
                ChatStreamEvent::Error(_) => return false,
                ChatStreamEvent::Token(_) => {}
            }
        }
        false
    })
    .await
    .unwrap_or(false);

    (conversation_service_impl, completed)
}

pub(super) fn assert_non_empty_tool_json<T: serde::de::DeserializeOwned>(
    maybe_json: Option<&str>,
    deserialize_err: &str,
    empty_err: &str,
) {
    if let Some(json) = maybe_json {
        let parsed: Vec<T> = serde_json::from_str(json).expect(deserialize_err);
        assert!(!parsed.is_empty(), "{empty_err}");
    }
}
