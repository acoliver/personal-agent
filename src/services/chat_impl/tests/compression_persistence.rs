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
    let skills_service = Arc::new(
        crate::services::SkillsServiceImpl::new(app_settings.clone())
            .expect("skills service should initialize"),
    ) as Arc<dyn crate::services::SkillsService>;
    let service = ChatServiceImpl::new(
        conversation_service,
        profile_service,
        app_settings,
        skills_service,
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

#[test]
fn stream_diagnostic_context_uses_profile_metadata_host_only() {
    let profile = crate::models::ModelProfile {
        name: "token=profile-secret".to_string(),
        provider_id: "provider-secret".to_string(),
        model_id: "model-secret".to_string(),
        base_url: "https://api.example.test/v1?api_key=hidden".to_string(),
        ..crate::models::ModelProfile::default()
    };

    let context = streaming::StreamDiagnosticContext::from_profile(&profile);

    assert_eq!(context.profile_id, profile.id);
    assert_eq!(context.profile_name, "token=profile-secret");
    assert_eq!(context.provider_id, "provider-secret");
    assert_eq!(context.model_id, "model-secret");
    assert_eq!(context.base_url_host.as_deref(), Some("api.example.test"));
}

#[test]
fn handle_llm_stream_event_records_transcript_and_emits_events() {
    let conversation_id = Uuid::new_v4();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut response_text = String::new();
    let mut thinking_text = String::new();
    let mut tool_calls = Vec::new();
    let mut tool_results = Vec::new();
    let mut input_tokens = None;
    let mut output_tokens = None;
    let mut completed = false;

    streaming::handle_llm_stream_event(
        crate::llm::StreamEvent::TextDelta("hello".to_string()),
        conversation_id,
        &tx,
        &mut response_text,
        &mut thinking_text,
        &mut tool_calls,
        &mut tool_results,
        &mut input_tokens,
        &mut output_tokens,
        &mut completed,
    );
    streaming::handle_llm_stream_event(
        crate::llm::StreamEvent::ThinkingDelta("think".to_string()),
        conversation_id,
        &tx,
        &mut response_text,
        &mut thinking_text,
        &mut tool_calls,
        &mut tool_results,
        &mut input_tokens,
        &mut output_tokens,
        &mut completed,
    );
    streaming::handle_llm_stream_event(
        crate::llm::StreamEvent::ToolTranscript {
            tool_calls: vec![crate::llm::ToolUse::new(
                "call-1",
                "search",
                serde_json::json!({"query":"rust"}),
            )],
            tool_results: vec![crate::llm::ToolResult::success("call-1", "ok")],
        },
        conversation_id,
        &tx,
        &mut response_text,
        &mut thinking_text,
        &mut tool_calls,
        &mut tool_results,
        &mut input_tokens,
        &mut output_tokens,
        &mut completed,
    );
    streaming::handle_llm_stream_event(
        crate::llm::StreamEvent::Complete {
            input_tokens: Some(7),
            output_tokens: Some(11),
        },
        conversation_id,
        &tx,
        &mut response_text,
        &mut thinking_text,
        &mut tool_calls,
        &mut tool_results,
        &mut input_tokens,
        &mut output_tokens,
        &mut completed,
    );

    assert_eq!(response_text, "hello");
    assert_eq!(thinking_text, "think");
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_results.len(), 1);
    assert_eq!(input_tokens, Some(7));
    assert_eq!(output_tokens, Some(11));
    assert!(completed);
    match rx.try_recv().expect("text token should be sent") {
        ChatStreamEvent::Token(token) => assert_eq!(token, "hello"),
        other => panic!("expected token event, got {other:?}"),
    }
    match rx.try_recv().expect("completion event should be sent") {
        ChatStreamEvent::Complete {
            input_tokens,
            output_tokens,
        } => {
            assert_eq!(input_tokens, Some(7));
            assert_eq!(output_tokens, Some(11));
        }
        other => panic!("expected completion event, got {other:?}"),
    }
}

#[test]
fn build_stream_error_diagnostics_sanitizes_and_summarizes_context() {
    let profile_id = Uuid::new_v4();
    let context = streaming::StreamDiagnosticContext {
        profile_id,
        profile_name: "secret=profile-token".to_string(),
        provider_id: "provider".to_string(),
        model_id: "model".to_string(),
        base_url_host: Some("api.example.test".to_string()),
    };
    let transcript = streaming::StreamTranscript {
        response_text: "partial assistant text".to_string(),
        thinking_text: "hidden thinking".to_string(),
        tool_calls: vec![crate::llm::ToolUse::new(
            "call-secret",
            "web_search",
            serde_json::json!({"query":"diagnostics"}),
        )],
        tool_results: vec![crate::llm::ToolResult::error(
            "call-secret",
            format!("{} token=abc123", "x".repeat(260)),
        )],
        input_tokens: Some(13),
        output_tokens: Some(17),
        completed: false,
    };

    let diagnostics = streaming::build_stream_error_diagnostics(
        Some("provider failed authorization: Bearer abc123"),
        &context,
        &transcript,
        crate::ui_gpui::error_log::ErrorLogStreamLifecycle::Failed,
    );

    assert_eq!(diagnostics.profile_id, Some(profile_id));
    assert_eq!(
        diagnostics.profile_name.as_deref(),
        Some("secret=[REDACTED]")
    );
    assert_eq!(
        diagnostics.base_url_host.as_deref(),
        Some("api.example.test")
    );
    assert_eq!(diagnostics.input_tokens, Some(13));
    assert_eq!(diagnostics.output_tokens, Some(17));
    assert_eq!(diagnostics.partial_assistant_response_len, Some(22));
    assert_eq!(diagnostics.thinking_len, Some(15));
    assert_eq!(diagnostics.tool_calls.len(), 1);
    let tool = &diagnostics.tool_calls[0];
    assert_eq!(tool.tool_name, "web_search");
    assert_eq!(tool.tool_call_id.as_deref(), Some("call-secret"));
    assert_eq!(tool.success, Some(false));
    let summary = tool
        .summary
        .as_deref()
        .expect("tool summary should be captured");
    assert!(summary.contains("chars total"));
    assert!(!summary.contains("abc123"));
    assert_eq!(
        diagnostics.underlying_error.as_deref(),
        Some("provider failed authorization: [REDACTED]")
    );
    assert_eq!(diagnostics.recent_events, vec!["stream error emitted"]);
}

#[tokio::test]
async fn persist_assistant_response_stores_content_thinking_and_tool_json() {
    let conversation_service_impl = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let conversation_service =
        conversation_service_impl.clone() as Arc<dyn super::super::ConversationService>;
    let conversation_id = Uuid::new_v4();
    let tool_calls = vec![crate::llm::ToolUse::new(
        "call-1",
        "web_search",
        serde_json::json!({"query":"coverage"}),
    )];
    let tool_results = vec![crate::llm::ToolResult::success("call-1", "result")];

    streaming::persist_assistant_response(
        &conversation_service,
        conversation_id,
        "assistant text",
        "thinking text",
        &tool_calls,
        &tool_results,
        "Coverage Model",
    )
    .await;

    let message = {
        let messages = conversation_service_impl.messages.read().await;
        assert_eq!(messages.len(), 1);
        messages[0].clone()
    };
    assert_eq!(message.content, "assistant text");
    assert_eq!(message.thinking_content.as_deref(), Some("thinking text"));
    assert_eq!(message.model_id.as_deref(), Some("Coverage Model"));

    assert_non_empty_tool_json::<crate::llm::ToolUse>(
        message.tool_calls.as_deref(),
        "tool calls should deserialize",
        "tool calls should not be empty",
    );
    assert_non_empty_tool_json::<crate::llm::ToolResult>(
        message.tool_results.as_deref(),
        "tool results should deserialize",
        "tool results should not be empty",
    );
}

#[tokio::test]
async fn persist_context_state_covers_existing_new_and_error_paths() {
    let conversation_service_impl = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let conversation_service =
        conversation_service_impl.clone() as Arc<dyn super::super::ConversationService>;
    let conversation_id = Uuid::new_v4();
    let compression = CompressionResult {
        messages: vec![crate::llm::Message::user("hello")],
        phase: crate::models::CompressionPhase::Summarized,
        masked_tool_seqs: Some(vec![2, 4]),
        summary_range: Some((1, 3)),
        preserved_facts: Some(vec!["fact".to_string()]),
        estimated_tokens: 42,
    };

    streaming::persist_context_state(
        &conversation_service,
        conversation_id,
        compression.clone(),
        Some(10),
        Some(20),
    )
    .await;

    let state = conversation_service_impl
        .context_state
        .read()
        .await
        .clone()
        .expect("context state should be persisted");
    assert_eq!(
        state.compression_phase,
        Some(crate::models::CompressionPhase::Summarized)
    );
    assert_eq!(state.masked_tool_seqs, Some(vec![2, 4]));
    assert_eq!(state.summary_range, Some((1, 3)));
    assert_eq!(state.preserved_facts, Some(vec!["fact".to_string()]));

    assert_eq!(state.last_input_tokens, Some(10));
    assert_eq!(state.last_output_tokens, Some(20));

    streaming::persist_context_state(
        &conversation_service,
        conversation_id,
        CompressionResult {
            messages: vec![],
            phase: crate::models::CompressionPhase::None,
            masked_tool_seqs: None,
            summary_range: None,
            preserved_facts: None,
            estimated_tokens: 0,
        },
        None,
        None,
    )
    .await;

    assert!(conversation_service_impl
        .context_state
        .read()
        .await
        .is_some());
}
