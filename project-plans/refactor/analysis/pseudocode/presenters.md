# Pseudocode: Presenter Layer Implementation

**Component**: Presenter Layer (src/presentation/)
**Pattern**: MVP (Model-View-Presenter) with EventBus mediation
**Dependencies**: tokio, uuid, tracing, EventBus

---

## Interface Contracts

### Inputs to Presenters
- User events from EventBus (subscribe to AppEvent::User)
- Domain events from services (subscribe to AppEvent::Chat, AppEvent::Mcp, etc.)

### Outputs from Presenters
- View updates (ViewCommands sent to UI layer)
- Service requests (via Service trait methods)
- Error events (emit to EventBus for ErrorPresenter)

### Dependencies (NEVER stubbed)
```rust
struct PresenterDependencies {
    event_bus: Arc<EventBus>,           // For subscribing to events
    services: Arc<ServiceRegistry>,     // Real service references
    view_tx: broadcast::Sender<ViewCommand>, // For commanding UI
}
```

---

## Base Presenter Trait

10: TRAIT Presenter: Send + Sync
11:   ASYNC FUNCTION start(&mut self) -> Result<(), PresenterError>
12:   ASYNC FUNCTION stop(&mut self) -> Result<(), PresenterError>
13:   FUNCTION is_running(&self) -> bool

---

## ChatPresenter

### State

20: STRUCT ChatPresenter
21:   rx: broadcast::Receiver<AppEvent>
22:   services: Arc<ServiceRegistry>
23:   view_tx: broadcast::Sender<ViewCommand>
24:   running: Arc<AtomicBool>
25:   runtime: Arc<GlobalRuntime>

### Lifecycle

30: IMPL ChatPresenter
31:   FUNCTION new(
32:     services: Arc<ServiceRegistry>,
33:     view_tx: broadcast::Sender<ViewCommand>
34:   ) -> Self
35:     LET rx = get_event_bus().subscribe()
36:     RETURN ChatPresenter {
37:       rx,
38:       services,
39:       view_tx,
40:       running: Arc::new(AtomicBool::new(false)),
41:       runtime: services.runtime.clone(),
42:     }

### Event Loop

50:   ASYNC FUNCTION start(&mut self) -> Result<(), PresenterError>
51:     IF self.running.load(Ordering::Relaxed) THEN RETURN Ok(())

53:     self.running.store(true, Ordering::Relaxed)
55:     SPAWN task in self.runtime:
56:       WHILE self.running.load(Ordering::Relaxed) {
57:         MATCH self.rx.recv().await {
58:           Ok(event) => self.handle_event(event).await,
59:           Err(e) => {
60:             // Lag detected (receiver dropped)
61:             WARN!("ChatPresenter lag: {}", e)
62:             CONTINUE
63:           }
64:         }
65:       }
67:       INFO!("ChatPresenter event loop ended")
69:     RETURN Ok(())

### Event Handling

80:   ASYNC FUNCTION handle_event(&self, event: AppEvent)
81:     MATCH event {
82:       AppEvent::User(user_evt) => self.handle_user_event(user_evt).await,
83:       AppEvent::Chat(chat_evt) => self.handle_chat_event(chat_evt).await,
84:       _ => {}, // Ignore other events
85:     }

### User Event Handlers

100:  ASYNC FUNCTION handle_user_event(&self, event: UserEvent)
101:    MATCH event {
102:      UserEvent::SendMessage { conversation_id, content } =>
103:        self.on_send_message(conversation_id, content).await,
104:      UserEvent::CancelRequest { conversation_id } =>
105:        self.on_cancel_request(conversation_id).await,
106:      _ => {},
107:    }

120:  ASYNC FUNCTION on_send_message(&self, conversation_id: Uuid, content: String)
121:    VALIDATE content is not empty
122:    IF content.trim().is_empty() THEN RETURN

124:    // If no conversation_id, create new conversation
125:    LET target_id = IF conversation_id.is_nil() {
126:      LET profile = self.services.profiles.get_default()
127:      IF profile.is_none() THEN {
128:        EMIT ViewCommand::ShowError("No default profile".to_string())
129:        RETURN
130:      }
132:      LET new_id = self.services.conversations.create(profile.id).await?
133:      self.services.conversations.set_active(new_id).await?
134:      new_id
135:    } ELSE {
136:      conversation_id
137:    }

139:    // Call ChatService to send message (triggers streaming)
140:    MATCH self.services.chat.send_message_stream(target_id, content).await {
141:      Ok(_) => {},
142:      Err(e) => {
143:        ERROR!("Failed to send message: {}", e)
144:        EMIT ViewCommand::ShowError(format!("Failed: {}", e))
145:      }
146:    }

### Chat Event Handlers

160:  ASYNC FUNCTION handle_chat_event(&self, event: ChatEvent)
161:    MATCH event {
162:      ChatEvent::ConversationStarted { id, profile_id } =>
163:        self.on_conversation_started(id, profile_id).await,
164:      ChatEvent::MessageReceived { conversation_id, message } =>
165:        self.on_message_received(conversation_id, message).await,
166:      ChatEvent::ThinkingStarted { conversation_id } =>
167:        self.on_thinking_started(conversation_id).await,
168:      ChatEvent::ThinkingEnded { conversation_id } =>
169:        self.on_thinking_ended(conversation_id).await,
170:      ChatEvent::ResponseGenerated { conversation_id, tokens } =>
171:        self.on_response_generated(conversation_id, tokens).await,
172:      ChatEvent::StreamChunk { conversation_id, chunk } =>
173:        self.on_stream_chunk(conversation_id, chunk).await,
174:      ChatEvent::Error { conversation_id, error } =>
175:        self.on_chat_error(conversation_id, error).await,
176:      _ => {},
177:    }

190:  FUNCTION on_conversation_started(&self, id: Uuid, profile_id: Uuid)
191:    EMIT ViewCommand::ConversationCreated { id, profile_id } via self.view_tx

200:  FUNCTION on_message_received(&self, conversation_id: Uuid, message: Message)
201:    EMIT ViewCommand::MessageAppended {
202:      conversation_id,
203:      role: message.role,
204:      content: message.content,
205:    } via self.view_tx

210:  FUNCTION on_thinking_started(&self, conversation_id: Uuid)
211:    EMIT ViewCommand::ShowThinking { conversation_id } via self.view_tx

215:  FUNCTION on_thinking_ended(&self, conversation_id: Uuid)
216:    EMIT ViewCommand::HideThinking { conversation_id } via self.view_tx

220:  FUNCTION on_stream_chunk(&self, conversation_id: Uuid, chunk: String)
221:    EMIT ViewCommand::AppendStream { conversation_id, chunk } via self.view_tx

230:  FUNCTION on_response_generated(&self, conversation_id: Uuid, tokens: u64)
231:    EMIT ViewCommand::FinalizeStream { conversation_id, tokens } via self.view_tx

240:  FUNCTION on_chat_error(&self, conversation_id: Uuid, error: String)
241:    EMIT ViewCommand::ShowError { conversation_id, error } via self.view_tx

### Shutdown

250:  ASYNC FUNCTION stop(&mut self) -> Result<(), PresenterError>
251:    self.running.store(false, Ordering::Relaxed)
252:    EMIT AppEvent::System(SystemEvent::Shutdown) via EventBus
253:    RETURN Ok(())

---

## McpPresenter

### State

260: STRUCT McpPresenter
261:  rx: broadcast::Receiver<AppEvent>
262:  services: Arc<ServiceRegistry>
263:  view_tx: broadcast::Sender<ViewCommand>
264:  running: Arc<AtomicBool>
265:  runtime: Arc<GlobalRuntime>

### Event Handling

280:  ASYNC FUNCTION handle_user_event(&self, event: UserEvent)
281:    MATCH event {
282:      UserEvent::StartMcpServer { config } =>
283:        self.on_start_server(config).await,
284:      UserEvent::StopMcpServer { id } =>
285:        self.on_stop_server(id).await,
286:      UserEvent::RefreshMcpTools { } =>
287:        self.on_refresh_tools().await,
288:      _ => {},
289:    }

300:  ASYNC FUNCTION on_start_server(&self, config: McpConfig)
301:    VALIDATE config is complete (name, command, args)
302:    IF invalid THEN {
303:      EMIT ViewCommand::ShowError("Invalid MCP config".to_string())
304:      RETURN
305:    }

307:    MATCH self.services.mcp.start_server(config).await {
308:      Ok(id) => {
309:        INFO!("MCP server started: {}", id)
310:        // Wait for McpEvent::ServerStarted to update UI
311:      }
312:      Err(e) => {
313:        ERROR!("Failed to start MCP server: {}", e)
314:        EMIT ViewCommand::ShowError(format!("Failed: {}", e))
315:      }
316:    }

330:  ASYNC FUNCTION handle_mcp_event(&self, event: McpEvent)
331:    MATCH event {
332:      McpEvent::ServerStarted { id, tool_count } =>
333:        self.on_server_started(id, tool_count).await,
334:      McpEvent::ServerFailed { id, error } =>
335:        self.on_server_failed(id, error).await,
336:      McpEvent::ToolsUpdated { tools } =>
337:        self.on_tools_updated(tools).await,
338:      _ => {},
339:    }

350:  FUNCTION on_server_started(&self, id: Uuid, tool_count: usize)
351:    EMIT ViewCommand::McpServerStarted { id, tool_count } via self.view_tx

360:  FUNCTION on_server_failed(&self, id: Uuid, error: String)
361:    EMIT ViewCommand::ShowError { error } via self.view_tx

370:  FUNCTION on_tools_updated(&self, tools: Vec<ToolDefinition>)
371:    EMIT ViewCommand::McpToolsUpdated { tools } via self.view_tx

---

## SettingsPresenter

### State

380: STRUCT SettingsPresenter
381:  rx: broadcast::Receiver<AppEvent>
382:  services: Arc<ServiceRegistry>
383:  view_tx: broadcast::Sender<ViewCommand>
384:  running: Arc<AtomicBool>
385:  runtime: Arc<GlobalRuntime>

### Event Handling

400:  ASYNC FUNCTION handle_user_event(&self, event: UserEvent)
401:    MATCH event {
402:      UserEvent::OpenSettings { } =>
403:        self.on_open_settings().await,
404:      UserEvent::UpdateProfile { profile } =>
405:        self.on_update_profile(profile).await,
406:      UserEvent::DeleteProfile { id } =>
407:        self.on_delete_profile(id).await,
408:      _ => {},
409:    }

420:  ASYNC FUNCTION on_open_settings(&self)
421:    LET profiles = self.services.profiles.list_profiles()
422:    EMIT ViewCommand::ShowSettings { profiles } via self.view_tx

430:  ASYNC FUNCTION on_update_profile(&self, profile: ModelProfile)
431:    VALIDATE profile data
432:    IF invalid THEN {
433:      EMIT ViewCommand::ShowError("Invalid profile".to_string())
434:      RETURN
435:    }

437:    MATCH self.services.profiles.update_profile(profile).await {
438:      Ok(_) => {
439:        EMIT ViewCommand::ShowNotification("Profile updated".to_string())
440:      }
441:      Err(e) => {
442:        EMIT ViewCommand::ShowError(format!("Failed: {}", e))
443:      }
444:    }

---

## ErrorPresenter

### State

450: STRUCT ErrorPresenter
451:  rx: broadcast::Receiver<AppEvent>
452:  view_tx: broadcast::Sender<ViewCommand>
453:  running: Arc<AtomicBool>
454:  runtime: Arc<GlobalRuntime>

### Event Handling

470:  ASYNC FUNCTION handle_event(&self, event: AppEvent)
471:    MATCH event {
472:      AppEvent::System(SystemEvent::Error { error }) =>
473:        self.on_system_error(error).await,
474:      AppEvent::Chat(ChatEvent::Error { conversation_id, error }) =>
475:        self.on_chat_error(conversation_id, error).await,
476:      AppEvent::Mcp(McpEvent::ServerFailed { id, error }) =>
477:        self.on_mcp_error(id, error).await,
478:      _ => {},
479:    }

490:  FUNCTION on_system_error(&self, error: String)
491:    EMIT ViewCommand::ShowError {
492:      title: "System Error".to_string(),
493:      message: error,
494:      severity: ErrorSeverity::Critical,
495:    } via self.view_tx

500:  FUNCTION on_chat_error(&self, conversation_id: Uuid, error: String)
501:    EMIT ViewCommand::ShowError {
502:      title: "Chat Error".to_string(),
503:      message: error,
504:      severity: ErrorSeverity::Warning,
505:    } via self.view_tx

---

## ViewCommand Type

510: ENUM ViewCommand
511:  // Chat commands
512:  ConversationCreated { id: Uuid, profile_id: Uuid }
513:  MessageAppended { conversation_id: Uuid, role: Role, content: String }
514:  ShowThinking { conversation_id: Uuid }
515:  HideThinking { conversation_id: Uuid }
516:  AppendStream { conversation_id: Uuid, chunk: String }
517:  FinalizeStream { conversation_id: Uuid, tokens: u64 }

520:  // MCP commands
521:  McpServerStarted { id: Uuid, tool_count: usize }
522:  McpToolsUpdated { tools: Vec<ToolDefinition> }

530:  // Settings commands
531:  ShowSettings { profiles: Vec<(Uuid, ModelProfile)> }
532:  ShowNotification { message: String }

540:  // Error commands
541:  ShowError { title: String, message: String, severity: ErrorSeverity }

---

## Integration Points

### Line 50-69: Presenter Event Loop
- **Subscribes to**: EventBus (AppEvent stream)
- **Runs in**: Background task (spawned in runtime)
- **Handles**: All relevant events for this presenter
- **Lag handling**: Catches RecvError and logs (continues running)

### Line 120-146: Message Send Flow
- **Input**: UserEvent::SendMessage from UI
- **Validates**: Content not empty
- **Creates**: New conversation if needed (via ConversationService)
- **Delegates**: ChatService.send_message_stream (async, non-blocking)
- **Emits**: ViewCommands for UI updates

### Line 300-316: MCP Server Start Flow
- **Input**: UserEvent::StartMcpServer from UI
- **Validates**: Config completeness
- **Calls**: McpService.start_server (spawns MCP process)
- **Waits for**: McpEvent::ServerStarted (from service) to update UI
- **Emits**: ViewCommands for success/error

---

## Concurrency Patterns

**Event-driven, non-blocking**:
- Presenters never block on service calls
- All service operations are async/await
- Long-running operations spawned in background tasks

**No shared state**:
- Presenters are stateless (except event receivers)
- All state in services (with Arc<Mutex<T>>)
- View updates via ViewCommand events

**Graceful shutdown**:
- Line 250-253: AtomicBool for stop flag
- Emits SystemEvent::Shutdown
- Background tasks check flag and exit

---

## Error Handling

550: ENUM PresenterError
551:  EventStreamClosed
552:  ServiceCallFailed(String)
553:  InvalidState(String)

---

## Anti-Pattern Warnings

[ERROR] DO NOT:
```rust
// WRONG: Block on async in event loop
WHILE running {
  let event = rx.recv().await?;
  self.services.chat.send_blocking(event)?;  // Blocks event loop!
}
```

[OK] DO:
```rust
// RIGHT: Spawn task for long operations
WHILE running {
  let event = rx.recv().await?;
  // Call async service method (non-blocking)
  self.services.chat.send_message_stream(id, msg).await?;
}
```

[ERROR] DO NOT:
```rust
// WRONG: Presenter holds UI state
STRUCT ChatPresenter {
  messages: Vec<Message>,  // WRONG - UI state
  current_conversation: Uuid,  // WRONG
}
```

[OK] DO:
```rust
// RIGHT: Presenter is stateless (except receiver)
STRUCT ChatPresenter {
  rx: broadcast::Receiver<AppEvent>,  // OK - event stream
  services: Arc<ServiceRegistry>,  // OK - injected dependency
  view_tx: broadcast::Sender<ViewCommand>,  // OK - UI command channel
}
```

[ERROR] DO NOT:
```rust
// WRONG: Direct UI manipulation
ASYNC FUNCTION on_message(&self, msg: Message) {
  ui_widget.append_text(msg.content);  // WRONG - tight coupling
}
```

[OK] DO:
```rust
// RIGHT: Emit view command
ASYNC FUNCTION on_message(&self, msg: Message) {
  self.view_tx.send(ViewCommand::MessageAppended {
    content: msg.content,
  })?;  // OK - decoupled via ViewCommand
}
```

[ERROR] DO NOT:
```rust
// WRONG: Subscribe to all events
LET rx = event_bus.subscribe();  // Receives ALL events (wasteful)
```

[OK] DO:
```rust
// RIGHT: Filter events in handler
// (tokio broadcast doesn't support server-side filtering)
// Filter in handle_event:
MATCH event {
  AppEvent::Chat(_) => {},  // Process
  _ => {},  // Ignore
}
```
