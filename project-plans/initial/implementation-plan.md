# PersonalAgent - Implementation Plan

**Version:** 0.3 (Updated)  
**Date:** 2026-01-14

---

## Reference Documents

- **Requirements:** `project-plans/initial/requirements.md`
- **UI Mockup:** `project-plans/initial/ui-mockup-v2.html` (open in browser to view)
- **Icon Source:** `assets/ai_eye.svg` (evil red eye icon)
- **SerdesAI Source:** `research/serdesAI/` (cloned for reference)
- **BarTranslate Reference:** `research/BarTranslate/` (macOS menu bar app example)

---

## Development Philosophy

- **Test-First Development (TDD)**: Write tests before implementation
- **High Code Quality**: Strict linting, complexity limits, coverage enforcement
- **Coverage Gate**: 80% minimum coverage enforced locally via pre-commit hook
- **Incremental Milestones**: Get visible progress early (menu bar icon first)

---

## Technology Stack

### GUI Architecture
**IMPORTANT:** We use **native macOS APIs via objc2** for the menu bar/popover, NOT eframe/egui for the main window.

- **`objc2` + `objc2-app-kit` + `objc2-foundation`**: Direct Rust bindings to Cocoa for native NSPopover
- **`NSStatusItem`**: Menu bar icon (like BarTranslate)
- **`NSPopover`**: Native popover with arrow anchored to status item
- **`NSViewController`**: Hosts the content view inside the popover

This approach is identical to how BarTranslate (Swift) works, just implemented in Rust.

### Why NOT eframe + tray-icon?
The original plan used `eframe` + `egui` + `tray-icon`. This creates a **regular window** and tries to position it below the tray icon. Problems:
1. macOS doesn't provide accurate tray icon coordinates
2. Regular windows don't behave like native popovers (no arrow, wrong dismissal behavior)
3. The window appears in wrong locations (e.g., lower-left quadrant)

### Current Working Stack
```toml
[dependencies]
objc2 = "0.6"
objc2-app-kit = { version = "0.3", features = [...] }
objc2-foundation = { version = "0.3", features = [...] }
```

---

## Phase 0: Minimal Viable Menu Bar App - COMPLETE

**Goal:** Icon in menu bar, click shows popover panel. Nothing else.

### 0.1 Initialize Project with Quality Gates - COMPLETE
- [x] `cargo new personal-agent`
- [x] Create `.clippy.toml`
- [x] Create `.rustfmt.toml`
- [x] Configure Cargo.toml with lints

### 0.2 Implement Native Menu Bar + NSPopover - COMPLETE
- [x] Create `main_menubar.rs` using pure objc2 bindings
- [x] `NSStatusBar.systemStatusBar.statusItemWithLength()` for menu bar item
- [x] `NSPopover` with `Transient` behavior
- [x] `NSViewController` to host content
- [x] `showRelativeToRect:ofView:preferredEdge:` to show popover anchored to button
- [x] `NSApplicationActivationPolicy::Accessory` for no dock icon
- [x] Click status item toggles popover visibility
- [x] Popover auto-dismisses when clicking elsewhere

**Milestone 0 Complete:** Menu bar icon visible, popover opens on click with arrow pointing to it.

---

## Phase 0.5: Icon Assets - COMPLETE

**Goal:** Create proper menu bar icon and app icon from ai_eye.svg

### 0.5.1 Menu Bar Icon - COMPLETE
- [x] Created colored red eye icon (NOT template - kept original colors)
- [x] Exported at 16, 32, 64px sizes
- [x] Saved to `assets/MenuBarIcon.imageset/`
- [x] Integrated into app with 17.6x17.6 point size (10% larger than standard)

### 0.5.2 App Icon - COMPLETE
- [x] Created full color evil red eye `.icns` file
- [x] All standard sizes (16, 32, 128, 256, 512, 1024)
- [x] Saved to `assets/AppIcon.icns`

### 0.5.3 Icon Integration - COMPLETE
- [x] Load menu bar icon in `main_menubar.rs` via `NSImage`
- [x] Icon displays correctly in menu bar with red eye visible

**Milestone 0.5 Complete:** Red evil eye icon shows in menu bar at correct size.

---

## Phase 1: Core Infrastructure - COMPLETE

**Goal:** Configuration system, model profiles, conversation storage

### 1.1 Configuration System - COMPLETE
- [x] Write tests for config load/save/default
- [x] Define `Config` struct with serde
- [x] Implement config loading from `~/Library/Application Support/PersonalAgent/config.json`
- [x] Implement config saving
- [x] Create default config on first run
- [x] File permissions (600)

### 1.2 Model Profiles - COMPLETE
- [x] Write tests for profile CRUD
- [x] Define `ModelProfile` struct
- [x] Fields: id, name, provider_id, model_id, base_url, auth (key/keyfile), parameters
- [x] Parameters: temperature, top_p, max_tokens, thinking_budget, enable_thinking, show_thinking
- [x] CRUD operations for profiles

### 1.3 Conversation Storage - COMPLETE
- [x] Write tests for conversation save/load/list
- [x] Define `Conversation` struct
- [x] Define `Message` struct (role, content, thinking_content, timestamp)
- [x] Implement save (append mode for each message)
- [x] Implement load
- [x] Implement list all conversations
- [x] Timestamp-based naming: `YYYYMMDDHHMMSSmmm.json`

### 1.4 Directory Structure - COMPLETE
```
src/
├── main_menubar.rs      # Entry point
├── lib.rs               # Library root
├── config/
│   ├── mod.rs
│   └── settings.rs      # Config struct and load/save
├── models/
│   ├── mod.rs
│   ├── profile.rs       # ModelProfile struct
│   └── conversation.rs  # Conversation/Message structs
├── storage/
│   ├── mod.rs
│   ├── config.rs        # Config file I/O
│   └── conversations.rs # Conversation file I/O
├── error.rs             # Error types
└── ui/
    ├── mod.rs
    ├── chat_view.rs     # Chat UI
    └── theme.rs         # Dark theme colors
```

**40 tests passing**

---

## Phase 2: models.dev Integration - COMPLETE

**Goal:** Fetch and cache model registry from models.dev

### 2.1 Model Registry - COMPLETE
- [x] Write tests with mocked HTTP responses (24 tests)
- [x] Fetch `https://models.dev/api.json`
- [x] Parse into `ModelRegistry` struct with Provider, ModelInfo types
- [x] Cache to `~/Library/Application Support/PersonalAgent/cache/models.json`
- [x] 24-hour cache expiry with automatic refresh
- [x] Manual refresh via `RegistryManager::refresh()`
- [x] Provider/model lookup helpers (search by provider, capabilities, custom predicates)

### 2.2 Directory Structure Added
```
src/registry/
├── mod.rs           # Public API, RegistryManager
├── types.rs         # ModelRegistry, Provider, ModelInfo structs
├── cache.rs         # Cache load/save with expiry
└── models_dev.rs    # API client
```

**91 library tests passing**

---

## Phase 3: UI Implementation - COMPLETE

**Goal:** Build the chat interface inside the popover

### 3.1 Chat View - COMPLETE
- [x] Top bar with conversation title and model name
- [x] Icon buttons: T (thinking), S (save), H (history), + (new), gear (settings)
- [x] NSScrollView for message area
- [x] Message bubbles with rounded corners
- [x] NSTextField for input with placeholder
- [x] Send button (functional - echoes messages)
- [x] Dark theme colors from theme.rs

### 3.2 Settings View - COMPLETE (`src/ui/settings_view.rs`)
- [x] List of model profiles from config
- [x] Shows profile name, provider/model, API key status
- [x] Select button for each profile
- [x] Refresh Models button
- [x] Back button to return to chat
- [x] Scrollable list with empty state handling

### 3.3 History View - COMPLETE (`src/ui/history_view.rs`)
- [x] List conversations from storage
- [x] Shows title, date, and message count
- [x] Load and Delete buttons for each conversation
- [x] Back button to return to chat
- [x] Scrollable list with empty state

### 3.4 Button Wiring - COMPLETE
- [x] T (Thinking) - Toggle show_thinking mode
- [x] S (Save) - Save current conversation
- [x] H (History) - Show history view
- [x] + (New) - Create new conversation
- [x]  (Settings) - Show settings view

### 3.5 View Switching Architecture
- [x] NSNotificationCenter for view switching
- [x] Three view controllers: Chat, Settings, History
- [x] Back navigation from all views to chat

---

## Phase 4: LLM Integration

**Goal:** Connect to LLM providers and stream responses

### 4.1 SerdesAI Client
- [ ] Integrate serdes-ai crate
- [ ] Create client from profile
- [ ] Handle streaming responses

### 4.2 Chat Execution
- [ ] Build message history from conversation
- [ ] Call `agent.run_stream()` with `StreamConfig { emit_thinking: show_thinking, ... }`
- [ ] Process stream events: `TextDelta`, `ThinkingDelta`, `RunComplete`, `Error`
- [ ] Update UI in real-time
- [ ] Append message to conversation file on complete

### 4.3 Context Management (Sandwich Summarization)
- [ ] Calculate current token count
- [ ] Check against 80% of model's context limit
- [ ] If exceeded: preserve top/bottom 20%, summarize middle 60%
- [ ] Notify user

---

## Milestones

| Milestone | Status | Description |
|-----------|--------|-------------|
| M0 | COMPLETE | Menu bar + NSPopover working |
| M0.5 | COMPLETE | Red eye icon in menu bar |
| M1 | COMPLETE | Config/storage infrastructure (70 tests) |
| M2 | COMPLETE | models.dev integration (91 tests) |
| M3 | COMPLETE | Full chat UI with settings/history views |
| M4 | IN PROGRESS | LLM integration with streaming |
| M5 | Pending | Polish, testing |

---

## Quality Enforcement Summary

| Check | Tool | Warn | Error | When |
|-------|------|------|-------|------|
| Formatting | `cargo fmt` | - | Any diff | Pre-commit, CI |
| Linting | `cargo clippy` | - | Any warning | Pre-commit, CI |
| Complexity (CCN) | `lizard -C` | - | > 50 | Pre-commit, CI |
| Function length | `lizard -L` | > 80 lines | > 100 lines | Pre-commit, CI |
| File length | Custom script | > 750 lines | > 1000 lines | Pre-commit, CI |
| Coverage | `cargo-llvm-cov` | < 90% | < 80% | Pre-commit, CI |

---

## Appendix: Key Technical Decisions

### A.1 Why objc2 Instead of eframe/tray-icon
The original plan failed because:
- `tray-icon` + `eframe` creates a regular window, not an NSPopover
- Regular windows can't anchor to the menu bar with an arrow
- Window positioning was incorrect (appeared in wrong screen quadrant)

The solution uses `objc2` to call native Cocoa APIs directly:
- Same architecture as BarTranslate (Swift reference app)
- True NSPopover with arrow pointing to status item
- Correct transient behavior (auto-dismiss on click outside)

### A.2 Icon Implementation
- Menu bar icon: Colored PNG (NOT template) to preserve red eye
- Size: 17.6x17.6 points (10% larger than standard 16x16)
- App icon: Full `.icns` with all standard sizes

### A.3 Config File Location
`~/Library/Application Support/PersonalAgent/config.json`

### A.4 Conversation Storage
`~/Library/Application Support/PersonalAgent/conversations/YYYYMMDDHHMMSSmmm.json`
