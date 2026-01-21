//! Behavioral tests for conversation title and naming features
//!
//! These tests verify the observable behaviors:
//! 1. New conversations get a displayable default title (not just UUID)
//! 2. Conversations can be renamed and the new title persists
//! 3. All saved conversation titles are available for selection
//! 4. Loading a conversation by title finds the correct one

use personal_agent::{Conversation, ConversationStorage, Message};
use tempfile::TempDir;
use uuid::Uuid;

/// A new conversation should have a human-readable default title (timestamp format)
/// not None or a raw UUID
#[test]
fn new_conversation_has_readable_default_title() {
    let profile_id = Uuid::new_v4();
    let conversation = Conversation::new(profile_id);

    // Title should exist
    assert!(conversation.title.is_some(), "New conversation should have a title");

    let title = conversation.title.as_ref().unwrap();

    // Title should be timestamp format (17 chars: YYYYMMDDHHMMSSmmm)
    assert_eq!(title.len(), 17, "Default title should be 17-char timestamp, got: {}", title);

    // Title should be all digits (timestamp format)
    assert!(
        title.chars().all(|c| c.is_ascii_digit()),
        "Default title should be numeric timestamp, got: {}",
        title
    );
}

/// Renaming a conversation should update the title and persist through save/load
#[test]
fn rename_conversation_persists_through_storage() {
    let temp_dir = TempDir::new().unwrap();
    let storage = ConversationStorage::new(temp_dir.path());

    let profile_id = Uuid::new_v4();
    let mut conversation = Conversation::new(profile_id);
    let original_title = conversation.title.clone();

    // Rename the conversation
    conversation.set_title("My Custom Title".to_string());

    // Save it
    storage.save(&conversation).unwrap();

    // Load it back
    let filename = conversation.filename();
    let loaded = storage.load(&filename).unwrap();

    // Title should be the new one, not the original
    assert_eq!(loaded.title, Some("My Custom Title".to_string()));
    assert_ne!(loaded.title, original_title);
}

/// When listing conversations, all titles should be available for a popup/selector
#[test]
fn all_conversation_titles_available_for_selection() {
    let temp_dir = TempDir::new().unwrap();
    let storage = ConversationStorage::new(temp_dir.path());

    let profile_id = Uuid::new_v4();

    // Create and save several conversations with distinct titles
    // Add small delay between creates to ensure unique timestamps/filenames
    let titles = vec!["First Chat", "Second Chat", "Third Chat"];
    for title in &titles {
        let mut conv = Conversation::new(profile_id);
        conv.set_title(title.to_string());
        storage.save(&conv).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    // Load all conversations
    let all_conversations = storage.load_all().unwrap();

    // Should have all 3 conversations
    assert_eq!(
        all_conversations.len(),
        3,
        "Should have 3 conversations, got {}",
        all_conversations.len()
    );

    // All titles should be present
    let loaded_titles: Vec<String> = all_conversations
        .iter()
        .filter_map(|c| c.title.clone())
        .collect();

    for expected_title in &titles {
        assert!(
            loaded_titles.contains(&expected_title.to_string()),
            "Expected title '{}' not found in {:?}",
            expected_title,
            loaded_titles
        );
    }
}

/// Loading a conversation by title should find the correct one (not just any conversation)
#[test]
fn load_conversation_by_title_finds_correct_one() {
    let temp_dir = TempDir::new().unwrap();
    let storage = ConversationStorage::new(temp_dir.path());

    let profile_id = Uuid::new_v4();

    // Create conversations with different content
    // Add delays to ensure unique filenames
    let mut conv1 = Conversation::new(profile_id);
    conv1.set_title("Alpha".to_string());
    conv1.add_message(Message::user("Message in Alpha".to_string()));
    storage.save(&conv1).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));

    let mut conv2 = Conversation::new(profile_id);
    conv2.set_title("Beta".to_string());
    conv2.add_message(Message::user("Message in Beta".to_string()));
    storage.save(&conv2).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));

    let mut conv3 = Conversation::new(profile_id);
    conv3.set_title("Gamma".to_string());
    conv3.add_message(Message::user("Message in Gamma".to_string()));
    storage.save(&conv3).unwrap();

    // Load all and find by title
    let all_conversations = storage.load_all().unwrap();

    assert_eq!(all_conversations.len(), 3, "Should have 3 conversations");

    let found_beta = all_conversations
        .iter()
        .find(|c| c.title.as_deref() == Some("Beta"));

    assert!(found_beta.is_some(), "Should find conversation with title 'Beta'");

    let beta = found_beta.unwrap();
    assert_eq!(beta.messages.len(), 1);
    assert_eq!(beta.messages[0].content, "Message in Beta");
}

/// Creating a new conversation should NOT overwrite existing conversations
#[test]
fn new_conversation_does_not_overwrite_existing() {
    let temp_dir = TempDir::new().unwrap();
    let storage = ConversationStorage::new(temp_dir.path());

    let profile_id = Uuid::new_v4();

    // Create and save first conversation
    let mut conv1 = Conversation::new(profile_id);
    conv1.set_title("Original".to_string());
    conv1.add_message(Message::user("Original message".to_string()));
    storage.save(&conv1).unwrap();

    // Small delay to ensure different timestamp for filename
    std::thread::sleep(std::time::Duration::from_millis(2));

    // Create a second conversation (simulating "+" button)
    let conv2 = Conversation::new(profile_id);
    storage.save(&conv2).unwrap();

    // Both should exist
    let all = storage.load_all().unwrap();
    assert_eq!(all.len(), 2, "Should have 2 conversations");

    // Original should still have its content
    let original = all.iter().find(|c| c.title.as_deref() == Some("Original"));
    assert!(original.is_some());
    assert_eq!(original.unwrap().messages.len(), 1);
}
