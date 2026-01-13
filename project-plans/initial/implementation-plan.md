# PersonalAgent - Implementation Plan

**Version:** 0.1  
**Date:** 2026-01-13

---

## Reference Documents

- **Requirements:** `project-plans/initial/requirements.md`
- **UI Mockup:** `project-plans/initial/ui-mockup-v2.html` (open in browser to view)
- **Icon Assets:** `assets/MenuIcon.imageset/` (16, 32, 64 px PNGs)
- **SerdesAI Source:** `research/serdesAI/` (cloned for reference)
- **BarTranslate Reference:** `research/BarTranslate/` (macOS menu bar app example)

---

## Development Philosophy

- **Test-First Development (TDD)**: Write tests before implementation
- **High Code Quality**: Strict linting, complexity limits, coverage enforcement
- **Coverage Gate**: 80% minimum coverage enforced locally via pre-commit hook
- **Incremental Milestones**: Get visible progress early (menu bar icon first)

---

## Phase 0: Minimal Viable Menu Bar App

**Goal:** Icon in menu bar, click shows empty panel. Nothing else.

### 0.1 Initialize Project with Quality Gates
- [ ] `cargo new personal-agent`
- [ ] Create `.clippy.toml`:
  ```toml
  cognitive-complexity-threshold = 50
  type-complexity-threshold = 250
  too-many-arguments-threshold = 7
  ```
- [ ] Create `.rustfmt.toml`:
  ```toml
  max_width = 100
  tab_spaces = 4
  hard_tabs = false
  edition = "2021"
  ```
- [ ] Create `.github/workflows/rust-quality.yaml` (CI pipeline)
- [ ] Create `scripts/check-quality.sh` (shared by pre-commit and CI)
- [ ] Create pre-commit hook for local quality/coverage checks
- [ ] Install local dev tools: `cargo install cargo-llvm-cov cargo-audit`
- [ ] Install lizard: `pip3 install lizard`

### 0.2 Minimal Cargo.toml
```toml
[package]
name = "personal-agent"
version = "0.1.0"
edition = "2021"

[dependencies]
eframe = "0.29"
egui = "0.29"
tray-icon = "0.19"
image = "0.25"
tracing = "0.1"
tracing-subscriber = "0.3"

[dev-dependencies]
tempfile = "3"
rstest = "0.18"

[lints.rust]
unsafe_code = "forbid"

[lints.clippy]
all = "deny"
pedantic = "warn"
nursery = "warn"
cognitive_complexity = "warn"
```

### 0.3 Quality Check Script
Create `scripts/check-quality.sh`:
```bash
#!/bin/bash
set -e

WARN_EXIT=0
ERROR_EXIT=0

echo "=== Running quality checks ==="

# Format check
echo "Checking formatting..."
cargo fmt -- --check || { echo "ERROR: Format check failed"; exit 1; }

# Clippy
echo "Running clippy..."
cargo clippy -- -D warnings || { echo "ERROR: Clippy failed"; exit 1; }

# Complexity check (CCN 50, function length error at 100, warn at 80)
echo "Checking complexity..."
lizard -C 50 -L 100 -w src/ || { echo "ERROR: Complexity/function length exceeded"; ERROR_EXIT=1; }

# Function length warnings (80 lines)
long_funcs=$(lizard -L 80 src/ 2>/dev/null | grep -c "warning" || true)
if [ "$long_funcs" -gt 0 ]; then
    echo "WARNING: $long_funcs functions exceed 80 lines"
    WARN_EXIT=1
fi

# File length check
echo "Checking file lengths..."
for file in $(find src -name "*.rs"); do
    lines=$(wc -l < "$file")
    if [ "$lines" -gt 1000 ]; then
        echo "ERROR: $file has $lines lines (max 1000)"
        ERROR_EXIT=1
    elif [ "$lines" -gt 750 ]; then
        echo "WARNING: $file has $lines lines (recommended max 750)"
        WARN_EXIT=1
    fi
done

# Tests with coverage
echo "Running tests with coverage..."
cargo llvm-cov --summary-only > /tmp/cov_summary.txt 2>&1 || true
coverage=$(grep -oE 'line: [0-9.]+' /tmp/cov_summary.txt | grep -oE '[0-9.]+' || echo "0")

if (( $(echo "$coverage < 80" | bc -l) )); then
    echo "ERROR: Coverage ${coverage}% is below 80%"
    ERROR_EXIT=1
elif (( $(echo "$coverage < 90" | bc -l) )); then
    echo "WARNING: Coverage ${coverage}% is below 90%"
    WARN_EXIT=1
else
    echo "Coverage: ${coverage}%"
fi

# Summary
if [ "$ERROR_EXIT" -eq 1 ]; then
    echo "=== FAILED: Quality errors found ==="
    exit 1
elif [ "$WARN_EXIT" -eq 1 ]; then
    echo "=== PASSED with warnings ==="
    exit 0
else
    echo "=== PASSED: All checks clean ==="
    exit 0
fi
```

### 0.4 Pre-commit Hook
Create `.git/hooks/pre-commit`:
```bash
#!/bin/bash
exec ./scripts/check-quality.sh
```

### 0.5 Implement Menu Bar Icon + Empty Panel (TDD)
- [ ] Write test: `tray_icon_created_successfully`
- [ ] Write test: `panel_opens_on_click`
- [ ] Write test: `panel_closes_on_focus_loss`
- [ ] Implement `main.rs`: tray icon setup, event loop
- [ ] Implement empty egui panel (400x500, dark background)
- [ ] Verify icon appears in menu bar
- [ ] Verify click opens panel below icon
- [ ] All tests pass, clippy clean, fmt clean, coverage >= 80%

### 0.6 Quality Gate Check
- [ ] `./scripts/check-quality.sh` passes
- [ ] No errors (complexity <= 50, function <= 100 lines, file <= 1000 lines, coverage >= 80%)
- [ ] Warnings acceptable (function > 80 lines, file > 750 lines, coverage < 90%)

**Milestone 0 Complete:** Menu bar icon visible, empty panel opens on click.

---

## Phase 1: Project Structure & Full Dependencies

### 1.1 Full Cargo.toml
```toml
[package]
name = "personal-agent"
version = "0.1.0"
edition = "2021"

[dependencies]
# GUI
eframe = "0.29"
egui = "0.29"
tray-icon = "0.19"
muda = "0.15"
global-hotkey = "0.6"
image = "0.25"

# LLM
serdes-ai = { git = "https://github.com/janfeddersen-wq/serdesAI" }

# Async
tokio = { version = "1", features = ["full"] }
futures = "0.3"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# HTTP (for models.dev)
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }

# Utils
chrono = { version = "0.4", features = ["serde"] }
dirs = "5"
uuid = { version = "1", features = ["v4", "serde"] }
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"

[dev-dependencies]
tempfile = "3"
rstest = "0.18"
mockall = "0.12"
tokio-test = "0.4"

[lints.rust]
unsafe_code = "forbid"

[lints.clippy]
all = "deny"
pedantic = "warn"
nursery = "warn"
cognitive_complexity = "warn"
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
│   ├── components/
│   │   ├── mod.rs
│   │   ├── message.rs
│   │   ├── thinking_block.rs
│   │   ├── input_field.rs
│   │   └── toggle.rs
│   └── theme.rs
├── llm/
│   ├── mod.rs
│   ├── client.rs
│   ├── streaming.rs
│   └── context.rs
├── models/
│   ├── mod.rs
│   ├── config.rs
│   ├── profile.rs
│   ├── conversation.rs
│   └── registry.rs
├── storage/
│   ├── mod.rs
│   ├── config.rs
│   └── conversations.rs
└── error.rs
```

---

## Phase 2: Core Infrastructure (TDD)

### 2.1 Configuration System
- [ ] Write tests for config load/save/default
- [ ] Define `Config` struct with serde
- [ ] Implement config loading from `~/Library/Application Support/PersonalAgent/config.json`
- [ ] Implement config saving
- [ ] Create default config on first run
- [ ] File permissions (600)

### 2.2 Model Profiles
- [ ] Write tests for profile CRUD
- [ ] Define `ModelProfile` struct
- [ ] Fields: id, name, provider_id, model_id, base_url, auth (key/keyfile), parameters
- [ ] Parameters: temperature, top_p, max_tokens, thinking_budget, enable_thinking, show_thinking
- [ ] CRUD operations for profiles

### 2.3 Conversation Storage
- [ ] Write tests for conversation save/load/list
- [ ] Define `Conversation` struct
- [ ] Define `Message` struct (role, content, thinking_content, timestamp)
- [ ] Implement save (append mode for each message)
- [ ] Implement load
- [ ] Implement list all conversations
- [ ] Timestamp-based naming: `YYYYMMDDHHMMSSmmm.json`

### 2.4 models.dev Integration
- [ ] Write tests with mocked HTTP responses
- [ ] Fetch `https://models.dev/api.json`
- [ ] Parse into `ModelRegistry` struct
- [ ] Cache to `~/Library/Application Support/PersonalAgent/cache/models.json`
- [ ] Manual refresh via settings button
- [ ] Provider/model lookup helpers

---

## Phase 3: Menu Bar & Window (already done in Phase 0, expand here)

### 3.1 Global Hotkey
- [ ] Write tests for hotkey registration
- [ ] Register default hotkey (Cmd+Shift+Space)
- [ ] Handle hotkey to toggle panel visibility
- [ ] Make hotkey configurable via settings

### 3.2 Event Loop Integration
- [ ] Integrate tray-icon event loop with egui
- [ ] Handle app lifecycle (show, hide, quit)

---

## Phase 4: UI Implementation (TDD where practical)

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

## Phase 5: LLM Integration (TDD)

### 5.1 SerdesAI Client Wrapper
- [ ] Write tests with mocked SerdesAI responses
- [ ] Create client from profile (provider, model, auth)
- [ ] Handle keyfile reading
- [ ] Apply model parameters

### 5.2 Chat Execution
- [ ] Write tests for stream event processing
- [ ] Build message history from conversation
- [ ] Call `agent.run_stream()` with `StreamConfig { emit_thinking: show_thinking, ... }`
- [ ] Process stream events: `TextDelta`, `ThinkingDelta`, `RunComplete`, `Error`
- [ ] Update UI in real-time
- [ ] Append message to conversation file on complete

### 5.3 Context Management (Sandwich Summarization)
- [ ] Write tests for summarization trigger and execution
- [ ] Calculate current token count (estimate from chars)
- [ ] Check against 80% of model's context limit
- [ ] If exceeded:
  - [ ] Identify top 20% messages (preserve)
  - [ ] Identify bottom 20% messages (preserve)
  - [ ] Send middle 60% to LLM with summarization prompt
  - [ ] Target: 50% of original length
  - [ ] Replace middle with summary
  - [ ] Notify user (toast)

### 5.4 Error Handling
- [ ] Write tests for error scenarios
- [ ] Catch API errors (rate limit, auth, network)
- [ ] Show toast notification
- [ ] Show inline error in chat
- [ ] Retry logic for transient errors

---

## Phase 6: Integration Testing

### 6.1 Test Providers
- [ ] Test with zai profile (GLM 4.7 via Anthropic provider)
- [ ] Test with synthetic profile (K2-thinking)
- [ ] Test conversation save/load
- [ ] Test profile switching
- [ ] Test thinking block toggle
- [ ] Test streaming

### 6.2 Edge Cases
- [ ] Very long messages (scroll, truncation)
- [ ] Rapid send (debounce/disable during streaming)
- [ ] Empty conversation list
- [ ] First run experience
- [ ] No profiles configured - prompt to create
- [ ] Network offline - show connection error

### 6.3 Build & Run
- [ ] `cargo build --release`
- [ ] Verify menu bar icon appears
- [ ] Verify panel drops down correctly
- [ ] Verify all flows work
- [ ] All quality gates pass

---

## Milestones

| Milestone | Target | Description |
|-----------|--------|-------------|
| M0 | Day 1 | Menu bar icon + empty panel (quality gates in place) |
| M1 | Day 2-3 | Config/storage working, UI shell navigable |
| M2 | Day 4-5 | models.dev integration, profile CRUD |
| M3 | Day 6-7 | Basic chat with zai provider |
| M4 | Day 8-9 | Streaming, thinking blocks, synthetic provider |
| M5 | Day 10-11 | Sandwich summarization, polish |
| M6 | Day 12 | Final testing, documentation |

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
| Security | `cargo audit` | - | Any vulnerability | CI |

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
- OpenAI/Anthropic OAuth integration (for paid tiers)

---

## Appendix A: Key Technical Details

### A.1 models.dev API Structure
```json
{
  "provider_id": {
    "id": "provider_id",
    "env": ["ENV_VAR_NAME"],
    "api": "https://api.example.com/v1",
    "name": "Provider Display Name",
    "doc": "https://docs.example.com",
    "models": {
      "model_id": {
        "id": "model_id",
        "name": "Model Display Name",
        "family": "model-family",
        "tool_call": true,
        "reasoning": true,
        "structured_output": true,
        "temperature": true,
        "knowledge": "2024-08",
        "release_date": "2025-01-01",
        "modalities": { "input": ["text"], "output": ["text"] },
        "cost": { "input": 1.0, "output": 5.0 },
        "limit": { "context": 200000, "output": 8192 }
      }
    }
  }
}
```

### A.2 Config File Structure
Location: `~/Library/Application Support/PersonalAgent/config.json`
```json
{
  "version": "1.0",
  "theme": "dark",
  "global_hotkey": "Cmd+Shift+Space",
  "default_profile": "uuid",
  "context_management": {
    "trigger_threshold": 0.80,
    "preserve_top": 0.20,
    "preserve_bottom": 0.20,
    "summary_target_ratio": 0.50
  },
  "profiles": [
    {
      "id": "uuid",
      "name": "Profile Name",
      "provider_id": "anthropic",
      "model_id": "claude-sonnet-4",
      "base_url": "https://api.anthropic.com/v1",
      "auth": { "type": "key", "value": "sk-..." },
      "parameters": {
        "temperature": 0.7,
        "top_p": 0.95,
        "max_tokens": 4096,
        "thinking_budget": 10000,
        "enable_thinking": true,
        "show_thinking": true
      }
    }
  ]
}
```

### A.3 Conversation File Structure
Location: `~/Library/Application Support/PersonalAgent/conversations/YYYYMMDDHHMMSSmmm.json`
```json
{
  "id": "YYYYMMDDHHMMSSmmm",
  "name": "Optional custom name",
  "created_at": "2026-01-13T14:30:00Z",
  "updated_at": "2026-01-13T15:45:00Z",
  "profile_id": "uuid",
  "messages": [
    {
      "role": "user",
      "content": "Hello",
      "timestamp": "2026-01-13T14:30:00Z"
    },
    {
      "role": "assistant",
      "content": "Hi there!",
      "thinking_content": "User greeted me, I should respond friendly...",
      "timestamp": "2026-01-13T14:30:05Z"
    }
  ]
}
```

### A.4 UI Theme Colors (Dark Mode)
```rust
// From ui-mockup-v2.html CSS
const BG_DARKEST: Color32 = Color32::from_rgb(13, 13, 13);    // #0d0d0d
const BG_DARKER: Color32 = Color32::from_rgb(26, 26, 26);     // #1a1a1a
const BG_DARK: Color32 = Color32::from_rgb(36, 36, 36);       // #242424
const BG_MEDIUM: Color32 = Color32::from_rgb(46, 46, 46);     // #2e2e2e
const BG_LIGHT: Color32 = Color32::from_rgb(58, 58, 58);      // #3a3a3a
const BG_LIGHTER: Color32 = Color32::from_rgb(68, 68, 68);    // #444444
const TEXT_PRIMARY: Color32 = Color32::from_rgb(229, 229, 229); // #e5e5e5
const TEXT_SECONDARY: Color32 = Color32::from_rgb(136, 136, 136); // #888888
const TEXT_MUTED: Color32 = Color32::from_rgb(85, 85, 85);    // #555555
const BORDER: Color32 = Color32::from_rgb(51, 51, 51);        // #333333
const ACCENT: Color32 = Color32::from_rgb(110, 110, 110);     // #6e6e6e
const SUCCESS: Color32 = Color32::from_rgb(74, 159, 74);      // #4a9f4a
const DANGER: Color32 = Color32::from_rgb(168, 64, 64);       // #a84040
```

### A.5 Panel Dimensions
- Width: 400px
- Height: 500px
- Positioned below menu bar icon with arrow pointing up
