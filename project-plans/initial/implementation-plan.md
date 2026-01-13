# PersonalAgent - Implementation Plan

**Version:** 0.1  
**Date:** 2026-01-13

---

## Phase 1: Project Scaffolding

### 1.1 Initialize Rust Project
- [ ] `cargo new personal-agent`
- [ ] Set up workspace structure if needed
- [ ] Configure Cargo.toml with dependencies:
  ```toml
  [dependencies]
  # GUI
  eframe = "0.29"
  egui = "0.29"
  tray-icon = "0.19"
  muda = "0.15"
  global-hotkey = "0.6"
  
  # LLM
  serdes-ai = { git = "https://github.com/janfeddersen-wq/serdesAI" }
  
  # Async
  tokio = { version = "1", features = ["full"] }
  futures = "0.3"
  
  # Serialization
  serde = { version = "1", features = ["derive"] }
  serde_json = "1"
  
  # HTTP (for models.dev)
  reqwest = { version = "0.12", features = ["json"] }
  
  # Utils
  chrono = { version = "0.4", features = ["serde"] }
  dirs = "5"
  uuid = { version = "1", features = ["v4", "serde"] }
  thiserror = "1"
  anyhow = "1"
  tracing = "0.1"
  tracing-subscriber = "0.3"
  ```

### 1.2 Directory Structure
```
src/
├── main.rs              # Entry point, tray icon setup, event loop
├── app.rs               # Main application state
├── ui/
│   ├── mod.rs
│   ├── chat_view.rs     # Chat interface
│   ├── settings_view.rs # Settings panel
│   ├── conversations_view.rs
│   ├── profile_editor.rs
│   ├── components/      # Reusable UI components
│   │   ├── mod.rs
│   │   ├── message.rs   # Chat message bubble
│   │   ├── thinking_block.rs
│   │   ├── input_field.rs
│   │   └── toggle.rs
│   └── theme.rs         # Dark mode colors
├── llm/
│   ├── mod.rs
│   ├── client.rs        # SerdesAI wrapper
│   ├── streaming.rs     # Stream handling
│   └── context.rs       # Sandwich summarization
├── models/
│   ├── mod.rs
│   ├── config.rs        # App configuration
│   ├── profile.rs       # Model profiles
│   ├── conversation.rs  # Conversation data
│   └── registry.rs      # models.dev integration
├── storage/
│   ├── mod.rs
│   ├── config.rs        # Config file I/O
│   └── conversations.rs # Conversation persistence
└── error.rs             # Error types
```

---

## Phase 2: Core Infrastructure

### 2.1 Configuration System
- [ ] Define `Config` struct with serde
- [ ] Implement config loading from `~/Library/Application Support/PersonalAgent/config.json`
- [ ] Implement config saving
- [ ] Create default config on first run
- [ ] File permissions (600)

### 2.2 Model Profiles
- [ ] Define `ModelProfile` struct
- [ ] Fields: id, name, provider_id, model_id, base_url, auth (key/keyfile), parameters
- [ ] Parameters: temperature, top_p, max_tokens, thinking_budget, enable_thinking, show_thinking
- [ ] CRUD operations for profiles

### 2.3 Conversation Storage
- [ ] Define `Conversation` struct
- [ ] Define `Message` struct (role, content, thinking_content, timestamp)
- [ ] Implement save (append mode for each message)
- [ ] Implement load
- [ ] Implement list all conversations
- [ ] Timestamp-based naming: `YYYYMMDDHHMMSSmmm.json`

### 2.4 models.dev Integration
- [ ] Fetch `https://models.dev/api.json`
- [ ] Parse into `ModelRegistry` struct
- [ ] Cache to `~/Library/Application Support/PersonalAgent/cache/models.json`
- [ ] Manual refresh via settings button
- [ ] Provider/model lookup helpers

---

## Phase 3: Menu Bar & Window

### 3.1 Tray Icon
- [ ] Load icon from assets (16/32/64 PNG)
- [ ] Create `TrayIcon` with `tray-icon` crate
- [ ] Handle click to show/hide panel

### 3.2 Panel Window
- [ ] Create borderless egui window
- [ ] Position below tray icon
- [ ] Size: 400x500
- [ ] Handle focus loss (hide panel)

### 3.3 Global Hotkey
- [ ] Register default hotkey (Cmd+Shift+Space)
- [ ] Handle hotkey to toggle panel visibility
- [ ] Make hotkey configurable via settings

### 3.4 Event Loop
- [ ] Integrate tray-icon event loop with egui
- [ ] Handle app lifecycle (show, hide, quit)

---

## Phase 4: UI Implementation

### 4.1 Theme
- [ ] Define color palette (all dark mode)
- [ ] Create `Theme` struct with egui `Visuals`
- [ ] Apply theme on startup

### 4.2 Chat View (Main)
- [ ] Top bar: conversation name (editable), thinking toggle, save, history, new, settings buttons
- [ ] Chat area: scrollable message list
- [ ] Message bubbles: user (right), assistant (left)
- [ ] Thinking blocks: collapsible, muted style
- [ ] Streaming cursor animation
- [ ] Input area: multiline text field, send button
- [ ] Basic markdown: bold, italic, code spans

### 4.3 Settings View
- [ ] Back button navigation
- [ ] Appearance section: dark mode toggle (always on for v0.1)
- [ ] Keyboard section: hotkey display/edit
- [ ] Context management: trigger %, preserve top/bottom %, summary ratio %
- [ ] Model registry: refresh button
- [ ] Model profiles: list with active indicator, click to edit, add button

### 4.4 Conversations View
- [ ] Back button navigation
- [ ] List of saved conversations
- [ ] Each item: title, preview, date, model
- [ ] Click to load/resume
- [ ] New conversation button

### 4.5 Profile Editor
- [ ] Back/cancel button
- [ ] Profile name input
- [ ] Provider selector (searchable list from models.dev)
- [ ] Model selector (filtered by provider)
- [ ] Auth type toggle: API Key / Keyfile
- [ ] API key input (masked) or keyfile path input
- [ ] Base URL override (optional)
- [ ] Model parameters: temperature, top_p, max_tokens sliders/inputs
- [ ] Thinking settings: enable toggle, budget input, show toggle
- [ ] Save/Delete buttons

### 4.6 Notifications
- [ ] Toast component for errors
- [ ] Position: top of panel
- [ ] Auto-dismiss after 5 seconds
- [ ] Types: error (red tint), info (neutral)

---

## Phase 5: LLM Integration

### 5.1 SerdesAI Client Wrapper
- [ ] Create client from profile (provider, model, auth)
- [ ] Handle keyfile reading
- [ ] Apply model parameters

### 5.2 Chat Execution
- [ ] Build message history from conversation
- [ ] Call `agent.run_stream()` with `StreamConfig { emit_thinking: show_thinking, ... }`
- [ ] Process stream events: `TextDelta`, `ThinkingDelta`, `RunComplete`, `Error`
- [ ] Update UI in real-time
- [ ] Append message to conversation file on complete

### 5.3 Context Management (Sandwich Summarization)
- [ ] Calculate current token count (estimate from chars or use model's tokenizer if available)
- [ ] Check against 80% of model's context limit
- [ ] If exceeded:
  - [ ] Identify top 20% messages (preserve)
  - [ ] Identify bottom 20% messages (preserve)
  - [ ] Send middle 60% to LLM with summarization prompt
  - [ ] Target: 50% of original length
  - [ ] Replace middle with summary
  - [ ] Notify user (toast)

### 5.4 Error Handling
- [ ] Catch API errors (rate limit, auth, network)
- [ ] Show toast notification
- [ ] Show inline error in chat
- [ ] Retry logic for transient errors (via SerdesAI's built-in retries)

---

## Phase 6: Polish & Testing

### 6.1 Error States
- [ ] No profiles configured - prompt to create
- [ ] No API key - show error in profile editor
- [ ] Network offline - show connection error
- [ ] Model not found - graceful error

### 6.2 Edge Cases
- [ ] Very long messages (scroll, truncation)
- [ ] Rapid send (debounce/disable during streaming)
- [ ] Empty conversation list
- [ ] First run experience

### 6.3 Local Testing
- [ ] Test with OpenAI
- [ ] Test with Anthropic (including thinking blocks)
- [ ] Test with Ollama (local)
- [ ] Test conversation save/load
- [ ] Test profile switching
- [ ] Test hotkey on various keyboard layouts

### 6.4 Build & Run
- [ ] `cargo build --release`
- [ ] Verify menu bar icon appears
- [ ] Verify panel drops down correctly
- [ ] Verify all flows work

---

## Milestones

| Milestone | Target | Description |
|-----------|--------|-------------|
| M1 | Day 1-2 | Scaffolding complete, tray icon visible, empty panel opens |
| M2 | Day 3-4 | Config/storage working, UI views navigable (no LLM) |
| M3 | Day 5-6 | models.dev integration, profile CRUD working |
| M4 | Day 7-8 | Basic chat working with one provider (OpenAI) |
| M5 | Day 9-10 | Streaming, thinking blocks, multi-provider |
| M6 | Day 11-12 | Sandwich summarization, polish, testing |

---

## Future (v0.2+)

- Tool calling / function execution
- Agent workflows (graph-based)
- MCP tool server integration
- macOS Keychain for secure key storage
- Custom system prompts per profile
- Conversation search
- Export (Markdown, JSON)
- Code signing & notarization for distribution
- Light mode theme option
