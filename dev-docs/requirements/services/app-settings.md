# App Settings Service Requirements

The App Settings Service manages application-wide settings and state that persist across sessions. This is the single source of truth for global app configuration.

---

## Responsibilities

- Store and retrieve global app settings
- Manage default profile selection
- Track current conversation selection
- Persist window state
- Store hotkey configuration
- Auto-save on changes
- Load settings on app startup

---

## Service Interface

```rust
pub trait AppSettingsService: Send + Sync {
    /// Get all settings
    fn get(&self) -> Result<AppSettings>;
    
    /// Update settings (partial update, merges with existing)
    fn update(&self, updates: &AppSettingsUpdate) -> Result<AppSettings>;
    
    /// Get the default profile ID
    fn get_default_profile_id(&self) -> Result<Option<Uuid>>;
    
    /// Set the default profile ID
    fn set_default_profile_id(&self, id: Uuid) -> Result<()>;
    
    /// Clear the default profile (e.g., when profile deleted)
    fn clear_default_profile(&self) -> Result<()>;
    
    /// Get the current conversation ID
    fn get_current_conversation_id(&self) -> Result<Option<Uuid>>;
    
    /// Set the current conversation ID
    fn set_current_conversation_id(&self, id: Uuid) -> Result<()>;
    
    /// Clear the current conversation (e.g., when conversation deleted)
    fn clear_current_conversation(&self) -> Result<()>;
    
    /// Get hotkey configuration
    fn get_hotkey(&self) -> Result<Option<HotkeyConfig>>;
    
    /// Set hotkey configuration
    fn set_hotkey(&self, hotkey: &HotkeyConfig) -> Result<()>;
    
    /// Get window state
    fn get_window_state(&self) -> Result<Option<WindowState>>;
    
    /// Set window state
    fn set_window_state(&self, state: &WindowState) -> Result<()>;
}
```

---

## Data Model

### App Settings

```rust
pub struct AppSettings {
    /// Schema version for migrations
    pub version: u32,
    
    /// ID of the default (selected) profile
    pub default_profile_id: Option<Uuid>,
    
    /// ID of the currently selected conversation
    pub current_conversation_id: Option<Uuid>,
    
    /// Global hotkey configuration
    pub hotkey: Option<HotkeyConfig>,
    
    /// Window position and size
    pub window: Option<WindowState>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: 1,
            default_profile_id: None,
            current_conversation_id: None,
            hotkey: Some(HotkeyConfig::default()),
            window: None,
        }
    }
}
```

### App Settings Update (Partial)

```rust
pub struct AppSettingsUpdate {
    pub default_profile_id: Option<Option<Uuid>>,  // Some(None) clears, None leaves unchanged
    pub current_conversation_id: Option<Option<Uuid>>,
    pub hotkey: Option<Option<HotkeyConfig>>,
    pub window: Option<Option<WindowState>>,
}
```

### Hotkey Config

```rust
pub struct HotkeyConfig {
    /// Key code (e.g., "Space", "A", "1")
    pub key: String,
    
    /// Modifier flags
    pub modifiers: HotkeyModifiers,
    
    /// Whether hotkey is enabled
    pub enabled: bool,
}

pub struct HotkeyModifiers {
    pub command: bool,
    pub shift: bool,
    pub option: bool,
    pub control: bool,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            key: "Space".to_string(),
            modifiers: HotkeyModifiers {
                command: true,
                shift: true,
                option: false,
                control: false,
            },
            enabled: true,
        }
    }
}
```

### Window State

```rust
pub struct WindowState {
    /// Window X position (screen coordinates)
    pub x: f64,
    
    /// Window Y position (screen coordinates)  
    pub y: f64,
    
    /// Window width
    pub width: f64,
    
    /// Window height
    pub height: f64,
    
    /// Which screen the window is on (for multi-monitor)
    pub screen_id: Option<u32>,
}
```

---

## Storage Format

### File Location

```
~/Library/Application Support/PersonalAgent/settings.json
```

### JSON Format

```json
{
  "version": 1,
  "default_profile_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "current_conversation_id": "b2c3d4e5-f6a7-8901-bcde-f12345678901",
  "hotkey": {
    "key": "Space",
    "modifiers": {
      "command": true,
      "shift": true,
      "option": false,
      "control": false
    },
    "enabled": true
  },
  "window": {
    "x": 100.0,
    "y": 200.0,
    "width": 400.0,
    "height": 500.0,
    "screen_id": 1
  }
}
```

---

## Operations

### Load Settings (App Startup)

| Step | Action |
|------|--------|
| 1 | Check if settings.json exists |
| 2a | If exists: read and parse |
| 2b | If not exists: create with defaults |
| 3 | Validate version, migrate if needed |
| 4 | Return AppSettings |

```rust
fn load(&self) -> Result<AppSettings> {
    let path = self.settings_path();
    
    if !path.exists() {
        let settings = AppSettings::default();
        self.save(&settings)?;
        return Ok(settings);
    }
    
    let content = std::fs::read_to_string(&path)?;
    let mut settings: AppSettings = serde_json::from_str(&content)?;
    
    // Migrate if needed
    settings = self.migrate(settings)?;
    
    Ok(settings)
}
```

### Save Settings

| Step | Action |
|------|--------|
| 1 | Serialize to JSON (pretty) |
| 2 | Write to temp file |
| 3 | Atomic rename to settings.json |

```rust
fn save(&self, settings: &AppSettings) -> Result<()> {
    let path = self.settings_path();
    let json = serde_json::to_string_pretty(settings)?;
    
    // Atomic write
    let temp_path = path.with_extension("json.tmp");
    std::fs::write(&temp_path, &json)?;
    std::fs::rename(&temp_path, &path)?;
    
    Ok(())
}
```

### Set Default Profile

| Step | Action |
|------|--------|
| 1 | Load current settings |
| 2 | Update default_profile_id |
| 3 | Save settings |

```rust
fn set_default_profile_id(&self, id: Uuid) -> Result<()> {
    let mut settings = self.get()?;
    settings.default_profile_id = Some(id);
    self.save(&settings)?;
    Ok(())
}
```

### Clear Default Profile (On Profile Deletion)

```rust
fn clear_default_profile(&self) -> Result<()> {
    let mut settings = self.get()?;
    settings.default_profile_id = None;
    self.save(&settings)?;
    Ok(())
}
```

### Set Current Conversation

```rust
fn set_current_conversation_id(&self, id: Uuid) -> Result<()> {
    let mut settings = self.get()?;
    settings.current_conversation_id = Some(id);
    self.save(&settings)?;
    Ok(())
}
```

---

## Migration

### Version History

| Version | Changes |
|---------|---------|
| 1 | Initial schema |

### Migration Logic

```rust
fn migrate(&self, mut settings: AppSettings) -> Result<AppSettings> {
    // Future migrations go here
    // if settings.version < 2 {
    //     // migrate v1 -> v2
    //     settings.version = 2;
    // }
    
    Ok(settings)
}
```

---

## Integration with Other Services

### ProfileService Changes

**Remove from ProfileService:**
- `is_default: bool` from `ModelProfile`
- `get_default()` method
- `set_default(id)` method

**ProfileService now:**
- Pure CRUD for profiles
- No knowledge of which profile is "default"

**New pattern:**
```rust
// Old: ProfileService.get_default()
// New: 
let profile_id = app_settings_service.get_default_profile_id()?;
let profile = profile_service.get(profile_id)?;
```

### ConversationService

No changes needed. ConversationService remains pure storage.

**New pattern for current conversation:**
```rust
// On app startup or conversation switch:
let conv_id = app_settings_service.get_current_conversation_id()?;
if let Some(id) = conv_id {
    let conversation = conversation_service.load(id)?;
}
```

### Cleanup on Deletion

When a profile or conversation is deleted, the caller must clear the reference:

```rust
// In ProfileService.delete() or wherever deletion is coordinated:
fn delete_profile(&self, id: Uuid) -> Result<()> {
    // Check if this is the default profile
    if self.app_settings_service.get_default_profile_id()? == Some(id) {
        self.app_settings_service.clear_default_profile()?;
    }
    
    self.profile_service.delete(id)?;
    Ok(())
}

// Similarly for conversations
fn delete_conversation(&self, id: Uuid) -> Result<()> {
    if self.app_settings_service.get_current_conversation_id()? == Some(id) {
        self.app_settings_service.clear_current_conversation()?;
    }
    
    self.conversation_service.delete(id)?;
    Ok(())
}
```

---

## UI Integration

### App Startup

| Action | Service Call |
|--------|--------------|
| Load settings | `get()` |
| Get default profile | `get_default_profile_id()` then `ProfileService.get()` |
| Get current conversation | `get_current_conversation_id()` then `ConversationService.load()` |
| Restore window position | `get_window_state()` |
| Register hotkey | `get_hotkey()` |

### Chat View

| Action | Service Call |
|--------|--------------|
| Switch conversation | `set_current_conversation_id(id)` |
| Create new conversation | `set_current_conversation_id(new_id)` |

### Settings View

| Action | Service Call |
|--------|--------------|
| Set default profile | `set_default_profile_id(id)` |
| Configure hotkey | `set_hotkey(config)` |

### Window Management

| Action | Service Call |
|--------|--------------|
| Window moved/resized | `set_window_state(state)` |
| App closing | `set_window_state(current_state)` |

---

## Event Emissions

AppSettingsService emits events via the EventBus when settings change.

| Operation | Event Emitted |
|-----------|---------------|
| set_default_profile_id() | `ProfileEvent::DefaultChanged { profile_id }` |
| clear_default_profile() | `ProfileEvent::DefaultChanged { profile_id: None }` |
| set_current_conversation_id() | `ConversationEvent::Activated { id }` |
| clear_current_conversation() | `ConversationEvent::Deactivated` |
| set_hotkey() | `SystemEvent::HotkeyChanged { hotkey }` |

**Note:** `ProfileEvent::DefaultChanged` is emitted here, not by ProfileService, because AppSettingsService owns the "default profile" concept.

**Integration with EventBus:**

```rust
fn set_default_profile_id(&self, id: Uuid) -> Result<()> {
    let mut settings = self.get()?;
    settings.default_profile_id = Some(id);
    self.save(&settings)?;
    
    self.event_bus.emit(ProfileEvent::DefaultChanged {
        profile_id: Some(id),
    }.into());
    
    Ok(())
}

fn clear_default_profile(&self) -> Result<()> {
    let mut settings = self.get()?;
    settings.default_profile_id = None;
    self.save(&settings)?;
    
    self.event_bus.emit(ProfileEvent::DefaultChanged {
        profile_id: None,
    }.into());
    
    Ok(())
}

fn set_current_conversation_id(&self, id: Uuid) -> Result<()> {
    let mut settings = self.get()?;
    settings.current_conversation_id = Some(id);
    self.save(&settings)?;
    
    self.event_bus.emit(ConversationEvent::Activated { id }.into());
    
    Ok(())
}
```

---

## Test Requirements

| ID | Test |
|----|------|
| AS-T1 | get() returns defaults when file missing |
| AS-T2 | get() creates file with defaults |
| AS-T3 | set_default_profile_id() persists |
| AS-T4 | clear_default_profile() sets to None |
| AS-T5 | set_current_conversation_id() persists |
| AS-T6 | clear_current_conversation() sets to None |
| AS-T7 | set_hotkey() persists |
| AS-T8 | set_window_state() persists |
| AS-T9 | Atomic write prevents corruption |
| AS-T10 | Migration runs on old version |
| AS-T11 | Invalid JSON returns error |
| AS-T12 | update() merges partial updates |
| AS-T13 | set_default_profile_id() emits ProfileEvent::DefaultChanged |
| AS-T14 | clear_default_profile() emits ProfileEvent::DefaultChanged with None |
| AS-T15 | set_current_conversation_id() emits ConversationEvent::Activated |
| AS-T16 | set_hotkey() emits SystemEvent::HotkeyChanged |
