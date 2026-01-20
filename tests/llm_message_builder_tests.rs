use personal_agent::{Conversation, Message, MessageRole};
use uuid::Uuid;

#[test]
fn message_builder_sets_thinking_and_roles() {
    let assistant = Message::assistant("Answer".to_string());
    assert_eq!(assistant.role, MessageRole::Assistant);
    assert_eq!(assistant.thinking_content, None);

    let thinking = Message::assistant_with_thinking("Answer".to_string(), "Reasoning".to_string());
    assert_eq!(thinking.role, MessageRole::Assistant);
    assert_eq!(thinking.thinking_content.as_deref(), Some("Reasoning"));

    let system = Message::system("System".to_string());
    assert_eq!(system.role, MessageRole::System);
}

#[test]
fn conversation_add_message_updates_title_and_messages() {
    let profile_id = Uuid::new_v4();
    let mut conversation = Conversation::new(profile_id);
    let original_title = conversation.title.clone();

    conversation.add_message(Message::user("Hello".to_string()));
    conversation.set_title("Updated".to_string());

    assert_eq!(conversation.messages.len(), 1);
    assert_eq!(conversation.title, Some("Updated".to_string()));
    assert_ne!(conversation.title, original_title);
}
