//! GPUI Integration Tests
//!
//! @plan PLAN-20250128-GPUI.P15
//! End-to-end tests for the GPUI UI system

use std::sync::Arc;

use personal_agent::events::types::UserEvent;
use personal_agent::ui_gpui::bridge::GpuiBridge;
use personal_agent::ui_gpui::views::chat_view::{ChatState, ChatMessage, StreamingState};
use personal_agent::ui_gpui::views::main_panel::MainPanel;
use personal_agent::presentation::view_command::ViewId;

// ============================================================================
// MainPanel Navigation Tests
// ============================================================================

#[test]
fn test_main_panel_initial_state() {
    let panel = MainPanel::new();
    assert_eq!(panel.current_view(), ViewId::Chat);
}

#[test]
fn test_chat_state_with_thinking() {
    let mut state = ChatState::new();
    
    // Enable thinking
    state.set_thinking(true, Some("Analyzing the request...".to_string()));
    assert!(state.show_thinking);
    assert_eq!(state.thinking_content, Some("Analyzing the request...".to_string()));
    
    // Toggle thinking off
    state.show_thinking = false;
    assert!(!state.show_thinking);
}

// ============================================================================
// Bridge Integration Tests
// ============================================================================

#[test]
fn test_bridge_creation() {
    let (user_tx, user_rx) = flume::unbounded();
    let (_view_cmd_tx, view_cmd_rx) = flume::unbounded();
    
    let bridge = Arc::new(GpuiBridge::new(user_tx, view_cmd_rx));
    
    // Verify bridge was created
    assert!(Arc::strong_count(&bridge) >= 1);
    
    // We can't test emit_user_event directly without running GPUI,
    // but we can verify the channels are working
    drop(bridge);
    assert!(user_rx.try_recv().is_err()); // No events sent
}

#[test]
fn test_channel_communication() {
    let (user_tx, user_rx) = flume::unbounded::<UserEvent>();
    let (view_cmd_tx, view_cmd_rx) = flume::unbounded::<personal_agent::presentation::ViewCommand>();
    
    // Send a user event
    user_tx.send(UserEvent::ToggleThinking).unwrap();
    
    // Receive it
    let received = user_rx.try_recv().unwrap();
    assert!(matches!(received, UserEvent::ToggleThinking));
    
    // Send a view command
    use personal_agent::presentation::ViewCommand;
    view_cmd_tx.send(ViewCommand::ShowThinking { 
        conversation_id: uuid::Uuid::new_v4() 
    }).unwrap();
    
    // Receive it
    let cmd = view_cmd_rx.try_recv().unwrap();
    assert!(matches!(cmd, ViewCommand::ShowThinking { .. }));
}

// ============================================================================
// State Persistence Tests
// ============================================================================

#[test]
fn test_chat_state_message_persistence() {
    let mut state = ChatState::new();
    
    // Add multiple messages
    state.add_message(ChatMessage::new("user", "First message"));
    state.add_message(ChatMessage::new("assistant", "First response"));
    state.add_message(ChatMessage::new("user", "Second message"));
    state.add_message(ChatMessage::new("assistant", "Second response"));
    
    assert_eq!(state.messages.len(), 4);
    assert_eq!(state.messages[0].content, "First message");
    assert_eq!(state.messages[1].content, "First response");
    assert_eq!(state.messages[2].content, "Second message");
    assert_eq!(state.messages[3].content, "Second response");
}

#[test]
fn test_chat_message_with_metadata() {
    let msg = ChatMessage::new("assistant", "Response")
        .with_timestamp(1234567890)
        .with_model_id("claude-3");
    
    assert_eq!(msg.role, "assistant");
    assert_eq!(msg.content, "Response");
    assert_eq!(msg.timestamp, Some(1234567890));
    assert_eq!(msg.model_id, Some("claude-3".to_string()));
}

#[test]
fn test_streaming_state_transitions() {
    let mut state = ChatState::new();
    
    // Initial state
    assert!(matches!(state.streaming, StreamingState::Idle));
    
    // Start streaming
    state.set_streaming(StreamingState::Streaming {
        content: String::new(),
        done: false,
    });
    assert!(matches!(state.streaming, StreamingState::Streaming { done: false, .. }));
    
    // Update content
    state.set_streaming(StreamingState::Streaming {
        content: "Partial response...".to_string(),
        done: false,
    });
    
    // Mark as done
    state.set_streaming(StreamingState::Streaming {
        content: "Complete response".to_string(),
        done: true,
    });
    assert!(matches!(state.streaming, StreamingState::Streaming { done: true, .. }));
    
    // Back to idle
    state.set_streaming(StreamingState::Idle);
    assert!(matches!(state.streaming, StreamingState::Idle));
}

#[test]
fn test_streaming_error_state() {
    let mut state = ChatState::new();
    
    state.set_streaming(StreamingState::Error("API error".to_string()));
    
    match &state.streaming {
        StreamingState::Error(msg) => assert_eq!(msg, "API error"),
        _ => panic!("Expected error state"),
    }
}
