//! Bridge integration tests
//!
//! @plan PLAN-20250128-GPUI.P04
//! @requirement REQ-GPUI-006

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use flume;
use uuid::Uuid;

use personal_agent::events::types::UserEvent;
use personal_agent::events::{AppEvent, EventBus};
use personal_agent::presentation::ViewCommand;
use personal_agent::ui_gpui::bridge::{GpuiBridge, ViewCommandSink, spawn_user_event_forwarder, GpuiNotifier};

// === Mock Notifier ===

/// Mock notifier that counts notify() calls
#[derive(Clone)]
struct MockNotifier {
    count: Arc<AtomicUsize>,
}

impl MockNotifier {
    fn new() -> Self {
        Self { count: Arc::new(AtomicUsize::new(0)) }
    }

    fn notify_count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
}

impl GpuiNotifier for MockNotifier {
    fn notify(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}

// === GpuiBridge Tests ===

/// @plan PLAN-20250128-GPUI.P04
/// @requirement REQ-GPUI-006.1
/// @scenario GpuiBridge can be created with flume channels
#[test]
fn test_gpui_bridge_creation() {
    let (user_tx, _user_rx) = flume::bounded::<UserEvent>(16);
    let (_view_tx, view_rx) = flume::bounded::<ViewCommand>(16);

    let bridge = GpuiBridge::new(user_tx, view_rx);
    
    // Bridge should be created without panic
    assert!(!bridge.has_pending_commands());
}

/// @plan PLAN-20250128-GPUI.P04
/// @requirement REQ-GPUI-006.2
/// @scenario emit() sends UserEvent through channel
#[test]
fn test_gpui_bridge_emit_user_event() {
    let (user_tx, user_rx) = flume::bounded::<UserEvent>(16);
    let (_view_tx, view_rx) = flume::bounded::<ViewCommand>(16);

    let bridge = GpuiBridge::new(user_tx, view_rx);

    // Emit a UserEvent
    let result = bridge.emit(UserEvent::SendMessage { text: "Hello".to_string() });
    assert!(result, "emit should return true on success");

    // Verify it arrived in channel
    let received = user_rx.try_recv();
    assert!(received.is_ok(), "Should receive the event");
    
    match received.unwrap() {
        UserEvent::SendMessage { text } => assert_eq!(text, "Hello"),
        _ => panic!("Wrong event type"),
    }
}

/// @plan PLAN-20250128-GPUI.P04
/// @requirement REQ-GPUI-006.2
/// @scenario emit() returns false when channel is full (non-blocking)
#[test]
fn test_gpui_bridge_emit_non_blocking_when_full() {
    let (user_tx, _user_rx) = flume::bounded::<UserEvent>(1); // Tiny buffer
    let (_view_tx, view_rx) = flume::bounded::<ViewCommand>(16);

    let bridge = GpuiBridge::new(user_tx, view_rx);

    // Fill the channel
    let _ = bridge.emit(UserEvent::StopStreaming);

    // Next emit should fail but NOT block
    let start = std::time::Instant::now();
    let result = bridge.emit(UserEvent::StopStreaming);
    let elapsed = start.elapsed();

    assert!(!result, "emit should return false when full");
    assert!(elapsed.as_millis() < 10, "emit must not block");
}

/// @plan PLAN-20250128-GPUI.P04
/// @requirement REQ-GPUI-006.3
/// @scenario drain_commands() returns pending ViewCommands
#[test]
fn test_gpui_bridge_drain_commands() {
    let (user_tx, _user_rx) = flume::bounded::<UserEvent>(16);
    let (view_tx, view_rx) = flume::bounded::<ViewCommand>(16);

    let bridge = GpuiBridge::new(user_tx, view_rx);

    // Send some ViewCommands
    view_tx.send(ViewCommand::ShowThinking { conversation_id: Uuid::new_v4() }).unwrap();
    view_tx.send(ViewCommand::HideThinking { conversation_id: Uuid::new_v4() }).unwrap();

    // Drain should get both
    let commands = bridge.drain_commands();
    assert_eq!(commands.len(), 2);
}

/// @plan PLAN-20250128-GPUI.P04
/// @requirement REQ-GPUI-006.3
/// @scenario drain_commands() is non-blocking when empty
#[test]
fn test_gpui_bridge_drain_non_blocking_when_empty() {
    let (user_tx, _user_rx) = flume::bounded::<UserEvent>(16);
    let (_view_tx, view_rx) = flume::bounded::<ViewCommand>(16);

    let bridge = GpuiBridge::new(user_tx, view_rx);

    // Drain empty channel should not block
    let start = std::time::Instant::now();
    let commands = bridge.drain_commands();
    let elapsed = start.elapsed();

    assert!(commands.is_empty());
    assert!(elapsed.as_millis() < 10, "drain must not block");
}

/// @plan PLAN-20250128-GPUI.P04
/// @requirement REQ-GPUI-006.3
/// @scenario has_pending_commands() returns correct state
#[test]
fn test_gpui_bridge_has_pending_commands() {
    let (user_tx, _user_rx) = flume::bounded::<UserEvent>(16);
    let (view_tx, view_rx) = flume::bounded::<ViewCommand>(16);

    let bridge = GpuiBridge::new(user_tx, view_rx);

    // Initially empty
    assert!(!bridge.has_pending_commands());

    // Send a command
    view_tx.send(ViewCommand::ClearError).unwrap();

    // Now has pending
    assert!(bridge.has_pending_commands());

    // Drain clears it
    let _ = bridge.drain_commands();
    assert!(!bridge.has_pending_commands());
}

// === ViewCommandSink Tests ===

/// @plan PLAN-20250128-GPUI.P04
/// @requirement REQ-GPUI-006.4
/// @scenario ViewCommandSink sends commands through channel
#[test]
fn test_view_command_sink_send() {
    let (view_tx, view_rx) = flume::bounded::<ViewCommand>(16);
    let notifier = MockNotifier::new();

    let sink = ViewCommandSink::new(view_tx, notifier.clone());

    sink.send(ViewCommand::ClearError);

    let received = view_rx.try_recv();
    assert!(received.is_ok());
    assert!(matches!(received.unwrap(), ViewCommand::ClearError));
}

/// @plan PLAN-20250128-GPUI.P04
/// @requirement REQ-GPUI-006.5
/// @scenario ViewCommandSink calls notifier after sending
#[test]
fn test_view_command_sink_notifies() {
    let (view_tx, _view_rx) = flume::bounded::<ViewCommand>(16);
    let notifier = MockNotifier::new();

    let sink = ViewCommandSink::new(view_tx, notifier.clone());

    assert_eq!(notifier.notify_count(), 0);

    sink.send(ViewCommand::ClearError);
    assert_eq!(notifier.notify_count(), 1);

    sink.send(ViewCommand::ClearError);
    assert_eq!(notifier.notify_count(), 2);
}

/// @plan PLAN-20250128-GPUI.P04
/// @requirement REQ-GPUI-006.5
/// @scenario ViewCommandSink still notifies when channel full
#[test]
fn test_view_command_sink_notifies_when_full() {
    let (view_tx, _view_rx) = flume::bounded::<ViewCommand>(1);
    let notifier = MockNotifier::new();

    let sink = ViewCommandSink::new(view_tx, notifier.clone());

    // Fill channel
    sink.send(ViewCommand::ClearError);
    assert_eq!(notifier.notify_count(), 1);

    // Try to send again (should still notify even though send fails)
    sink.send(ViewCommand::ClearError);
    assert_eq!(notifier.notify_count(), 2);
}

/// @plan PLAN-20250128-GPUI.P04
/// @requirement REQ-GPUI-006.4
/// @scenario ViewCommandSink can be cloned for multiple presenters
#[test]
fn test_view_command_sink_clone() {
    let (view_tx, view_rx) = flume::bounded::<ViewCommand>(16);
    let notifier = MockNotifier::new();

    let sink1 = ViewCommandSink::new(view_tx, notifier.clone());
    let sink2 = sink1.clone();

    sink1.send(ViewCommand::ClearError);
    sink2.send(ViewCommand::DismissModal);

    // Both should arrive
    assert!(view_rx.try_recv().is_ok());
    assert!(view_rx.try_recv().is_ok());
    assert_eq!(notifier.notify_count(), 2);
}

// === UserEvent Forwarder Tests ===

/// @plan PLAN-20250128-GPUI.P04
/// @requirement REQ-GPUI-006.1
/// @scenario Forwarder publishes UserEvents to EventBus
#[tokio::test]
async fn test_user_event_forwarder_publishes() {
    let event_bus = Arc::new(EventBus::new(16));
    let (user_tx, user_rx) = flume::bounded::<UserEvent>(16);

    // Subscribe to EventBus
    let mut bus_rx = event_bus.subscribe();

    // Spawn forwarder
    let _handle = spawn_user_event_forwarder(event_bus.clone(), user_rx);

    // Send UserEvent through flume
    user_tx.send(UserEvent::NewConversation).unwrap();

    // Give forwarder time to process
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Should arrive at EventBus
    let received = bus_rx.try_recv();
    assert!(received.is_ok());
    
    match received.unwrap() {
        AppEvent::User(UserEvent::NewConversation) => {},
        other => panic!("Expected NewConversation, got {:?}", other),
    }
}

/// @plan PLAN-20250128-GPUI.P04
/// @requirement REQ-GPUI-006.1
/// @scenario Forwarder exits when sender is dropped
#[tokio::test]
async fn test_user_event_forwarder_exits_on_disconnect() {
    let event_bus = Arc::new(EventBus::new(16));
    let (user_tx, user_rx) = flume::bounded::<UserEvent>(16);

    let handle = spawn_user_event_forwarder(event_bus.clone(), user_rx);

    // Drop sender
    drop(user_tx);

    // Forwarder should exit
    let result = tokio::time::timeout(
        tokio::time::Duration::from_millis(100),
        handle
    ).await;

    assert!(result.is_ok(), "Forwarder should exit when sender dropped");
}

// === End-to-End Flow Test ===

/// @plan PLAN-20250128-GPUI.P04
/// @requirement REQ-GPUI-006
/// @scenario Full round-trip: GPUI emits UserEvent, presenter sends ViewCommand
#[tokio::test]
async fn test_full_bridge_round_trip() {
    let event_bus = Arc::new(EventBus::new(16));
    let (user_tx, user_rx) = flume::bounded::<UserEvent>(16);
    let (view_tx, view_rx) = flume::bounded::<ViewCommand>(16);
    let notifier = MockNotifier::new();

    // Create bridge (GPUI side)
    let bridge = GpuiBridge::new(user_tx, view_rx);

    // Create sink (presenter side)
    let sink = ViewCommandSink::new(view_tx, notifier.clone());

    // Subscribe to EventBus
    let mut bus_rx = event_bus.subscribe();

    // Spawn forwarder
    let _handle = spawn_user_event_forwarder(event_bus.clone(), user_rx);

    // === Simulate GPUI -> tokio ===
    bridge.emit(UserEvent::SendMessage { text: "Test".to_string() });

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // EventBus should have received it
    let event = bus_rx.try_recv();
    assert!(event.is_ok());

    // === Simulate tokio -> GPUI ===
    sink.send(ViewCommand::ShowThinking { conversation_id: Uuid::new_v4() });

    // GPUI should be notified
    assert_eq!(notifier.notify_count(), 1);

    // GPUI drains the command
    let commands = bridge.drain_commands();
    assert_eq!(commands.len(), 1);
    assert!(matches!(commands[0], ViewCommand::ShowThinking { .. }));
}

// === Behavioral E2E Test (with state application) ===

/// @plan PLAN-20250128-GPUI.P04
/// @requirement REQ-GPUI-006
/// @scenario End-to-end: UserEvent -> EventBus -> Presenter -> ViewCommand -> UI state update
#[tokio::test]
async fn test_e2e_with_state_application() {
    let event_bus = Arc::new(EventBus::new(16));
    let (user_tx, user_rx) = flume::bounded::<UserEvent>(16);
    let (view_tx, view_rx) = flume::bounded::<ViewCommand>(16);
    let notifier = MockNotifier::new();

    // Create bridge (GPUI side)
    let bridge = GpuiBridge::new(user_tx.clone(), view_rx);
    
    // Create sink (presenter side)
    let sink = ViewCommandSink::new(view_tx, notifier.clone());

    // Spawn forwarder
    let _forwarder = spawn_user_event_forwarder(event_bus.clone(), user_rx);

    // Subscribe to EventBus (simulating presenter)
    let mut bus_rx = event_bus.subscribe();

    // 1. GPUI emits UserEvent
    bridge.emit(UserEvent::SendMessage { text: "Hello".to_string() });

    // 2. Verify EventBus received it
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let received = bus_rx.try_recv();
    assert!(received.is_ok());
    assert!(matches!(
        received.unwrap(),
        AppEvent::User(UserEvent::SendMessage { text }) if text == "Hello"
    ));

    // 3. Simulate presenter sending ViewCommand
    let conv_id = Uuid::new_v4();
    sink.send(ViewCommand::ShowThinking { conversation_id: conv_id });
    sink.send(ViewCommand::AppendStream { 
        conversation_id: conv_id, 
        chunk: "Hi there!".to_string() 
    });
    sink.send(ViewCommand::FinalizeStream { 
        conversation_id: conv_id, 
        tokens: 10 
    });

    // 4. Verify notifier was called
    assert_eq!(notifier.notify_count(), 3);

    // 5. GPUI drains commands
    let commands = bridge.drain_commands();
    assert_eq!(commands.len(), 3);

    // 6. Simulate state application (what GPUI would do)
    let mut is_streaming = false;
    let mut streaming_content = String::new();
    let mut final_tokens = 0u64;

    for cmd in commands {
        match cmd {
            ViewCommand::ShowThinking { .. } => {
                is_streaming = true;
            }
            ViewCommand::AppendStream { chunk, .. } => {
                streaming_content.push_str(&chunk);
            }
            ViewCommand::FinalizeStream { tokens, .. } => {
                is_streaming = false;
                final_tokens = tokens;
            }
            _ => {}
        }
    }

    // 7. Verify state was correctly updated
    assert!(!is_streaming); // Finalized
    assert_eq!(streaming_content, "Hi there!");
    assert_eq!(final_tokens, 10);
}

// === Channel Overflow Test ===

/// @plan PLAN-20250128-GPUI.P04
/// @requirement REQ-GPUI-006
/// @scenario ViewCommand channel overflow triggers notify but doesn't block
#[tokio::test]
async fn test_view_command_overflow_behavior() {
    let (view_tx, view_rx) = flume::bounded::<ViewCommand>(2); // Tiny buffer
    let notifier = MockNotifier::new();
    let sink = ViewCommandSink::new(view_tx, notifier.clone());

    // Fill channel
    sink.send(ViewCommand::ClearError);
    sink.send(ViewCommand::ClearError);
    
    // Next send should overflow but not block
    let start = std::time::Instant::now();
    sink.send(ViewCommand::ClearError); // This one will be dropped
    let elapsed = start.elapsed();

    // Should not block
    assert!(elapsed.as_millis() < 10);
    
    // Notifier should still be called (3 times)
    assert_eq!(notifier.notify_count(), 3);

    // Only 2 commands in channel (one was dropped)
    let (user_tx, _) = flume::bounded::<UserEvent>(1);
    let bridge = GpuiBridge::new(user_tx, view_rx);
    let commands = bridge.drain_commands();
    assert_eq!(commands.len(), 2);
}
