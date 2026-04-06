use super::*;
use serdes_ai::core::messages::parts::ToolCallArgs;
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
    let waiter = gate.wait_for_approval(request_id.clone(), "WriteFile".to_string());

    drop(waiter);

    assert!(gate.resolve(&request_id, true).is_none());
}

#[tokio::test]
async fn resolve_returns_none_when_waiter_was_dropped() {
    let gate = ApprovalGate::new();
    let request_id = Uuid::new_v4().to_string();
    let waiter = gate.wait_for_approval(request_id.clone(), "WriteFile".to_string());

    drop(waiter);

    assert!(gate
        .resolve_and_take_identifier(&request_id, true)
        .is_none());
}

#[tokio::test]
async fn resolve_returns_identifier_when_waiter_is_alive() {
    let gate = ApprovalGate::new();
    let request_id = Uuid::new_v4().to_string();
    let waiter = gate.wait_for_approval(request_id.clone(), "WriteFile".to_string());

    let resolver = {
        let gate = gate;
        let request_id = request_id.clone();
        tokio::spawn(async move { gate.resolve_and_take_identifier(&request_id, true) })
    };

    let approved = waiter.wait().await.expect("waiter should receive decision");
    let identifier = resolver
        .await
        .expect("resolver task should complete")
        .expect("identifier should be returned for live waiter");

    assert!(approved);
    assert_eq!(identifier, "WriteFile");
}

#[tokio::test]
async fn resolve_returns_all_identifiers_when_waiter_is_alive() {
    let gate = ApprovalGate::new();
    let request_id = Uuid::new_v4().to_string();
    let waiter = gate.wait_for_approvals(
        request_id.clone(),
        vec!["ls".to_string(), "pwd".to_string()],
    );

    let resolver = {
        let gate = gate;
        let request_id = request_id.clone();
        tokio::spawn(async move { gate.resolve_and_take_identifiers(&request_id, true) })
    };

    let approved = waiter.wait().await.expect("waiter should receive decision");
    let identifiers = resolver
        .await
        .expect("resolver task should complete")
        .expect("identifiers should be returned for live waiter");

    assert!(approved);
    assert_eq!(identifiers, vec!["ls".to_string(), "pwd".to_string()]);
}

#[tokio::test]
async fn resolve_all_resolves_every_pending_waiter() {
    let gate = ApprovalGate::new();

    let request_id_a = Uuid::new_v4().to_string();
    let request_id_b = Uuid::new_v4().to_string();

    let waiter_a = gate.wait_for_approval(request_id_a.clone(), "WriteFile".to_string());
    let waiter_b = gate.wait_for_approval(request_id_b.clone(), "Search".to_string());

    let resolved_ids = gate.resolve_all(false);

    assert_eq!(resolved_ids.len(), 2);
    assert!(resolved_ids.contains(&request_id_a));
    assert!(resolved_ids.contains(&request_id_b));

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

    let (tool_calls, tool_results) = crate::llm::LlmClient::collect_tool_transcript(&[
        request_with_tool_call,
        request_with_tool_return,
        request_with_tool_error,
    ]);

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
