use super::*;
use crate::agent::McpApprovalMode;
use crate::models::{AuthConfig, ContextState, Message, Skill};
use crate::services::{AppSettingsService, ProfileService, SkillsService};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Barrier;
use uuid::Uuid;

mod approval_persistence;
#[path = "support.rs"]
mod chat_test_support;
mod compression_persistence;
mod concurrent_streams;
mod three_stream_concurrency;

use chat_test_support::*;

struct FailingSkillsService;

#[async_trait]
impl SkillsService for FailingSkillsService {
    async fn list_skills(&self) -> ServiceResult<Vec<Skill>> {
        Err(ServiceError::Internal(
            "simulated skills failure".to_string(),
        ))
    }

    async fn get_skill(&self, _name: &str) -> ServiceResult<Option<Skill>> {
        Err(ServiceError::Internal(
            "simulated skills failure".to_string(),
        ))
    }

    async fn get_skill_body(&self, _name: &str) -> ServiceResult<Option<String>> {
        Err(ServiceError::Internal(
            "simulated skills failure".to_string(),
        ))
    }

    async fn set_skill_enabled(&self, _name: &str, _enabled: bool) -> ServiceResult<()> {
        Err(ServiceError::Internal(
            "simulated skills failure".to_string(),
        ))
    }

    async fn get_enabled_skills(&self) -> ServiceResult<Vec<Skill>> {
        Err(ServiceError::Internal(
            "simulated skills failure".to_string(),
        ))
    }

    async fn refresh(&self) -> ServiceResult<()> {
        Err(ServiceError::Internal(
            "simulated skills failure".to_string(),
        ))
    }

    async fn watched_directories(&self) -> ServiceResult<Vec<std::path::PathBuf>> {
        Err(ServiceError::Internal(
            "simulated skills failure".to_string(),
        ))
    }

    async fn add_watched_directory(&self, _path: std::path::PathBuf) -> ServiceResult<()> {
        Err(ServiceError::Internal(
            "simulated skills failure".to_string(),
        ))
    }

    async fn remove_watched_directory(&self, _path: &std::path::Path) -> ServiceResult<()> {
        Err(ServiceError::Internal(
            "simulated skills failure".to_string(),
        ))
    }

    fn default_user_skills_dir(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(".")
    }

    async fn install_skill_from_url(&self, _url: &str) -> ServiceResult<Skill> {
        Err(ServiceError::Internal(
            "simulated skills failure".to_string(),
        ))
    }
}

#[tokio::test]
async fn resolve_tool_approval_denied_does_not_update_policy() {
    let app_settings = Arc::new(InMemoryAppSettingsService::new()) as Arc<dyn AppSettingsService>;
    let (service, mut view_rx, approval_gate) =
        make_approval_test_chat_service(app_settings.clone());
    let request_id = Uuid::new_v4().to_string();
    let waiter =
        approval_gate.wait_for_approval(request_id.clone(), "WriteFile".to_string(), Uuid::nil());

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
            conversation_id: Uuid::nil(),
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

    let denied_waiter = approval_gate.wait_for_approval(
        denied_request_id.clone(),
        "WriteFile".to_string(),
        Uuid::nil(),
    );
    let secondary_waiter = approval_gate.wait_for_approval(
        secondary_request_id.clone(),
        "Search".to_string(),
        Uuid::nil(),
    );

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
                conversation_id,
                request_id,
                approved,
            } => {
                assert_eq!(
                    conversation_id,
                    Uuid::nil(),
                    "denied flow should report request ownership"
                );
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
async fn resolve_tool_approval_proceed_session_updates_all_identifiers() {
    let app_settings = Arc::new(InMemoryAppSettingsService::new()) as Arc<dyn AppSettingsService>;
    let (service, mut view_rx, approval_gate) =
        make_approval_test_chat_service(app_settings.clone());
    let request_id = Uuid::new_v4().to_string();
    let waiter = approval_gate.wait_for_approvals(
        request_id.clone(),
        vec!["git status".to_string(), "pwd".to_string()],
        Uuid::nil(),
    );

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
            conversation_id: Uuid::nil(),
            request_id,
            approved: true,
        }
    );

    let policy_after = service.policy.lock().await.clone();
    assert_eq!(
        policy_after.evaluate("git status --short"),
        crate::agent::tool_approval_policy::ToolApprovalDecision::Allow,
        "ProceedSession should add every identifier to in-memory session allow rules"
    );
    assert_eq!(
        policy_after.evaluate("pwd"),
        crate::agent::tool_approval_policy::ToolApprovalDecision::Allow,
        "ProceedSession should add every identifier to in-memory session allow rules"
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
async fn resolve_tool_approval_proceed_always_persists_all_identifiers() {
    let app_settings = Arc::new(InMemoryAppSettingsService::new()) as Arc<dyn AppSettingsService>;
    let (service, mut view_rx, approval_gate) =
        make_approval_test_chat_service(app_settings.clone());
    let request_id = Uuid::new_v4().to_string();
    let waiter = approval_gate.wait_for_approvals(
        request_id.clone(),
        vec!["git status".to_string(), "pwd".to_string()],
        Uuid::nil(),
    );

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
            conversation_id: Uuid::nil(),
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
            skills_auto_approve: false,
            mcp_approval_mode: McpApprovalMode::PerTool,
            persistent_allowlist: vec!["git status".to_string(), "pwd".to_string()],
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
    assert!(persisted.contains("git status"));
    assert!(persisted.contains("pwd"));

    let policy_after = service.policy.lock().await.clone();
    assert_eq!(
        policy_after.evaluate_compound_command("git status && pwd"),
        crate::agent::tool_approval_policy::ToolApprovalDecision::Allow,
        "ProceedAlways should persist every identifier needed for compound command evaluation"
    );
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
        skills_auto_approve: false,
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
        skills_auto_approve: false,
        mcp_approval_mode: McpApprovalMode::PerTool,
        persistent_allowlist: Vec::new(),
        persistent_denylist: Vec::new(),
        session_allowlist,
    };

    let skills_service = Arc::new(
        crate::services::SkillsServiceImpl::new(app_settings.clone())
            .expect("skills service should initialize"),
    ) as Arc<dyn crate::services::SkillsService>;
    let chat_service = ChatServiceImpl::new(
        conversation_service,
        profile_service,
        app_settings,
        skills_service,
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
    let _waiter =
        approval_gate.wait_for_approval(request_id.clone(), "WriteFile".to_string(), Uuid::nil());

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
async fn test_send_message() {
    let (conversation_service_impl, completed) = setup_send_message_test().await;

    if completed {
        let maybe_assistant = {
            let messages = conversation_service_impl.messages.read().await;
            messages
                .iter()
                .rfind(|message| matches!(message.role, crate::models::MessageRole::Assistant))
                .cloned()
        };

        let assistant_message = maybe_assistant
            .expect("assistant message should be persisted after successful stream completion");

        assert!(
            assistant_message.tool_calls.is_some() || assistant_message.tool_results.is_some(),
            "expected at least one persisted tool transcript field"
        );
        assert_non_empty_tool_json::<crate::llm::tools::ToolUse>(
            assistant_message.tool_calls.as_deref(),
            "persisted tool calls JSON should deserialize",
            "persisted tool calls should not be empty",
        );
        assert_non_empty_tool_json::<crate::llm::tools::ToolResult>(
            assistant_message.tool_results.as_deref(),
            "persisted tool results JSON should deserialize",
            "persisted tool results should not be empty",
        );
    }

    let _ = crate::services::secure_store::api_keys::delete("_test_send_msg");
}

#[tokio::test]
async fn persist_assistant_response_skips_empty_turn_with_only_tool_transcript() {
    let conversation_service_impl = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let conversation_service =
        conversation_service_impl.clone() as Arc<dyn super::super::ConversationService>;
    let conversation_id = Uuid::new_v4();
    let tool_calls = vec![crate::llm::tools::ToolUse::new(
        "historical-tool-call",
        "web_search",
        serde_json::json!({"query":"old"}),
    )];
    let tool_results = vec![crate::llm::tools::ToolResult::success(
        "historical-tool-call",
        "old result",
    )];

    streaming::persist_assistant_response(
        &conversation_service,
        conversation_id,
        "",
        "",
        &tool_calls,
        &tool_results,
        "Test Profile",
    )
    .await;

    assert!(
        conversation_service_impl.messages.read().await.is_empty(),
        "empty assistant turns must not persist historical tool transcript"
    );
}

#[tokio::test]
async fn support_app_settings_services_cover_success_and_failure_paths() {
    let in_memory = InMemoryAppSettingsService::new();
    let profile_id = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();

    assert_eq!(in_memory.get_default_profile_id().await.unwrap(), None);
    in_memory.set_default_profile_id(profile_id).await.unwrap();
    in_memory.clear_default_profile_id().await.unwrap();
    assert_eq!(in_memory.get_current_conversation_id().await.unwrap(), None);
    in_memory
        .set_current_conversation_id(conversation_id)
        .await
        .unwrap();
    assert_eq!(in_memory.get_hotkey().await.unwrap(), None);
    in_memory
        .set_hotkey("Cmd+Shift+K".to_string())
        .await
        .unwrap();
    assert_eq!(in_memory.get_theme().await.unwrap(), None);
    in_memory.set_theme("dark".to_string()).await.unwrap();
    assert_eq!(in_memory.get_setting("missing").await.unwrap(), None);
    in_memory
        .set_setting("skills.disabled", "[\"drafting\"]".to_string())
        .await
        .unwrap();
    assert_eq!(
        in_memory.get_setting("skills.disabled").await.unwrap(),
        Some("[\"drafting\"]".to_string())
    );
    in_memory.reset_to_defaults().await.unwrap();
    assert_eq!(
        in_memory.get_setting("skills.disabled").await.unwrap(),
        None
    );

    let failing = FailingAppSettingsService;
    assert_eq!(failing.get_default_profile_id().await.unwrap(), None);
    failing.set_default_profile_id(profile_id).await.unwrap();
    failing.clear_default_profile_id().await.unwrap();
    assert_eq!(failing.get_current_conversation_id().await.unwrap(), None);
    failing
        .set_current_conversation_id(conversation_id)
        .await
        .unwrap();
    assert_eq!(failing.get_hotkey().await.unwrap(), None);
    failing.set_hotkey("Cmd+Shift+K".to_string()).await.unwrap();
    assert_eq!(failing.get_theme().await.unwrap(), None);
    failing.set_theme("dark".to_string()).await.unwrap();
    assert_eq!(failing.get_setting("missing").await.unwrap(), None);
    let error = failing
        .set_setting("tool_approval.policy", "{}".to_string())
        .await
        .expect_err("failing settings should surface storage errors");
    assert!(error
        .to_string()
        .contains("simulated settings persistence failure"));
    failing.reset_to_defaults().await.unwrap();
}

#[tokio::test]
async fn support_mock_conversation_service_covers_crud_and_lookup_paths() {
    let profile_id = Uuid::new_v4();
    let conversation_service = MockConversationService::new(profile_id);

    let created = conversation_service
        .create(Some("Hello".to_string()), profile_id)
        .await
        .expect("conversation create should succeed");
    assert_eq!(created.profile_id, profile_id);
    assert!(conversation_service
        .list_metadata(None, None)
        .await
        .unwrap()
        .is_empty());

    let stored = conversation_service
        .add_message(Uuid::new_v4(), Message::user("hello".to_string()))
        .await
        .expect("add_message should succeed");
    assert_eq!(stored.content, "hello");

    let loaded = conversation_service
        .load(Uuid::new_v4())
        .await
        .expect("conversation load should succeed");
    assert_eq!(loaded.messages.len(), 1);
    assert_eq!(
        conversation_service
            .search("hello", None, None)
            .await
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        conversation_service
            .message_count(Uuid::new_v4())
            .await
            .unwrap(),
        0
    );
    conversation_service
        .update_context_state(
            Uuid::new_v4(),
            &ContextState {
                strategy: Some("summary".to_string()),
                summary: Some("condensed".to_string()),
                visible_range: Some((0, 1)),
                ..ContextState::default()
            },
        )
        .await
        .unwrap();
    let stored_state = conversation_service
        .get_context_state(Uuid::new_v4())
        .await
        .unwrap()
        .expect("context state should be stored by the mock service");
    assert_eq!(stored_state.strategy.as_deref(), Some("summary"));
    conversation_service
        .rename(Uuid::new_v4(), "Renamed".to_string())
        .await
        .unwrap();
    conversation_service.delete(Uuid::new_v4()).await.unwrap();
    conversation_service
        .set_active(Uuid::new_v4())
        .await
        .unwrap();
    assert_eq!(conversation_service.get_active().await.unwrap(), None);
    assert!(conversation_service
        .get_messages(Uuid::new_v4())
        .await
        .unwrap()
        .is_empty());
    assert!(conversation_service
        .update(Uuid::new_v4(), Some("title".to_string()), Some(profile_id))
        .await
        .is_err());
}

#[tokio::test]
async fn support_mock_profile_service_covers_crud_and_lookup_paths() {
    let profile = crate::models::ModelProfile::new(
        "Support Profile".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: "support-key".to_string(),
        },
    );
    let profile_id = profile.id;
    let profile_service = MockProfileService::new();

    profile_service.set_default_profile(profile.clone()).await;
    profile_service.add_profile(profile.clone()).await;
    assert!(profile_service.list().await.unwrap().is_empty());
    assert_eq!(
        profile_service.get(profile_id).await.unwrap().id,
        profile_id
    );
    assert_eq!(
        profile_service
            .get_default()
            .await
            .unwrap()
            .expect("default profile")
            .id,
        profile_id
    );
    profile_service.set_default(profile_id).await.unwrap();
    profile_service.test_connection(profile_id).await.unwrap();

    let created_profile = profile_service
        .create(
            "Created".to_string(),
            "openai".to_string(),
            "gpt-4.1".to_string(),
            Some("https://api.example.com/v1".to_string()),
            AuthConfig::Keychain {
                label: "created-key".to_string(),
            },
            crate::models::ModelParameters::default(),
            None,
        )
        .await
        .expect("profile create should succeed");
    assert_eq!(created_profile.name, "Created");
    assert!(profile_service
        .update(profile_id, None, None, None, None, None, None, None)
        .await
        .is_err());
    assert!(profile_service.delete(profile_id).await.is_err());
}

async fn make_basic_chat_service() -> ChatServiceImpl {
    let profile = crate::models::ModelProfile::new(
        "Test Profile".to_string(),
        "openai".to_string(),
        "gpt-4".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: "test-key".to_string(),
        },
    );

    let conversation_service = Arc::new(MockConversationService::new(profile.id))
        as Arc<dyn super::super::ConversationService>;
    let mock_profile_service = Arc::new(MockProfileService::new());
    mock_profile_service.set_default_profile(profile).await;

    let profile_service: Arc<dyn crate::services::ProfileService> = mock_profile_service;
    ChatServiceImpl::new_for_tests(conversation_service, profile_service)
}

#[tokio::test]
async fn test_cancel() {
    let chat_service = make_basic_chat_service().await;
    let conversation_id = Uuid::new_v4();
    chat_service.cancel(conversation_id);
    assert!(!chat_service.is_streaming());
}

#[tokio::test]
async fn prepare_message_context_uses_conversation_profile_id() {
    crate::services::secure_store::use_mock_backend();
    crate::services::secure_store::api_keys::store(
        "_test_conv_prof",
        "fake-key-for-conv-profile-test",
    )
    .expect("store test key");

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

    let default_profile = crate::models::ModelProfile::new(
        "Default".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: "_test_conv_prof".to_string(),
        },
    );

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
async fn prepare_message_context_ignores_skill_lookup_failures() {
    crate::services::secure_store::use_mock_backend();
    crate::services::secure_store::api_keys::store(
        "_test_skills_failure",
        "fake-key-for-skills-failure-test",
    )
    .expect("store test key");

    let profile = crate::models::ModelProfile::new(
        "Default".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: "_test_skills_failure".to_string(),
        },
    );
    let profile_id = profile.id;

    let conversation_service = Arc::new(MockConversationService::new(profile_id))
        as Arc<dyn super::super::ConversationService>;
    let mock_profile_service = Arc::new(MockProfileService::new());
    mock_profile_service
        .set_default_profile(profile.clone())
        .await;
    mock_profile_service.add_profile(profile).await;
    let profile_service: Arc<dyn crate::services::ProfileService> = mock_profile_service;

    let app_settings = Arc::new(InMemoryAppSettingsService::new()) as Arc<dyn AppSettingsService>;
    let (view_tx, _view_rx) = tokio::sync::mpsc::channel(8);
    let chat_service = ChatServiceImpl::new(
        conversation_service,
        profile_service,
        app_settings,
        Arc::new(FailingSkillsService),
        view_tx,
        Arc::new(ApprovalGate::new()),
        Arc::new(tokio::sync::Mutex::new(ToolApprovalPolicy::default())),
    );

    let prepared = chat_service
        .prepare_message_context(Uuid::new_v4(), "hello".to_string())
        .await
        .expect("prepare_message_context should succeed even when skills lookup fails");

    assert!(
        !prepared.system_prompt.contains("available_skills"),
        "skills prompt block should be omitted when skill lookup fails"
    );

    let _ = crate::services::secure_store::api_keys::delete("_test_skills_failure");
}

#[tokio::test]
async fn cancel_clears_current_conversation_and_pending_approvals() {
    let app_settings = Arc::new(InMemoryAppSettingsService::new()) as Arc<dyn AppSettingsService>;
    let (service, mut view_rx, approval_gate) = make_approval_test_chat_service(app_settings);

    let conversation_id = Uuid::new_v4();
    service
        .begin_stream_for_test(conversation_id)
        .expect("begin_stream should succeed");

    let pending_request_id = Uuid::new_v4().to_string();
    let waiter = approval_gate.wait_for_approval(
        pending_request_id.clone(),
        "WriteFile".to_string(),
        conversation_id,
    );

    service.cancel(conversation_id);

    assert!(!waiter
        .wait()
        .await
        .expect("pending waiter should be resolved as denied on cancel"));
    assert!(!service.is_streaming_for(conversation_id));
    assert!(!service.is_streaming());

    let resolved_command = view_rx
        .recv()
        .await
        .expect("cancel should emit approval resolution for pending request");
    assert_eq!(
        resolved_command,
        ViewCommand::ToolApprovalResolved {
            conversation_id,
            request_id: pending_request_id,
            approved: false,
        }
    );
}
