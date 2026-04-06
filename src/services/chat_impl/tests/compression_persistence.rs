use super::*;

#[test]
fn build_llm_messages_preserves_assistant_tool_transcript() {
    let profile = crate::models::ModelProfile::default();
    let mut conversation = crate::models::Conversation::new(profile.id);
    conversation.add_message(Message::user("hello".to_string()));

    let mut assistant = Message::assistant("answer with tool context".to_string());
    assistant.tool_calls = Some(
        serde_json::to_string(&[crate::llm::tools::ToolUse::new(
            "tool-call-1",
            "read_file",
            serde_json::json!({"path":"/tmp/text.txt"}),
        )])
        .expect("tool calls should serialize"),
    );
    assistant.tool_results = Some(
        serde_json::to_string(&[crate::llm::tools::ToolResult::success(
            "tool-call-1",
            "contents",
        )])
        .expect("tool results should serialize"),
    );
    conversation.add_message(assistant);

    let assistant = ChatServiceImpl::build_llm_messages(&conversation, &profile)
        .into_iter()
        .find(|message| matches!(message.role, crate::llm::Role::Assistant))
        .expect("assistant message should be present");

    assert_eq!(assistant.tool_uses.len(), 1);
    assert_eq!(assistant.tool_results.len(), 1);
    assert_eq!(assistant.tool_uses[0].id, "tool-call-1");
    assert_eq!(assistant.tool_results[0].tool_use_id, "tool-call-1");
    assert_eq!(assistant.tool_results[0].content, "contents");
    assert!(!assistant.tool_results[0].is_error);
}

#[tokio::test]
async fn prepare_message_context_uses_persisted_compression_config() {
    crate::services::secure_store::use_mock_backend();
    crate::services::secure_store::api_keys::store(
        "_test_compression_config",
        "fake-key-for-compression-config-test",
    )
    .expect("store test key");

    let profile = crate::models::ModelProfile::new(
        "Compression Test".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: "_test_compression_config".to_string(),
        },
    );
    let conversation_service_impl = Arc::new(MockConversationService::new(profile.id));
    let conversation_service =
        conversation_service_impl.clone() as Arc<dyn super::super::ConversationService>;
    let mock_profile_service = Arc::new(MockProfileService::new());
    mock_profile_service
        .set_default_profile(profile.clone())
        .await;
    mock_profile_service.add_profile(profile.clone()).await;
    let profile_service: Arc<dyn crate::services::ProfileService> = mock_profile_service;

    let app_settings_impl = Arc::new(InMemoryAppSettingsService::new());
    app_settings_impl
        .set_setting(
            "compression",
            serde_json::to_string(&crate::config::CompressionConfig {
                observation_mask_threshold: 0.0,
                sandwich_summary_threshold: 2.0,
                truncation_threshold: 3.0,
                mask_recent_count: 0,
                mask_size_threshold: 1,
                ..crate::config::CompressionConfig::default()
            })
            .expect("compression config should serialize"),
        )
        .await
        .expect("compression config should persist");
    let app_settings = app_settings_impl as Arc<dyn AppSettingsService>;

    let (view_tx, _view_rx) = tokio::sync::mpsc::channel(8);
    let approval_gate = Arc::new(ApprovalGate::new());
    let service = ChatServiceImpl::new(
        conversation_service,
        profile_service,
        app_settings,
        view_tx,
        approval_gate,
        Arc::new(AsyncMutex::new(ToolApprovalPolicy::default())),
    );

    let mut assistant = Message::assistant(String::new());
    assistant.tool_calls = Some(
        serde_json::to_string(&[crate::llm::tools::ToolUse::new(
            "tool-call-1",
            "read_file",
            serde_json::json!({"path":"/tmp/file.txt"}),
        )])
        .expect("tool calls should serialize"),
    );
    assistant.tool_results = Some(
        serde_json::to_string(&[crate::llm::tools::ToolResult::success(
            "tool-call-1",
            "12345678910",
        )])
        .expect("tool results should serialize"),
    );
    conversation_service_impl
        .messages
        .write()
        .await
        .push(assistant);

    let prepared = service
        .prepare_message_context(Uuid::new_v4(), "hello".to_string())
        .await
        .expect("prepare_message_context should succeed");

    assert_eq!(
        prepared.compression_result.phase,
        crate::models::CompressionPhase::ObservationMasked,
        "persisted compression settings should drive live compression behavior"
    );

    let _ = crate::services::secure_store::api_keys::delete("_test_compression_config");
}

#[tokio::test]
async fn persist_context_state_preserves_existing_fields() {
    let conversation_service_impl = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let conversation_service =
        conversation_service_impl.clone() as Arc<dyn super::super::ConversationService>;
    let conversation_id = Uuid::new_v4();

    *conversation_service_impl.context_state.write().await = Some(crate::models::ContextState {
        strategy: Some("keep-existing-strategy".to_string()),
        summary: Some("existing summary".to_string()),
        visible_range: Some((3, 9)),
        ..crate::models::ContextState::default()
    });

    persist_context_state(
        &conversation_service,
        conversation_id,
        crate::compression::pipeline::CompressionResult {
            messages: vec![crate::llm::Message::user("hello")],
            phase: crate::models::CompressionPhase::ObservationMasked,
            masked_tool_seqs: Some(vec![1]),
            summary_range: Some((1, 2)),
            preserved_facts: Some(vec!["fact".to_string()]),
            estimated_tokens: 42,
        },
        Some(10),
        Some(20),
    )
    .await;

    let stored_state = conversation_service_impl
        .context_state
        .read()
        .await
        .clone()
        .expect("context state should be persisted");

    assert_eq!(
        stored_state.strategy.as_deref(),
        Some("keep-existing-strategy")
    );
    assert_eq!(stored_state.summary.as_deref(), Some("existing summary"));
    assert_eq!(stored_state.visible_range, Some((3, 9)));
    assert_eq!(
        stored_state.compression_phase,
        Some(crate::models::CompressionPhase::ObservationMasked)
    );
    assert_eq!(stored_state.last_input_tokens, Some(10));
    assert_eq!(stored_state.last_output_tokens, Some(20));
}
