use personal_agent::llm::tools::{Tool, ToolResult, ToolUse};

#[test]
fn tool_use_and_result_builders_keep_inputs() {
    let tool_use = ToolUse::new("id", "search", serde_json::json!({"q": "hi"}));
    assert_eq!(tool_use.id, "id");
    assert_eq!(tool_use.name, "search");
    assert_eq!(tool_use.input["q"], "hi");

    let result = ToolResult::success("id", "ok");
    assert!(!result.is_error);
    assert_eq!(result.tool_use_id, "id");
    assert_eq!(result.content, "ok");
}

#[test]
fn tool_result_error_sets_flag() {
    let error = ToolResult::error("call-err", "boom");
    assert!(error.is_error);
    assert_eq!(error.tool_use_id, "call-err");
    assert_eq!(error.content, "boom");
}

#[test]
fn tool_builder_sets_schema() {
    let tool = Tool::new(
        "summarize",
        "Summarize input",
        serde_json::json!({"type": "object", "properties": {"text": {"type": "string"}}}),
    );
    assert_eq!(tool.name, "summarize");
    assert_eq!(tool.description, "Summarize input");
    assert_eq!(tool.input_schema["type"], "object");
    assert!(tool.input_schema["properties"]["text"].is_object());
}
