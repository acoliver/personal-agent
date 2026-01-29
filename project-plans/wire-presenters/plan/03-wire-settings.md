# Phase 03: Wire EventBus to SettingsPresenter

**Phase ID**: P03
**Type**: Implementation
**Status**: Pending
**Prerequisites**: P02a completion marker exists with PASS verdict

## Objective

Connect `SettingsPresenter` to the event bus to receive and react to profile and MCP lifecycle events. Per dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md section "Presentation Layer Isolation", the presenter must subscribe to `AppEvent` for settings events and emit `ViewCommand` to update the settings view.

## Event Mapping

### SettingsPresenter Subscriptions

Per `src/events/types.rs`, SettingsPresenter must handle:

| Event Type | Rust Enum | ViewCommand Response |
|------------|-----------|---------------------|
| **Profile Events** |
| User selects profile | `AppEvent::User(UserEvent::SelectProfile { id })` | Update active profile indicator |
| User creates profile | `AppEvent::User(UserEvent::CreateProfile)` | Show profile editor |
| User edits profile | `AppEvent::User(UserEvent::EditProfile { id })` | Show profile editor with data |
| User saves profile | `AppEvent::User(UserEvent::SaveProfile { profile })` | Update profile list item |
| User deletes profile | `AppEvent::User(UserEvent::DeleteProfile { id })` | Show confirmation dialog |
| User confirms delete | `AppEvent::User(UserEvent::ConfirmDeleteProfile { id })` | Remove from list |
| User tests connection | `AppEvent::User(UserEvent::TestProfileConnection { id })` | Show test status |
| Profile created | `AppEvent::Profile(ProfileEvent::Created { id, name })` | Append to list |
| Profile updated | `AppEvent::Profile(ProfileEvent::Updated { id, name })` | Update list item |
| Profile deleted | `AppEvent::Profile(ProfileEvent::Deleted { id, name })` | Remove from list |
| Default changed | `AppEvent::Profile(ProfileEvent::DefaultChanged { profile_id })` | Update default indicator |
| Test started | `AppEvent::Profile(ProfileEvent::TestStarted { id })` | Show loading spinner |
| Test completed | `AppEvent::Profile(ProfileEvent::TestCompleted { id, success, response_time_ms, error })` | Show test result |
| Validation failed | `AppEvent::Profile(ProfileEvent::ValidationFailed { id, errors })` | Show validation errors |
| **MCP Events** |
| User toggles MCP | `AppEvent::User(UserEvent::ToggleMcp { id, enabled })` | Update toggle state |
| User adds MCP | `AppEvent::User(UserEvent::AddMcp)` | Show MCP registry search |
| User configures MCP | `AppEvent::User(UserEvent::ConfigureMcp { id })` | Show MCP config form |
| User saves MCP config | `AppEvent::User(UserEvent::SaveMcpConfig { id, config })` | Update MCP item |
| User deletes MCP | `AppEvent::User(UserEvent::DeleteMcp { id })` | Show confirmation |
| User confirms delete | `AppEvent::User(UserEvent::ConfirmDeleteMcp { id })` | Remove from list |
| MCP starting | `AppEvent::Mcp(McpEvent::Starting { id, name })` | Show starting state |
| MCP started | `AppEvent::Mcp(McpEvent::Started { id, name, tools, tool_count })` | Update to running state |
| MCP start failed | `AppEvent::Mcp(McpEvent::StartFailed { id, name, error })` | Show error state |
| MCP stopped | `AppEvent::Mcp(McpEvent::Stopped { id, name })` | Update to stopped state |
| MCP unhealthy | `AppEvent::Mcp(McpEvent::Unhealthy { id, name, error })` | Show unhealthy warning |
| MCP recovered | `AppEvent::Mcp(McpEvent::Recovered { id, name })` | Update to healthy state |
| MCP config saved | `AppEvent::Mcp(McpEvent::ConfigSaved { id })` | Update config display |
| MCP deleted | `AppEvent::Mcp(McpEvent::Deleted { id, name })` | Remove from list |
| **System Events** |
| Config loaded | `AppEvent::System(SystemEvent::ConfigLoaded)` | Refresh settings view |
| Config saved | `AppEvent::System(SystemEvent::ConfigSaved)` | Show save confirmation |
| Models refreshed | `AppEvent::System(SystemEvent::ModelsRegistryRefreshed { provider_count, model_count })` | Update model counts |

## Implementation Requirements

### 1. Add EventBus Subscription

**File**: `src/presentation/settings_presenter.rs`

```rust
use crate::events::{AppEvent, UserEvent, ProfileEvent, McpEvent, SystemEvent};
use crate::events::bus::EventBus;

impl SettingsPresenter {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        let presenter = Self {
            event_bus: event_bus.clone(),
            // ... existing fields
        };

        // Subscribe to relevant events
        presenter.subscribe_to_events();
        presenter
    }

    fn subscribe_to_events(&self) {
        // Subscribe to user profile events
        self.event_bus.subscribe(
            |event| {
                matches!(event,
                    AppEvent::User(UserEvent::SelectProfile { .. }) |
                    AppEvent::User(UserEvent::CreateProfile) |
                    AppEvent::User(UserEvent::EditProfile { .. }) |
                    AppEvent::User(UserEvent::SaveProfile { .. }) |
                    AppEvent::User(UserEvent::DeleteProfile { .. }) |
                    AppEvent::User(UserEvent::ConfirmDeleteProfile { .. }) |
                    AppEvent::User(UserEvent::TestProfileConnection { .. })
                )
            },
            self.clone(),
        );

        // Subscribe to user MCP events
        self.event_bus.subscribe(
            |event| {
                matches!(event,
                    AppEvent::User(UserEvent::ToggleMcp { .. }) |
                    AppEvent::User(UserEvent::AddMcp) |
                    AppEvent::User(UserEvent::ConfigureMcp { .. }) |
                    AppEvent::User(UserEvent::SaveMcpConfig { .. }) |
                    AppEvent::User(UserEvent::DeleteMcp { .. }) |
                    AppEvent::User(UserEvent::ConfirmDeleteMcp { .. })
                )
            },
            self.clone(),
        );

        // Subscribe to profile events
        self.event_bus.subscribe(
            |event| matches!(event, AppEvent::Profile(_)),
            self.clone(),
        );

        // Subscribe to MCP events
        self.event_bus.subscribe(
            |event| matches!(event, AppEvent::Mcp(_)),
            self.clone(),
        );

        // Subscribe to relevant system events
        self.event_bus.subscribe(
            |event| {
                matches!(event,
                    AppEvent::System(SystemEvent::ConfigLoaded) |
                    AppEvent::System(SystemEvent::ConfigSaved) |
                    AppEvent::System(SystemEvent::ModelsRegistryRefreshed { .. })
                )
            },
            self.clone(),
        );
    }
}
```

### 2. Implement EventHandler Trait

```rust
impl EventHandler for SettingsPresenter {
    fn handle_event(&self, event: &AppEvent) {
        match event {
            // Profile user events
            AppEvent::User(UserEvent::SelectProfile { id }) => {
                self.on_select_profile(*id);
            }

            AppEvent::User(UserEvent::CreateProfile) => {
                self.on_create_profile();
            }

            AppEvent::User(UserEvent::EditProfile { id }) => {
                self.on_edit_profile(*id);
            }

            AppEvent::User(UserEvent::SaveProfile { profile }) => {
                self.on_save_profile(profile);
            }

            AppEvent::User(UserEvent::DeleteProfile { id }) => {
                self.on_delete_profile(*id);
            }

            AppEvent::User(UserEvent::ConfirmDeleteProfile { id }) => {
                self.on_confirm_delete_profile(*id);
            }

            AppEvent::User(UserEvent::TestProfileConnection { id }) => {
                self.on_test_profile_connection(*id);
            }

            // MCP user events
            AppEvent::User(UserEvent::ToggleMcp { id, enabled }) => {
                self.on_toggle_mcp(*id, *enabled);
            }

            AppEvent::User(UserEvent::AddMcp) => {
                self.on_add_mcp();
            }

            AppEvent::User(UserEvent::ConfigureMcp { id }) => {
                self.on_configure_mcp(*id);
            }

            AppEvent::User(UserEvent::SaveMcpConfig { id, config }) => {
                self.on_save_mcp_config(*id, config);
            }

            AppEvent::User(UserEvent::DeleteMcp { id }) => {
                self.on_delete_mcp(*id);
            }

            AppEvent::User(UserEvent::ConfirmDeleteMcp { id }) => {
                self.on_confirm_delete_mcp(*id);
            }

            // Profile lifecycle events
            AppEvent::Profile(ProfileEvent::Created { id, name }) => {
                self.on_profile_created(*id, name);
            }

            AppEvent::Profile(ProfileEvent::Updated { id, name }) => {
                self.on_profile_updated(*id, name);
            }

            AppEvent::Profile(ProfileEvent::Deleted { id, name }) => {
                self.on_profile_deleted(*id, name);
            }

            AppEvent::Profile(ProfileEvent::DefaultChanged { profile_id }) => {
                self.on_default_profile_changed(*profile_id);
            }

            AppEvent::Profile(ProfileEvent::TestStarted { id }) => {
                self.on_profile_test_started(*id);
            }

            AppEvent::Profile(ProfileEvent::TestCompleted { id, success, response_time_ms, error }) => {
                self.on_profile_test_completed(*id, *success, *response_time_ms, error);
            }

            AppEvent::Profile(ProfileEvent::ValidationFailed { id, errors }) => {
                self.on_profile_validation_failed(*id, errors);
            }

            // MCP lifecycle events
            AppEvent::Mcp(McpEvent::Starting { id, name }) => {
                self.on_mcp_starting(*id, name);
            }

            AppEvent::Mcp(McpEvent::Started { id, name, tools, tool_count }) => {
                self.on_mcp_started(*id, name, tools, *tool_count);
            }

            AppEvent::Mcp(McpEvent::StartFailed { id, name, error }) => {
                self.on_mcp_start_failed(*id, name, error);
            }

            AppEvent::Mcp(McpEvent::Stopped { id, name }) => {
                self.on_mcp_stopped(*id, name);
            }

            AppEvent::Mcp(McpEvent::Unhealthy { id, name, error }) => {
                self.on_mcp_unhealthy(*id, name, error);
            }

            AppEvent::Mcp(McpEvent::Recovered { id, name }) => {
                self.on_mcp_recovered(*id, name);
            }

            AppEvent::Mcp(McpEvent::ConfigSaved { id }) => {
                self.on_mcp_config_saved(*id);
            }

            AppEvent::Mcp(McpEvent::Deleted { id, name }) => {
                self.on_mcp_deleted(*id, name);
            }

            // System events
            AppEvent::System(SystemEvent::ConfigLoaded) => {
                self.on_config_loaded();
            }

            AppEvent::System(SystemEvent::ConfigSaved) => {
                self.on_config_saved();
            }

            AppEvent::System(SystemEvent::ModelsRegistryRefreshed { provider_count, model_count }) => {
                self.on_models_refreshed(*provider_count, *model_count);
            }

            _ => {} // Ignore other events
        }
    }
}
```

### 3. Emit ViewCommands

Each event handler must emit appropriate `ViewCommand`:

```rust
impl SettingsPresenter {
    fn on_mcp_started(&self, id: Uuid, name: &str, tools: &[String], tool_count: usize) {
        let cmd = ViewCommand::UpdateMcpStatus {
            id,
            status: McpStatus::Running,
            tool_count,
        };
        self.emit_view_command(cmd);
    }

    fn on_mcp_unhealthy(&self, id: Uuid, name: &str, error: &str) {
        let cmd = ViewCommand::ShowMcpError {
            id,
            error: error.to_string(),
        };
        self.emit_view_command(cmd);
    }

    fn on_profile_test_completed(&self, id: Uuid, success: bool, response_time_ms: Option<u64>, error: &Option<String>) {
        let cmd = if success {
            ViewCommand::ShowProfileTestSuccess {
                id,
                response_time_ms,
            }
        } else {
            ViewCommand::ShowProfileTestError {
                id,
                error: error.clone().unwrap_or_else(|| "Unknown error".to_string()),
            }
        };
        self.emit_view_command(cmd);
    }

    // ... other handlers
}
```

## Inputs

### Files to Read
- `src/presentation/settings_presenter.rs` - Current SettingsPresenter implementation
- `src/events/types.rs` - Event enum definitions (ProfileEvent, McpEvent, SystemEvent)
- `src/events/bus.rs` - EventBus API
- `dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md` - Architecture patterns
- `dev-docs/requirements/presentation.md` - SettingsPresenter requirements

### State Required
- EventBus is already implemented and running
- ViewCommand enum exists
- SettingsPresenter struct exists
- P02a passed (HistoryPresenter is working reference)

## Outputs

### Files to Modify
- `src/presentation/settings_presenter.rs` - Add event subscription and handlers

### Evidence Files
- `project-plans/wire-presenters/plan/.completed/P03.md` - Phase completion evidence

## Verification Commands

```bash
# Build check
cargo build --all-targets

# Placeholder detection
grep -rn "unimplemented!\|todo!" src/presentation/settings_presenter.rs
grep -rn "placeholder\|not yet implemented" src/presentation/settings_presenter.rs

# Verify event subscription
grep -c "subscribe_to_events\|handle_event" src/presentation/settings_presenter.rs

# Verify ProfileEvent handlers
grep -c "AppEvent::Profile" src/presentation/settings_presenter.rs

# Verify McpEvent handlers
grep -c "AppEvent::Mcp" src/presentation/settings_presenter.rs

# Verify SystemEvent handlers
grep -c "AppEvent::System.*ConfigLoaded\|AppEvent::System.*ConfigSaved" src/presentation/settings_presenter.rs
```

## PASS/FAIL Criteria

### PASS Conditions
- Exit code 0 from `cargo build --all-targets`
- `grep -rn "unimplemented!" src/presentation/settings_presenter.rs` returns no matches
- `grep -rn "todo!" src/presentation/settings_presenter.rs` returns no matches
- `grep -c "subscribe_to_events" src/presentation/settings_presenter.rs` returns count >= 1
- `grep -c "AppEvent::Profile" src/presentation/settings_presenter.rs` returns count >= 5
- `grep -c "AppEvent::Mcp" src/presentation/settings_presenter.rs` returns count >= 5

### FAIL Conditions
- Build fails with compilation errors
- Any `unimplemented!()`, `todo!()`, or placeholder strings found
- Missing event subscription code
- Missing ProfileEvent or McpEvent handlers

## Related Requirements

- REQ-019.2: Event-driven architecture
- dev-docs/requirements/presentation.md: SettingsPresenter must react to ProfileEvent and McpEvent
- dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md: Presenters must not directly call services
