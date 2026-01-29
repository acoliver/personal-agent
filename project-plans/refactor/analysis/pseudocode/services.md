# Pseudocode: Service Layer Implementation

**Component**: Service Layer (src/services/)
**Pattern**: Service Traits with Arc<Mutex<T>> for state
**Dependencies**: tokio, uuid, serde, thiserror, tracing

---

## Interface Contracts

### Inputs to Services
- Configuration data (ModelProfile, McpConfig, etc.)
- Request parameters (conversation IDs, messages, tool calls)
- Events from EventBus (via subscriptions)

### Outputs from Services
- Response data (messages, tool results, status)
- Domain events emitted to EventBus
- State changes (conversations, profiles, connections)

### Shared Dependencies
```rust
struct ServiceDependencies {
    event_bus: Arc<EventBus>,      // For emitting domain events
    secrets: Arc<SecretsManager>,  // For API key storage
    registry: Arc<RegistryService>, // For provider/model lookups
    runtime: Arc<GlobalRuntime>,    // For spawning background tasks
}
```

---

## Service Traits (Base Interfaces)

10: TRAIT Service: Send + Sync
11:   FUNCTION name(&self) -> String
12:   FUNCTION is_healthy(&self) -> bool
13:   FUNCTION get_metrics(&self) -> ServiceMetrics
14:   ASYNC FUNCTION shutdown(&mut self) -> Result<(), ServiceError>

20: TRAIT RequestHandler<Req, Resp>: Service
21:   ASYNC FUNCTION handle(&self, request: Req) -> Result<Resp, ServiceError>

---

## ConversationService

### State

30: STRUCT ConversationService
31:   conversations: Arc<Mutex<HashMap<Uuid, Conversation>>>
32:   storage: Arc<ConversationRepository>
33:   active_id: Arc<Mutex<Option<Uuid>>>
34:   event_tx: broadcast::Sender<AppEvent>
35:   metrics: Arc<Mutex<ServiceMetrics>>

### Lifecycle

40: IMPL ConversationService
41:   FUNCTION new(
42:     storage: Arc<ConversationRepository>,
43:     event_tx: broadcast::Sender<AppEvent>
44:   ) -> Self
45:     LET service = ConversationService {
46:       conversations: Arc::new(Mutex::new(HashMap::new())),
47:       storage,
48:       active_id: Arc::new(Mutex::new(None)),
49:       event_tx,
50:       metrics: Arc::new(Mutex::new(ServiceMetrics::default())),
51:     }
52:     RETURN service

### Core Operations

60:   ASYNC FUNCTION create_conversation(&self, profile_id: Uuid) -> Result<Uuid, ServiceError>
61:     VALIDATE profile_id is valid (check with ProfileService)
62:     IF invalid THEN RETURN Err(ServiceError::InvalidInput("Invalid profile_id"))

64:     LET id = Uuid::new_v4()
65:     LET conversation = Conversation {
66:       id,
67:       profile_id,
68:       messages: Vec::new(),
69:       created_at: Utc::now(),
70:       updated_at: Utc::now(),
71:     }

73:     MUTEX_LOCK self.conversations
74:     self.conversations.insert(id, conversation)
75:     DROP mutex

77:     EMIT ChatEvent::ConversationStarted { id, profile_id } via self.event_tx
78:     UPDATE self.metrics (increment creation_count)
79:     RETURN Ok(id)

90:   ASYNC FUNCTION send_message(
91:     &self,
92:     conversation_id: Uuid,
93:     content: String
94:   ) -> Result<Message, ServiceError>
95:     VALIDATE conversation_id exists
96:     IF not exists THEN RETURN Err(ServiceError::NotFound("Conversation not found"))

98:     MUTEX_LOCK self.conversations
99:     LET conversation = self.conversations.get_mut(&conversation_id)
100:    IF conversation.is_none() THEN RETURN Err(ServiceError::NotFound("Conversation not found"))

102:    LET user_msg = Message {
103:      id: Uuid::new_v4(),
104:      role: Role::User,
105:      content,
106:      created_at: Utc::now(),
107:      model_id: None,
108:      cancelled: false,
109:      tool_calls: Vec::new(),
110:    }

112:    conversation.messages.push(user_msg.clone())
113:    conversation.updated_at = Utc::now()
114:    DROP mutex

116:    EMIT ChatEvent::MessageReceived { conversation_id, message: user_msg } via self.event_tx
117:    RETURN Ok(user_msg)

130:  FUNCTION get_conversation(&self, id: Uuid) -> Option<Conversation>
131:    MUTEX_LOCK self.conversations
132:    LET conv = self.conversations.get(&id).cloned()
133:    DROP mutex
134:    RETURN conv

140:  FUNCTION list_conversations(&self) -> Vec<Uuid>
141:    MUTEX_LOCK self.conversations
142:    LET ids = self.conversations.keys().cloned().collect()
143:    DROP mutex
144:    RETURN ids

150:  ASYNC FUNCTION set_active(&self, id: Uuid) -> Result<(), ServiceError>
151:    VALIDATE conversation exists
152:    IF not exists THEN RETURN Err(ServiceError::NotFound("Conversation not found"))

154:    MUTEX_LOCK self.active_id
155:    *self.active_id = Some(id)
156:    DROP mutex
157:    EMIT ChatEvent::ActiveConversationChanged { id } via self.event_tx
158:    RETURN Ok(())

160:  FUNCTION get_active(&self) -> Option<Uuid>
161:    MUTEX_LOCK self.active_id
162:    LET id = *self.active_id
163:    DROP mutex
164:    RETURN id

---

## ChatService

### State

170: STRUCT ChatService
171:  conversations: Arc<ConversationService>
172:  profiles: Arc<ProfileService>
173:  llm: Arc<LlmService>
174:  mcp: Arc<McpService>
175:  event_tx: broadcast::Sender<AppEvent>
176:  runtime: Arc<GlobalRuntime>

### Core Operations

180: IMPL ChatService
181:  FUNCTION new(
182:    conversations: Arc<ConversationService>,
183:    profiles: Arc<ProfileService>,
184:    llm: Arc<LlmService>,
185:    mcp: Arc<McpService>,
186:    event_tx: broadcast::Sender<AppEvent>,
187:    runtime: Arc<GlobalRuntime>
188:  ) -> Self
189:    RETURN ChatService { /* all fields */ }

200:  ASYNC FUNCTION send_message_stream(
201:    &self,
202:    conversation_id: Uuid,
203:    message: String
204:  ) -> Result<(), ServiceError>
205:    VALIDATE conversation exists
206:    LET conversation = self.conversations.get_conversation(conversation_id)
207:    IF conversation.is_none() THEN RETURN Err(ServiceError::NotFound("Conversation not found"))

209:    LET profile = self.profiles.get_profile(conversation.profile_id)
210:    IF profile.is_none() THEN RETURN Err(ServiceError::NotFound("Profile not found"))

212:    EMIT ChatEvent::ThinkingStarted { conversation_id } via self.event_tx

214:    SPAWN task in self.runtime:
215:      // Build message history
216:      LET history = self.build_message_history(&conversation)

218:      // Call LLM service with streaming
219:      LET callback = |chunk| {
220:        EMIT ChatEvent::StreamChunk { conversation_id, chunk } via self.event_tx
221:      }

223:      MATCH self.llm.request_stream(&profile, history, callback).await
224:        Ok(response) => {
225:          EMIT ChatEvent::ResponseGenerated { conversation_id, tokens: response.tokens }
226:          // Store assistant message
227:          self.conversations.send_message(conversation_id, response.content).await?
228:        }
229:        Err(e) => {
230:          EMIT ChatEvent::Error { conversation_id, error: e.to_string() }
231:        }

233:      EMIT ChatEvent::ThinkingEnded { conversation_id } via self.event_tx

235:    RETURN Ok(())

250:  FUNCTION build_message_history(&self, conversation: &Conversation) -> Vec<Message>
251:    RETURN conversation.messages.clone()

---

## McpService

### State

260: STRUCT McpService
261:  connections: Arc<Mutex<HashMap<Uuid, McpConnection>>>
262:  tools: Arc<Mutex<HashMap<String, Uuid>>>  // tool_name -> mcp_id
263:  secrets: Arc<SecretsManager>
264:  event_tx: broadcast::Sender<AppEvent>
265:  runtime: Arc<GlobalRuntime>

### Lifecycle

270: IMPL McpService
271:  FUNCTION new(
272:    secrets: Arc<SecretsManager>,
273:    event_tx: broadcast::Sender<AppEvent>,
274:    runtime: Arc<GlobalRuntime>
275:  ) -> Self
276:    RETURN McpService {
277:      connections: Arc::new(Mutex::new(HashMap::new())),
278:      tools: Arc::new(Mutex::new(HashMap::new())),
279:      secrets,
280:      event_tx,
281:      runtime,
282:    }

### Core Operations

290:  ASYNC FUNCTION start_server(&self, config: McpConfig) -> Result<Uuid, ServiceError>
291:    VALIDATE config.name is unique
292:    LET id = Uuid::new_v4()

294:    EMIT McpEvent::ServerStarting { name: config.name.clone() } via self.event_tx

296:    SPAWN task in self.runtime:
297:      LET connection = McpConnection::spawn(config.clone(), self.secrets.clone()).await

299:      MATCH connection
300:        Ok(conn) => {
301:          MUTEX_LOCK self.connections
302:          self.connections.insert(id, conn.clone())
303:          DROP mutex

305:          // Update tool registry
306:          FOR tool IN conn.tools.iter() {
307:            MUTEX_LOCK self.tools
308:            self.tools.insert(tool.name.clone(), id)
309:            DROP mutex
310:          }

312:          EMIT McpEvent::ServerStarted { id, tool_count: conn.tools.len() }
313:        }
314:        Err(e) => {
315:          EMIT McpEvent::ServerFailed { id, error: e.to_string() }
316:          RETURN Err(e)
317:        }

319:    RETURN Ok(id)

330:  FUNCTION list_tools(&self) -> Vec<ToolDefinition>
330:    MUTEX_LOCK self.connections
331:    LET all_tools = self.connections.values()
332:      .flat_map(|conn| conn.tools.clone())
333:      .collect()
334:    DROP mutex
335:    RETURN all_tools

340:  ASYNC FUNCTION call_tool(
341:    &self,
342:    tool_name: &str,
343:    args: Value
344:  ) -> Result<Value, ServiceError>
345:    MUTEX_LOCK self.tools
346:    LET mcp_id = self.tools.get(tool_name)
347:    DROP mutex

348:    IF mcp_id.is_none() THEN RETURN Err(ServiceError::NotFound("Tool not found"))

350:    MUTEX_LOCK self.connections
351:    LET connection = self.connections.get(&mcp_id.unwrap())
352:    DROP mutex

353:    IF connection.is_none() THEN RETURN Err(ServiceError::NotFound("MCP connection not found"))

355:    EMIT McpEvent::ToolCalled { tool_name: tool_name.to_string(), args: args.clone() }

357:    LET result = connection.unwrap().call_tool(tool_name, args).await?

359:    EMIT McpEvent::ToolResult { tool_name: tool_name.to_string(), result: result.clone() }
360:    RETURN Ok(result)

---

## ProfileService

### State

370: STRUCT ProfileService
371:  profiles: Arc<Mutex<HashMap<Uuid, ModelProfile>>>
372:  storage: Arc<ProfileRepository>
373:  event_tx: broadcast::Sender<AppEvent>

### Core Operations

380: IMPL ProfileService
381:  FUNCTION new(
382:    storage: Arc<ProfileRepository>,
383:    event_tx: broadcast::Sender<AppEvent>
384:  ) -> Self
385:    RETURN ProfileService {
386:      profiles: Arc::new(Mutex::new(HashMap::new())),
387:      storage,
388:      event_tx,
389:    }

390:  ASYNC FUNCTION add_profile(&self, profile: ModelProfile) -> Result<Uuid, ServiceError>
391:    VALIDATE profile data (name, provider, model)
392:    IF invalid THEN RETURN Err(ServiceError::InvalidInput("Invalid profile"))

394:    LET id = Uuid::new_v4()

395:    MUTEX_LOCK self.profiles
396:    self.profiles.insert(id, profile.clone())
397:    DROP mutex

398:    self.storage.save(id, &profile).await?
399:    EMIT ChatEvent::ProfileAdded { id, profile }
400:    RETURN Ok(id)

410:  FUNCTION get_profile(&self, id: Uuid) -> Option<ModelProfile>
411:    MUTEX_LOCK self.profiles
412:    LET profile = self.profiles.get(&id).cloned()
413:    DROP mutex
414:    RETURN profile

420:  FUNCTION list_profiles(&self) -> Vec<(Uuid, ModelProfile)>
421:    MUTEX_LOCK self.profiles
422:    LET profiles = self.profiles.iter()
423:      .map(|(id, p)| (*id, p.clone()))
424:      .collect()
425:    DROP mutex
426:    RETURN profiles

---

## SecretsService

### State

430: STRUCT SecretsService
431:  secrets: Arc<SecretsManager>

### Core Operations

440: IMPL SecretsService
441:  FUNCTION new(secrets: Arc<SecretsManager>) -> Self
442:    RETURN SecretsService { secrets }

445:  ASYNC FUNCTION get_api_key(&self, profile_id: Uuid) -> Result<String, ServiceError>
446:    VALIDATE profile_id is valid
447:    LET key = self.secrets.get_api_key(profile_id).await?
448:    RETURN Ok(key)

450:  ASYNC FUNCTION set_api_key(&self, profile_id: Uuid, key: String) -> Result<(), ServiceError>
451:    self.secrets.set_api_key(profile_id, key).await?
452:    RETURN Ok(())

---

## Integration Points

### Line 60-79: Conversation Creation
- **Called by**: ChatPresenter (handle UserEvent::SendMessage with new conversation)
- **Emits**: ChatEvent::ConversationStarted
- **Updates**: Metrics, in-memory HashMap

### Line 200-234: Message Streaming
- **Called by**: ChatPresenter (handle UserEvent::SendMessage)
- **Spawns**: Background task in global runtime
- **Emits**: ThinkingStarted, StreamChunk, ResponseGenerated, ThinkingEnded
- **Delegates**: LlmService for actual API call

### Line 290-319: MCP Server Start
- **Called by**: McpPresenter (handle UserEvent::StartMcpServer)
- **Spawns**: Background task for MCP process
- **Emits**: McpEvent events for lifecycle
- **Updates**: Tool registry

### Line 340-360: Tool Call
- **Called by**: AgentService during chat processing
- **Routes**: Tool to correct MCP server via registry
- **Emits**: ToolCalled, ToolResult events

---

## Concurrency Patterns

**Arc<Mutex<T>> for shared state**:
- Line 31: `conversations: Arc<Mutex<HashMap<...>>>`
- Lock duration: Minimal (only during access)
- Lock ordering: Avoid deadlock by consistent ordering

**Arc for read-only shared access**:
- Line 172: `profiles: Arc<ProfileService>` (no lock needed for service reference)
- Line 173: `llm: Arc<LlmService>` (service handles its own locking)

**Spawn tasks in runtime**:
- Line 214: Spawn background task for streaming
- Line 296: Spawn background task for MCP server
- Tasks hold Arc references to services

---

## Error Handling

500: ENUM ServiceError
501:  NotFound(String)
502:  InvalidInput(String)
503:  Initialization(String)
504:  Request(String)
505:  Timeout(Duration)
506:  Network(String)
507:  Auth(String)

---

## Anti-Pattern Warnings

[ERROR] DO NOT:
```rust
// WRONG: Long-held mutex lock
let mut lock = self.conversations.lock().unwrap;
tokio::time::sleep(Duration::from_secs(1)).await;  // Blocks other threads!
lock.insert(id, conv);
```

[OK] DO:
```rust
// RIGHT: Minimal lock scope
{
  let mut lock = self.conversations.lock().unwrap;
  lock.insert(id, conv);
}  // Lock released here
tokio::time::sleep(Duration::from_secs(1)).await;  // No lock held
```

[ERROR] DO NOT:
```rust
// WRONG: Forget to emit events
self.conversations.insert(id, conv);
return Ok(id);  // No event emitted!
```

[OK] DO:
```rust
// RIGHT: Always emit domain events
self.conversations.insert(id, conv);
self.event_tx.send(ChatEvent::ConversationStarted { id })?;
return Ok(id);
```

[ERROR] DO NOT:
```rust
// WRONG: Block on async in mutex lock
let lock = self.conversations.lock().unwrap;
let result = self.storage.save(conv).await?;  // Deadlock risk!
```

[OK] DO:
```rust
// RIGHT: Release lock before async
let conv = {
  let lock = self.conversations.lock().unwrap;
  lock.get(&id).cloned()
};  // Lock released
let result = self.storage.save(&conv?).await?;
```
