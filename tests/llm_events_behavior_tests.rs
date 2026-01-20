use personal_agent::llm::events::ChatStreamEvent;

#[test]
fn chat_stream_event_helpers_expose_payloads() {
    let text = ChatStreamEvent::text("hello".to_string());
    let thinking = ChatStreamEvent::thinking("plan".to_string());
    let complete = ChatStreamEvent::complete(Some(10), Some(20));
    let error = ChatStreamEvent::error("oops".to_string(), true);

    assert!(text.is_text());
    assert_eq!(text.as_text(), Some("hello"));
    assert!(thinking.is_thinking());
    assert_eq!(thinking.as_thinking(), Some("plan"));
    assert!(complete.is_complete());
    assert!(!complete.is_error());
    assert!(error.is_error());
    assert_eq!(error.as_text(), None);
}
