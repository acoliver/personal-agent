use personal_agent::llm::{Message, Role};

#[test]
fn llm_message_builder_tracks_thinking() {
    let message = Message::assistant("Answer").with_thinking("Thought");
    assert_eq!(message.role, Role::Assistant);
    assert_eq!(message.content, "Answer");
    assert_eq!(message.thinking_content.as_deref(), Some("Thought"));
}

#[test]
fn llm_message_builder_tracks_tool_results() {
    let result = personal_agent::llm::tools::ToolResult::success("tool1", "ok");
    let message = Message::user("input").with_tool_results(vec![result]);

    assert_eq!(message.role, Role::User);
    assert_eq!(message.tool_results.len(), 1);
    assert!(message.has_tool_results());
}
