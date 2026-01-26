# PersonalAgent Canonical Data Models

This glossary defines canonical data models and ownership. All service and UI requirements must align to these definitions.

---

## Conversation (owned by ConversationService)

A persisted conversation with metadata and message history.

```rust
struct Conversation {
    id: Uuid,
    title: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    profile_id: Option<Uuid>,
    messages: Vec<Message>,
    context_state: Option<ContextState>,
}
```

---

## Message (owned by ConversationService)

A single message within a conversation.

```rust
struct Message {
    role: MessageRole, // user | assistant | system
    content: String,
    ts: DateTime<Utc>,
    thinking: Option<String>,
    tool_calls: Option<Vec<ToolCall>>,
    tool_results: Option<Vec<ToolResult>>,
}
```

---

## ContextState (owned by ConversationService)

Cached summary describing the compressed middle section of a conversation. Produced by ContextStrategy, persisted by ConversationService.

```rust
struct ContextState {
    strategy: String,
    summary: String,
    summary_range: (usize, usize),
    compressed_at: DateTime<Utc>,
}
```

---

## ModelProfile (owned by ProfileService)

User-configured profile describing which model to call and how.

```rust
struct ModelProfile {
    id: Uuid,
    name: String,
    provider_id: String,
    model_id: String,
    base_url: Option<String>,
    api_key: Option<String>,
    keyfile_path: Option<PathBuf>,
    system_prompt: String,
    parameters: ModelParameters,
}
```

---

## AuthMethod (owned by ProfileService)

Represents how a profile authenticates. Derived from ModelProfile fields and used by UI state.

```rust
enum AuthMethod {
    None,
    ApiKey,
    KeyFile,
}
```

---

## ModelParameters (owned by ProfileService)

Configurable parameters attached to a ModelProfile.

```rust
struct ModelParameters {
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    top_p: Option<f64>,
    thinking_budget: Option<u32>,
    enable_thinking: bool,
    show_thinking: bool,
}
```

---

## McpConfig (owned by McpService)

Configuration for a single MCP server.

```rust
struct McpConfig {
    id: Uuid,
    name: String,
    enabled: bool,
    source: McpSource,
    package: McpPackage,
    transport: McpTransport,
    env_vars: HashMap<String, EnvVarSpec>,
    oauth_tokens: Option<OAuthTokens>,
    config: serde_json::Value,
}
```

---

## McpSource (owned by McpService)

Origin of an MCP definition.

```rust
enum McpSource {
    Official { name: String, version: String },
    Smithery { qualified_name: String },
    Manual,
}
```

---

## AuthMethod (owned by McpService)

How an MCP server authenticates with external services.

```rust
enum AuthMethod {
    None,
    ApiKey,
    OAuth,
}
```

---

## EnvVarSpec (owned by McpService)

Required/optional environment variable specification for MCP config.

```rust
struct EnvVarSpec {
    name: String,
    required: bool,
    secret: bool,
    description: Option<String>,
}
```

---

## OAuthTokens (owned by McpService)

Stored OAuth tokens for an MCP server.

```rust
struct OAuthTokens {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<DateTime<Utc>>,
    scope: Option<String>,
    token_type: Option<String>,
}
```

---

## ModelInfo (owned by ModelsRegistryService)

Normalized model metadata returned to UI and profile creation flows.

```rust
struct ModelInfo {
    provider_id: String,
    model_id: String,
    display_name: String,
    description: Option<String>,
    capabilities: ModelCapabilities,
    limits: ModelLimits,
    cost: Option<ModelCost>,
}
```

---

## Provider (owned by ModelsRegistryService)

Provider and its associated models.

```rust
struct Provider {
    id: String,
    name: String,
    api_url: String,
    models: Vec<ModelInfo>,
}
```

---

## ServiceError (standard error shape)

All services return a consistent error shape.

```rust
struct ServiceError {
    code: String,
    message: String,
    field: Option<String>,
}
```
