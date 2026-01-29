# Pseudocode: EventBus Implementation

**Component**: Event System (src/events/)
**Pattern**: Publisher-Subscriber with tokio::sync::broadcast
**Dependencies**: tokio, tracing, serde

---

## Interface Contracts

### Inputs
- Event emissions from any component
- Subscription requests from components

### Outputs
- Event delivery to all active subscribers
- Event metadata (timestamp, source)

### Dependencies
```rust
struct EventBusDependencies {
    // No external dependencies for core event bus
    // Self-contained broadcast channel
}
```

---

## EventBus Core Implementation

10: STRUCT EventBus
11:   tx: broadcast::Sender<AppEvent>
12:   _rx: broadcast::Receiver<AppEvent>  // Keep channel alive

20: IMPL EventBus
21:   FUNCTION new(capacity: usize) -> Self
22:     LET (tx, rx) = broadcast::channel(capacity)
23:     RETURN EventBus { tx, _rx: rx }

30:   FUNCTION publish(&self, event: AppEvent) -> Result<usize, EventBusError>
31:     LET event_count = self.tx.send(event.clone())
32:     MATCH event_count
33:       Ok(count) => 
34:         LOG_EMIT(event, count)
35:         RETURN Ok(count)
36:       Err(e) => 
37:         LOG_NO_SUBSCRIBERS(event)
38:         RETURN Err(EventBusError::NoSubscribers)

40:   FUNCTION subscribe(&self) -> broadcast::Receiver<AppEvent>
41:     RETURN self.tx.subscribe()

45:   FUNCTION subscriber_count(&self) -> usize
46:     RETURN self.tx.receiver_count()

---

## Global Event Bus Singleton

50: STATIC GLOBAL_BUS: OnceLock<Arc<EventBus>> = OnceLock::new()

55: FUNCTION init_event_bus() -> Arc<EventBus>
56:   IF GLOBAL_BUS.get().is_some()
57:     RETURN GLOBAL_BUS.get().unwrap().clone()
59:   LET bus = Arc::new(EventBus::new(16))
60:   LET _ = GLOBAL_BUS.set(bus.clone()).expect("once lock initialized")
61:   RETURN bus

65: FUNCTION emit(event: AppEvent) -> Result<(), EventBusError>
66:   LET bus = get_or_init_event_bus()
67:   MATCH bus.publish(event)
68:     Ok(_) => RETURN Ok(())
69:     Err(e) => RETURN Err(e)

73: FUNCTION subscribe() -> broadcast::Receiver<AppEvent>
74:   LET bus = get_or_init_event_bus()
75:   RETURN bus.subscribe()

---

## Event Type Hierarchy

80: ENUM AppEvent
81:   User(UserEvent)      // Emitted by UI views
82:   Chat(ChatEvent)      // Emitted by ChatService
83:   Mcp(McpEvent)        // Emitted by McpService
84:   System(SystemEvent)  // Emitted by core system

90: ENUM UserEvent
91:   SendMessage { conversation_id: Uuid, content: String }
92:   CancelRequest { conversation_id: Uuid }
93:   OpenSettings { }
94:   OpenHistory { }
95:   Quit { }

100: ENUM ChatEvent
101:  ConversationStarted { id: Uuid, profile_id: Uuid }
102:  MessageReceived { conversation_id: Uuid, message: Message }
103:  ThinkingStarted { conversation_id: Uuid }
104:  ThinkingEnded { conversation_id: Uuid }
105:  ResponseGenerated { conversation_id: Uuid, tokens: u64 }
106:  Error { conversation_id: Uuid, error: String }

110: ENUM McpEvent
111:  ServerStarting { name: String }
112:  ServerStarted { id: Uuid, tool_count: usize }
113:  ServerFailed { id: Uuid, error: String }
114:  ServerStopped { id: Uuid }
115:  ToolsUpdated { tools: Vec<ToolDefinition> }
116:  ToolCalled { tool_name: String, args: Value }
117:  ToolResult { tool_name: String, result: Value }

120: ENUM SystemEvent
121:  Shutdown { }
122:  Error { error: String }
123:  ConfigChanged { key: String }

---

## Event Logging

130: FUNCTION LOG_EMIT(event: AppEvent, count: usize)
131:  MATCH &event
132:    AppEvent::User(e) => info!("User event: {:?} ({} subscribers)", e, count)
133:    AppEvent::Chat(e) => debug!("Chat event: {:?} ({} subscribers)", e, count)
134:    AppEvent::Mcp(e) => info!("MCP event: {:?} ({} subscribers)", e, count)
135:    AppEvent::System(e) => warn!("System event: {:?} ({} subscribers)", e, count)

140: FUNCTION LOG_NO_SUBSCRIBERS(event: AppEvent)
141:  warn!("Event dropped (no subscribers): {:?}", event)

---

## Helper: Get or Initialize

150: FUNCTION get_or_init_event_bus() -> Arc<EventBus>
151:  MATCH GLOBAL_BUS.get()
152:    Some(bus) => RETURN bus.clone()
153:    None => 
154:      LET bus = Arc::new(EventBus::new(16))
155:      LET _ = GLOBAL_BUS.set(bus.clone()).expect("once lock initialized")
156:      RETURN bus

---

## Integration Points

### Line 30-38: Event Publishing
- **Called by**: UI views (emit UserEvent), Services (emit domain events)
- **Behavior**: Broadcasts to ALL active subscribers
- **Error case**: If no subscribers, returns Err (caller decides what to do)
- **Async**: NO (broadcast::send is synchronous, but non-blocking)

### Line 40-41: Subscription
- **Called by**: Presenters, background services
- **Returns**: Receiver that can be cloned for multiple listeners
- **Async**: NO (subscription is synchronous)

### Line 65-69: Global emit()
- **Called by**: Any code that needs to emit events
- **Behavior**: Convenience wrapper around EventBus::publish
- **Initializes**: Creates EventBus on first call (lazy init)

---

## Error Handling

160: ENUM EventBusError
161:  NoSubscribers
162:  ChannelClosed

---

## Concurrency Considerations

**tokio::sync::broadcast properties**:
- Multiple senders NOT supported (single Sender in EventBus)
- Multiple receivers supported (via subscribe())
- Channel buffer: capacity (16 events)
- When buffer full: Oldest event dropped (Lag error receiver)
- send() is lock-free (atomic reference count)

**Thread Safety**:
- EventBus is Send + Sync (Arc<EventBus> safe to share)
- publish() can be called from any thread
- subscribe() can be called from any thread

---

## Anti-Pattern Warnings

[ERROR] DO NOT:
```rust
// WRONG: Create multiple event buses
let bus1 = EventBus::new(16);
let bus2 = EventBus::new(16);  // Fragmented event flow
```

[OK] DO:
```rust
// RIGHT: Use singleton global bus
emit(AppEvent::User(UserEvent::SendMessage { ... }));
```

[ERROR] DO NOT:
```rust
// WRONG: Blocking recv on main thread
let rx = subscribe();
let event = rx.recv().unwrap();  // Blocks UI!
```

[OK] DO:
```rust
// RIGHT: Async recv in presenter task
let mut rx = subscribe();
while let Ok(event) = rx.recv().await {
    // Handle event in background task
}
```

[ERROR] DO NOT:
```rust
// WRONG: Assume delivery guarantees
emit(event);  // If no subscribers, event is lost
```

[OK] DO:
```rust
// RIGHT: Handle no-subscriber case
match emit(event) {
    Ok(_) => {},
    Err(EventBusError::NoSubscribers) => {
        warn!("Event unhandled: {:?}", event);
    }
}
```
