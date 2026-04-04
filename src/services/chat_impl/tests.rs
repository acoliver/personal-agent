use super::*;
use crate::models::{AuthConfig, Message, ModelParameters};
use std::sync::Arc;

struct MockConversationService {
    profile_id: Uuid,
    messages: Arc<RwLock<Vec<Message>>>,
}
use crate::agent::McpApprovalMode;
use crate::services::{AppSettingsService, ServiceError, ServiceResult};
use std::collections::HashMap;
use tokio::sync::{Barrier, RwLock};

struct InMemoryAppSettingsService {
    settings: RwLock<HashMap<String, String>>,
}

impl InMemoryAppSettingsService {
    fn new() -> Self {
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

struct FailingAppSettingsService;

#[async_trait::async_trait]
impl AppSettingsService for FailingAppSettingsService {
    async fn get_default_profile_id(&self) -> ServiceResult<Option<Uuid>> {
        Ok(None)
    }

    async fn set_default_profile_id(&self, _id: Uuid) -> ServiceResult<()> {
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

fn make_approval_test_chat_service(
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
        mcp_approval_mode: McpApprovalMode::PerTool,
        persistent_allowlist: Vec::new(),
        persistent_denylist: Vec::new(),
        session_allowlist: std::collections::HashSet::new(),
    }));

    let service = ChatServiceImpl::new(
        conversation_service,
        profile_service,
        app_settings_service,
        view_tx,
        approval_gate.clone(),
        policy,
    );

    (service, view_rx, approval_gate)
}

#[tokio::test]
async fn resolve_tool_approval_denied_does_not_update_policy() {
    let app_settings = Arc::new(InMemoryAppSettingsService::new()) as Arc<dyn AppSettingsService>;
    let (service, mut view_rx, approval_gate) =
        make_approval_test_chat_service(app_settings.clone());
    let request_id = Uuid::new_v4().to_string();
    let waiter = approval_gate.wait_for_approval(request_id.clone(), "WriteFile".to_string());

    service
        .resolve_tool_approval(request_id.clone(), ToolApprovalResponseAction::Denied)
        .await
        .expect("denied resolution should succeed");

    let approved = waiter.wait().await.expect("waiter should receive decision");
    assert!(
        !approved,
        "denied decision should propagate false to waiter"
    );

    let resolved = view_rx
        .recv()
        .await
        .expect("view should receive ToolApprovalResolved");
    assert_eq!(
        resolved,
        ViewCommand::ToolApprovalResolved {
            request_id,
            approved: false,
        }
    );

    let policy_after = service.policy.lock().await.clone();
    assert_eq!(
        policy_after.evaluate("WriteFile"),
        crate::agent::tool_approval_policy::ToolApprovalDecision::AskUser,
        "Denied should not add session or persistent allow rules"
    );
}

#[tokio::test]
async fn resolve_tool_approval_denied_resolves_all_pending_approvals() {
    let app_settings = Arc::new(InMemoryAppSettingsService::new()) as Arc<dyn AppSettingsService>;
    let (service, mut view_rx, approval_gate) = make_approval_test_chat_service(app_settings);

    let denied_request_id = Uuid::new_v4().to_string();
    let secondary_request_id = Uuid::new_v4().to_string();

    let denied_waiter =
        approval_gate.wait_for_approval(denied_request_id.clone(), "WriteFile".to_string());
    let secondary_waiter =
        approval_gate.wait_for_approval(secondary_request_id.clone(), "Search".to_string());

    service
        .resolve_tool_approval(
            denied_request_id.clone(),
            ToolApprovalResponseAction::Denied,
        )
        .await
        .expect("denied resolution should succeed");

    assert!(!denied_waiter
        .wait()
        .await
        .expect("denied waiter should receive decision"));
    assert!(!secondary_waiter
        .wait()
        .await
        .expect("secondary waiter should receive bulk denied decision"));

    let first = view_rx
        .recv()
        .await
        .expect("first ToolApprovalResolved should be emitted");
    let second = view_rx
        .recv()
        .await
        .expect("second ToolApprovalResolved should be emitted");

    let resolved_ids = [first, second]
        .into_iter()
        .map(|command| match command {
            ViewCommand::ToolApprovalResolved {
                request_id,
                approved,
            } => {
                assert!(!approved, "denied flow should not approve any request");
                request_id
            }
            other => panic!("expected ToolApprovalResolved, got {other:?}"),
        })
        .collect::<Vec<_>>();

    assert!(resolved_ids.contains(&denied_request_id));
    assert!(resolved_ids.contains(&secondary_request_id));
}

#[tokio::test]
async fn resolve_tool_approval_proceed_session_updates_session_policy() {
    let app_settings = Arc::new(InMemoryAppSettingsService::new()) as Arc<dyn AppSettingsService>;
    let (service, mut view_rx, approval_gate) =
        make_approval_test_chat_service(app_settings.clone());
    let request_id = Uuid::new_v4().to_string();
    let waiter = approval_gate.wait_for_approval(request_id.clone(), "WriteFile".to_string());

    service
        .resolve_tool_approval(
            request_id.clone(),
            ToolApprovalResponseAction::ProceedSession,
        )
        .await
        .expect("session resolution should succeed");

    let approved = waiter.wait().await.expect("waiter should receive decision");
    assert!(approved, "ProceedSession should propagate true to waiter");

    let resolved = view_rx
        .recv()
        .await
        .expect("view should receive ToolApprovalResolved");
    assert_eq!(
        resolved,
        ViewCommand::ToolApprovalResolved {
            request_id,
            approved: true,
        }
    );

    let policy_after = service.policy.lock().await.clone();
    assert_eq!(
        policy_after.evaluate("WriteFile"),
        crate::agent::tool_approval_policy::ToolApprovalDecision::Allow,
        "ProceedSession should add an in-memory session allow rule"
    );

    let persisted = app_settings
        .get_setting(crate::agent::tool_approval_policy::TOOL_APPROVAL_POLICY_SETTINGS_KEY)
        .await
        .expect("settings read should succeed");
    assert!(
        persisted.is_none(),
        "ProceedSession should not persist policy to settings"
    );
}

#[tokio::test]
async fn resolve_tool_approval_proceed_always_persists_policy() {
    let app_settings = Arc::new(InMemoryAppSettingsService::new()) as Arc<dyn AppSettingsService>;
    let (service, mut view_rx, approval_gate) =
        make_approval_test_chat_service(app_settings.clone());
    let request_id = Uuid::new_v4().to_string();
    let waiter = approval_gate.wait_for_approval(request_id.clone(), "WriteFile".to_string());

    service
        .resolve_tool_approval(
            request_id.clone(),
            ToolApprovalResponseAction::ProceedAlways,
        )
        .await
        .expect("persistent resolution should succeed");

    let approved = waiter.wait().await.expect("waiter should receive decision");
    assert!(approved, "ProceedAlways should propagate true to waiter");

    let resolved = view_rx
        .recv()
        .await
        .expect("view should receive ToolApprovalResolved");
    assert_eq!(
        resolved,
        ViewCommand::ToolApprovalResolved {
            request_id: request_id.clone(),
            approved: true,
        }
    );

    let policy_snapshot = view_rx
        .recv()
        .await
        .expect("view should receive ToolApprovalPolicyUpdated after ProceedAlways");
    assert_eq!(
        policy_snapshot,
        ViewCommand::ToolApprovalPolicyUpdated {
            yolo_mode: false,
            auto_approve_reads: false,
            mcp_approval_mode: McpApprovalMode::PerTool,
            persistent_allowlist: vec!["WriteFile".to_string()],
            persistent_denylist: Vec::new(),
        }
    );

    let yolo_snapshot = view_rx
        .recv()
        .await
        .expect("view should receive YoloModeChanged after ProceedAlways");
    assert_eq!(
        yolo_snapshot,
        ViewCommand::YoloModeChanged { active: false }
    );

    let persisted = app_settings
        .get_setting(crate::agent::tool_approval_policy::TOOL_APPROVAL_POLICY_SETTINGS_KEY)
        .await
        .expect("settings read should succeed")
        .expect("ProceedAlways should persist policy payload");
    assert!(persisted.contains("WriteFile"));
}

#[tokio::test]
async fn send_message_clears_session_allowlist_when_yolo_turns_off_in_settings() {
    crate::services::secure_store::use_mock_backend();
    crate::services::secure_store::api_keys::store("_test_yolo_refresh", "fake-key-for-test")
        .expect("store test key");

    let profile = crate::models::ModelProfile::new(
        "Test Profile".to_string(),
        "openai".to_string(),
        "gpt-4".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: "_test_yolo_refresh".to_string(),
        },
    );
    let profile_id = profile.id;

    let conversation_service = Arc::new(MockConversationService::new(profile_id))
        as Arc<dyn super::super::ConversationService>;
    let mock_profile_service = Arc::new(MockProfileService::new());
    mock_profile_service
        .set_default_profile(profile.clone())
        .await;
    mock_profile_service.add_profile(profile.clone()).await;
    let profile_service: Arc<dyn crate::services::ProfileService> = mock_profile_service;

    let app_settings = Arc::new(InMemoryAppSettingsService::new()) as Arc<dyn AppSettingsService>;
    let persisted_policy = crate::agent::ToolApprovalPolicy {
        yolo_mode: false,
        auto_approve_reads: false,
        mcp_approval_mode: McpApprovalMode::PerTool,
        persistent_allowlist: Vec::new(),
        persistent_denylist: Vec::new(),
        session_allowlist: std::collections::HashSet::new(),
    };
    persisted_policy
        .save_to_settings(app_settings.as_ref())
        .await
        .expect("persisted policy should be writable");

    let (view_tx, _view_rx) = tokio::sync::mpsc::channel(8);
    let approval_gate = Arc::new(ApprovalGate::new());
    let mut session_allowlist = std::collections::HashSet::new();
    session_allowlist.insert("WriteFile".to_string());
    let in_memory_policy = crate::agent::ToolApprovalPolicy {
        yolo_mode: true,
        auto_approve_reads: false,
        mcp_approval_mode: McpApprovalMode::PerTool,
        persistent_allowlist: Vec::new(),
        persistent_denylist: Vec::new(),
        session_allowlist,
    };

    let chat_service = ChatServiceImpl::new(
        conversation_service,
        profile_service,
        app_settings,
        view_tx,
        approval_gate,
        Arc::new(AsyncMutex::new(in_memory_policy)),
    );

    let conversation_id = Uuid::new_v4();
    let stream = chat_service
        .send_message(conversation_id, "hello".to_string())
        .await
        .expect("send_message should return a stream");
    drop(stream);

    let policy_after = chat_service.policy.lock().await.clone();
    assert!(
        policy_after.session_allowlist.is_empty(),
        "session allowlist should be cleared when persisted yolo mode turns off"
    );

    let _ = crate::services::secure_store::api_keys::delete("_test_yolo_refresh");
}

#[tokio::test]
async fn resolve_tool_approval_is_atomic_between_competing_decisions() {
    let app_settings = Arc::new(InMemoryAppSettingsService::new()) as Arc<dyn AppSettingsService>;
    let (service, _view_rx, approval_gate) = make_approval_test_chat_service(app_settings.clone());

    let request_id = Uuid::new_v4().to_string();
    let _waiter = approval_gate.wait_for_approval(request_id.clone(), "WriteFile".to_string());

    let barrier = Arc::new(Barrier::new(2));
    let service_denied = Arc::new(service);
    let service_always = service_denied.clone();

    let request_id_denied = request_id.clone();
    let barrier_denied = barrier.clone();
    let denied_handle = tokio::spawn(async move {
        barrier_denied.wait().await;
        service_denied
            .resolve_tool_approval(request_id_denied, ToolApprovalResponseAction::Denied)
            .await
    });

    let request_id_always = request_id.clone();
    let barrier_always = barrier.clone();
    let always_handle = tokio::spawn(async move {
        barrier_always.wait().await;
        service_always
            .resolve_tool_approval(request_id_always, ToolApprovalResponseAction::ProceedAlways)
            .await
    });

    let denied_result = denied_handle.await.expect("denied task should join");
    let always_result = always_handle.await.expect("always task should join");

    let winner_is_denied = denied_result.is_ok();
    let winner_is_always = always_result.is_ok();
    assert!(
        winner_is_denied ^ winner_is_always,
        "exactly one resolver should win the approval claim"
    );

    let persisted = app_settings
        .get_setting(crate::agent::tool_approval_policy::TOOL_APPROVAL_POLICY_SETTINGS_KEY)
        .await
        .expect("settings read should succeed");

    if winner_is_always {
        let payload = persisted.expect("ProceedAlways winner should persist policy");
        assert!(payload.contains("WriteFile"));
    } else {
        assert!(
            persisted.is_none(),
            "Denied winner must not persist allowlist entries"
        );
    }
}

#[tokio::test]
async fn resolve_tool_approval_returns_error_when_persistence_fails() {
    let app_settings = Arc::new(FailingAppSettingsService) as Arc<dyn AppSettingsService>;
    let (service, _view_rx, approval_gate) = make_approval_test_chat_service(app_settings);
    let request_id = Uuid::new_v4().to_string();
    let waiter = approval_gate.wait_for_approval(request_id.clone(), "WriteFile".to_string());

    let error = service
        .resolve_tool_approval(request_id, ToolApprovalResponseAction::ProceedAlways)
        .await
        .expect_err("ProceedAlways should fail when persistence fails");

    assert!(
        matches!(error, ServiceError::Storage(_)),
        "persistence failure should bubble up as storage error"
    );

    drop(waiter);
}

impl MockConversationService {
    fn new(profile_id: Uuid) -> Self {
        Self {
            profile_id,
            messages: Arc::new(RwLock::new(Vec::new())),
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
        // Return a valid conversation so the test can proceed
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
        _state: &crate::models::ContextState,
    ) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn get_context_state(
        &self,
        _id: Uuid,
    ) -> Result<Option<crate::models::ContextState>, crate::services::ServiceError> {
        Ok(None)
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

struct MockProfileService {
    profile: Arc<RwLock<Option<crate::models::ModelProfile>>>,
    profiles_by_id: Arc<RwLock<std::collections::HashMap<Uuid, crate::models::ModelProfile>>>,
}

impl MockProfileService {
    fn new() -> Self {
        Self {
            profile: Arc::new(RwLock::new(None)),
            profiles_by_id: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    async fn set_default_profile(&self, profile: crate::models::ModelProfile) {
        *self.profile.write().await = Some(profile);
    }

    async fn add_profile(&self, profile: crate::models::ModelProfile) {
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
        _name: String,
        _provider: String,
        _model: String,
        _base_url: Option<String>,
        _auth: AuthConfig,
        _parameters: ModelParameters,
        _system_prompt: Option<String>,
    ) -> Result<crate::models::ModelProfile, crate::services::ServiceError> {
        // Return a dummy profile for testing
        Ok(crate::models::ModelProfile::new(
            _name,
            _provider,
            _model,
            _base_url.unwrap_or_else(|| "https://api.test.com/v1".to_string()),
            _auth,
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

#[tokio::test]
async fn test_send_message() {
    crate::services::secure_store::use_mock_backend();
    crate::services::secure_store::api_keys::store("_test_send_msg", "fake-key-for-test")
        .expect("store test key");

    // Set default profile
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

    let conversation_service = Arc::new(MockConversationService::new(profile_id))
        as Arc<dyn super::super::ConversationService>;
    let mock_profile_service = Arc::new(MockProfileService::new());

    // Set the default profile directly on the mock
    mock_profile_service.set_default_profile(profile).await;

    let profile_service: Arc<dyn crate::services::ProfileService> = mock_profile_service;

    let chat_service = ChatServiceImpl::new_for_tests(conversation_service, profile_service);

    let conversation_id = Uuid::new_v4();
    let result = chat_service
        .send_message(conversation_id, "Hello, world!".to_string())
        .await;

    // The send_message call should succeed in creating the stream
    // The actual LLM call happens asynchronously and will fail with invalid API key
    // but the important thing is we got a stream back (not a placeholder)
    assert!(
        result.is_ok(),
        "send_message should return Ok with a stream, got: {:?}",
        result.err()
    );

    // Clean up test key
    let _ = crate::services::secure_store::api_keys::delete("_test_send_msg");
}

#[tokio::test]
async fn test_cancel() {
    let profile = crate::models::ModelProfile::new(
        "Test Profile".to_string(),
        "openai".to_string(),
        "gpt-4".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: "test-key".to_string(),
        },
    );
    let profile_id = profile.id;

    let conversation_service = Arc::new(MockConversationService::new(profile_id))
        as Arc<dyn super::super::ConversationService>;
    let mock_profile_service = Arc::new(MockProfileService::new());

    // Set the default profile directly on the mock
    mock_profile_service.set_default_profile(profile).await;

    let profile_service: Arc<dyn crate::services::ProfileService> = mock_profile_service;

    let chat_service = ChatServiceImpl::new_for_tests(conversation_service, profile_service);

    // Cancel should work even without streaming
    chat_service.cancel();
    assert!(!chat_service.is_streaming());
}

#[tokio::test]
async fn test_is_streaming() {
    let profile = crate::models::ModelProfile::new(
        "Test Profile".to_string(),
        "openai".to_string(),
        "gpt-4".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: "test-key".to_string(),
        },
    );
    let profile_id = profile.id;

    let conversation_service = Arc::new(MockConversationService::new(profile_id))
        as Arc<dyn super::super::ConversationService>;
    let mock_profile_service = Arc::new(MockProfileService::new());

    // Set the default profile directly on the mock
    mock_profile_service.set_default_profile(profile).await;

    let profile_service: Arc<dyn crate::services::ProfileService> = mock_profile_service;

    let chat_service = ChatServiceImpl::new_for_tests(conversation_service, profile_service);

    // Initially not streaming
    assert!(!chat_service.is_streaming());
}

/// Proves that `prepare_message_context` resolves the profile via the
/// conversation's `profile_id` rather than always using the global default.
#[tokio::test]
async fn prepare_message_context_uses_conversation_profile_id() {
    crate::services::secure_store::use_mock_backend();
    crate::services::secure_store::api_keys::store(
        "_test_conv_prof",
        "fake-key-for-conv-profile-test",
    )
    .expect("store test key");

    // Create a "kimi" profile that we want the conversation to use
    let kimi_profile = crate::models::ModelProfile::new(
        "Kimi Test".to_string(),
        "kimi-for-coding".to_string(),
        "kimi-k2-0711-preview".to_string(),
        String::new(),
        AuthConfig::Keychain {
            label: "_test_conv_prof".to_string(),
        },
    );
    let kimi_profile_id = kimi_profile.id;

    // Default profile is OpenAI — should NOT be used
    let default_profile = crate::models::ModelProfile::new(
        "Default".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: "_test_conv_prof".to_string(),
        },
    );

    // Conversation is bound to the kimi profile
    let conversation_service = Arc::new(MockConversationService::new(kimi_profile_id))
        as Arc<dyn super::super::ConversationService>;
    let mock_profile_service = Arc::new(MockProfileService::new());
    mock_profile_service
        .set_default_profile(default_profile)
        .await;
    mock_profile_service.add_profile(kimi_profile).await;

    let profile_service: Arc<dyn crate::services::ProfileService> = mock_profile_service;

    let chat_service = ChatServiceImpl::new_for_tests(conversation_service, profile_service);

    let prepared = chat_service
        .prepare_message_context(Uuid::new_v4(), "hello".to_string())
        .await
        .expect("prepare_message_context should succeed");

    assert_eq!(
        prepared.profile.id, kimi_profile_id,
        "prepared context should use the conversation's profile, not the default"
    );
    assert_eq!(prepared.profile.provider_id, "kimi-for-coding");

    let _ = crate::services::secure_store::api_keys::delete("_test_conv_prof");
}

#[tokio::test]
async fn cancel_clears_current_conversation_and_pending_approvals() {
    let app_settings = Arc::new(InMemoryAppSettingsService::new()) as Arc<dyn AppSettingsService>;
    let (service, mut view_rx, approval_gate) = make_approval_test_chat_service(app_settings);

    let conversation_id = Uuid::new_v4();
    service
        .begin_stream(conversation_id)
        .await
        .expect("begin_stream should succeed");

    let pending_request_id = Uuid::new_v4().to_string();
    let waiter =
        approval_gate.wait_for_approval(pending_request_id.clone(), "WriteFile".to_string());

    service.cancel();

    assert!(!waiter
        .wait()
        .await
        .expect("pending waiter should be resolved as denied on cancel"));
    assert!(!service.is_streaming());

    let current_conversation = *service
        .current_conversation_id
        .lock()
        .expect("current conversation mutex poisoned");
    assert!(
        current_conversation.is_none(),
        "cancel should clear active conversation tracking"
    );

    let resolved_command = view_rx
        .recv()
        .await
        .expect("cancel should emit approval resolution for pending request");
    assert_eq!(
        resolved_command,
        ViewCommand::ToolApprovalResolved {
            request_id: pending_request_id,
            approved: false,
        }
    );
}

#[test]
fn build_llm_messages_preserves_assistant_thinking_content() {
    let profile = crate::models::ModelProfile::new(
        "Test Profile".to_string(),
        "openai".to_string(),
        "gpt-4".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: "test-key".to_string(),
        },
    );

    let mut conversation = crate::models::Conversation::new(profile.id);
    conversation.add_message(Message::user("hello".to_string()));
    conversation.add_message(Message::assistant_with_thinking(
        "answer".to_string(),
        "chain-of-thought".to_string(),
    ));

    let messages = ChatServiceImpl::build_llm_messages(&conversation, &profile);
    let assistant = messages
        .iter()
        .find(|message| matches!(message.role, crate::llm::Role::Assistant))
        .expect("assistant message should be present");

    assert_eq!(
        assistant.thinking_content.as_deref(),
        Some("chain-of-thought")
    );
}
