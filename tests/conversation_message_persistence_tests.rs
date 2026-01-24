//! Tests for conversation message persistence
//!
//! These tests verify that messages sent in a conversation are properly
//! saved to storage and can be loaded when the conversation is reopened.

use personal_agent::models::{Conversation, Message, MessageRole};
use personal_agent::storage::ConversationStorage;
use tempfile::TempDir;
use uuid::Uuid;

/// Helper to create a storage in a temp directory
fn create_temp_storage() -> (ConversationStorage, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let storage = ConversationStorage::new(temp_dir.path().to_path_buf());
    (storage, temp_dir)
}

/// Helper to find and load a conversation by ID from storage
fn load_by_id(storage: &ConversationStorage, id: Uuid) -> Option<Conversation> {
    if let Ok(filenames) = storage.list() {
        for filename in filenames {
            if let Ok(conv) = storage.load(&filename) {
                if conv.id == id {
                    return Some(conv);
                }
            }
        }
    }
    None
}

/// When a user sends a message, it should be saved to the conversation file
#[test]
fn user_message_is_persisted_to_storage() {
    let (storage, _temp) = create_temp_storage();
    
    // Create a new conversation
    let mut conversation = Conversation::new(Uuid::new_v4());
    conversation.title = Some("Test Conversation".to_string());
    
    // Add a user message (simulating send_message)
    conversation.add_message(Message::user("Hello, world!".to_string()));
    
    // Save the conversation (this should happen after adding message)
    storage.save(&conversation).unwrap();
    
    // Reload and verify the message was persisted
    let loaded = load_by_id(&storage, conversation.id).expect("Should find conversation");
    
    assert_eq!(loaded.messages.len(), 1);
    assert_eq!(loaded.messages[0].content, "Hello, world!");
    assert!(matches!(loaded.messages[0].role, MessageRole::User));
}

/// When an assistant responds, the response should be saved to the conversation file
#[test]
fn assistant_response_is_persisted_to_storage() {
    let (storage, _temp) = create_temp_storage();
    
    // Create a conversation with a user message
    let mut conversation = Conversation::new(Uuid::new_v4());
    conversation.add_message(Message::user("What is 2+2?".to_string()));
    storage.save(&conversation).unwrap();
    
    // Simulate assistant response (after streaming completes)
    conversation.add_message(Message::assistant("2+2 equals 4.".to_string()));
    storage.save(&conversation).unwrap();
    
    // Reload and verify both messages were persisted
    let loaded = load_by_id(&storage, conversation.id).expect("Should find conversation");
    
    assert_eq!(loaded.messages.len(), 2);
    assert_eq!(loaded.messages[0].content, "What is 2+2?");
    assert!(matches!(loaded.messages[0].role, MessageRole::User));
    assert_eq!(loaded.messages[1].content, "2+2 equals 4.");
    assert!(matches!(loaded.messages[1].role, MessageRole::Assistant));
}

/// Multiple messages in a conversation should all be persisted
#[test]
fn multiple_messages_are_persisted() {
    let (storage, _temp) = create_temp_storage();
    
    let mut conversation = Conversation::new(Uuid::new_v4());
    
    // Simulate a multi-turn conversation
    conversation.add_message(Message::user("Hi".to_string()));
    storage.save(&conversation).unwrap();
    
    conversation.add_message(Message::assistant("Hello! How can I help?".to_string()));
    storage.save(&conversation).unwrap();
    
    conversation.add_message(Message::user("What's the weather?".to_string()));
    storage.save(&conversation).unwrap();
    
    conversation.add_message(Message::assistant("I don't have access to weather data.".to_string()));
    storage.save(&conversation).unwrap();
    
    // Reload and verify all messages
    let loaded = load_by_id(&storage, conversation.id).expect("Should find conversation");
    
    assert_eq!(loaded.messages.len(), 4);
    assert_eq!(loaded.messages[0].content, "Hi");
    assert_eq!(loaded.messages[1].content, "Hello! How can I help?");
    assert_eq!(loaded.messages[2].content, "What's the weather?");
    assert_eq!(loaded.messages[3].content, "I don't have access to weather data.");
}

/// When switching to a conversation, its messages should be loaded
#[test]
fn switching_conversation_loads_messages() {
    let (storage, _temp) = create_temp_storage();
    
    // Create first conversation with messages
    let mut conv1 = Conversation::new(Uuid::new_v4());
    conv1.title = Some("Conversation 1".to_string());
    conv1.add_message(Message::user("Message in conv 1".to_string()));
    conv1.add_message(Message::assistant("Response in conv 1".to_string()));
    storage.save(&conv1).unwrap();
    
    // Small delay to ensure different timestamps (filename is timestamp-based)
    std::thread::sleep(std::time::Duration::from_millis(5));
    
    // Create second conversation with different messages
    let mut conv2 = Conversation::new(Uuid::new_v4());
    conv2.title = Some("Conversation 2".to_string());
    conv2.add_message(Message::user("Message in conv 2".to_string()));
    storage.save(&conv2).unwrap();
    
    // "Switch" to conversation 1 by loading it
    let loaded_conv1 = load_by_id(&storage, conv1.id).expect("Should find conv1");
    assert_eq!(loaded_conv1.messages.len(), 2);
    assert_eq!(loaded_conv1.messages[0].content, "Message in conv 1");
    
    // "Switch" to conversation 2 by loading it
    let loaded_conv2 = load_by_id(&storage, conv2.id).expect("Should find conv2");
    assert_eq!(loaded_conv2.messages.len(), 1);
    assert_eq!(loaded_conv2.messages[0].content, "Message in conv 2");
}

/// Assistant response with thinking content should be persisted
#[test]
fn assistant_thinking_content_is_persisted() {
    let (storage, _temp) = create_temp_storage();
    
    let mut conversation = Conversation::new(Uuid::new_v4());
    conversation.add_message(Message::user("Think about this".to_string()));
    
    // Add assistant message with thinking content
    let mut response = Message::assistant("Here's my answer".to_string());
    response.thinking_content = Some("Let me think step by step...".to_string());
    conversation.add_message(response);
    
    storage.save(&conversation).unwrap();
    
    // Reload and verify thinking content was persisted
    let loaded = load_by_id(&storage, conversation.id).expect("Should find conversation");
    
    assert_eq!(loaded.messages.len(), 2);
    assert_eq!(loaded.messages[1].thinking_content, Some("Let me think step by step...".to_string()));
}

/// Conversation should be saved after each message to prevent data loss
#[test]
fn conversation_saved_incrementally() {
    let (storage, _temp) = create_temp_storage();
    
    let mut conversation = Conversation::new(Uuid::new_v4());
    conversation.title = Some("Incremental Save Test".to_string());
    
    // After user message, save immediately
    conversation.add_message(Message::user("First message".to_string()));
    storage.save(&conversation).unwrap();
    
    // Verify it's saved
    let loaded1 = load_by_id(&storage, conversation.id).expect("Should find conversation");
    assert_eq!(loaded1.messages.len(), 1);
    
    // After assistant message, save again
    conversation.add_message(Message::assistant("Response".to_string()));
    storage.save(&conversation).unwrap();
    
    // Verify both are saved
    let loaded2 = load_by_id(&storage, conversation.id).expect("Should find conversation");
    assert_eq!(loaded2.messages.len(), 2);
}

// ============================================================================
// Tests that simulate the ACTUAL UI flow - these document expected behavior
// ============================================================================

/// This test documents what SHOULD happen in send_message:
/// 1. User types message and hits enter
/// 2. Message is added to conversation
/// 3. Conversation is SAVED to storage  <-- This is what's missing!
/// 4. LLM request is started
#[test]
fn send_message_should_save_user_message_immediately() {
    let (storage, _temp) = create_temp_storage();
    
    // Simulate: conversation exists (like when app loads)
    let mut conversation = Conversation::new(Uuid::new_v4());
    conversation.title = Some("Chat Session".to_string());
    storage.save(&conversation).unwrap();
    
    // Simulate: user types "hello" and hits enter
    // In send_message(), after conversation.add_message(Message::user(...))
    // there MUST be a storage.save(&conversation) call
    conversation.add_message(Message::user("hello".to_string()));
    
    // THIS IS THE KEY: save must happen here, before LLM request
    storage.save(&conversation).unwrap();
    
    // If we reload now (before LLM responds), user message should be there
    let loaded = load_by_id(&storage, conversation.id).expect("Should find conversation");
    assert_eq!(loaded.messages.len(), 1, "User message should be persisted before LLM request");
    assert_eq!(loaded.messages[0].content, "hello");
}

/// This test documents what SHOULD happen when streaming completes:
/// 1. Streaming finishes
/// 2. Assistant message is added to conversation
/// 3. Conversation is SAVED to storage  <-- This is what's missing!
#[test]
fn streaming_complete_should_save_assistant_response() {
    let (storage, _temp) = create_temp_storage();
    
    // Setup: conversation with user message already saved
    let mut conversation = Conversation::new(Uuid::new_v4());
    conversation.add_message(Message::user("What is 2+2?".to_string()));
    storage.save(&conversation).unwrap();
    
    // Simulate: streaming completes with response "4"
    // In mark_streaming_complete() or equivalent, after adding assistant message
    // there MUST be a storage.save(&conversation) call
    conversation.add_message(Message::assistant("4".to_string()));
    
    // THIS IS THE KEY: save must happen after streaming completes
    storage.save(&conversation).unwrap();
    
    // If we reload, both messages should be there
    let loaded = load_by_id(&storage, conversation.id).expect("Should find conversation");
    assert_eq!(loaded.messages.len(), 2, "Both messages should be persisted after streaming");
}

/// Regression test: conversation should not lose messages when app restarts
#[test]
fn messages_survive_simulated_app_restart() {
    let (storage, _temp) = create_temp_storage();
    
    // Session 1: User sends message, gets response
    let conv_id = {
        let mut conversation = Conversation::new(Uuid::new_v4());
        conversation.title = Some("Persistent Chat".to_string());
        
        conversation.add_message(Message::user("Remember this".to_string()));
        storage.save(&conversation).unwrap();
        
        conversation.add_message(Message::assistant("I will remember.".to_string()));
        storage.save(&conversation).unwrap();
        
        conversation.id
    };
    // conversation dropped here (simulating app close)
    
    // Session 2: App restarts, loads conversation
    let loaded = load_by_id(&storage, conv_id).expect("Should find conversation after restart");
    
    assert_eq!(loaded.messages.len(), 2, "Messages should persist across sessions");
    assert_eq!(loaded.messages[0].content, "Remember this");
    assert_eq!(loaded.messages[1].content, "I will remember.");
}
