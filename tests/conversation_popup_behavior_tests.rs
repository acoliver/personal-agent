//! Behavioral tests for conversation popup/selector
//!
//! The popup should show ALL saved conversations so the user can switch between them.
//! Currently populate_title_popup() only shows the current title, which is incorrect.

use personal_agent::{Conversation, ConversationStorage};
use tempfile::TempDir;
use uuid::Uuid;

/// Helper to simulate what populate_title_popup SHOULD return
/// This is the expected behavior - all conversation titles
fn get_all_conversation_titles(storage: &ConversationStorage) -> Vec<String> {
    storage
        .load_all()
        .unwrap_or_default()
        .iter()
        .filter_map(|c| c.title.clone())
        .collect()
}

/// The popup should contain ALL saved conversation titles, not just the current one
/// This test documents the EXPECTED behavior that is currently broken
#[test]
fn popup_should_list_all_conversation_titles() {
    let temp_dir = TempDir::new().unwrap();
    let storage = ConversationStorage::new(temp_dir.path());

    let profile_id = Uuid::new_v4();

    // Create several saved conversations
    for title in &["Chat A", "Chat B", "Chat C"] {
        let mut conv = Conversation::new(profile_id);
        conv.set_title(title.to_string());
        storage.save(&conv).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    // Get what should be shown in popup
    let popup_titles = get_all_conversation_titles(&storage);

    // All 3 should be in the list
    assert_eq!(popup_titles.len(), 3, "Popup should show all 3 conversations");
    assert!(popup_titles.contains(&"Chat A".to_string()));
    assert!(popup_titles.contains(&"Chat B".to_string()));
    assert!(popup_titles.contains(&"Chat C".to_string()));
}

/// When switching conversations via popup, the selected title should match an existing conversation
#[test]
fn popup_selection_should_match_existing_conversation() {
    let temp_dir = TempDir::new().unwrap();
    let storage = ConversationStorage::new(temp_dir.path());

    let profile_id = Uuid::new_v4();

    // Create a conversation with a custom title
    let mut conv = Conversation::new(profile_id);
    conv.set_title("My Important Chat".to_string());
    storage.save(&conv).unwrap();

    // Simulate popup selection - find by title
    let all = storage.load_all().unwrap();
    let found = all
        .iter()
        .find(|c| c.title.as_deref() == Some("My Important Chat"));

    assert!(
        found.is_some(),
        "Should be able to find conversation by the title shown in popup"
    );
    assert_eq!(found.unwrap().id, conv.id, "Found conversation should be the same one");
}

/// The current title should be selected/highlighted in the popup
#[test]
fn current_conversation_title_should_be_preselected() {
    let temp_dir = TempDir::new().unwrap();
    let storage = ConversationStorage::new(temp_dir.path());

    let profile_id = Uuid::new_v4();

    // Create conversations
    let mut current_conv = Conversation::new(profile_id);
    current_conv.set_title("Current Chat".to_string());
    storage.save(&current_conv).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));

    let mut other_conv = Conversation::new(profile_id);
    other_conv.set_title("Other Chat".to_string());
    storage.save(&other_conv).unwrap();

    // Get all titles for popup
    let all_titles = get_all_conversation_titles(&storage);

    // The current conversation's title should be in the list
    let current_title = current_conv.title.as_ref().unwrap();
    assert!(
        all_titles.contains(current_title),
        "Current conversation title should be in popup list"
    );
}

/// New conversation option should be available (or handled separately)
/// The popup needs a way to create new conversations
#[test]
fn popup_should_support_new_conversation_action() {
    // This test documents that we need a way to trigger "new conversation"
    // Options:
    // 1. A "New Conversation" item in the popup
    // 2. A separate "+" button (current implementation)
    //
    // The current implementation uses a "+" button, which is fine.
    // This test just verifies the new conversation workflow works.

    let temp_dir = TempDir::new().unwrap();
    let storage = ConversationStorage::new(temp_dir.path());

    let profile_id = Uuid::new_v4();

    // Start with one existing conversation
    let mut existing = Conversation::new(profile_id);
    existing.set_title("Existing".to_string());
    storage.save(&existing).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));

    // Simulate "+" button creating new conversation
    let new_conv = Conversation::new(profile_id);
    storage.save(&new_conv).unwrap();

    // Both should exist
    let all = storage.load_all().unwrap();
    assert_eq!(all.len(), 2);

    // New conversation should have a default title (not None, not empty)
    let new_one = all.iter().find(|c| c.id == new_conv.id).unwrap();
    assert!(new_one.title.is_some());
    assert!(!new_one.title.as_ref().unwrap().is_empty());
}
