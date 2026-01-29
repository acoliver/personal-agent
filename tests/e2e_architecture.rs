//! End-to-End Architecture Tests
//!
//! These tests verify core components work together correctly.
//! They use real service implementations with temp directories and the real EventBus.
//!
//! @plan PLAN-20250125-REFACTOR.P16
//! @requirement REQ-E2E.1, REQ-E2E.2, REQ-E2E.3, REQ-E2E.4

use std::time::Duration;
use tokio::time::sleep;
use tempfile::TempDir;
use uuid::Uuid;

use personal_agent::events::{EventBus, AppEvent, types::SystemEvent};
use personal_agent::services::{
    ConversationServiceImpl, SecretsServiceImpl, AppSettingsServiceImpl,
    ConversationService, SecretsService, AppSettingsService,
};

/// ============================================================================
/// Test 1: EventBus Round Trip
/// ============================================================================
/// @plan PLAN-20250125-REFACTOR.P16
/// @requirement REQ-E2E.1
#[tokio::test]
async fn test_eventbus_round_trip() {
    // Initialize EventBus with capacity
    let event_bus = EventBus::new(100);
    
    // Subscribe to events
    let mut rx = event_bus.subscribe();
    
    // Verify subscriber count
    assert_eq!(event_bus.subscriber_count(), 1, "Should have 1 subscriber");
    
    // Emit AppEvent::System(SystemEvent::AppLaunched)
    let event = AppEvent::System(SystemEvent::AppLaunched);
    let result = event_bus.publish(event.clone());
    
    // Assert publish succeeded
    assert!(result.is_ok(), "Publish should succeed with subscribers");
    assert_eq!(result.unwrap(), 1, "Event should be received by 1 subscriber");
    
    // Assert event was received
    let received = rx.recv().await;
    assert!(received.is_ok(), "Should receive event");
    assert_eq!(received.unwrap(), event, "Received event should match published event");
}

/// ============================================================================
/// Test 2: Conversation Service CRUD
/// ============================================================================
/// @plan PLAN-20250125-REFACTOR.P16
/// @requirement REQ-E2E.2
#[tokio::test]
async fn test_conversation_service_crud() {
    // Create temp directory for isolation
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_dir = temp_dir.path().join("conversations");
    
    // Create ConversationServiceImpl with temp dir
    let service = ConversationServiceImpl::new(storage_dir.clone())
        .expect("Failed to create ConversationServiceImpl");
    
    let profile_id = Uuid::new_v4();
    
    // Test 1: Create conversation
    let conversation = service
        .create(Some("Test Conversation".to_string()), profile_id)
        .await
        .expect("Failed to create conversation");
    
    assert_eq!(conversation.title, Some("Test Conversation".to_string()));
    assert_eq!(conversation.profile_id, profile_id);
    assert_eq!(conversation.messages.len(), 0);
    
    // Test 2: Load conversation
    let loaded = service
        .load(conversation.id)
        .await
        .expect("Failed to load conversation");
    
    assert_eq!(loaded.id, conversation.id);
    assert_eq!(loaded.title, Some("Test Conversation".to_string()));
    assert_eq!(loaded.profile_id, profile_id);
    
    // Test 3: Rename conversation
    service
        .rename(conversation.id, "Updated Title".to_string())
        .await
        .expect("Failed to rename conversation");
    
    let renamed = service
        .load(conversation.id)
        .await
        .expect("Failed to load renamed conversation");
    
    assert_eq!(renamed.title, Some("Updated Title".to_string()));
    
    // Test 4: Delete conversation
    service
        .delete(conversation.id)
        .await
        .expect("Failed to delete conversation");
    
    // Verify conversation is deleted
    let load_result = service.load(conversation.id).await;
    assert!(load_result.is_err(), "Loading deleted conversation should fail");
}

/// ============================================================================
/// Test 3: Secrets Service CRUD
/// ============================================================================
/// @plan PLAN-20250125-REFACTOR.P16
/// @requirement REQ-E2E.3
#[tokio::test]
async fn test_secrets_service_crud() {
    // Create temp directory for isolation
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let secrets_dir = temp_dir.path().join("secrets");
    
    // Create SecretsServiceImpl with temp dir
    let service = SecretsServiceImpl::new(secrets_dir)
        .expect("Failed to create SecretsServiceImpl");
    
    // Test 1: Store secret
    service
        .store("test_key".to_string(), "test_value".to_string())
        .await
        .expect("Failed to store secret");
    
    // Test 2: Get secret
    let value = service
        .get("test_key")
        .await
        .expect("Failed to get secret");
    
    assert_eq!(value, Some("test_value".to_string()));
    
    // Test 3: Store another secret
    service
        .store("another_key".to_string(), "another_value".to_string())
        .await
        .expect("Failed to store second secret");
    
    // Test 4: List keys
    let keys = service
        .list_keys()
        .await
        .expect("Failed to list keys");
    
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&"test_key".to_string()));
    assert!(keys.contains(&"another_key".to_string()));
    
    // Test 5: Delete secret
    service
        .delete("test_key")
        .await
        .expect("Failed to delete secret");
    
    // Verify deletion
    let value = service
        .get("test_key")
        .await
        .expect("Failed to get deleted secret");
    
    assert_eq!(value, None);
    
    // Verify list_keys updated
    let keys = service
        .list_keys()
        .await
        .expect("Failed to list keys after deletion");
    
    assert_eq!(keys.len(), 1);
    assert!(!keys.contains(&"test_key".to_string()));
    assert!(keys.contains(&"another_key".to_string()));
}

/// ============================================================================
/// Test 4: App Settings Service
/// ============================================================================
/// @plan PLAN-20250125-REFACTOR.P16
/// @requirement REQ-E2E.4
#[tokio::test]
async fn test_app_settings_service() {
    // Create temp directory for isolation
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let settings_file = temp_dir.path().join("app_settings.json");
    
    // Create AppSettingsServiceImpl with temp file
    let service = AppSettingsServiceImpl::new(settings_file)
        .expect("Failed to create AppSettingsServiceImpl");
    
    let profile_id = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();
    
    // Test 1: Set default_profile_id
    service
        .set_default_profile_id(profile_id)
        .await
        .expect("Failed to set default_profile_id");
    
    // Test 2: Get default_profile_id
    let retrieved_profile_id = service
        .get_default_profile_id()
        .await
        .expect("Failed to get default_profile_id");
    
    assert_eq!(retrieved_profile_id, Some(profile_id));
    
    // Test 3: Set current_conversation_id
    service
        .set_current_conversation_id(conversation_id)
        .await
        .expect("Failed to set current_conversation_id");
    
    // Test 4: Get current_conversation_id
    let retrieved_conversation_id = service
        .get_current_conversation_id()
        .await
        .expect("Failed to get current_conversation_id");
    
    assert_eq!(retrieved_conversation_id, Some(conversation_id));
    
    // Test 5: Verify persistence - create new service instance with same file
    let service2 = AppSettingsServiceImpl::new(temp_dir.path().join("app_settings.json"))
        .expect("Failed to create second AppSettingsServiceImpl");
    
    let retrieved_profile_id2 = service2
        .get_default_profile_id()
        .await
        .expect("Failed to get default_profile_id from new instance");
    
    let retrieved_conversation_id2 = service2
        .get_current_conversation_id()
        .await
        .expect("Failed to get current_conversation_id from new instance");
    
    assert_eq!(retrieved_profile_id2, Some(profile_id), "Settings should persist across instances");
    assert_eq!(retrieved_conversation_id2, Some(conversation_id), "Settings should persist across instances");
}
