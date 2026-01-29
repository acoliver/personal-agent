# Full Theoretical Stack Trace: First-Time User Scenario

This document traces every screen, event, method call, service function, I/O operation, SerdesAI API call, state change, and event consumption through a complete user scenario - from first launch through profile creation, chat, MCP configuration, tool use, and stream cancellation.

---

## Table of Contents

1. [Scenario Overview](#scenario-overview)
2. [Phase 1: App Launch (Empty State)](#phase-1-app-launch-empty-state)
3. [Phase 2: Navigate to Configure](#phase-2-navigate-to-configure)
4. [Phase 3: Add Profile → Model Selector](#phase-3-add-profile--model-selector)
5. [Phase 4: Select Model & Configure Profile](#phase-4-select-model--configure-profile)
6. [Phase 5: Return to Chat & Send Message](#phase-5-return-to-chat--send-message)
7. [Phase 6: Toggle Thinking Off](#phase-6-toggle-thinking-off)
8. [Phase 7: Add MCP (Exa Search)](#phase-7-add-mcp-exa-search)
9. [Phase 8: Use MCP in Conversation](#phase-8-use-mcp-in-conversation)
10. [Phase 9: Toggle Thinking On](#phase-9-toggle-thinking-on)
11. [Phase 10: New Conversation with Cancel](#phase-10-new-conversation-with-cancel)
12. [Event Bounce Analysis](#event-bounce-analysis)
13. [State Management Analysis](#state-management-analysis)
14. [Gap Analysis](#gap-analysis)

---

## Scenario Overview

**User Journey:**
1. First app launch - no data, no settings
2. See blank chat screen, click Configure
3. Add a profile: glm-4.7 on Synthetic (OpenAI-compatible)
4. Select keyfile auth: `~/.synthetic_key`
5. Enable "show thinking" and "do thinking"
6. Return to chat, type "write me a haiku"
7. Turn off "show thinking"
8. Go to Configure → MCPs → Add → search "exa" → select ExaSearch
9. Accept defaults (no key required), return to chat
10. Same conversation: "search on haikus and give me the history of the form"
11. Turn "show thinking" back on
12. Click [+] for new conversation
13. Type "search for all recent llm research published in 2026"
14. Get tired of waiting, click [] cancel

---

## Phase 1: App Launch (Empty State)

### Startup Sequence

| Step | Component | Action | I/O |
|------|-----------|--------|-----|
| 1 | macOS | Launches app | - |
| 2 | `main()` | Initialize tokio runtime | - |
| 3 | `AppDelegate` | `applicationDidFinishLaunching` called | - |
| 4 | `AppDelegate` | Initialize logging (tracing) | File: logs/ |
| 5 | `AppDelegate` | Create `EventBus::new(256)` | Memory |
| 6 | `AppDelegate` | Create `ServiceContainer` | - |

### Service Initialization

| Step | Service | Method | I/O | Result |
|------|---------|--------|-----|--------|
| 7 | `SecretsService` | `new()` | Keychain: derive encryption key | Ok |
| 8 | `AppSettingsService` | `new()` | Read: `~/Library/.../settings.json` | **NotFound** → create defaults |
| 9 | `AppSettingsService` | `save()` | Write: `~/Library/.../settings.json` | `{ default_profile_id: null, current_conversation_id: null }` |
| 10 | `ProfileService` | `new(secrets)` | List: `~/Library/.../profiles/` | Empty dir |
| 11 | `ConversationService` | `new()` | List: `~/Library/.../conversations/` | Empty dir |
| 12 | `McpRegistryService` | `new()` | - | HTTP client ready |
| 13 | `McpService` | `new(secrets)` | List: `~/Library/.../mcps/` | Empty dir |
| 14 | `ChatService` | `new(profiles, conversations, mcp)` | - | Ready |

### Events Emitted During Startup

| Event | Payload | Emitter |
|-------|---------|---------|
| `SystemEvent::AppLaunched` | - | AppDelegate |
| `SystemEvent::ConfigLoaded` | - | AppSettingsService |

### UI Setup

| Step | Component | Action |
|------|-----------|--------|
| 15 | `StatusBarController` | Create `NSStatusItem` |
| 16 | `StatusBarController` | Set menubar icon (template) |
| 17 | `StatusBarController` | Create `NSPopover` (400x500) |
| 18 | `ViewRouter` | Initialize with ServiceContainer |
| 19 | `ViewRouter` | Push `ViewType::Chat` as root |
| 20 | `ChatPresenter` | `start()` - subscribe to events |
| 21 | `ChatPresenter` | Check `app_settings.get_default_profile_id()` → **None** |

### State After Phase 1

```
AppState:
  default_profile_id: None
  current_conversation_id: None
  
ChatView State:
  conversation_id: None
  is_streaming: false
  show_thinking: false (no profile to init from)
  
View Stack: [Chat]
```

### Screen Display

**Chat View (Empty State):**
```
┌──────────────────────────────────────────────────────────────┐
│ [Icon] PersonalAgent              [T] [S] [H] [+] []       │
├──────────────────────────────────────────────────────────────┤
│ ┌─────────────────────────────┐                              │
│ │ (no conversations)        ▼ │  (no model)                  │
│ └─────────────────────────────┘                              │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│              No profile configured.                          │
│              Click  to add one.                            │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ [Input field - disabled]                      [Send-disabled]│
└──────────────────────────────────────────────────────────────┘
```

---

## Phase 2: Navigate to Configure

### User Action: Click  (Settings Button)

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | ChatView | `on_settings_click()` | User clicks gear icon |
| 2 | ChatView | `event_bus.emit()` | `UserEvent::Navigate { to: ViewId::Settings }` |
| 3 | EventBus | `broadcast()` | All subscribers receive |
| 4 | ChatPresenter | receives but ignores (not subscribed to Navigate) | - |
| 5 | ViewRouter | `handle(UserEvent::Navigate)` | Handles navigation |
| 6 | ViewRouter | `push(ViewType::Settings)` | Add to stack |
| 7 | ViewRouter | `create_view(Settings)` | Instantiate SettingsView |
| 8 | SettingsPresenter | `start()` | Subscribe to events |
| 9 | SettingsPresenter | `load_data()` | Load profiles and MCPs |
| 10 | ProfileService | `list()` | Read: `profiles/` → **Empty** |
| 11 | McpService | `list()` | Read: `mcps/` → **Empty** |
| 12 | McpService | `all_status()` | → **Empty** |
| 13 | ViewRouter | emit | `NavigationEvent::Navigated { view: Settings }` |
| 14 | SettingsView | `render(profiles: [], mcps: [])` | Show empty state |

### Events Flow

```
UserEvent::Navigate { to: Settings }
    │
    ├──▶ ViewRouter handles → push Settings
    │
    └──▶ NavigationEvent::Navigated { view: Settings }
             │
             └──▶ SettingsPresenter receives → calls view.did_appear()
```

### State After Phase 2

```
View Stack: [Chat, Settings]

SettingsView State:
  profiles: []
  mcps: []
  selected_profile_id: None
```

### Screen Display

**Settings View (Empty State):**
```
┌──────────────────────────────────────────────────────────────┐
│ [<Back]              Settings                                │
├──────────────────────────────────────────────────────────────┤
│ PROFILES                                                     │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │                                                          │ │
│ │     No profiles configured.                              │ │
│ │                                                          │ │
│ └──────────────────────────────────────────────────────────┘ │
│                                         [+ Add Profile]      │
├──────────────────────────────────────────────────────────────┤
│ MCPs                                                         │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │                                                          │ │
│ │     No MCPs configured.                                  │ │
│ │                                                          │ │
│ └──────────────────────────────────────────────────────────┘ │
│                                              [+ Add MCP]     │
├──────────────────────────────────────────────────────────────┤
│ GLOBAL HOTKEY                                                │
│ [Cmd+Shift+Space                                         ]   │
└──────────────────────────────────────────────────────────────┘
```

---

## Phase 3: Add Profile → Model Selector

### User Action: Click "+ Add Profile"

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | SettingsView | `on_add_profile_click()` | Button clicked |
| 2 | SettingsView | `event_bus.emit()` | `UserEvent::CreateProfile` |
| 3 | SettingsPresenter | handles | Decides to navigate to ModelSelector |
| 4 | SettingsPresenter | `event_bus.emit()` | `UserEvent::Navigate { to: ViewId::ModelSelector }` |
| 5 | ViewRouter | handles | `push(ViewType::ModelSelector { return_to: Settings, context: NewProfile })` |
| 6 | ModelSelectorView | instantiated | - |
| 7 | ModelSelectorPresenter | `start()` | Subscribe to events |
| 8 | ModelSelectorPresenter | `load_models()` | Trigger model fetch |
| 9 | ModelsRegistryService | `get_models()` | Check cache |
| 10 | ModelsRegistryService | - | Cache miss, need to fetch |
| 11 | ModelsRegistryService | HTTP GET | `https://models.dev/api.json` |
| 12 | ModelsRegistryService | `save_cache()` | Write: `cache/models.json` |
| 13 | ModelsRegistryService | emit | `SystemEvent::ModelsRegistryRefreshed { provider_count: 50, model_count: 500 }` |
| 14 | ModelSelectorPresenter | receives | Update view with models |
| 15 | ModelSelectorView | `render(providers, models)` | Display list |

### Network I/O

```
HTTP GET https://models.dev/api.json
Response: 200 OK
Body: {
  "providers": [
    {
      "id": "openai",
      "name": "OpenAI",
      "api": "https://api.openai.com/v1",
      "models": [...]
    },
    {
      "id": "synthetic",
      "name": "Synthetic",
      "api": "https://api.synthetic.com/v1",
      "api_type": "openai",  // OpenAI-compatible
      "models": [
        {
          "id": "glm-4.7",
          "name": "GLM 4.7",
          "context_length": 128000,
          "tool_call": true,
          "reasoning": true,
          "cost": { "input": 0.5, "output": 1.5 }
        },
        ...
      ]
    },
    ...
  ]
}
```

### Events Flow

```
UserEvent::CreateProfile
    │
    └──▶ SettingsPresenter handles
             │
             └──▶ UserEvent::Navigate { to: ModelSelector }
                      │
                      └──▶ ViewRouter handles → push ModelSelector
                               │
                               └──▶ ModelsRegistryService fetches
                                        │
                                        └──▶ SystemEvent::ModelsRegistryRefreshed
```

### State After Phase 3

```
View Stack: [Chat, Settings, ModelSelector]

ModelSelectorView State:
  providers: [...50 providers...]
  models: [...500 models...]
  filtered_models: [...500 models...]
  search_query: ""
  selected_provider: None
  selected_model: None
```

### Screen Display

**Model Selector View:**
```
┌──────────────────────────────────────────────────────────────┐
│ [Cancel]            Select Model                    [Next]   │
├──────────────────────────────────────────────────────────────┤
│ SEARCH                                                       │
│ [Search models...                                         ]  │
│                                                              │
│ PROVIDER                                                     │
│ [All Providers                                           ▼]  │
│                                                              │
│ CAPABILITIES         [[OK] Tools] [[OK] Reasoning] [ ] Vision     │
├──────────────────────────────────────────────────────────────┤
│ MODELS                                                       │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │ Provider    Model           Context    Cost              │ │
│ │ ──────────────────────────────────────────────────────── │ │
│ │ Anthropic   claude-sonnet-4  200K      $3/$15           │ │
│ │ OpenAI      gpt-4o           128K      $5/$15           │ │
│ │ Synthetic   glm-4.7          128K      $0.5/$1.5        │ │
│ │ ...                                                      │ │
│ └──────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

---

## Phase 4: Select Model & Configure Profile

### User Action: Select GLM 4.7 from Synthetic

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | ModelSelectorView | `on_row_click(provider: "synthetic", model: "glm-4.7")` | Row selected |
| 2 | ModelSelectorView | update local state | `selected_model = Some(...)` |
| 3 | ModelSelectorView | re-render | Highlight row, enable Next |

### User Action: Click "Next"

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | ModelSelectorView | `on_next_click()` | Button clicked |
| 2 | ModelSelectorView | `event_bus.emit()` | `UserEvent::SelectModel { provider_id: "synthetic", model_id: "glm-4.7" }` |
| 3 | ModelSelectorPresenter | handles | Prepare selected model data |
| 4 | ViewRouter | `replace_top(ProfileEditor { from_model_selector: Some(SelectedModel { ... }) })` | Navigate to editor |
| 5 | ProfileEditorView | instantiated with pre-filled data | - |
| 6 | ProfileEditorPresenter | `start()` | Subscribe to events |
| 7 | ProfileEditorPresenter | `init_from_selection()` | Pre-populate fields |

### Pre-populated Data (from models.dev + selection)

```rust
SelectedModel {
    provider_id: "synthetic",
    provider_name: "Synthetic",
    model_id: "glm-4.7",
    model_name: "GLM 4.7",
    api_type: ApiType::OpenAI,  // from models.dev "api_type" field
    base_url: "https://api.synthetic.com/v1",  // from models.dev "api" field
    context_limit: 128000,  // from models.dev "context_length"
    supports_tools: true,
    supports_reasoning: true,
}
```

### Screen Display: Profile Editor (Pre-filled)

```
┌──────────────────────────────────────────────────────────────┐
│ [Cancel]           New Profile                     [Save]    │
├──────────────────────────────────────────────────────────────┤
│ NAME                                                         │
│ [Synthetic GLM 4.7                                        ]  │
│                                                              │
│ MODEL                                                        │
│ [glm-4.7 (Synthetic)                           ] [Change]    │
│                                                              │
│ API TYPE                                                     │
│ [OpenAI Compatible                                       ▼]  │
│                                                              │
│ BASE URL                                                     │
│ [https://api.synthetic.com/v1                             ]  │
│                                                              │
│ AUTHENTICATION                                               │
│ ( ) None   ( ) API Key   (●) Keyfile                        │
│                                                              │
│ KEYFILE PATH                                                 │
│ [~/.synthetic_key                                         ]  │
├──────────────────────────────────────────────────────────────┤
│ PARAMETERS                                                   │
│                                                              │
│ Temperature    [0.7     ]  Max Tokens    [4096    ]         │
│ Context Limit  [128000  ]  (from model)                      │
│                                                              │
│ [[OK]] Enable Thinking      Budget: [10000  ] tokens           │
│ [[OK]] Show Thinking                                           │
├──────────────────────────────────────────────────────────────┤
│ SYSTEM PROMPT                                                │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │ You are a helpful assistant.                             │ │
│ │                                                          │ │
│ └──────────────────────────────────────────────────────────┘ │
│                                              [Test Connection]│
└──────────────────────────────────────────────────────────────┘
```

### User Actions in Profile Editor

1. **Select Keyfile auth**: Click "Keyfile" radio button
2. **Enter keyfile path**: Type `~/.synthetic_key`
3. **Enable thinking**: Check "Enable Thinking" (already checked if model supports)
4. **Enable show thinking**: Check "Show Thinking"

### User Action: Click "Save"

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | ProfileEditorView | `on_save_click()` | Button clicked |
| 2 | ProfileEditorView | `collect_form_data()` | Build ProfileData |
| 3 | ProfileEditorView | `event_bus.emit()` | `UserEvent::SaveProfile { profile: ProfileData }` |
| 4 | ProfileEditorPresenter | handles | Validate and save |
| 5 | ProfileEditorPresenter | validate | Check required fields |
| 6 | ProfileService | `create(profile_data)` | Create new profile |
| 7 | ProfileService | `generate_uuid()` | New UUID |
| 8 | ProfileService | `save_to_file()` | Write: `profiles/{uuid}.json` |
| 9 | ProfileService | emit | `ProfileEvent::Created { id, name: "Synthetic GLM 4.7" }` |
| 10 | AppSettingsService | `set_default_profile_id(id)` | First profile becomes default |
| 11 | AppSettingsService | `save()` | Write: `settings.json` |
| 12 | AppSettingsService | emit | `ProfileEvent::DefaultChanged { profile_id: Some(id) }` |
| 13 | ViewRouter | `pop()` then `pop()` | Return to Settings then to Chat |
| 14 | NavigationEvent::Navigated | `{ view: Chat }` | - |

### File I/O: Profile Created

**Write: `~/Library/.../profiles/a1b2c3d4-....json`**
```json
{
  "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "name": "Synthetic GLM 4.7",
  "model_id": "glm-4.7",
  "api_type": "openai",
  "base_url": "https://api.synthetic.com/v1",
  "auth_method": {
    "type": "keyfile",
    "path": "~/.synthetic_key"
  },
  "parameters": {
    "temperature": 0.7,
    "max_tokens": 4096,
    "context_limit": 128000,
    "enable_thinking": true,
    "thinking_budget": 10000,
    "show_thinking": true
  },
  "system_prompt": "You are a helpful assistant.",
  "created_at": "2026-01-25T00:40:00Z",
  "updated_at": "2026-01-25T00:40:00Z"
}
```

**Write: `~/Library/.../settings.json`**
```json
{
  "default_profile_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "current_conversation_id": null,
  "hotkey": {
    "enabled": true,
    "modifiers": ["command", "shift"],
    "key": "Space"
  },
  "window_state": null
}
```

### Events Flow (Save Profile)

```
UserEvent::SaveProfile { profile }
    │
    └──▶ ProfileEditorPresenter handles
             │
             ├──▶ ProfileService.create()
             │        │
             │        └──▶ ProfileEvent::Created { id, name }
             │
             └──▶ AppSettingsService.set_default_profile_id()
                      │
                      └──▶ ProfileEvent::DefaultChanged { profile_id: Some(id) }

NavigationEvent::Navigated { view: Chat }
    │
    └──▶ ChatPresenter receives
             │
             └──▶ Reloads with new default profile
```

### State After Phase 4

```
AppState:
  default_profile_id: Some("a1b2c3d4-...")
  current_conversation_id: None

ChatView State:
  conversation_id: None
  is_streaming: false
  show_thinking: true  ← initialized from profile.parameters.show_thinking
  current_profile: Some(Profile { ... })
  
View Stack: [Chat]
```

---

## Phase 5: Return to Chat & Send Message

### Chat View Reloaded with Profile

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | ChatPresenter | receives `ProfileEvent::DefaultChanged` | Profile changed |
| 2 | ChatPresenter | `reload_profile()` | Load new default profile |
| 3 | ProfileService | `get(profile_id)` | Read profile |
| 4 | ChatPresenter | `update_view()` | Update UI state |
| 5 | ChatView | `set_show_thinking(true)` | From profile.parameters.show_thinking |
| 6 | ChatView | `enable_input()` | Now have a profile |
| 7 | ChatView | `set_model_label("glm-4.7")` | Show current model |

### Screen Display: Chat View (Ready)

```
┌──────────────────────────────────────────────────────────────┐
│ [Icon] PersonalAgent              [T[OK]] [S] [H] [+] []      │
├──────────────────────────────────────────────────────────────┤
│ ┌─────────────────────────────┐                              │
│ │ (no conversations)        ▼ │  glm-4.7                     │
│ └─────────────────────────────┘                              │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│              Start a conversation!                           │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ [Type a message...                         ]  [Send]         │
└──────────────────────────────────────────────────────────────┘
```

### User Action: Type "write me a haiku" and press Enter

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | ChatView | `on_input_change("write me a haiku")` | Text entered |
| 2 | ChatView | `on_enter_pressed()` | Enter key |
| 3 | ChatView | `event_bus.emit()` | `UserEvent::SendMessage { text: "write me a haiku" }` |
| 4 | ChatPresenter | handles `UserEvent::SendMessage` | - |
| 5 | ChatPresenter | `view.clear_input()` | Clear input field |
| 6 | ChatPresenter | `view.add_user_bubble("write me a haiku")` | Show user message |
| 7 | ChatPresenter | `view.show_assistant_loading()` | Show cursor ▌ |
| 8 | ChatPresenter | check conversation | `current_conversation_id == None` |
| 9 | ConversationService | `create()` | Create new conversation |
| 10 | ConversationService | generate timestamp | `20260125004000001` |
| 11 | ConversationService | `save_meta()` | Write: `20260125004000001.meta.json` |
| 12 | ConversationService | emit | `ConversationEvent::Created { id, title }` |
| 13 | AppSettingsService | `set_current_conversation_id(id)` | Set as current |
| 14 | AppSettingsService | emit | `ConversationEvent::Activated { id }` |
| 15 | ChatPresenter | persist user message | - |
| 16 | ConversationService | `append_message(user_msg)` | Write: append to `.jsonl` |
| 17 | ConversationService | emit | `ChatEvent::MessageSaved { ... }` |
| 18 | ChatPresenter | `chat_service.send_message(...)` | Start streaming |

### Conversation Files Created

**Write: `~/Library/.../conversations/20260125004000001.meta.json`**
```json
{
  "id": "b2c3d4e5-f6a7-8901-bcde-f23456789012",
  "title": "New 2026-01-25 00:40",
  "created_at": "2026-01-25T00:40:00Z",
  "updated_at": "2026-01-25T00:40:00Z",
  "message_count": 0
}
```

**Append: `~/Library/.../conversations/20260125004000001.jsonl`**
```json
{"id":"c3d4e5f6-...","role":"user","content":"write me a haiku","model_id":null,"timestamp":"2026-01-25T00:40:00Z"}
```

### ChatService.send_message() - SerdesAI Integration

| Step | Component | Method | Details |
|------|-----------|--------|--------|
| 1 | ChatService | `send_message(conversation_id, text)` | Entry point |
| 2 | ChatService | `get_profile_config()` | Load resolved profile |
| 3 | ProfileService | `get_model_config(profile_id)` | Get ResolvedModelConfig |
| 4 | ProfileService | `resolve_api_key()` | Read keyfile |
| 5 | SecretsService | `read_keyfile("~/.synthetic_key")` | File I/O |
| 6 | ProfileService | return | `ResolvedModelConfig { api_key: "sk-synth-...", ... }` |
| 7 | ChatService | `load_conversation_history()` | Get messages |
| 8 | ConversationService | `get_messages(conversation_id)` | Read .jsonl |
| 9 | ChatService | `apply_context_compression()` | Check token count |
| 10 | ChatService | `build_agent()` | Create SerdesAI Agent |
| 11 | ChatService | `get_toolsets()` | From McpService |
| 12 | McpService | `get_toolsets()` | Returns [] (no MCPs yet) |
| 13 | ChatService | emit | `ChatEvent::StreamStarted { model_id: "glm-4.7", ... }` |
| 14 | ChatService | `agent.stream()` | Start SerdesAI streaming |

### Keyfile Read I/O

```
Read: ~/.synthetic_key
Content: "sk-synth-abc123xyz789..."
```

### SerdesAI Agent Creation

```rust
// Build the client based on api_type
let client = match config.api_type {
    ApiType::OpenAI => {
        OpenAIClient::new()
            .with_base_url(&config.base_url)  // "https://api.synthetic.com/v1"
            .with_api_key(&config.api_key)    // from keyfile
            .build()
    }
    ApiType::Anthropic => { ... }
};

// Build the agent
let agent = Agent::new(client)
    .with_model(&config.model_id)        // "glm-4.7"
    .with_system_prompt(&config.system_prompt)
    .with_temperature(config.parameters.temperature)
    .with_max_tokens(config.parameters.max_tokens);

// Add thinking if enabled
let agent = if config.parameters.enable_thinking {
    agent.with_extended_thinking(config.parameters.thinking_budget)
} else {
    agent
};

// Add history processor for context management
let agent = agent.with_history_processor(
    TruncateByTokens::new(config.parameters.context_limit)
);

// No toolsets yet - MCPs not configured
// agent.with_toolsets(toolsets)

// Start streaming
let stream = agent.stream(&messages).await?;
```

### Network I/O: LLM API Call

```
POST https://api.synthetic.com/v1/chat/completions
Headers:
  Authorization: Bearer sk-synth-abc123xyz789...
  Content-Type: application/json

Body:
{
  "model": "glm-4.7",
  "messages": [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "write me a haiku"}
  ],
  "temperature": 0.7,
  "max_tokens": 4096,
  "stream": true,
  "extended_thinking": {
    "enabled": true,
    "budget_tokens": 10000
  }
}

Response: SSE stream
event: thinking
data: {"content": "The user wants a haiku. A haiku has 5-7-5 syllables..."}

event: thinking
data: {"content": " Let me compose one about nature..."}

event: message
data: {"content": "Autumn"}

event: message  
data: {"content": " leaves falling"}

event: message
data: {"content": "\nSoft whispers in the breeze\nNature's quiet poem"}

event: done
data: {"usage": {"total_tokens": 150}}
```

### Streaming Events

| Event | Payload | UI Update |
|-------|---------|-----------|
| `ChatEvent::StreamStarted` | `{ model_id: "glm-4.7" }` | Add model label, show cursor |
| `ChatEvent::ThinkingDelta` | `"The user wants..."` | Append to thinking section |
| `ChatEvent::ThinkingDelta` | `" Let me compose..."` | Append to thinking section |
| `ChatEvent::TextDelta` | `"Autumn"` | Append to assistant bubble |
| `ChatEvent::TextDelta` | `" leaves falling"` | Append |
| `ChatEvent::TextDelta` | `"\nSoft whispers..."` | Append |
| `ChatEvent::StreamCompleted` | `{ total_tokens: 150 }` | Remove cursor, finalize |

### Event Consumption During Streaming

```
ChatEvent::StreamStarted
    │
    ├──▶ ChatPresenter receives
    │        │
    │        ├──▶ view.add_model_label("glm-4.7")
    │        └──▶ view.add_assistant_bubble_with_cursor()
    │
    └──▶ (other presenters ignore - not subscribed)

ChatEvent::ThinkingDelta { text: "..." }
    │
    └──▶ ChatPresenter receives
             │
             ├──▶ (show_thinking == true)
             └──▶ view.append_thinking(text)

ChatEvent::TextDelta { text: "..." }
    │
    └──▶ ChatPresenter receives
             │
             └──▶ view.append_to_assistant_bubble(text)

ChatEvent::StreamCompleted
    │
    └──▶ ChatPresenter receives
             │
             ├──▶ view.remove_cursor()
             ├──▶ view.set_streaming(false)
             └──▶ persist assistant message
```

### Persist Assistant Message

| Step | Component | Method | Details |
|------|-----------|--------|--------|
| 1 | ChatPresenter | `persist_assistant_message()` | After stream complete |
| 2 | ConversationService | `append_message(assistant_msg)` | - |
| 3 | ConversationService | append to file | Write: append to `.jsonl` |
| 4 | ConversationService | `update_meta()` | Update message count |

**Append: `~/Library/.../conversations/20260125004000001.jsonl`**
```json
{"id":"d4e5f6a7-...","role":"assistant","content":"Autumn leaves falling\nSoft whispers in the breeze\nNature's quiet poem","thinking":"The user wants a haiku. A haiku has 5-7-5 syllables... Let me compose one about nature...","model_id":"glm-4.7","timestamp":"2026-01-25T00:40:05Z"}
```

### Screen Display: After Response

```
┌──────────────────────────────────────────────────────────────┐
│ [Icon] PersonalAgent              [T[OK]] [S] [H] [+] []      │
├──────────────────────────────────────────────────────────────┤
│ ┌─────────────────────────────┐                              │
│ │ New 2026-01-25 00:40      ▼ │  glm-4.7                     │
│ └─────────────────────────────┘                              │
├──────────────────────────────────────────────────────────────┤
│                        ┌──────────────────────────────┐      │
│                        │ write me a haiku             │      │
│                        └──────────────────────────────┘      │
│                                                              │
│  glm-4.7                                                     │
│  ┌──────────────────────────────┐                            │
│  │ ▼ Thinking...                │                            │
│  │ ┌──────────────────────────┐ │                            │
│  │ │ The user wants a haiku.  │ │                            │
│  │ │ A haiku has 5-7-5...     │ │                            │
│  │ └──────────────────────────┘ │                            │
│  └──────────────────────────────┘                            │
│  ┌──────────────────────────────┐                            │
│  │ Autumn leaves falling        │                            │
│  │ Soft whispers in the breeze  │                            │
│  │ Nature's quiet poem          │                            │
│  └──────────────────────────────┘                            │
├──────────────────────────────────────────────────────────────┤
│ [Type a message...                         ]  [Send]         │
└──────────────────────────────────────────────────────────────┘
```

### State After Phase 5

```
AppState:
  default_profile_id: Some("a1b2c3d4-...")
  current_conversation_id: Some("b2c3d4e5-...")

ChatView State:
  conversation_id: Some("b2c3d4e5-...")
  is_streaming: false
  show_thinking: true
  messages: [user_msg, assistant_msg]
```

---

## Phase 6: Toggle Thinking Off

### User Action: Click [T] Button

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | ChatView | `on_thinking_toggle_click()` | Button clicked |
| 2 | ChatView | `event_bus.emit()` | `UserEvent::ToggleThinking` |
| 3 | ChatPresenter | handles | Toggle local state |
| 4 | ChatPresenter | `self.show_thinking = !self.show_thinking` | `true → false` |
| 5 | ChatPresenter | `view.set_show_thinking(false)` | Update button state |
| 6 | ChatView | `update_thinking_visibility()` | Hide all thinking sections |
| 7 | ChatView | `thinking_button.set_active(false)` | Remove blue highlight |

### Important: No Persistence

- The toggle is **transient** - NOT saved to profile or conversation
- Thinking content remains stored in messages (just hidden)
- Toggle state only changes on:
  - Profile change (reset to new profile's default)
  - App restart (reset to default profile's setting)
  - NOT on conversation switch

### Events Flow

```
UserEvent::ToggleThinking
    │
    └──▶ ChatPresenter handles
             │
             ├──▶ self.show_thinking = false (local state)
             └──▶ view.set_show_thinking(false)
                      │
                      └──▶ Hide thinking sections (visual only)
```

### Screen Display: Thinking Hidden

```
┌──────────────────────────────────────────────────────────────┐
│ [Icon] PersonalAgent              [T] [S] [H] [+] []       │
│                                    ↑ no highlight            │
├──────────────────────────────────────────────────────────────┤
│ ┌─────────────────────────────┐                              │
│ │ New 2026-01-25 00:40      ▼ │  glm-4.7                     │
│ └─────────────────────────────┘                              │
├──────────────────────────────────────────────────────────────┤
│                        ┌──────────────────────────────┐      │
│                        │ write me a haiku             │      │
│                        └──────────────────────────────┘      │
│                                                              │
│  glm-4.7                                                     │
│  ┌──────────────────────────────┐                            │
│  │ Autumn leaves falling        │  ← Thinking section hidden │
│  │ Soft whispers in the breeze  │                            │
│  │ Nature's quiet poem          │                            │
│  └──────────────────────────────┘                            │
├──────────────────────────────────────────────────────────────┤
│ [Type a message...                         ]  [Send]         │
└──────────────────────────────────────────────────────────────┘
```

### State After Phase 6

```
ChatView State:
  show_thinking: false  ← changed
  (everything else unchanged)
```

---

## Phase 7: Add MCP (Exa Search)

### User Action: Click  → Settings

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | ChatView | `on_settings_click()` | Gear clicked |
| 2 | ChatView | emit | `UserEvent::Navigate { to: Settings }` |
| 3 | ViewRouter | push | `ViewType::Settings` |
| 4 | SettingsPresenter | `load_data()` | Refresh |
| 5 | ProfileService | `list()` | Returns 1 profile |
| 6 | McpService | `list()` | Returns [] |

### User Action: Click "+ Add MCP"

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | SettingsView | `on_add_mcp_click()` | Button clicked |
| 2 | SettingsView | emit | `UserEvent::AddMcp` |
| 3 | SettingsPresenter | handles | Navigate to McpAdd |
| 4 | ViewRouter | push | `ViewType::McpAdd` |
| 5 | McpAddPresenter | `start()` | Subscribe |
| 6 | McpAddView | render | Show empty search |

### Screen Display: MCP Add View

```
┌──────────────────────────────────────────────────────────────┐
│ [Cancel]               Add MCP                      [Next]   │
├──────────────────────────────────────────────────────────────┤
│ MANUAL ENTRY                                                 │
│ [npx @scope/package or docker or URL                     ]   │
│                                                              │
│ ─────────────── or search registry ───────────────           │
│                                                              │
│ REGISTRY                                                     │
│ [Both                                                    ▼]  │
│                                                              │
│ SEARCH                                                       │
│ [                                                         ]  │
│                                                              │
│ RESULTS                                                      │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │                                                          │ │
│ │              Search for MCPs above                       │ │
│ │                                                          │ │
│ └──────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

### User Action: Type "exa" in Search Field

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | McpAddView | `on_search_change("exa")` | Text changed |
| 2 | McpAddView | start debounce timer | 500ms |
| 3 | (500ms passes) | timer fires | - |
| 4 | McpAddView | emit | `UserEvent::SearchMcpRegistry { query: "exa", source: Both }` |
| 5 | McpAddPresenter | handles | Call registry service |
| 6 | McpAddPresenter | `view.show_loading()` | Show spinner |
| 7 | McpRegistryService | `search("exa", Both)` | Search both registries |
| 8 | McpRegistryService | HTTP GET | Official registry |
| 9 | McpRegistryService | HTTP GET | Smithery registry |
| 10 | McpRegistryService | merge & dedupe | Combine results |
| 11 | McpRegistryService | return | `Vec<McpSearchResult>` |
| 12 | McpAddPresenter | `view.show_results(results)` | Display list |

### Network I/O: Registry Search

```
GET https://registry.modelcontextprotocol.io/api/search?q=exa
Response: 200 OK
{
  "results": [
    {
      "name": "exa-search",
      "display_name": "Exa Search",
      "description": "Web search using Exa AI",
      "package": "npx -y @anthropic/exa-mcp",
      "env_vars": []  // No auth required for basic usage
    }
  ]
}

GET https://registry.smithery.ai/api/v1/search?q=exa
Response: 200 OK
{
  "servers": [
    {
      "qualifiedName": "@anthropic/exa-search",
      "displayName": "Exa Search (Smithery)",
      "description": "Exa AI search MCP",
      "runCommand": "npx -y @anthropic/exa-mcp"
    }
  ]
}
```

### Events Flow

```
UserEvent::SearchMcpRegistry { query: "exa", source: Both }
    │
    └──▶ McpAddPresenter handles
             │
             ├──▶ McpRegistryService.search()
             │        │
             │        ├──▶ HTTP GET (Official)
             │        └──▶ HTTP GET (Smithery)
             │
             └──▶ view.show_results([...])
```

### User Action: Select "Exa Search" from Results

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | McpAddView | `on_result_click(index: 0)` | Row clicked |
| 2 | McpAddView | `self.selected_index = Some(0)` | Local state |
| 3 | McpAddView | `highlight_row(0)` | Visual update |
| 4 | McpAddView | `enable_next_button()` | Enable Next |

### User Action: Click "Next"

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | McpAddView | `on_next_click()` | Button clicked |
| 2 | McpAddView | emit | `UserEvent::SelectMcpFromRegistry { source: Official { name: "exa-search" } }` |
| 3 | McpAddPresenter | handles | Prepare selected data |
| 4 | ViewRouter | replace_top | `ViewType::McpConfigure { from_mcp_add: Some(SelectedMcp) }` |
| 5 | McpConfigureView | instantiated | Pre-filled from selection |
| 6 | McpConfigurePresenter | `start()` | Subscribe |

### Screen Display: MCP Configure View (Pre-filled)

```
┌──────────────────────────────────────────────────────────────┐
│ [Cancel]           Configure MCP                   [Save]    │
├──────────────────────────────────────────────────────────────┤
│ NAME                                                         │
│ [Exa Search                                               ]  │
│                                                              │
│ SOURCE                                                       │
│ Official Registry: exa-search                                │
│                                                              │
│ COMMAND                                                      │
│ [npx -y @anthropic/exa-mcp                                ]  │
│ (read-only, from registry)                                   │
├──────────────────────────────────────────────────────────────┤
│ AUTHENTICATION                                               │
│                                                              │
│ This MCP does not require authentication.                    │
│                                                              │
│ (no env vars to configure)                                   │
├──────────────────────────────────────────────────────────────┤
│ OPTIONS                                                      │
│                                                              │
│ [[OK]] Auto-start on launch                                    │
│ [[OK]] Enabled                                                 │
└──────────────────────────────────────────────────────────────┘
```

### User Action: Click "Save" (Accept Defaults)

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | McpConfigureView | `on_save_click()` | Button clicked |
| 2 | McpConfigureView | `collect_config()` | Build McpConfig |
| 3 | McpConfigureView | emit | `UserEvent::SaveMcpConfig { id: None, config }` |
| 4 | McpConfigurePresenter | handles | Save new MCP |
| 5 | McpService | `add(config)` | Create MCP config |
| 6 | McpService | generate UUID | New ID |
| 7 | McpService | `save_config()` | Write: `mcps/{id}.json` |
| 8 | McpService | emit | `McpEvent::ConfigSaved { id }` |
| 9 | McpService | `start(id)` | Auto-start (enabled + auto_start) |
| 10 | McpService | emit | `McpEvent::Starting { id, name: "Exa Search" }` |
| 11 | McpService | `spawn_stdio_process()` | Spawn npx |
| 12 | McpService | initialize MCP protocol | Handshake |
| 13 | McpService | `list_tools()` | Get tool definitions |
| 14 | McpService | emit | `McpEvent::Started { id, tools: ["exa_search"], tool_count: 1 }` |
| 15 | ViewRouter | pop twice | Back to Chat |

### File I/O: MCP Created

**Write: `~/Library/.../mcps/e5f6a7b8-....json`**
```json
{
  "id": "e5f6a7b8-c9d0-1234-ef56-789012345678",
  "name": "Exa Search",
  "description": "Web search using Exa AI",
  "transport": {
    "type": "stdio",
    "command": "npx",
    "args": ["-y", "@anthropic/exa-mcp"]
  },
  "env_vars": [],
  "auto_start": true,
  "enabled": true,
  "created_at": "2026-01-25T00:42:00Z",
  "updated_at": "2026-01-25T00:42:00Z",
  "source": {
    "type": "official",
    "name": "exa-search"
  }
}
```

### MCP Startup I/O

```
Process spawn: npx -y @anthropic/exa-mcp
  stdin/stdout: MCP JSON-RPC protocol

→ {"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2024-11-05"}}
← {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}}}}

→ {"jsonrpc":"2.0","method":"tools/list","id":2}
← {"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"exa_search","description":"Search the web using Exa AI","inputSchema":{...}}]}}
```

### Events Flow (MCP Save & Start)

```
UserEvent::SaveMcpConfig { config }
    │
    └──▶ McpConfigurePresenter handles
             │
             ├──▶ McpService.add(config)
             │        │
             │        └──▶ McpEvent::ConfigSaved { id }
             │
             └──▶ McpService.start(id)
                      │
                      ├──▶ McpEvent::Starting { id, name }
                      │        │
                      │        └──▶ SettingsPresenter receives (if visible)
                      │                 │
                      │                 └──▶ view.show_mcp_loading(id)
                      │
                      └──▶ McpEvent::Started { id, tools, tool_count }
                               │
                               ├──▶ SettingsPresenter receives
                               │        │
                               │        └──▶ view.show_mcp_active(id)
                               │
                               └──▶ ChatPresenter receives
                                        │
                                        └──▶ (tools now available for agent)
```

### State After Phase 7

```
AppState:
  default_profile_id: Some("a1b2c3d4-...")
  current_conversation_id: Some("b2c3d4e5-...")

McpService State:
  running_mcps: {
    "e5f6a7b8-...": RunningMcp {
      client: McpClient,
      tools: [McpTool { name: "exa_search", ... }],
      started_at: ...
    }
  }

ChatView State:
  show_thinking: false  ← preserved from earlier toggle
  
View Stack: [Chat]
```

---

## Phase 8: Use MCP in Conversation

### User Action: Type "search on haikus and give me the history of the form"

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | ChatView | `on_input_change(...)` | Text entered |
| 2 | ChatView | `on_enter_pressed()` | Enter key |
| 3 | ChatView | emit | `UserEvent::SendMessage { text: "search on haikus..." }` |
| 4 | ChatPresenter | handles | Same flow as before |
| 5 | ChatPresenter | `view.add_user_bubble(...)` | Show user message |
| 6 | ConversationService | `append_message(user_msg)` | Append to .jsonl |
| 7 | ChatService | `send_message(...)` | Start streaming |

### ChatService.send_message() - Now With Tools

| Step | Component | Method | Details |
|------|-----------|--------|--------|
| 1 | ChatService | `get_profile_config()` | Load profile |
| 2 | ProfileService | `get_model_config()` | With keyfile resolved |
| 3 | ChatService | `load_conversation_history()` | Now 2 messages |
| 4 | ConversationService | `get_messages()` | Read .jsonl |
| 5 | ChatService | `get_toolsets()` | From McpService |
| 6 | McpService | `get_toolsets()` | Returns `[Arc<McpToolset>]` |
| 7 | ChatService | `build_agent()` | With toolsets this time |
| 8 | ChatService | emit | `ChatEvent::StreamStarted { ... }` |
| 9 | ChatService | `agent.stream()` | SerdesAI handles tool loop |

### SerdesAI Agent Creation - With Tools

```rust
let client = OpenAIClient::new()
    .with_base_url("https://api.synthetic.com/v1")
    .with_api_key(&api_key)
    .build();

let agent = Agent::new(client)
    .with_model("glm-4.7")
    .with_system_prompt("You are a helpful assistant.")
    .with_temperature(0.7)
    .with_max_tokens(4096)
    .with_extended_thinking(10000)
    .with_history_processor(TruncateByTokens::new(128000));

// Add MCP toolsets
let toolsets = mcp_service.get_toolsets();  // [McpToolset(exa_search)]
for toolset in toolsets {
    agent = agent.with_toolset(toolset);
}

let stream = agent.stream(&messages).await?;
```

### Network I/O: LLM API Call with Tools

```
POST https://api.synthetic.com/v1/chat/completions
Headers:
  Authorization: Bearer sk-synth-abc123xyz789...

Body:
{
  "model": "glm-4.7",
  "messages": [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "write me a haiku"},
    {"role": "assistant", "content": "Autumn leaves falling..."},
    {"role": "user", "content": "search on haikus and give me the history of the form"}
  ],
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "exa_search",
        "description": "Search the web using Exa AI",
        "parameters": {
          "type": "object",
          "properties": {
            "query": {"type": "string"},
            "num_results": {"type": "integer"}
          },
          "required": ["query"]
        }
      }
    }
  ],
  "temperature": 0.7,
  "max_tokens": 4096,
  "stream": true,
  "extended_thinking": {"enabled": true, "budget_tokens": 10000}
}
```

### LLM Response Stream (with Tool Call)

```
Response: SSE stream

event: thinking
data: {"content": "The user wants to know about haikus. I should search for information..."}

event: tool_call
data: {"id": "call_abc123", "function": {"name": "exa_search", "arguments": "{\"query\": \"history of haiku poetry form origin\", \"num_results\": 5}"}}

-- SerdesAI executes tool call via McpToolset --

event: tool_result
data: {"tool_call_id": "call_abc123", "content": "[search results about haiku history]"}

event: message
data: {"content": "Based on my search, haikus originated in Japan..."}

event: message
data: {"content": " The form evolved from hokku, the opening verse..."}

event: done
data: {"usage": {"total_tokens": 850}}
```

### Tool Execution Flow (Inside SerdesAI)

When SerdesAI receives a tool call from the LLM:

| Step | Component | Method | Details |
|------|-----------|--------|--------|
| 1 | Agent | receives tool_call event | `exa_search({"query": "..."})` |
| 2 | Agent | find matching toolset | `McpToolset` matches |
| 3 | McpToolset | `call("exa_search", args)` | Execute tool |
| 4 | McpToolset | MCP JSON-RPC | Send to MCP process |
| 5 | Exa MCP | executes | HTTP to Exa AI API |
| 6 | Exa MCP | returns | Search results |
| 7 | McpToolset | returns to Agent | Tool result |
| 8 | Agent | sends result to LLM | Continue conversation |

### MCP I/O (Tool Call)

```
→ {"jsonrpc":"2.0","method":"tools/call","id":3,"params":{"name":"exa_search","arguments":{"query":"history of haiku poetry form origin","num_results":5}}}

(Exa MCP internally calls Exa AI API)

← {"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"1. Haiku originated in Japan in the 17th century...\n2. The form evolved from renga collaborative poetry...\n3. Matsuo Basho is considered the master of haiku...\n4. Traditional haiku follows 5-7-5 syllable pattern...\n5. Modern haiku has evolved beyond strict syllable counts..."}]}}
```

### Events During Tool-Using Stream

| Event | Payload | UI Update |
|-------|---------|-----------|
| `ChatEvent::StreamStarted` | `{ model_id: "glm-4.7" }` | Add model label, cursor |
| `ChatEvent::ThinkingDelta` | `"The user wants..."` | (hidden - show_thinking=false) |
| `ChatEvent::ToolCallStarted` | `{ id: "call_abc123", name: "exa_search" }` | Show tool indicator |
| `McpEvent::ToolCalled` | `{ mcp_id, tool_name }` | (logged) |
| `McpEvent::ToolCompleted` | `{ success: true, duration_ms: 1200 }` | (logged) |
| `ChatEvent::ToolCallCompleted` | `{ success: true, result: "[...]" }` | Update tool indicator |
| `ChatEvent::TextDelta` | `"Based on my search..."` | Append to bubble |
| `ChatEvent::TextDelta` | `" The form evolved..."` | Append |
| `ChatEvent::StreamCompleted` | `{ total_tokens: 850 }` | Remove cursor |

### Event Consumption

```
ChatEvent::ToolCallStarted { tool_name: "exa_search" }
    │
    └──▶ ChatPresenter receives
             │
             └──▶ view.show_tool_indicator("exa_search")
                      │
                      └──▶ Show " Calling exa_search..." in chat

ChatEvent::ToolCallCompleted { success: true }
    │
    └──▶ ChatPresenter receives
             │
             └──▶ view.update_tool_indicator(success: true)
                      │
                      └──▶ Show "[OK] exa_search completed"
```

### Persist Assistant Message (with Tool Calls)

**Append: `~/Library/.../conversations/20260125004000001.jsonl`**
```json
{"id":"e5f6a7b8-...","role":"user","content":"search on haikus and give me the history of the form","model_id":null,"timestamp":"2026-01-25T00:43:00Z"}
{"id":"f6a7b8c9-...","role":"assistant","content":"Based on my search, haikus originated in Japan in the 17th century. The form evolved from hokku, the opening verse of collaborative renga poetry. Matsuo Basho (1644-1694) is considered the greatest haiku master. Traditional haiku follows a 5-7-5 syllable pattern and typically references nature and seasons. Modern haiku has evolved beyond strict syllable counts to focus on the essence: a moment of awareness captured in minimal words.","thinking":"The user wants to know about haikus. I should search for information...","tool_calls":[{"id":"call_abc123","name":"exa_search","arguments":{"query":"history of haiku poetry form origin"},"result":"[search results]"}],"model_id":"glm-4.7","timestamp":"2026-01-25T00:43:05Z"}
```

### Screen Display: After Tool-Using Response

```
┌──────────────────────────────────────────────────────────────┐
│ [Icon] PersonalAgent              [T] [S] [H] [+] []       │
├──────────────────────────────────────────────────────────────┤
│ ┌─────────────────────────────┐                              │
│ │ New 2026-01-25 00:40      ▼ │  glm-4.7                     │
│ └─────────────────────────────┘                              │
├──────────────────────────────────────────────────────────────┤
│                        ┌──────────────────────────────┐      │
│                        │ write me a haiku             │      │
│                        └──────────────────────────────┘      │
│  glm-4.7                                                     │
│  ┌──────────────────────────────┐                            │
│  │ Autumn leaves falling...     │                            │
│  └──────────────────────────────┘                            │
│                                                              │
│                        ┌──────────────────────────────┐      │
│                        │ search on haikus and give me │      │
│                        │ the history of the form      │      │
│                        └──────────────────────────────┘      │
│  glm-4.7                                                     │
│  ┌──────────────────────────────┐                            │
│  │ [OK] exa_search (1.2s)         │  ← Tool indicator           │
│  │                              │                            │
│  │ Based on my search, haikus  │                            │
│  │ originated in Japan in the  │                            │
│  │ 17th century...             │                            │
│  └──────────────────────────────┘                            │
├──────────────────────────────────────────────────────────────┤
│ [Type a message...                         ]  [Send]         │
└──────────────────────────────────────────────────────────────┘
```

---

## Phase 9: Toggle Thinking On

### User Action: Click [T] Button

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | ChatView | `on_thinking_toggle_click()` | Button clicked |
| 2 | ChatView | emit | `UserEvent::ToggleThinking` |
| 3 | ChatPresenter | handles | Toggle local state |
| 4 | ChatPresenter | `self.show_thinking = true` | `false → true` |
| 5 | ChatPresenter | `view.set_show_thinking(true)` | Update UI |
| 6 | ChatView | `update_thinking_visibility()` | Show all thinking sections |
| 7 | ChatView | `thinking_button.set_active(true)` | Add blue highlight |

### Screen Display: Thinking Visible Again

```
┌──────────────────────────────────────────────────────────────┐
│ [Icon] PersonalAgent              [T[OK]] [S] [H] [+] []      │
│                                    ↑ blue highlight          │
├──────────────────────────────────────────────────────────────┤
│  ... (messages with thinking sections now visible) ...       │
│                                                              │
│  glm-4.7                                                     │
│  ┌──────────────────────────────┐                            │
│  │ ▼ Thinking...                │  ← Now visible             │
│  │ ┌──────────────────────────┐ │                            │
│  │ │ The user wants to know   │ │                            │
│  │ │ about haikus. I should   │ │                            │
│  │ │ search for information...│ │                            │
│  │ └──────────────────────────┘ │                            │
│  └──────────────────────────────┘                            │
│  ┌──────────────────────────────┐                            │
│  │ [OK] exa_search (1.2s)         │                            │
│  │ Based on my search...       │                            │
│  └──────────────────────────────┘                            │
└──────────────────────────────────────────────────────────────┘
```

---

## Phase 10: New Conversation with Cancel

### User Action: Click [+] Button

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | ChatView | `on_new_conversation_click()` | Button clicked |
| 2 | ChatView | emit | `UserEvent::NewConversation` |
| 3 | ChatPresenter | handles | Create new conversation |
| 4 | ConversationService | `create()` | New conversation |
| 5 | ConversationService | emit | `ConversationEvent::Created { id, title: "New 2026-01-25 00:45" }` |
| 6 | AppSettingsService | `set_current_conversation_id(new_id)` | Switch current |
| 7 | AppSettingsService | emit | `ConversationEvent::Activated { id }` |
| 8 | ChatPresenter | `view.clear_messages()` | Clear chat area |
| 9 | ChatPresenter | `view.show_rename_field()` | Show title edit |

### Important: show_thinking Preserved

- `show_thinking = true` is **preserved** (not reset)
- Toggle only resets on profile change or app restart
- New conversation just clears messages, doesn't reset toggle

### Files Created

**Write: `~/Library/.../conversations/20260125004500002.meta.json`**
```json
{
  "id": "a7b8c9d0-...",
  "title": "New 2026-01-25 00:45",
  "created_at": "2026-01-25T00:45:00Z",
  "updated_at": "2026-01-25T00:45:00Z",
  "message_count": 0
}
```

### User Action: Type "search for all recent llm research published in 2026"

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | ChatView | `on_input_change(...)` | Text entered |
| 2 | ChatView | `on_enter_pressed()` | Enter key |
| 3 | ChatView | emit | `UserEvent::SendMessage { text: "search for..." }` |
| 4 | ChatPresenter | handles | - |
| 5 | ChatPresenter | `view.add_user_bubble(...)` | Show user message |
| 6 | ChatPresenter | `view.show_assistant_loading()` | Show cursor |
| 7 | ChatPresenter | `view.show_stop_button()` | Enable Stop |
| 8 | ConversationService | `append_message(user_msg)` | Persist user msg |
| 9 | ChatService | `send_message(...)` | Start streaming |
| 10 | ChatService | emit | `ChatEvent::StreamStarted { ... }` |

### Streaming Begins

The agent receives the request and decides to use exa_search tool...

```
LLM Response stream begins:

event: thinking
data: {"content": "The user wants recent LLM research from 2026. I should search..."}

event: tool_call
data: {"id": "call_xyz789", "function": {"name": "exa_search", "arguments": "{\"query\": \"LLM large language model research papers 2026\"}"}}

-- Tool execution begins (taking a while) --
```

### Screen Display: During Streaming

```
┌──────────────────────────────────────────────────────────────┐
│ [Icon] PersonalAgent              [T[OK]] [S] [H] [+] []      │
├──────────────────────────────────────────────────────────────┤
│ ┌─────────────────────────────┐                              │
│ │ New 2026-01-25 00:45      ▼ │  glm-4.7                     │
│ └─────────────────────────────┘                              │
├──────────────────────────────────────────────────────────────┤
│                        ┌──────────────────────────────┐      │
│                        │ search for all recent llm    │      │
│                        │ research published in 2026   │      │
│                        └──────────────────────────────┘      │
│                                                              │
│  glm-4.7                                                     │
│  ┌──────────────────────────────┐                            │
│  │ ▼ Thinking...                │                            │
│  │ ┌──────────────────────────┐ │                            │
│  │ │ The user wants recent    │ │                            │
│  │ │ LLM research from 2026...│ │                            │
│  │ └──────────────────────────┘ │                            │
│  └──────────────────────────────┘                            │
│  ┌──────────────────────────────┐                            │
│  │  Calling exa_search...    │  ← In progress              │
│  │ ▌                            │  ← Cursor                  │
│  └──────────────────────────────┘                            │
├──────────────────────────────────────────────────────────────┤
│ [Type a message... (disabled)]            [Send] [Stop]      │
│                                                   ↑ enabled  │
└──────────────────────────────────────────────────────────────┘
```

### User Action: Click [Stop] Button (Cancellation)

| Step | Component | Method/Event | Details |
|------|-----------|--------------|---------|
| 1 | ChatView | `on_stop_click()` | Button clicked |
| 2 | ChatView | emit | `UserEvent::StopStreaming` |
| 3 | ChatPresenter | handles | Cancel stream |
| 4 | ChatPresenter | `chat_service.cancel(stream_handle)` | Request cancel |
| 5 | ChatService | `stream_handle.cancel()` | Drop stream |
| 6 | ChatService | `cancel_pending_tool_calls()` | Best-effort cancel |
| 7 | McpService | `cancel_tool_call(mcp_id, request_id)` | Send MCP notification |

### MCP Cancellation Attempt

```
→ {"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"3","reason":"User requested cancellation"}}

(MCP server may or may not honor this - best effort)
```

### Cancel Events Flow

```
UserEvent::StopStreaming
    │
    └──▶ ChatPresenter handles
             │
             ├──▶ ChatService.cancel(handle)
             │        │
             │        ├──▶ Drop stream (stops receiving)
             │        │
             │        └──▶ McpService.cancel_tool_call() (best-effort)
             │
             └──▶ ChatEvent::StreamCancelled {
                      partial_content: "...",
                      partial_thinking: "The user wants..."
                  }
                      │
                      └──▶ ChatPresenter receives
                               │
                               ├──▶ view.remove_cursor()
                               ├──▶ view.append_cancelled_marker()
                               └──▶ persist cancelled message
```

### Persist Cancelled Message

**Append: `~/Library/.../conversations/20260125004500002.jsonl`**
```json
{"id":"b8c9d0e1-...","role":"user","content":"search for all recent llm research published in 2026","model_id":null,"timestamp":"2026-01-25T00:45:00Z"}
{"id":"c9d0e1f2-...","role":"assistant","content":"[cancelled]","thinking":"The user wants recent LLM research from 2026. I should search...","tool_calls":[{"id":"call_xyz789","name":"exa_search","status":"cancelled"}],"model_id":"glm-4.7","cancelled":true,"timestamp":"2026-01-25T00:45:05Z"}
```

### Screen Display: After Cancellation

```
┌──────────────────────────────────────────────────────────────┐
│ [Icon] PersonalAgent              [T[OK]] [S] [H] [+] []      │
├──────────────────────────────────────────────────────────────┤
│ ┌─────────────────────────────┐                              │
│ │ New 2026-01-25 00:45      ▼ │  glm-4.7                     │
│ └─────────────────────────────┘                              │
├──────────────────────────────────────────────────────────────┤
│                        ┌──────────────────────────────┐      │
│                        │ search for all recent llm    │      │
│                        │ research published in 2026   │      │
│                        └──────────────────────────────┘      │
│                                                              │
│  glm-4.7                                                     │
│  ┌──────────────────────────────┐                            │
│  │ ▼ Thinking...                │                            │
│  │ ┌──────────────────────────┐ │                            │
│  │ │ The user wants recent    │ │                            │
│  │ │ LLM research from 2026...│ │                            │
│  │ └──────────────────────────┘ │                            │
│  └──────────────────────────────┘                            │
│  ┌──────────────────────────────┐                            │
│  │  exa_search (cancelled)    │                            │
│  │ [cancelled]                  │  ← Simple marker           │
│  └──────────────────────────────┘                            │
├──────────────────────────────────────────────────────────────┤
│ [Type a message...                         ]  [Send]         │
│                                             ↑ re-enabled     │
└──────────────────────────────────────────────────────────────┘
```

### Final State

```
AppState:
  default_profile_id: Some("a1b2c3d4-...")
  current_conversation_id: Some("a7b8c9d0-...")  ← new conversation

ChatView State:
  conversation_id: Some("a7b8c9d0-...")
  is_streaming: false
  show_thinking: true  ← still true, preserved through all operations
  
McpService State:
  running_mcps: { "e5f6a7b8-...": RunningMcp { ... } }  ← still running
```

---

## Event Bounce Analysis

### What is Event Bounce?

Event bounce occurs when:
1. An event triggers a handler
2. The handler emits another event
3. That event triggers another handler
4. Which may emit more events...

This can cause:
- Infinite loops
- Out-of-order UI updates
- Race conditions
- Performance issues

### Identified Bounce Patterns

#### Pattern 1: Profile Save → Default Change → Chat Reload

```
UserEvent::SaveProfile
    │
    └──▶ ProfileEvent::Created
             │
             ├──▶ SettingsPresenter updates list
             │
             └──▶ ProfileEvent::DefaultChanged (if first profile)
                      │
                      └──▶ ChatPresenter reloads
                               │
                               └──▶ view.set_show_thinking() from profile
```

**Risk:** Low. Linear chain, no loops.

**Mitigation:** Events are async, presenter receives in order.

#### Pattern 2: Delete Default Profile → Auto-Select Next

```
UserEvent::ConfirmDeleteProfile
    │
    └──▶ ProfileService.delete()
             │
             ├──▶ ProfileEvent::Deleted
             │        │
             │        └──▶ SettingsPresenter removes from list
             │
             └──▶ ProfileEvent::DefaultChanged (to next profile)
                      │
                      └──▶ ChatPresenter reloads with new profile
```

**Risk:** Low. Linear chain.

#### Pattern 3: MCP Start → Tools Available → Chat Aware

```
McpEvent::Started { tools }
    │
    ├──▶ SettingsPresenter updates status
    │
    └──▶ ChatPresenter knows tools available
```

**Risk:** None. Parallel consumption, no emitting.

#### Pattern 4: Stream Events During Tool Call

```
ChatEvent::StreamStarted
    │
    ├──▶ ChatPresenter shows cursor
    │
    └──▶ ...streaming...
             │
             ├──▶ ChatEvent::ToolCallStarted
             │        │
             │        └──▶ ChatPresenter shows tool indicator
             │
             ├──▶ McpEvent::ToolCalled (from toolset)
             │        │
             │        └──▶ (logged, no UI action)
             │
             ├──▶ McpEvent::ToolCompleted
             │        │
             │        └──▶ (logged, no UI action)
             │
             ├──▶ ChatEvent::ToolCallCompleted
             │        │
             │        └──▶ ChatPresenter updates tool indicator
             │
             └──▶ ChatEvent::StreamCompleted
                      │
                      └──▶ ChatPresenter finalizes
```

**Risk:** Low. All events consumed by same presenter in order.

**Potential Issue:** If tool call takes long, user might navigate away. When `ToolCallCompleted` arrives, ChatPresenter might update a detached view.

**Mitigation:** Check if still active view before updating:
```rust
fn handle_tool_completed(&mut self, event: &ChatEvent) {
    if self.is_active && self.conversation_id == event.conversation_id {
        self.view.update_tool_indicator(...);
    }
}
```

#### Pattern 5: Thinking Toggle - No Bounce

```
UserEvent::ToggleThinking
    │
    └──▶ ChatPresenter handles
             │
             └──▶ view.set_show_thinking() (no event emitted)
```

**Risk:** None. Terminal action.

### Bounce Prevention Strategies

1. **Event Typing:** Separate `UserEvent` from `DomainEvent` - handlers know if they initiated the action

2. **Idempotency:** Handlers should be idempotent - same event handled twice = same result

3. **Guard Conditions:** Check state before emitting:
   ```rust
   fn set_default_profile(&mut self, id: Uuid) {
       if self.default_profile_id != Some(id) {
           self.default_profile_id = Some(id);
           self.emit(ProfileEvent::DefaultChanged { ... });
       }
   }
   ```

4. **Async Boundaries:** Use `tokio::spawn` for non-critical updates to avoid blocking

5. **View Lifecycle:** Presenters track `is_active` and ignore events when not visible

---

## State Management Analysis

### Global State (Persisted)

| Location | Data | Persistence |
|----------|------|-------------|
| `settings.json` | `default_profile_id`, `current_conversation_id`, hotkey | File |
| `profiles/*.json` | Profile configurations | Files |
| `conversations/*.jsonl` | Messages | Files |
| `conversations/*.meta.json` | Conversation metadata | Files |
| `mcps/*.json` | MCP configurations | Files |
| Keychain/Secrets | API keys (encrypted) | Secure storage |

### Service State (Runtime)

| Service | Runtime State | Notes |
|---------|--------------|-------|
| `McpService` | `running_mcps: HashMap<Uuid, RunningMcp>` | Process handles, tool lists |
| `ChatService` | `active_streams: HashMap<Uuid, StreamHandle>` | For cancellation |
| `ModelsRegistryService` | `cached_models: Vec<Provider>` | In-memory cache |

### View/Presenter State (Transient)

| Component | State | Lifetime |
|-----------|-------|----------|
| ChatPresenter | `show_thinking` | Until profile change or app restart |
| ChatPresenter | `is_streaming` | Until stream completes/cancels |
| ChatView | `selected_conversation_id` | Until navigation away |
| ModelSelectorView | `selected_model` | Until save/cancel |

### State Synchronization

```
┌─────────────────────────────────────────────────────────────┐
│                     Persisted State                          │
│                     (Source of Truth)                        │
│  settings.json ←──────────────────────────────────────────┐  │
│  profiles/*.json                                          │  │
│  conversations/*.jsonl                                    │  │
└───────────────────────────────────────────────────────────│──┘
                         │                                  │
                    read │                             write│
                         ▼                                  │
┌─────────────────────────────────────────────────────────────┐
│                     Service Layer                           │
│  - Reads persisted state on init                            │
│  - Updates persisted state on changes                       │
│  - Emits events when state changes                          │
└─────────────────────────────────────────────────────────────┘
                         │
                   events│
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                    Presenter Layer                          │
│  - Subscribes to relevant events                            │
│  - Maintains view-specific state (show_thinking, etc.)      │
│  - Updates views on state changes                           │
└─────────────────────────────────────────────────────────────┘
                         │
                  update │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                      View Layer                             │
│  - Renders current state                                    │
│  - Emits UserEvents on interaction                          │
│  - No internal state (purely presentational)                │
└─────────────────────────────────────────────────────────────┘
```

### Consistency Guarantees

1. **Profile → Chat:** When default profile changes, ChatPresenter receives `ProfileEvent::DefaultChanged` and reloads

2. **Conversation → Chat:** When current conversation changes, ChatPresenter receives `ConversationEvent::Activated` and loads messages

3. **MCP → Chat:** When MCP status changes, SettingsPresenter updates UI; ChatPresenter gets updated toolsets on next message send

4. **Toggle → Transient:** `show_thinking` is never persisted, always reset to profile default on profile change

---

## Gap Analysis

### Gaps Identified

| Gap | Severity | Description | Resolution |
|-----|----------|-------------|------------|
| Keyfile error handling | High | If `~/.synthetic_key` is deleted after profile creation, `get_model_config()` fails at runtime | **Added:** `ProfileError::KeyfileReadFailed` with user-friendly error display in chat |
| Delete default profile | Medium | What happens when you delete the default profile? | **Added:** Auto-select next profile as default |
| Delete current conversation | Medium | What happens when you delete the current conversation? | **Added:** Auto-select next conversation as current |
| Thinking toggle persistence | Low | Requirements didn't clearly specify when toggle resets | **Added:** Clear behavior table - resets on profile change and app restart only |
| MCP HTTP transport | Medium | Marked as "partial" in application.md | **Updated:** Full support via SerdesAI |
| Per-message model label | Low | Not clearly specified how displayed | **Added:** Model label spec above each assistant message |
| Tool call cancellation | Medium | MCP cancellation is best-effort | **Documented:** MCP spec compliance, graceful handling |

### Updates Made

1. **`services/profile.md`:**
   - Added `ProfileError::KeyfileReadFailed` error type
   - Added delete-selects-next-default behavior
   - Added test requirements

2. **`services/conversation.md`:**
   - Added delete-selects-next-current behavior
   - Added test requirements

3. **`ui/chat.md`:**
   - Added Thinking Toggle Behavior Summary table
   - Added Error Display section for keyfile errors
   - Clarified per-message model label positioning

4. **`services/mcp.md`:**
   - Added SerdesAI MCP Integration section
   - Documented full StdioTransport and HttpTransport support
   - Added test requirements

5. **`application.md`:**
   - Updated MR-4 (HTTP transport) from "WARNING: Partial" to "[OK]"

---

## Summary

This document traced the complete theoretical execution path through a first-time user scenario, covering:

- **20+ file I/O operations** (settings, profiles, conversations, MCPs)
- **10+ network requests** (models.dev, LLM API, MCP registries, Exa AI)
- **50+ events** emitted and consumed
- **15+ presenter method calls**
- **5 view transitions**

Key architectural insights:
1. Events flow unidirectionally: View → EventBus → Presenter → Service → EventBus
2. State is owned by services, reflected in views via events
3. Transient state (like `show_thinking`) is managed by presenters, not persisted
4. SerdesAI handles the agent loop including tool execution
5. MCP cancellation is best-effort per MCP specification
6. Event bounce is minimized through clear ownership and guard conditions
