//! Tests for conversation rename behavior
//!
//! These tests verify that renaming a conversation properly updates
//! all the places where the title is displayed.

use personal_agent::models::Conversation;
use personal_agent::storage::ConversationStorage;
use tempfile::TempDir;
use uuid::Uuid;

fn create_test_storage() -> (TempDir, ConversationStorage) {
    let temp_dir = TempDir::new().unwrap();
    let storage = ConversationStorage::new(temp_dir.path().to_path_buf());
    (temp_dir, storage)
}

/// When a conversation is renamed, the new title should be saved to storage
#[test]
fn rename_persists_to_storage() {
    let (_temp_dir, storage) = create_test_storage();
    
    // Create and save a conversation
    let mut conv = Conversation::new(Uuid::new_v4());
    conv.title = Some("Original Title".to_string());
    storage.save(&conv).unwrap();
    
    // Simulate rename
    conv.title = Some("Renamed Title".to_string());
    storage.save(&conv).unwrap();
    
    // Load and verify - load takes filename, use load_all to find by id
    let all = storage.load_all().unwrap();
    let loaded = all.iter().find(|c| c.id == conv.id).unwrap();
    assert_eq!(loaded.title, Some("Renamed Title".to_string()));
}

/// After rename, loading all conversations should show the new title
#[test]
fn rename_appears_in_load_all() {
    let (_temp_dir, storage) = create_test_storage();
    
    // Create first conversation
    let mut conv1 = Conversation::new(Uuid::new_v4());
    conv1.title = Some("First".to_string());
    storage.save(&conv1).unwrap();
    
    // Small delay to ensure different timestamp (filename is based on created_at)
    std::thread::sleep(std::time::Duration::from_millis(5));
    
    // Create second conversation
    let mut conv2 = Conversation::new(Uuid::new_v4());
    conv2.title = Some("Second".to_string());
    storage.save(&conv2).unwrap();
    
    // Verify both exist before rename
    let before = storage.load_all().unwrap();
    assert_eq!(before.len(), 2, "Should have 2 conversations before rename");
    
    // Rename first conversation
    conv1.title = Some("First - Renamed".to_string());
    storage.save(&conv1).unwrap();
    
    // Load all and verify
    let all = storage.load_all().unwrap();
    assert_eq!(all.len(), 2, "Should still have 2 conversations after rename");
    
    let titles: Vec<String> = all.iter().filter_map(|c| c.title.clone()).collect();
    
    assert!(titles.iter().any(|t| t == "First - Renamed"), 
            "Should have renamed title, got: {:?}", titles);
    assert!(titles.iter().any(|t| t == "Second"), 
            "Should have second title, got: {:?}", titles);
    assert!(!titles.iter().any(|t| t == "First"), 
            "Old title should be gone, got: {:?}", titles);
}

/// Renaming should not create a duplicate conversation
#[test]
fn rename_does_not_duplicate() {
    let (_temp_dir, storage) = create_test_storage();
    
    let mut conv = Conversation::new(Uuid::new_v4());
    conv.title = Some("Original".to_string());
    storage.save(&conv).unwrap();
    
    // Rename multiple times
    conv.title = Some("Renamed Once".to_string());
    storage.save(&conv).unwrap();
    
    conv.title = Some("Renamed Twice".to_string());
    storage.save(&conv).unwrap();
    
    // Should still only have one conversation
    let all = storage.load_all().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].title, Some("Renamed Twice".to_string()));
}

/// Empty title should not be saved (or should use fallback)
#[test]
fn empty_title_handling() {
    let (_temp_dir, storage) = create_test_storage();
    
    let mut conv = Conversation::new(Uuid::new_v4());
    conv.title = Some("Has Title".to_string());
    storage.save(&conv).unwrap();
    
    // Try to set empty title - in real UI this is prevented,
    // but we test that a None title is handled
    conv.title = None;
    storage.save(&conv).unwrap();
    
    // Load and verify
    let all = storage.load_all().unwrap();
    let loaded = all.iter().find(|c| c.id == conv.id).unwrap();
    // Title should be None (UI would show fallback timestamp)
    assert_eq!(loaded.title, None);
}
