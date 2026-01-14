# PersonalAgent - Requirements Document

**Version:** 0.1  
**Date:** 2026-01-13  
**Status:** Draft

---

## 1. Overview

PersonalAgent is a macOS menu bar application written in Rust that provides a conversational AI interface. It lives in the system menu bar (next to WiFi, battery, etc.), drops down when clicked, and allows users to chat with various LLM providers through a unified interface.

---

## 2. Platform & Technology Stack

### 2.1 Target Platform
- **Primary OS:** macOS (menu bar/status bar application)
- **Future consideration:** Cross-platform (Windows, Linux) may be considered post-v1.0

### 2.2 Language
- **Rust** (latest stable)

### 2.3 GUI Framework
- **Recommended:** `objc2` + `objc2-app-kit` + `objc2-foundation` (native macOS via Rust)
  - `objc2`: Safe Rust bindings to Objective-C runtime
  - `objc2-app-kit`: Rust bindings to AppKit (NSApplication, NSStatusItem, NSPopover, etc.)
  - `objc2-foundation`: Rust bindings to Foundation framework
- **Architecture:** Same as BarTranslate (Swift) but in Rust:
  - `NSStatusItem` for menu bar icon
  - `NSPopover` with `Transient` behavior for dropdown panel
  - `NSViewController` to host content view
- **Rationale:** Native macOS behavior (proper popover with arrow, correct positioning, auto-dismiss), pure Rust, identical to how Swift menu bar apps work

**Note:** The original plan used `eframe` + `egui` + `tray-icon`, but this creates a regular window instead of a native popover, causing incorrect positioning and behavior. The objc2 approach provides true native menu bar app functionality.

### 2.4 LLM Integration Library
- **Primary:** `serdes-ai` (SerdesAI - PydanticAI port to Rust)
  - Multi-provider support (OpenAI, Anthropic, Google, Groq, Mistral, Ollama, Bedrock, Azure)
  - Streaming responses with configurable `emit_thinking` for thinking block toggle
  - Tool/function calling support (for v0.2+)
  - Graph workflows for future agent features
  - MCP support for tool servers
  - Async/await native
- **Repository:** https://github.com/janfeddersen-wq/serdesAI

---

## 3. Core Features

### 3.1 Menu Bar Integration
- **Tray Icon:** Persistent icon in macOS menu bar
- **Dropdown Panel:** Custom egui-rendered panel appears on icon click
- **Global Hotkey:** Configurable keyboard shortcut to summon/dismiss the panel
- **Positioning:** Panel drops down from menu bar icon, properly positioned relative to icon location

### 3.2 Theme & Appearance
- **Default:** Dark mode
- **Configurable:** User can switch between dark/light mode in settings
- **Native Feel:** UI should feel at home on macOS (appropriate fonts, spacing, colors)

### 3.3 Chat Interface
- **Message Display:** Scrollable conversation view with clear user/assistant message distinction
- **Input Field:** Text input area at bottom of panel
- **Streaming:** Tokens displayed as they arrive from the LLM
- **Markdown Rendering:** Basic markdown support in assistant responses (code blocks, bold, italic, lists)

### 3.4 Model Profiles
- **Multiple Profiles:** Users can create, edit, delete, and switch between model configurations
- **Profile Fields:**
  - Profile name (user-defined)
  - Provider (selected from models.dev or custom)
  - Model ID
  - Base URL (auto-populated from models.dev or manually entered)
  - Authentication method:
    - Direct API key / PAT entry
    - Path to keyfile
  - Optional: Custom system prompt override
- **Active Profile:** One profile is active at a time; easily switchable from UI

### 3.5 Model Discovery
- **Primary Source:** Fetch available models from `https://models.dev/api.json`
  - Structure: `{ "provider_id": { id, env, api, name, doc, models: { "model_id": {...} } } }`
  - Model fields include: id, name, family, tool_call, reasoning, structured_output, modalities, cost, limit, etc.
- **Custom Entry:** Users can manually specify base_url + model_name for unlisted or self-hosted models
- **Refresh:** Ability to refresh model list from models.dev on demand

---

## 4. Conversation Management

### 4.1 Stateful Conversations
- Full conversation history maintained during a session
- History sent to model for context (subject to context window limits)

### 4.2 Persistence
- **Storage Format:** JSON files
- **Storage Location:** `~/Library/Application Support/PersonalAgent/conversations/`
- **Naming Convention:** `YYYYMMDDHHMMSSmmm.json` (timestamp with milliseconds)
  - Example: `20260113134915123.json`

### 4.3 Save & Resume
- Conversations auto-saved periodically and on close
- Users can browse saved conversations and continue them
- Users can rename conversations (stored as metadata, filename remains timestamp-based)
- Users can delete conversations

### 4.4 Context Window Management (Sandwich Summarization)

When conversation approaches context limit:

- **Trigger:** At 80% of model's context limit
- **Preservation:**
  - Top 20% of conversation (system prompt, initial context, early messages)
  - Bottom 20% of conversation (recent messages)
- **Summarization:**
  - Middle 60% is sent to LLM in a separate request with instruction to summarize
  - Target summary length: 50% of original middle section length (configurable)
  - Summary replaces the middle section in conversation context
- **Transparency:** User is notified when summarization occurs
- **Configurability:** All percentages (trigger threshold, top/bottom preservation, summary target length) are user-configurable

---

## 5. Configuration

### 5.1 Config File Location
- `~/Library/Application Support/PersonalAgent/config.json`

### 5.2 Config Structure
```json
{
  "version": "1.0",
  "theme": "dark",
  "global_hotkey": "Cmd+Shift+Space",
  "default_profile": "profile_uuid",
  "context_management": {
    "trigger_threshold": 0.80,
    "preserve_top": 0.20,
    "preserve_bottom": 0.20,
    "summary_target_ratio": 0.50
  },
  "profiles": [
    {
      "id": "uuid",
      "name": "Claude Sonnet",
      "provider_id": "anthropic",
      "model_id": "claude-sonnet-4-20250514",
      "base_url": "https://api.anthropic.com/v1",
      "auth": {
        "type": "key",
        "value": "sk-..."
      }
    },
    {
      "id": "uuid",
      "name": "Local Ollama",
      "provider_id": "custom",
      "model_id": "llama3",
      "base_url": "http://localhost:11434/v1",
      "auth": {
        "type": "keyfile",
        "path": "/path/to/keyfile"
      }
    }
  ]
}
```

### 5.3 Secure Storage
- API keys should be stored securely (consider macOS Keychain integration for v1.1+)
- For v0.1, keys stored in config file with appropriate file permissions (600)

---

## 6. System Prompt

### 6.1 Default System Prompt
```
You are a helpful assistant.
```

### 6.2 Future Enhancement (v0.2+)
- Per-profile custom system prompts
- System prompt templates

---

## 7. Version Roadmap

### 7.1 Version 0.1 (MVP)
- Menu bar icon with dropdown chat panel
- Single-provider chat (OpenAI-compatible initially)
- Streaming responses
- Basic conversation persistence
- Model profile management (CRUD)
- Dark mode UI
- Global hotkey
- Sandwich summarization for context management

### 7.2 Version 0.2
- Tool/function calling support
- Multiple provider support via Rig
- models.dev integration for model discovery

### 7.3 Version 0.3+
- Automation workflows
- Agent capabilities
- macOS Keychain integration for secure key storage
- Custom system prompts per profile
- Conversation search
- Export conversations (Markdown, JSON)

---

## 8. Directory Structure

```
~/Library/Application Support/PersonalAgent/
├── config.json
├── conversations/
│   ├── 20260113134915123.json
│   ├── 20260113150230456.json
│   └── ...
└── cache/
    └── models.json  (cached models.dev data)
```

---

## 9. Non-Functional Requirements

### 9.1 Performance
- App should launch quickly (< 1 second to show menu bar icon)
- UI should remain responsive during streaming
- Memory footprint should be minimal when idle

### 9.2 Reliability
- Graceful handling of network failures
- Auto-save conversations to prevent data loss
- Clear error messages for API failures

### 9.3 Security
- Config file permissions: 600 (owner read/write only)
- No logging of API keys
- Future: Keychain integration

### 9.4 Usability
- Intuitive UI requiring no documentation for basic use
- Keyboard-friendly (hotkey to open, Enter to send, Escape to close)

---

## 10. Open Questions / Future Considerations

1. **App icon design:** Need to design a menu bar icon (monochrome, appropriate size)
2. **Bundle ID:** `com.personalagent.app` or similar
3. **Code signing:** Required for distribution; developer certificate needed
4. **Auto-update mechanism:** Consider for v1.0+
5. **Analytics/telemetry:** None planned; privacy-focused
6. **Conversation branching:** Allow forking conversations at any point (future feature)

---

## 11. References

- models.dev API: https://models.dev/api.json
- Rig documentation: https://docs.rig.rs
- egui: https://github.com/emilk/egui
- eframe: https://docs.rs/eframe
- muda: https://docs.rs/muda
- tray-icon: https://docs.rs/tray-icon
