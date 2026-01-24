# Application Bootstrap & UI Composition Requirements

This document describes how the PersonalAgent application starts up, initializes services, composes the UI, and manages the menubar presence.

---

## Overview

PersonalAgent is a **macOS menubar application** (NSStatusBarButton) that shows a popover when clicked. It has no Dock icon and no main window - just a menubar icon that reveals views.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           macOS Menu Bar                                 │
│  [WiFi] [Battery] [Clock] ... [PersonalAgent Icon ◉] ...                │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ Click
                                    ▼
                    ┌───────────────────────────────┐
                    │         Popover (400x500)      │
                    │                               │
                    │   ┌───────────────────────┐   │
                    │   │      Active View      │   │
                    │   │   (Chat, Settings,    │   │
                    │   │    History, etc.)     │   │
                    │   └───────────────────────┘   │
                    │                               │
                    └───────────────────────────────┘
```

---

## Application Structure

### Component Hierarchy

```
AppDelegate (NSApplicationDelegate)
├── ServiceContainer (owns all services)
│   ├── AppSettingsService
│   ├── ProfileService
│   ├── ConversationService
│   ├── ChatService
│   ├── McpService
│   ├── McpRegistryService
│   └── SecretsService
│
├── StatusBarController (owns menubar presence)
│   ├── NSStatusItem (menubar icon)
│   ├── NSPopover (container for views)
│   └── HotkeyManager (global hotkey)
│
└── ViewRouter (manages view stack)
    ├── ChatView (default/root)
    ├── SettingsView
    ├── HistoryView
    ├── ModelSelectorView
    ├── ProfileEditorView
    ├── McpAddView
    └── McpConfigureView
```

---

## Startup Sequence

### Phase 1: App Launch

| Step | Component | Action |
|------|-----------|--------|
| 1 | macOS | Launches app, calls `applicationDidFinishLaunching` |
| 2 | AppDelegate | Initialize logging |
| 3 | AppDelegate | Set `LSUIElement = true` behavior (no Dock icon) |
| 4 | AppDelegate | Create ServiceContainer |

### Phase 2: Service Initialization

| Step | Service | Action |
|------|---------|--------|
| 5 | SecretsService | Initialize (derive encryption key) |
| 6 | AppSettingsService | Load settings.json (or create defaults) |
| 7 | ProfileService | Initialize (SecretsService injected) |
| 8 | ConversationService | Initialize (file system only) |
| 9 | McpRegistryService | Initialize (HTTP client) |
| 10 | McpService | Initialize (SecretsService injected) |
| 11 | ChatService | Initialize (ProfileService, ConversationService, McpService injected) |

```rust
struct ServiceContainer {
    pub secrets: Arc<dyn SecretsService>,
    pub app_settings: Arc<dyn AppSettingsService>,
    pub profiles: Arc<dyn ProfileService>,
    pub conversations: Arc<dyn ConversationService>,
    pub mcp_registry: Arc<dyn McpRegistryService>,
    pub mcp: Arc<dyn McpService>,
    pub chat: Arc<dyn ChatService>,
}

impl ServiceContainer {
    fn new() -> Result<Self> {
        // Initialize in dependency order
        let secrets = Arc::new(SecretsServiceImpl::new()?);
        let app_settings = Arc::new(AppSettingsServiceImpl::new()?);
        let profiles = Arc::new(ProfileServiceImpl::new(secrets.clone())?);
        let conversations = Arc::new(ConversationServiceImpl::new()?);
        let mcp_registry = Arc::new(McpRegistryServiceImpl::new()?);
        let mcp = Arc::new(McpServiceImpl::new(secrets.clone())?);
        let chat = Arc::new(ChatServiceImpl::new(
            profiles.clone(),
            conversations.clone(),
            mcp.clone(),
        )?);
        
        Ok(Self {
            secrets,
            app_settings,
            profiles,
            conversations,
            mcp_registry,
            mcp,
            chat,
        })
    }
}
```

### Phase 3: UI Setup

| Step | Component | Action |
|------|-----------|--------|
| 12 | StatusBarController | Create NSStatusItem |
| 13 | StatusBarController | Set menubar icon (template image) |
| 14 | StatusBarController | Create NSPopover |
| 15 | StatusBarController | Configure popover (size, behavior) |
| 16 | ViewRouter | Initialize with ServiceContainer reference |
| 17 | ViewRouter | Set ChatView as root view |
| 18 | StatusBarController | Set popover content to ViewRouter's current view |

### Phase 4: State Restoration

| Step | Component | Action |
|------|-----------|--------|
| 19 | AppDelegate | Get default profile ID from AppSettingsService |
| 20 | AppDelegate | Get current conversation ID from AppSettingsService |
| 21 | ChatView | Load profile via ProfileService |
| 22 | ChatView | Load conversation via ConversationService (if exists) |
| 23 | ChatView | Initialize show_thinking toggle from profile |

### Phase 5: MCP Startup

| Step | Component | Action |
|------|-----------|--------|
| 24 | McpService | Get list of enabled MCPs |
| 25 | McpService | Start each MCP server (async, parallel) |
| 26 | McpService | Report status (UI can show loading state) |

### Phase 6: Hotkey Registration

| Step | Component | Action |
|------|-----------|--------|
| 27 | AppDelegate | Get hotkey config from AppSettingsService |
| 28 | HotkeyManager | Register global hotkey with macOS |
| 29 | HotkeyManager | Set callback to toggle popover |

---

## Menubar Icon

### StatusBarController

```rust
struct StatusBarController {
    status_item: NSStatusItem,
    popover: NSPopover,
    view_router: ViewRouter,
    hotkey_manager: HotkeyManager,
    event_monitor: Option<EventMonitor>,
}

impl StatusBarController {
    fn setup(&mut self) {
        // Create status item
        let status_bar = NSStatusBar::system_status_bar();
        self.status_item = status_bar.status_item_with_length(NSSquareStatusItemLength);
        
        // Set icon (template for dark/light mode)
        let icon = NSImage::image_named("MenuBarIcon");
        icon.set_template(true);
        self.status_item.button().set_image(icon);
        
        // Set click action
        self.status_item.button().set_action(sel!(togglePopover:));
        self.status_item.button().set_target(self);
        
        // Configure popover
        self.popover.set_content_size(NSSize::new(400.0, 500.0));
        self.popover.set_behavior(NSPopoverBehavior::Transient);
        self.popover.set_animates(true);
    }
    
    fn toggle_popover(&mut self) {
        if self.popover.is_shown() {
            self.close_popover();
        } else {
            self.open_popover();
        }
    }
    
    fn open_popover(&mut self) {
        let button = self.status_item.button();
        self.popover.show_relative_to_rect(
            button.bounds(),
            button,
            NSRectEdge::MinY,
        );
        
        // Start monitoring clicks outside popover
        self.start_event_monitor();
    }
    
    fn close_popover(&mut self) {
        self.popover.perform_close(nil);
        self.stop_event_monitor();
    }
}
```

### Menubar Icon Specifications

| Property | Value |
|----------|-------|
| Size | 18x18 points (36x36 @2x) |
| Format | Template image (single color) |
| Color | Automatic (system handles dark/light) |
| File | MenuBarIcon.pdf or .png |

### Icon States

| State | Visual |
|-------|--------|
| Normal | Standard icon |
| Popover open | Highlighted (system automatic) |
| Streaming | Could animate (future enhancement) |

---

## View Router

The ViewRouter manages navigation between views within the popover.

### View Stack

```rust
struct ViewRouter {
    services: Arc<ServiceContainer>,
    view_stack: Vec<ViewType>,
    current_view: Box<dyn View>,
}

enum ViewType {
    Chat,
    Settings,
    History,
    ModelSelector { return_to: Box<ViewType>, context: ModelSelectorContext },
    ProfileEditor { profile_id: Option<Uuid>, from_model_selector: Option<SelectedModel> },
    McpAdd,
    McpConfigure { mcp_id: Option<Uuid>, from_mcp_add: Option<McpSearchResult> },
}
```

### Navigation Patterns

| From | To | Trigger | Stack Behavior |
|------|----|---------| ---------------|
| Chat | Settings | Gear button | Push |
| Chat | History | History button | Push |
| Settings | Chat | Back | Pop |
| Settings | ModelSelector | "Add Profile" | Push |
| Settings | ProfileEditor | Edit profile row | Push |
| Settings | McpAdd | "Add MCP" | Push |
| Settings | McpConfigure | Edit MCP row | Push |
| ModelSelector | ProfileEditor | Select model | Replace top |
| ProfileEditor | Settings | Save/Cancel | Pop |
| McpAdd | McpConfigure | Select MCP | Replace top |
| McpConfigure | Settings | Save/Cancel | Pop |
| History | Chat | Load conversation | Pop + notify |

### Navigation Implementation

```rust
impl ViewRouter {
    fn push(&mut self, view_type: ViewType) {
        self.view_stack.push(view_type.clone());
        self.current_view = self.create_view(view_type);
        self.notify_popover_content_changed();
    }
    
    fn pop(&mut self) {
        if self.view_stack.len() > 1 {
            self.view_stack.pop();
            let current = self.view_stack.last().unwrap().clone();
            self.current_view = self.create_view(current);
            self.notify_popover_content_changed();
        }
    }
    
    fn pop_to_root(&mut self) {
        self.view_stack.truncate(1);
        self.current_view = self.create_view(ViewType::Chat);
        self.notify_popover_content_changed();
    }
    
    fn replace_top(&mut self, view_type: ViewType) {
        self.view_stack.pop();
        self.view_stack.push(view_type.clone());
        self.current_view = self.create_view(view_type);
        self.notify_popover_content_changed();
    }
    
    fn create_view(&self, view_type: ViewType) -> Box<dyn View> {
        match view_type {
            ViewType::Chat => Box::new(ChatView::new(self.services.clone())),
            ViewType::Settings => Box::new(SettingsView::new(self.services.clone())),
            ViewType::History => Box::new(HistoryView::new(self.services.clone())),
            ViewType::ModelSelector { context, .. } => {
                Box::new(ModelSelectorView::new(self.services.clone(), context))
            }
            ViewType::ProfileEditor { profile_id, from_model_selector } => {
                Box::new(ProfileEditorView::new(
                    self.services.clone(),
                    profile_id,
                    from_model_selector,
                ))
            }
            ViewType::McpAdd => Box::new(McpAddView::new(self.services.clone())),
            ViewType::McpConfigure { mcp_id, from_mcp_add } => {
                Box::new(McpConfigureView::new(
                    self.services.clone(),
                    mcp_id,
                    from_mcp_add,
                ))
            }
        }
    }
}
```

---

## Global Hotkey

### HotkeyManager

```rust
struct HotkeyManager {
    hotkey_id: Option<HotkeyId>,
    callback: Box<dyn Fn()>,
}

impl HotkeyManager {
    fn register(&mut self, config: &HotkeyConfig) -> Result<()> {
        // Unregister existing if any
        self.unregister();
        
        if !config.enabled {
            return Ok(());
        }
        
        // Convert to macOS key code and modifiers
        let key_code = self.key_name_to_code(&config.key)?;
        let modifiers = self.build_modifiers(&config.modifiers);
        
        // Register with macOS (using something like HotKey crate or Carbon APIs)
        self.hotkey_id = Some(register_global_hotkey(key_code, modifiers, || {
            (self.callback)();
        })?);
        
        Ok(())
    }
    
    fn unregister(&mut self) {
        if let Some(id) = self.hotkey_id.take() {
            unregister_global_hotkey(id);
        }
    }
}
```

### Hotkey Behavior

| Condition | Action |
|-----------|--------|
| Popover closed | Open popover, focus input |
| Popover open | Close popover |
| App not running | macOS ignores hotkey |

---

## Popover Behavior

### Configuration

| Property | Value | Notes |
|----------|-------|-------|
| Size | 400 x 500 points | Fixed for all views |
| Behavior | Transient | Closes on click outside |
| Animates | Yes | Fade in/out |
| Arrow | MinY (bottom) | Points to menubar icon |

### Click-Outside Handling

```rust
impl StatusBarController {
    fn start_event_monitor(&mut self) {
        self.event_monitor = Some(NSEvent::add_global_monitor_for_events(
            NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown,
            |event| {
                // Check if click is outside popover
                if !self.popover.content_view_controller().view().mouse_in_rect(
                    event.location_in_window(),
                ) {
                    self.close_popover();
                }
            },
        ));
    }
    
    fn stop_event_monitor(&mut self) {
        if let Some(monitor) = self.event_monitor.take() {
            NSEvent::remove_monitor(monitor);
        }
    }
}
```

---

## App Lifecycle

### Application States

```
┌─────────────┐     Launch      ┌─────────────┐
│   Not       │ ───────────────▶│   Running   │
│   Running   │                 │  (menubar)  │
└─────────────┘                 └─────────────┘
                                      │
                           ┌──────────┴──────────┐
                           │                     │
                           ▼                     ▼
                    ┌─────────────┐       ┌─────────────┐
                    │   Popover   │       │   Popover   │
                    │   Closed    │◀─────▶│    Open     │
                    └─────────────┘       └─────────────┘
                           │
                           │ Quit
                           ▼
                    ┌─────────────┐
                    │ Terminating │
                    └─────────────┘
```

### Termination Sequence

| Step | Component | Action |
|------|-----------|--------|
| 1 | AppDelegate | `applicationWillTerminate` called |
| 2 | StatusBarController | Close popover if open |
| 3 | AppDelegate | Save window state to AppSettingsService |
| 4 | HotkeyManager | Unregister global hotkey |
| 5 | McpService | Stop all MCP servers |
| 6 | ChatService | Cancel any active streams |
| 7 | AppDelegate | Allow termination |

```rust
impl NSApplicationDelegate for AppDelegate {
    fn application_will_terminate(&self, _notification: &NSNotification) {
        // Save window state
        if let Some(state) = self.status_bar_controller.get_window_state() {
            let _ = self.services.app_settings.set_window_state(&state);
        }
        
        // Cleanup
        self.status_bar_controller.cleanup();
        self.services.mcp.stop_all();
    }
}
```

---

## Error Handling During Startup

### Graceful Degradation

| Error | Handling |
|-------|----------|
| settings.json corrupt | Reset to defaults, log warning |
| Default profile missing | Clear default, show first-run state |
| Current conversation missing | Clear current, start fresh |
| MCP fails to start | Log error, continue without it |
| Hotkey registration fails | Log warning, continue without hotkey |
| SecretsService init fails | Fatal - show error dialog, quit |

### First-Run Experience

| Condition | Behavior |
|-----------|----------|
| No profiles exist | Chat View shows "No profile configured" message |
| No profiles exist | Settings button pulses or is highlighted |
| User clicks Settings | ModelSelector shown to create first profile |

---

## Inter-View Communication

### Notification-Based

Views communicate through notifications for loose coupling:

| Notification | Payload | Sender | Receiver |
|--------------|---------|--------|----------|
| `ConversationSelected` | `Uuid` | History View | Chat View |
| `ProfileChanged` | `Uuid` | Settings View | Chat View |
| `McpStatusChanged` | `McpId, Status` | McpService | Settings View |
| `StreamCancelled` | `ConversationId` | Chat View | (internal) |

```rust
// Example: History View loads a conversation
impl HistoryView {
    fn load_conversation(&self, id: Uuid) {
        // Update app settings
        self.services.app_settings.set_current_conversation_id(id)?;
        
        // Post notification
        NotificationCenter::default().post(
            "ConversationSelected",
            Some(id.to_string()),
        );
        
        // Navigate back to Chat
        self.view_router.pop();
    }
}

// Chat View receives notification
impl ChatView {
    fn setup_notifications(&self) {
        NotificationCenter::default().add_observer(
            "ConversationSelected",
            |notification| {
                let id = Uuid::parse_str(notification.object())?;
                self.load_conversation(id);
            },
        );
    }
}
```

---

## Test Requirements

| ID | Test |
|----|------|
| AB-T1 | App launches without crash |
| AB-T2 | Services initialize in correct order |
| AB-T3 | Menubar icon appears |
| AB-T4 | Click on icon opens popover |
| AB-T5 | Click outside popover closes it |
| AB-T6 | Hotkey toggles popover |
| AB-T7 | Default profile loaded on startup |
| AB-T8 | Current conversation restored on startup |
| AB-T9 | Missing settings.json creates defaults |
| AB-T10 | View navigation push/pop works |
| AB-T11 | App terminates cleanly |
| AB-T12 | Window state saved on quit |
| AB-T13 | MCP servers start on launch |
| AB-T14 | First-run experience shows correctly |
