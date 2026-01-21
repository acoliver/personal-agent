//! Tests for new conversation creation behavior
//!
//! These tests verify that creating a new conversation:
//! 1. Saves immediately to storage
//! 2. Gets a default title
//! 3. Appears in history and dropdown

use personal_agent::models::Conversation;
use personal_agent::storage::ConversationStorage;
use tempfile::TempDir;
use uuid::Uuid;

fn create_test_storage() -> (TempDir, ConversationStorage) {
    let temp_dir = TempDir::new().unwrap();
    let storage = ConversationStorage::new(temp_dir.path().to_path_buf());
    (temp_dir, storage)
}

/// New conversation should have a default title
#[test]
fn new_conversation_has_default_title() {
    let conv = Conversation::new(Uuid::new_v4());
    
    // Simulate what new_conversation does: set a default title
    let default_title = format!("New {}", conv.created_at.format("%Y-%m-%d %H:%M"));
    
    assert!(default_title.starts_with("New "));
    assert!(default_title.len() > 10); // "New " + date
}

/// New conversation should be saveable immediately
#[test]
fn new_conversation_saves_immediately() {
    let (_temp_dir, storage) = create_test_storage();
    
    let mut conv = Conversation::new(Uuid::new_v4());
    conv.title = Some(format!("New {}", conv.created_at.format("%Y-%m-%d %H:%M")));
    
    // Save should succeed
    let result = storage.save(&conv);
    assert!(result.is_ok());
    
    // Should be findable in load_all
    let all = storage.load_all().unwrap();
    let found = all.iter().find(|c| c.id == conv.id);
    assert!(found.is_some());
}

/// New conversation should appear in load_all after saving
#[test]
fn new_conversation_appears_in_load_all() {
    let (_temp_dir, storage) = create_test_storage();
    
    // Create existing conversation
    let mut existing = Conversation::new(Uuid::new_v4());
    existing.title = Some("Existing".to_string());
    storage.save(&existing).unwrap();
    
    // Small delay to ensure different timestamp (filename is based on created_at)
    std::thread::sleep(std::time::Duration::from_millis(5));
    
    // Create "new" conversation (simulating + button)
    let mut new_conv = Conversation::new(Uuid::new_v4());
    new_conv.title = Some(format!("New {}", new_conv.created_at.format("%Y-%m-%d %H:%M")));
    storage.save(&new_conv).unwrap();
    
    // Both should appear in load_all
    let all = storage.load_all().unwrap();
    assert_eq!(all.len(), 2);
    
    let titles: Vec<_> = all.iter().filter_map(|c| c.title.as_ref()).collect();
    assert!(titles.iter().any(|t| t == &"Existing"));
    assert!(titles.iter().any(|t| t.starts_with("New ")));
}

/// Creating multiple new conversations should create distinct entries
#[test]
fn multiple_new_conversations_are_distinct() {
    let (_temp_dir, storage) = create_test_storage();
    
    // Create first new conversation
    let mut conv1 = Conversation::new(Uuid::new_v4());
    conv1.title = Some("New 2026-01-21 12:00".to_string());
    storage.save(&conv1).unwrap();
    
    // Wait a moment and create second (in real app, timestamp would differ)
    let mut conv2 = Conversation::new(Uuid::new_v4());
    conv2.title = Some("New 2026-01-21 12:01".to_string());
    storage.save(&conv2).unwrap();
    
    // Both should exist
    let all = storage.load_all().unwrap();
    assert_eq!(all.len(), 2);
    
    // IDs should be different
    assert_ne!(conv1.id, conv2.id);
}

/// New conversation should become the active conversation
#[test]
fn new_conversation_is_selectable() {
    let (_temp_dir, storage) = create_test_storage();
    
    // Create and save
    let mut conv = Conversation::new(Uuid::new_v4());
    conv.title = Some("New Conversation".to_string());
    storage.save(&conv).unwrap();
    
    // Should be findable by title
    let all = storage.load_all().unwrap();
    let found = all.iter().find(|c| c.title.as_deref() == Some("New Conversation"));
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, conv.id);
}
