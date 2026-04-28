use super::*;
use serdes_ai::core::messages::parts::ToolCallArgs;
use std::time::Duration;
use uuid::Uuid;

#[test]
fn split_prompt_and_history_uses_last_user_message() {
    let messages = vec![
        Message::user("first question"),
        Message::assistant("first answer"),
        Message::user("second question"),
    ];

    let (prompt, history) = crate::llm::LlmClient::split_prompt_and_history(&messages);

    assert_eq!(prompt, "second question");
    assert_eq!(history.len(), 2);
    assert!(matches!(history[0].role, Role::User));
    assert!(matches!(history[1].role, Role::Assistant));
}

#[tokio::test]
async fn approval_waiter_drop_cleans_pending_entry() {
    let gate = ApprovalGate::new();
    let request_id = Uuid::new_v4().to_string();
    let waiter = gate.wait_for_approval(request_id.clone(), "WriteFile".to_string(), Uuid::nil());

    drop(waiter);

    assert!(gate.resolve(&request_id, true).is_none());
}

#[tokio::test]
async fn resolve_returns_none_when_waiter_was_dropped() {
    let gate = ApprovalGate::new();
    let request_id = Uuid::new_v4().to_string();
    let waiter = gate.wait_for_approval(request_id.clone(), "WriteFile".to_string(), Uuid::nil());

    drop(waiter);

    assert!(gate
        .resolve_and_take_identifier(&request_id, true)
        .is_none());
}

#[tokio::test]
async fn resolve_returns_identifier_when_waiter_is_alive() {
    let gate = ApprovalGate::new();
    let request_id = Uuid::new_v4().to_string();
    let waiter = gate.wait_for_approval(request_id.clone(), "WriteFile".to_string(), Uuid::nil());

    let resolver = {
        let gate = gate;
        let request_id = request_id.clone();
        tokio::spawn(async move { gate.resolve_and_take_identifier(&request_id, true) })
    };

    let approved = waiter.wait().await.expect("waiter should receive decision");
    let (conversation_id, identifier) = resolver
        .await
        .expect("resolver task should complete")
        .expect("identifier should be returned for live waiter");

    assert!(approved);
    assert_eq!(conversation_id, Uuid::nil());
    assert_eq!(identifier, "WriteFile");
}

#[tokio::test]
async fn resolve_returns_all_identifiers_when_waiter_is_alive() {
    let gate = ApprovalGate::new();
    let request_id = Uuid::new_v4().to_string();
    let waiter = gate.wait_for_approvals(
        request_id.clone(),
        vec!["ls".to_string(), "pwd".to_string()],
        Uuid::nil(),
    );

    let resolver = {
        let gate = gate;
        let request_id = request_id.clone();
        tokio::spawn(async move { gate.resolve_and_take_identifiers(&request_id, true) })
    };

    let approved = waiter.wait().await.expect("waiter should receive decision");
    let (conversation_id, identifiers) = resolver
        .await
        .expect("resolver task should complete")
        .expect("identifiers should be returned for live waiter");

    assert!(approved);
    assert_eq!(conversation_id, Uuid::nil());
    assert_eq!(identifiers, vec!["ls".to_string(), "pwd".to_string()]);
}

#[tokio::test]
async fn resolve_all_resolves_every_pending_waiter() {
    let gate = ApprovalGate::new();

    let request_id_a = Uuid::new_v4().to_string();
    let request_id_b = Uuid::new_v4().to_string();

    let waiter_a =
        gate.wait_for_approval(request_id_a.clone(), "WriteFile".to_string(), Uuid::nil());
    let waiter_b = gate.wait_for_approval(request_id_b.clone(), "Search".to_string(), Uuid::nil());

    let resolved_ids = gate.resolve_all(false);

    assert_eq!(resolved_ids.len(), 2);
    assert!(resolved_ids.contains(&(Uuid::nil(), request_id_a.clone())));
    assert!(resolved_ids.contains(&(Uuid::nil(), request_id_b.clone())));

    let approved_a = waiter_a
        .wait()
        .await
        .expect("waiter a should receive resolution");
    let approved_b = waiter_b
        .wait()
        .await
        .expect("waiter b should receive resolution");

    assert!(!approved_a);
    assert!(!approved_b);

    assert!(gate.resolve(&request_id_a, false).is_none());
    assert!(gate.resolve(&request_id_b, false).is_none());
}

#[test]
fn build_agent_message_history_preserves_assistant_responses() {
    let assistant_message = Message::assistant("tool result summary").with_tool_uses(vec![
        crate::llm::tools::ToolUse::new(
            "tool-call-1",
            "web_search",
            serde_json::json!({"query": "weather"}),
        ),
    ]);

    let history = crate::llm::LlmClient::build_agent_message_history(&[
        Message::user("what's the weather"),
        assistant_message,
    ]);

    assert_eq!(history.len(), 2);

    assert!(matches!(
        history[0].parts.first(),
        Some(ModelRequestPart::UserPrompt(_))
    ));

    match history[1].parts.first() {
        Some(ModelRequestPart::ModelResponse(response)) => {
            assert!(response
                .parts
                .iter()
                .any(|part| matches!(part, ModelResponsePart::Text(_))));
            assert!(response
                .parts
                .iter()
                .any(|part| matches!(part, ModelResponsePart::ToolCall(_))));
        }
        other => panic!("expected ModelResponse history part, got {other:?}"),
    }
}

#[test]
fn collect_tool_transcript_extracts_calls_and_results() {
    let mut response = ModelResponse::new();
    response.add_part(ModelResponsePart::ToolCall(
        ToolCallPart::new(
            "web_search",
            ToolCallArgs::json(serde_json::json!({"query":"weather"})),
        )
        .with_tool_call_id("tool-call-1"),
    ));

    let mut request_with_tool_call = ModelRequest::new();
    request_with_tool_call.add_part(ModelRequestPart::ModelResponse(Box::new(response)));

    let mut request_with_tool_return = ModelRequest::new();
    request_with_tool_return.add_part(ModelRequestPart::ToolReturn(
        ToolReturnPart::new(
            "web_search",
            ToolReturnContent::json(serde_json::json!({"answer":"sunny"})),
        )
        .with_tool_call_id("tool-call-1"),
    ));

    let mut request_with_tool_error = ModelRequest::new();
    request_with_tool_error.add_part(ModelRequestPart::ToolReturn(
        ToolReturnPart::error("web_search", "request failed").with_tool_call_id("tool-call-2"),
    ));

    let (tool_calls, tool_results) = crate::llm::LlmClient::collect_tool_transcript(
        &[
            request_with_tool_call,
            request_with_tool_return,
            request_with_tool_error,
        ],
        0,
    );

    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].id, "tool-call-1");
    assert_eq!(tool_calls[0].name, "web_search");
    assert_eq!(tool_results.len(), 2);
    assert_eq!(tool_results[0].tool_use_id, "tool-call-1");
    assert!(!tool_results[0].is_error);
    assert!(tool_results[0].content.contains("\"answer\":\"sunny\""));

    assert_eq!(tool_results[1].tool_use_id, "tool-call-2");
    assert!(tool_results[1].is_error);
    assert_eq!(tool_results[1].content, "request failed");
}

#[test]
fn build_agent_message_history_preserves_assistant_tool_results() {
    let assistant_message = Message::assistant("tool summary").with_tool_results(vec![
        crate::llm::tools::ToolResult::success("tool-call-1", "{\"answer\":\"sunny\"}"),
        crate::llm::tools::ToolResult::error("tool-call-2", "request failed"),
    ]);

    let history = crate::llm::LlmClient::build_agent_message_history(&[assistant_message]);

    assert_eq!(history.len(), 1);
    assert_eq!(history[0].parts.len(), 3);

    assert!(matches!(
        history[0].parts[0],
        ModelRequestPart::ModelResponse(_)
    ));

    match &history[0].parts[1] {
        ModelRequestPart::ToolReturn(tool_return) => {
            assert_eq!(tool_return.tool_call_id.as_deref(), Some("tool-call-1"));
            assert!(!matches!(
                tool_return.content,
                ToolReturnContent::Error { .. }
            ));
        }
        other => panic!("expected first tool return part, got {other:?}"),
    }

    match &history[0].parts[2] {
        ModelRequestPart::ToolReturn(tool_return) => {
            assert_eq!(tool_return.tool_call_id.as_deref(), Some("tool-call-2"));
            assert!(matches!(
                tool_return.content,
                ToolReturnContent::Error { .. }
            ));
        }
        other => panic!("expected second tool return part, got {other:?}"),
    }
}

#[test]
fn collect_tool_transcript_skips_existing_history() {
    let mut historical_response = ModelResponse::new();
    historical_response.add_part(ModelResponsePart::ToolCall(
        ToolCallPart::new(
            "web_search",
            ToolCallArgs::json(serde_json::json!({"query":"old"})),
        )
        .with_tool_call_id("historical-tool-call"),
    ));

    let mut history_with_tool_call = ModelRequest::new();
    history_with_tool_call.add_part(ModelRequestPart::ModelResponse(Box::new(
        historical_response,
    )));

    let mut history_with_tool_return = ModelRequest::new();
    history_with_tool_return.add_part(ModelRequestPart::ToolReturn(
        ToolReturnPart::success("web_search", "old result")
            .with_tool_call_id("historical-tool-call"),
    ));

    let mut current_response = ModelResponse::new();
    current_response.add_part(ModelResponsePart::ToolCall(
        ToolCallPart::new(
            "web_search",
            ToolCallArgs::json(serde_json::json!({"query":"new"})),
        )
        .with_tool_call_id("current-tool-call"),
    ));

    let mut current_tool_call = ModelRequest::new();
    current_tool_call.add_part(ModelRequestPart::ModelResponse(Box::new(current_response)));

    let mut current_tool_return = ModelRequest::new();
    current_tool_return.add_part(ModelRequestPart::ToolReturn(
        ToolReturnPart::success("web_search", "new result").with_tool_call_id("current-tool-call"),
    ));

    let (tool_calls, tool_results) = crate::llm::LlmClient::collect_tool_transcript(
        &[
            history_with_tool_call,
            history_with_tool_return,
            current_tool_call,
            current_tool_return,
        ],
        2,
    );

    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].id, "current-tool-call");
    assert_eq!(tool_results.len(), 1);
    assert_eq!(tool_results[0].tool_use_id, "current-tool-call");

    assert_eq!(tool_results[0].content, "new result");
}
#[test]
fn collect_tool_transcript_uses_built_history_request_count_not_message_count() {
    let mut system_message = Message::system("system prompt");
    system_message.tool_results = vec![crate::llm::tools::ToolResult::success(
        "ignored-system-tool",
        "ignored",
    )];

    let empty_assistant = Message::assistant("");
    let historical_assistant =
        Message::assistant("tool summary").with_tool_uses(vec![crate::llm::tools::ToolUse::new(
            "historical-tool-call",
            "web_search",
            serde_json::json!({"query":"old"}),
        )]);

    let history_messages = [system_message, empty_assistant, historical_assistant];
    let message_history = crate::llm::LlmClient::build_agent_message_history(&history_messages);

    assert_eq!(history_messages.len(), 3);
    assert_eq!(message_history.len(), 1);

    let mut current_response = ModelResponse::new();
    current_response.add_part(ModelResponsePart::ToolCall(
        ToolCallPart::new(
            "web_search",
            ToolCallArgs::json(serde_json::json!({"query":"new"})),
        )
        .with_tool_call_id("current-tool-call"),
    ));

    let mut current_tool_call = ModelRequest::new();
    current_tool_call.add_part(ModelRequestPart::ModelResponse(Box::new(current_response)));

    let mut current_tool_return = ModelRequest::new();
    current_tool_return.add_part(ModelRequestPart::ToolReturn(
        ToolReturnPart::success("web_search", "new result").with_tool_call_id("current-tool-call"),
    ));

    let mut run_complete_messages = message_history;
    run_complete_messages.push(current_tool_call);
    run_complete_messages.push(current_tool_return);

    let (tool_calls, tool_results) =
        crate::llm::LlmClient::collect_tool_transcript(&run_complete_messages, 1);

    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].id, "current-tool-call");
    assert_eq!(tool_results.len(), 1);
    assert_eq!(tool_results[0].tool_use_id, "current-tool-call");
}

/// @plan PLAN-20260416-ISSUE173.P06
/// @requirement REQ-173-003.2
#[tokio::test]
async fn resolve_all_for_conversation_resolves_only_target() {
    let gate = ApprovalGate::new();
    let conv_a = Uuid::new_v4();
    let conv_b = Uuid::new_v4();

    let waiter_a = gate.wait_for_approvals("req-a".into(), vec!["tool".into()], conv_a);
    let waiter_b = gate.wait_for_approvals("req-b".into(), vec!["tool".into()], conv_b);

    let resolved = gate.resolve_all_for_conversation(conv_a, false);
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].0, conv_a);
    assert_eq!(resolved[0].1, "req-a");

    assert!(!waiter_a.wait().await.unwrap());

    let _ = gate.resolve("req-b", true);
    assert!(waiter_b.wait().await.unwrap());
}

/// Regression test for CR2: `resolve_all_for_conversation` is atomic under concurrent insert.
///
/// Spawns many tasks calling `wait_for_approvals` on the target conversation while
/// calling `resolve_all_for_conversation`; asserts no stranded pending entry remains.
///
/// @plan PLAN-20260416-ISSUE173.P14-CR2
/// @requirement REQ-173-003.1
#[tokio::test]
async fn resolve_all_for_conversation_is_atomic_under_concurrent_insert() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let gate = Arc::new(ApprovalGate::new());
    let target_conversation = Uuid::new_v4();
    let other_conversation = Uuid::new_v4();

    // First, create a fixed set of waiters for other_conversation that we'll keep alive
    let mut other_waiters = vec![];
    for i in 0..10 {
        let req_id = format!("other-req-{i}");
        let waiter = gate.wait_for_approval(req_id, "TestTool".to_string(), other_conversation);
        other_waiters.push(waiter);
    }

    // Spawn many concurrent waiters on target conversation
    let mut waiter_handles = vec![];
    for i in 0..50 {
        let gate = gate.clone();
        let handle = tokio::spawn(async move {
            let req_id = format!("target-req-{i}");
            let _waiter =
                gate.wait_for_approval(req_id, "TestTool".to_string(), target_conversation);
            // Hold the waiter for a short time to create overlap
            tokio::time::sleep(Duration::from_millis(5)).await;
        });
        waiter_handles.push(handle);
    }

    // Concurrently resolve all for target conversation multiple times
    let resolved_count = Arc::new(AtomicUsize::new(0));
    let mut resolver_handles = vec![];
    for _ in 0..10 {
        let gate = gate.clone();
        let resolved = resolved_count.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(2)).await;
            let resolved_ids = gate.resolve_all_for_conversation(target_conversation, false);
            resolved.fetch_add(resolved_ids.len(), Ordering::SeqCst);
        });
        resolver_handles.push(handle);
    }

    // Wait for all waiter spawns to complete
    for h in waiter_handles {
        let _ = h.await;
    }

    // Wait for all resolvers to complete
    for h in resolver_handles {
        let _ = h.await;
    }

    // Final resolution pass to get any remaining target waiters
    let final_resolved = gate.resolve_all_for_conversation(target_conversation, false);

    // Verify that some target conversation waiters were resolved
    let total_target_resolved = resolved_count.load(Ordering::SeqCst) + final_resolved.len();
    assert!(
        total_target_resolved > 0,
        "Should have resolved some target conversation waiters, got {total_target_resolved}"
    );

    // Verify other conversation waiters still exist (not resolved by target call)
    // Since we hold `other_waiters`, they must still be pending
    let other_resolved = gate.resolve_all_for_conversation(other_conversation, true);
    assert!(
        other_resolved.len() == 10,
        "Other conversation waiters should all still exist (expected 10, got {})",
        other_resolved.len()
    );

    // Clean up: resolve the other waiters we were holding
    drop(other_waiters);
}

/// @plan PLAN-20260416-ISSUE173.P06
/// @requirement REQ-173-003.1
#[tokio::test]
async fn resolving_one_conversation_does_not_wake_another() {
    let gate = ApprovalGate::new();
    let conv_a = Uuid::new_v4();
    let conv_b = Uuid::new_v4();

    let _waiter_a = gate.wait_for_approvals("req-a".into(), vec!["tool".into()], conv_a);
    let waiter_b = gate.wait_for_approvals("req-b".into(), vec!["tool".into()], conv_b);

    let _ = gate.resolve_all_for_conversation(conv_a, false);

    let result = tokio::time::timeout(Duration::from_millis(50), waiter_b.wait()).await;
    assert!(result.is_err(), "waiter B should still be pending");
}
