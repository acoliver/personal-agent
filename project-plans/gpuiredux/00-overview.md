# GPUI Redux: Implementation Plan

**Plan ID:** PLAN-20250130-GPUIREDUX
**Total Phases:** 24 (12 implementation phases + 12 verification phases)

## Development Approach

**Test-First Development:**
1. Each implementation phase writes tests FIRST
2. Tests must fail initially (red)
3. Implementation makes tests pass (green)
4. Refactor while keeping tests green

**Verification Phases:**
- Every implementation phase has a verification phase (suffix 'a')
- Verification checks: tests pass, events emitted, commands handled, visual match

## Phase Overview

| Phase | Title | Depends On | Primary Deliverable |
|-------|-------|------------|---------------------|
| P01 | Foundation & Navigation | - | Navigation system, view container |
| P01a | Foundation Verification | P01 | Tests pass, navigation works |
| P02 | Component Library | P01a | Atomic components (Button, TextField, etc.) |
| P02a | Component Verification | P02 | Component tests pass |
| P03 | Chat View - Layout | P02a | Chat UI structure (no logic) |
| P03a | Chat Layout Verification | P03 | Visual matches mockup |
| P04 | Chat View - Events & Commands | P03a | Full chat interactivity |
| P04a | Chat Events Verification | P04 | All events/commands work |
| P05 | History View | P04a | Complete history view |
| P05a | History Verification | P05 | Load/delete work |
| P06 | Settings View - Layout | P05a | Settings UI structure |
| P06a | Settings Layout Verification | P06 | Visual matches mockup |
| P07 | Settings View - Profiles | P06a | Profile list management |
| P07a | Settings Profiles Verification | P07 | Profile CRUD works |
| P08 | Settings View - MCPs | P07a | MCP list management |
| P08a | Settings MCPs Verification | P08 | MCP toggle/CRUD works |
| P09 | Model Selector View | P08a | Complete model selector |
| P09a | Model Selector Verification | P09 | Search/filter/select work |
| P10 | Profile Editor View | P09a | Complete profile editor |
| P10a | Profile Editor Verification | P10 | Save/validate works |
| P11 | MCP Views | P10a | MCP Add + Configure views |
| P11a | MCP Views Verification | P11 | Full MCP flow works |
| P12 | Integration & Polish | P11a | Full E2E, visual polish |
| P12a | Final Verification | P12 | Everything works together |

## Phase Details

### P01: Foundation & Navigation

**Goal:** Navigation system and main view container

**Tests First:**
```rust
#[test]
fn test_navigation_push_view() {
    // Navigate to Settings should push to stack
}

#[test]
fn test_navigation_pop_view() {
    // NavigateBack should pop stack
}

#[test]
fn test_main_panel_renders_current_view() {
    // MainPanel shows view matching current navigation
}
```

**Deliverables:**
- `src/ui_gpui/navigation.rs` - NavigationState, ViewId enum
- `src/ui_gpui/views/main_panel.rs` - View container with navigation
- `src/ui_gpui/app.rs` - GPUI app setup with ViewCommand receiver

### P02: Component Library

**Goal:** Reusable atomic components

**Tests First:**
```rust
#[test]
fn test_button_emits_click() {
    // Button click triggers callback
}

#[test]
fn test_text_field_captures_input() {
    // Typing updates text field state
}

#[test]
fn test_dropdown_shows_options() {
    // Dropdown click shows overlay
}
```

**Deliverables:**
- `src/ui_gpui/components/button.rs`
- `src/ui_gpui/components/text_field.rs`
- `src/ui_gpui/components/dropdown.rs`
- `src/ui_gpui/components/toggle.rs`
- `src/ui_gpui/components/list.rs`
- `src/ui_gpui/components/mod.rs`

### P03: Chat View - Layout

**Goal:** Chat UI matching mockup (static, no events)

**Tests First:**
```rust
#[test]
fn test_chat_view_has_top_bar() {
    // Top bar with icon, title, buttons
}

#[test]
fn test_chat_view_has_title_bar() {
    // Title bar with dropdown, model label
}

#[test]
fn test_chat_view_has_input_bar() {
    // Input field, send button, stop button
}

#[test]
fn test_message_bubble_user_right_aligned() {
    // User messages right with green bg
}

#[test]
fn test_message_bubble_assistant_left_aligned() {
    // Assistant messages left with dark bg
}
```

**Deliverables:**
- `src/ui_gpui/views/chat_view.rs` - Layout only
- `src/ui_gpui/components/message_bubble.rs`
- `src/ui_gpui/components/thinking_block.rs`
- `src/ui_gpui/components/top_bar.rs`

### P04: Chat View - Events & Commands

**Goal:** Full chat interactivity

**Tests First:**
```rust
#[test]
fn test_send_button_emits_send_message() {
    // Click Send -> UserEvent::SendMessage
}

#[test]
fn test_enter_key_sends_message() {
    // Enter in input -> UserEvent::SendMessage
}

#[test]
fn test_stop_button_emits_stop_streaming() {
    // Click Stop -> UserEvent::StopStreaming
}

#[test]
fn test_text_delta_command_updates_bubble() {
    // ViewCommand::TextDelta -> text appended
}

#[test]
fn test_stream_started_command_shows_cursor() {
    // ViewCommand::StreamStarted -> cursor visible
}
```

**Deliverables:**
- Chat view event emission
- Chat view command handling
- Streaming state management

### P05: History View

**Goal:** Complete history view

**Tests First:**
```rust
#[test]
fn test_history_shows_conversation_cards() {
    // Conversations rendered as cards
}

#[test]
fn test_load_button_emits_select_conversation() {
    // Click Load -> UserEvent::SelectConversation
}

#[test]
fn test_delete_button_emits_delete_conversation() {
    // Click Delete -> UserEvent::DeleteConversation
}

#[test]
fn test_empty_state_when_no_conversations() {
    // Empty list shows empty state message
}
```

**Deliverables:**
- `src/ui_gpui/views/history_view.rs`
- `src/ui_gpui/components/conversation_card.rs`

### P06: Settings View - Layout

**Goal:** Settings UI structure

**Tests First:**
```rust
#[test]
fn test_settings_has_profiles_section() {
    // Profiles list with toolbar
}

#[test]
fn test_settings_has_mcp_section() {
    // MCP list with status indicators
}

#[test]
fn test_settings_has_hotkey_field() {
    // Global hotkey input
}
```

**Deliverables:**
- `src/ui_gpui/views/settings_view.rs` - Layout only
- `src/ui_gpui/components/profile_row.rs`
- `src/ui_gpui/components/mcp_row.rs`

### P07: Settings View - Profiles

**Goal:** Profile list management

**Tests First:**
```rust
#[test]
fn test_profile_click_emits_select() {
    // Click row -> UserEvent::SelectProfile
}

#[test]
fn test_add_profile_navigates_to_selector() {
    // Click + -> UserEvent::Navigate to ModelSelector
}

#[test]
fn test_edit_profile_navigates_to_editor() {
    // Click Edit -> UserEvent::EditProfile
}

#[test]
fn test_delete_profile_emits_delete() {
    // Click - -> UserEvent::DeleteProfile
}
```

**Deliverables:**
- Profile selection handling
- Profile toolbar actions
- Profile list updates from ViewCommand

### P08: Settings View - MCPs

**Goal:** MCP list management

**Tests First:**
```rust
#[test]
fn test_mcp_toggle_emits_toggle() {
    // Toggle switch -> UserEvent::ToggleMcp
}

#[test]
fn test_mcp_status_indicator_colors() {
    // Green=running, gray=stopped, red=error
}

#[test]
fn test_add_mcp_navigates_to_add_view() {
    // Click + -> UserEvent::Navigate to McpAdd
}
```

**Deliverables:**
- MCP toggle handling
- MCP status display
- MCP toolbar actions

### P09: Model Selector View

**Goal:** Complete model selector

**Tests First:**
```rust
#[test]
fn test_search_filters_models() {
    // Type in search -> filtered list
}

#[test]
fn test_provider_dropdown_filters() {
    // Select provider -> only that provider's models
}

#[test]
fn test_model_click_emits_select() {
    // Click row -> UserEvent::SelectModel
}

#[test]
fn test_capability_toggles_filter() {
    // Check Reasoning -> only reasoning models
}
```

**Deliverables:**
- `src/ui_gpui/views/model_selector_view.rs`
- `src/ui_gpui/components/model_row.rs`
- Search/filter logic

### P10: Profile Editor View

**Goal:** Complete profile editor

**Tests First:**
```rust
#[test]
fn test_save_disabled_when_name_empty() {
    // Empty name -> Save disabled
}

#[test]
fn test_auth_method_shows_correct_fields() {
    // API Key -> shows key field
    // Key File -> shows file field
}

#[test]
fn test_save_emits_save_profile() {
    // Click Save -> UserEvent::SaveProfile
}

#[test]
fn test_temperature_stepper_updates_value() {
    // Click +/- -> value changes
}
```

**Deliverables:**
- `src/ui_gpui/views/profile_editor_view.rs`
- Form validation
- Conditional field display

### P11: MCP Views

**Goal:** MCP Add + Configure views

**Tests First:**
```rust
#[test]
fn test_manual_entry_enables_next() {
    // Type in manual -> Next enabled
}

#[test]
fn test_search_shows_results() {
    // Search -> results list populated
}

#[test]
fn test_configure_save_emits_config() {
    // Click Save -> UserEvent::SaveMcpConfig
}
```

**Deliverables:**
- `src/ui_gpui/views/mcp_add_view.rs`
- `src/ui_gpui/views/mcp_configure_view.rs`
- Dynamic config fields

### P12: Integration & Polish

**Goal:** Full E2E testing, visual polish

**Tests First:**
```rust
#[test]
fn test_full_chat_flow() {
    // Type -> Send -> Stream -> Complete
}

#[test]
fn test_full_profile_creation_flow() {
    // Settings -> + -> Select Model -> Edit -> Save
}

#[test]
fn test_navigation_stack_works() {
    // Navigate deep -> back -> back -> correct views
}
```

**Deliverables:**
- E2E integration tests
- Visual polish (spacing, colors, animations)
- Performance optimization

## File Structure

```
src/ui_gpui/
  app.rs                    # GPUI Application setup
  navigation.rs             # Navigation state
  theme.rs                  # Colors, fonts, spacing (exists)
  bridge/                   # flume bridge (exists)
    mod.rs
    gpui_bridge.rs
    view_command_sink.rs
    forwarder.rs
  components/
    mod.rs
    button.rs
    text_field.rs
    secure_text_field.rs
    text_area.rs
    dropdown.rs
    toggle.rs
    checkbox.rs
    stepper.rs
    list.rs
    list_row.rs
    badge.rs
    divider.rs
    spinner.rs
    top_bar.rs
    message_bubble.rs
    thinking_block.rs
    conversation_card.rs
    profile_row.rs
    mcp_row.rs
    model_row.rs
    form_field.rs
    array_field.rs
  views/
    mod.rs
    main_panel.rs
    chat_view.rs
    history_view.rs
    settings_view.rs
    model_selector_view.rs
    profile_editor_view.rs
    mcp_add_view.rs
    mcp_configure_view.rs
```

## Verification Checklist

Each verification phase confirms:

- [ ] All tests pass (`cargo test`)
- [ ] No compiler warnings
- [ ] Events emitted correctly (checked via test mocks)
- [ ] Commands handled correctly (checked via state assertions)
- [ ] Visual matches mockup (manual inspection)
- [ ] Navigation works (push/pop stack)
- [ ] No regressions in previous phases
