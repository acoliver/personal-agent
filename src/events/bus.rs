//! EventBus Core Implementation
//!
//! Provides the central event bus using tokio::sync::broadcast.
//!
//! @plan PLAN-20250125-REFACTOR.P04
//! @requirement REQ-019.1
//! @pseudocode event-bus.md lines 10-46

use crate::events::{AppEvent, EventBusError};
use tokio::sync::broadcast;

/// EventBus stub implementation
///
/// Central event distribution system using tokio::sync::broadcast.
///
/// @plan PLAN-20250125-REFACTOR.P04
/// @requirement REQ-019.1
/// @pseudocode event-bus.md lines 10-12
#[derive(Debug)]
pub struct EventBus {
    /// Sender for broadcasting events to all subscribers
    sender: broadcast::Sender<AppEvent>,
}

impl EventBus {
    /// Create a new EventBus with the specified channel capacity
    ///
    /// @plan PLAN-20250125-REFACTOR.P06
    /// @requirement REQ-021.1
    /// @pseudocode event-bus.md lines 20-23
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Publish an event to all subscribers
    ///
    /// Returns the number of subscribers who received the event.
    ///
    /// @plan PLAN-20250125-REFACTOR.P06
    /// @requirement REQ-021.2
    /// @requirement REQ-021.5
    /// @pseudocode event-bus.md lines 30-38
    pub fn publish(&self, event: AppEvent) -> Result<usize, EventBusError> {
        self.sender.send(event).map_err(|_| EventBusError::NoSubscribers)
    }

    /// Subscribe to receive all events
    ///
    /// Returns a Receiver that will receive all future events.
    ///
    /// @plan PLAN-20250125-REFACTOR.P06
    /// @requirement REQ-021.3
    /// @pseudocode event-bus.md lines 40-41
    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.sender.subscribe()
    }

    /// Get the current number of active subscribers
    ///
    /// @plan PLAN-20250125-REFACTOR.P06
    /// @pseudocode event-bus.md lines 45-46
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::{
        ChatEvent, ConversationEvent, McpEvent, NavigationEvent, ProfileEvent, SystemEvent, UserEvent,
        ViewId,
    };
    use tokio::time::Duration;

    /// EventBus creation test
    ///
    /// GIVEN: No EventBus exists
    /// WHEN: EventBus::new(capacity) is called with capacity=16
    /// THEN: EventBus instance is returned
    /// AND: Channel capacity is 16
    ///
    /// @plan PLAN-20250125-REFACTOR.P05
    /// @requirement REQ-020.1
    #[test]
    fn test_event_bus_creation() {
        // Given
        let capacity = 16;

        // When
        let bus = EventBus::new(capacity);

        // Then
        assert_eq!(bus.subscriber_count(), 0, "New bus has no subscribers");
    }

    /// Single subscription test
    ///
    /// GIVEN: EventBus instance
    /// WHEN: subscribe() is called once
    /// THEN: Receiver is returned
    /// AND: subscriber_count() returns 1
    ///
    /// @plan PLAN-20250125-REFACTOR.P05
    /// @requirement REQ-020.4
    #[test]
    fn test_single_subscription() {
        // Given
        let bus = EventBus::new(16);

        // When
        let _rx = bus.subscribe();

        // Then
        assert_eq!(bus.subscriber_count(), 1, "Bus has 1 subscriber");
    }

    /// Multiple subscriptions test
    ///
    /// GIVEN: EventBus instance
    /// WHEN: subscribe() is called 3 times
    /// THEN: 3 unique Receivers are returned
    /// AND: subscriber_count() returns 3
    ///
    /// @plan PLAN-20250125-REFACTOR.P05
    /// @requirement REQ-020.4
    #[test]
    fn test_multiple_subscriptions() {
        // Given
        let bus = EventBus::new(16);

        // When
        let _rx1 = bus.subscribe();
        let _rx2 = bus.subscribe();
        let _rx3 = bus.subscribe();

        // Then
        assert_eq!(bus.subscriber_count(), 3, "Bus has 3 subscribers");
    }

    /// Publish to single subscriber test
    ///
    /// GIVEN: EventBus with 1 subscriber
    /// WHEN: publish(event) is called
    /// THEN: Subscriber receives event
    /// AND: publish() returns Ok(1)
    ///
    /// @plan PLAN-20250125-REFACTOR.P05
    /// @requirement REQ-020.2
    #[tokio::test]
    async fn test_publish_to_single_subscriber() {
        // Given
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();
        let event = AppEvent::User(UserEvent::SendMessage {
            text: "Hello".to_string(),
        });

        // When
        let result = bus.publish(event.clone());

        // Then
        assert!(result.is_ok(), "Publish succeeds");
        assert_eq!(result.unwrap(), 1, "One subscriber received event");

        // Verify subscriber received event
        let received = rx.recv().await;
        assert!(received.is_ok(), "Subscriber received event");
        assert_eq!(received.unwrap(), event, "Received event matches published");
    }

    /// EV-T1: EventBus delivers events to all subscribers
    ///
    /// GIVEN: EventBus with 3 subscribers
    /// WHEN: publish(event) is called
    /// THEN: All 3 subscribers receive event
    /// AND: publish() returns Ok(3)
    ///
    /// @plan PLAN-20250125-REFACTOR.P05
    /// @requirement REQ-020.2
    #[tokio::test]
    async fn test_ev_t1_delivers_to_all_subscribers() {
        // Given
        let bus = EventBus::new(16);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();
        let mut rx3 = bus.subscribe();
        let event = AppEvent::User(UserEvent::NewConversation);

        // When
        let result = bus.publish(event.clone());

        // Then
        assert!(result.is_ok(), "Publish succeeds");
        assert_eq!(result.unwrap(), 3, "Three subscribers received event");

        // Verify all subscribers received event
        let received1 = rx1.recv().await;
        let received2 = rx2.recv().await;
        let received3 = rx3.recv().await;

        assert!(received1.is_ok(), "Subscriber 1 received event");
        assert!(received2.is_ok(), "Subscriber 2 received event");
        assert!(received3.is_ok(), "Subscriber 3 received event");

        assert_eq!(received1.unwrap(), event, "Subscriber 1 got correct event");
        assert_eq!(received2.unwrap(), event, "Subscriber 2 got correct event");
        assert_eq!(received3.unwrap(), event, "Subscriber 3 got correct event");
    }

    /// Publish with no subscribers error test
    ///
    /// GIVEN: EventBus with 0 subscribers
    /// WHEN: publish(event) is called
    /// THEN: publish() returns Err(EventBusError::NoSubscribers)
    /// AND: Event is not delivered anywhere
    ///
    /// @plan PLAN-20250125-REFACTOR.P05
    /// @requirement REQ-020.3
    #[test]
    fn test_publish_no_subscribers_error() {
        // Given
        let bus = EventBus::new(16);
        let event = AppEvent::System(SystemEvent::AppLaunched);

        // When
        let result = bus.publish(event);

        // Then
        assert!(result.is_err(), "Publish fails with no subscribers");
        assert!(
            matches!(result, Err(EventBusError::NoSubscribers)),
            "Error is NoSubscribers"
        );
    }

    /// EV-T2: Events are delivered in order
    ///
    /// GIVEN: EventBus with 1 subscriber
    /// WHEN: 5 events are emitted in sequence
    /// THEN: Events are received in the same order
    ///
    /// @plan PLAN-20250125-REFACTOR.P05
    /// @requirement EV-T2
    #[tokio::test]
    async fn test_ev_t2_events_delivered_in_order() {
        // Given
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        let events = vec![
            AppEvent::User(UserEvent::SendMessage {
                text: "First".to_string(),
            }),
            AppEvent::User(UserEvent::SendMessage {
                text: "Second".to_string(),
            }),
            AppEvent::User(UserEvent::SendMessage {
                text: "Third".to_string(),
            }),
            AppEvent::User(UserEvent::SendMessage {
                text: "Fourth".to_string(),
            }),
            AppEvent::User(UserEvent::SendMessage {
                text: "Fifth".to_string(),
            }),
        ];

        // When
        for event in &events {
            let _ = bus.publish(event.clone());
        }

        // Then - verify order
        for expected in &events {
            let received = rx.recv().await;
            assert!(received.is_ok(), "Event received");
            assert_eq!(received.unwrap(), *expected, "Events received in order");
        }
    }

    /// EV-T3: Slow subscriber doesn't block fast subscribers
    ///
    /// GIVEN: EventBus with 2 subscribers (one slow, one fast)
    /// WHEN: Events are published rapidly
    /// THEN: Fast subscriber receives events normally
    /// AND: Slow subscriber may lag but doesn't block fast subscriber
    ///
    /// @plan PLAN-20250125-REFACTOR.P05
    /// @requirement EV-T3
    #[tokio::test]
    async fn test_ev_t3_slow_subscriber_doesnt_block_fast() {
        // Given
        let bus = EventBus::new(16);
        let mut fast_rx = bus.subscribe();
        let mut slow_rx = bus.subscribe();

        let event = AppEvent::Chat(ChatEvent::StreamStarted {
            conversation_id: uuid::Uuid::new_v4(),
            message_id: uuid::Uuid::new_v4(),
            model_id: "test-model".to_string(),
        });

        // When - publish event
        let _ = bus.publish(event.clone());

        // Then - fast subscriber receives immediately
        let fast_result = tokio::time::timeout(Duration::from_millis(100), fast_rx.recv()).await;
        assert!(fast_result.is_ok(), "Fast subscriber receives event");
        assert_eq!(fast_result.unwrap().unwrap(), event, "Fast got correct event");

        // Slow subscriber can take its time
        let slow_result = slow_rx.recv().await;
        assert!(slow_result.is_ok(), "Slow subscriber eventually receives event");
    }

    /// EV-T5: Multiple event types can be emitted and received
    ///
    /// GIVEN: EventBus with subscriber
    /// WHEN: Each event type is published
    /// THEN: All events are received successfully
    ///
    /// @plan PLAN-20250125-REFACTOR.P05
    /// @requirement REQ-020.6
    #[tokio::test]
    async fn test_ev_t5_multiple_event_types() {
        // Given
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        // Create IDs for events
        let mcp_id = uuid::Uuid::new_v4();
        let profile_id = uuid::Uuid::new_v4();
        let conversation_id = uuid::Uuid::new_v4();

        // When - publish all event types
        let _ = bus.publish(AppEvent::User(UserEvent::ToggleThinking));
        let _ = bus.publish(AppEvent::Chat(ChatEvent::TextDelta {
            text: "test".to_string(),
        }));
        let _ = bus.publish(AppEvent::Mcp(McpEvent::Starting {
            id: mcp_id,
            name: "test-mcp".to_string(),
        }));
        let _ = bus.publish(AppEvent::Profile(ProfileEvent::Created {
            id: profile_id,
            name: "test-profile".to_string(),
        }));
        let _ = bus.publish(AppEvent::Conversation(ConversationEvent::Activated {
            id: conversation_id,
        }));
        let _ = bus.publish(AppEvent::Navigation(NavigationEvent::Navigated {
            view: ViewId::Settings,
        }));
        let _ = bus.publish(AppEvent::System(SystemEvent::AppBecameActive));

        // Then - all events received
        let user_event = AppEvent::User(UserEvent::ToggleThinking);
        let chat_event = AppEvent::Chat(ChatEvent::TextDelta {
            text: "test".to_string(),
        });
        let mcp_event = AppEvent::Mcp(McpEvent::Starting {
            id: mcp_id,
            name: "test-mcp".to_string(),
        });
        let profile_event = AppEvent::Profile(ProfileEvent::Created {
            id: profile_id,
            name: "test-profile".to_string(),
        });
        let conversation_event = AppEvent::Conversation(ConversationEvent::Activated {
            id: conversation_id,
        });
        let navigation_event = AppEvent::Navigation(NavigationEvent::Navigated {
            view: ViewId::Settings,
        });
        let system_event = AppEvent::System(SystemEvent::AppBecameActive);

        assert_eq!(rx.recv().await.unwrap(), user_event);
        assert_eq!(rx.recv().await.unwrap(), chat_event);
        assert_eq!(rx.recv().await.unwrap(), mcp_event);
        assert_eq!(rx.recv().await.unwrap(), profile_event);
        assert_eq!(rx.recv().await.unwrap(), conversation_event);
        assert_eq!(rx.recv().await.unwrap(), navigation_event);
        assert_eq!(rx.recv().await.unwrap(), system_event);
    }

    /// EV-T6: Full send message flow emits correct event sequence
    ///
    /// GIVEN: EventBus with subscriber
    /// WHEN: User sends a message (UserEvent::SendMessage)
    /// THEN: ChatService responds with appropriate ChatEvents
    ///
    /// @plan PLAN-20250125-REFACTOR.P05
    /// @requirement EV-T6
    #[tokio::test]
    async fn test_ev_t6_send_message_flow() {
        // Given
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();
        let conversation_id = uuid::Uuid::new_v4();
        let message_id = uuid::Uuid::new_v4();

        // When - simulate send message flow
        let _ = bus.publish(AppEvent::User(UserEvent::SendMessage {
            text: "Hello".to_string(),
        }));
        let _ = bus.publish(AppEvent::Chat(ChatEvent::StreamStarted {
            conversation_id,
            message_id,
            model_id: "claude-3-5-sonnet".to_string(),
        }));
        let _ = bus.publish(AppEvent::Chat(ChatEvent::TextDelta {
            text: "Hi".to_string(),
        }));
        let _ = bus.publish(AppEvent::Chat(ChatEvent::TextDelta {
            text: " there".to_string(),
        }));
        let _ = bus.publish(AppEvent::Chat(ChatEvent::StreamCompleted {
            conversation_id,
            message_id,
            total_tokens: Some(10),
        }));

        // Then - verify sequence
        let received1 = rx.recv().await.unwrap();
        assert!(matches!(received1, AppEvent::User(UserEvent::SendMessage { .. })));

        let received2 = rx.recv().await.unwrap();
        assert!(matches!(received2, AppEvent::Chat(ChatEvent::StreamStarted { .. })));

        let received3 = rx.recv().await.unwrap();
        assert!(matches!(received3, AppEvent::Chat(ChatEvent::TextDelta { .. })));

        let received4 = rx.recv().await.unwrap();
        assert!(matches!(received4, AppEvent::Chat(ChatEvent::TextDelta { .. })));

        let received5 = rx.recv().await.unwrap();
        assert!(matches!(received5, AppEvent::Chat(ChatEvent::StreamCompleted { .. })));
    }

    /// EV-T7: MCP toggle flow emits correct event sequence
    ///
    /// GIVEN: EventBus with subscriber
    /// WHEN: User toggles MCP enabled
    /// THEN: McpService emits Starting -> Started events
    ///
    /// @plan PLAN-20250125-REFACTOR.P05
    /// @requirement EV-T7
    #[tokio::test]
    async fn test_ev_t7_mcp_toggle_flow() {
        // Given
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();
        let mcp_id = uuid::Uuid::new_v4();
        let mcp_name = "test-mcp".to_string();

        // When - simulate MCP toggle flow
        let _ = bus.publish(AppEvent::User(UserEvent::ToggleMcp {
            id: mcp_id,
            enabled: true,
        }));
        let _ = bus.publish(AppEvent::Mcp(McpEvent::Starting {
            id: mcp_id,
            name: mcp_name.clone(),
        }));
        let _ = bus.publish(AppEvent::Mcp(McpEvent::Started {
            id: mcp_id,
            name: mcp_name.clone(),
            tools: vec!["tool1".to_string(), "tool2".to_string()],
            tool_count: 2,
        }));

        // Then - verify sequence
        let received1 = rx.recv().await.unwrap();
        assert!(matches!(received1, AppEvent::User(UserEvent::ToggleMcp { .. })));

        let received2 = rx.recv().await.unwrap();
        assert!(matches!(received2, AppEvent::Mcp(McpEvent::Starting { .. })));

        let received3 = rx.recv().await.unwrap();
        if let AppEvent::Mcp(McpEvent::Started { name, .. }) = received3 {
            assert_eq!(name, mcp_name);
        } else {
            panic!("Expected McpEvent::Started");
        }
    }

    /// EV-T8: Error events work correctly
    ///
    /// GIVEN: EventBus with subscriber
    /// WHEN: Error events are emitted
    /// THEN: Error events are received with correct structure
    ///
    /// @plan PLAN-20250125-REFACTOR.P05
    /// @requirement EV-T8
    #[tokio::test]
    async fn test_ev_t8_error_events() {
        // Given
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        // When - emit various error events
        let _ = bus.publish(AppEvent::Chat(ChatEvent::StreamError {
            conversation_id: uuid::Uuid::new_v4(),
            error: "Connection failed".to_string(),
            recoverable: true,
        }));
        let _ = bus.publish(AppEvent::Mcp(McpEvent::StartFailed {
            id: uuid::Uuid::new_v4(),
            name: "failing-mcp".to_string(),
            error: "Timeout".to_string(),
        }));
        let _ = bus.publish(AppEvent::System(SystemEvent::Error {
            source: "ChatService".to_string(),
            error: "Unexpected error".to_string(),
            context: Some("During streaming".to_string()),
        }));

        // Then - verify error events
        let received1 = rx.recv().await.unwrap();
        if let AppEvent::Chat(ChatEvent::StreamError { error, recoverable, .. }) = received1 {
            assert_eq!(error, "Connection failed");
            assert!(recoverable);
        } else {
            panic!("Expected ChatEvent::StreamError");
        }

        let received2 = rx.recv().await.unwrap();
        if let AppEvent::Mcp(McpEvent::StartFailed { error, .. }) = received2 {
            assert_eq!(error, "Timeout");
        } else {
            panic!("Expected McpEvent::StartFailed");
        }

        let received3 = rx.recv().await.unwrap();
        if let AppEvent::System(SystemEvent::Error { source, error, context }) = received3 {
            assert_eq!(source, "ChatService");
            assert_eq!(error, "Unexpected error");
            assert_eq!(context, Some("During streaming".to_string()));
        } else {
            panic!("Expected SystemEvent::Error");
        }
    }
}

