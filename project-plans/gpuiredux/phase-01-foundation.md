# Phase 01: Foundation & Navigation

**Phase ID:** PLAN-20250130-GPUIREDUX.P01
**Depends On:** None
**Estimated Effort:** 4-6 hours

## Objective

Establish the navigation system and main view container that will host all views. This phase creates the skeleton that all subsequent phases build upon.

## Test-First Requirements

Write these tests FIRST, before any implementation:

### Navigation Tests

```rust
// tests/ui_gpui/navigation_tests.rs

use personal_agent::ui_gpui::navigation::{NavigationState, ViewId};

#[test]
fn test_initial_state_is_chat() {
    let nav = NavigationState::new();
    assert_eq!(nav.current(), ViewId::Chat);
    assert_eq!(nav.stack_depth(), 1);
}

#[test]
fn test_navigate_pushes_to_stack() {
    let mut nav = NavigationState::new();
    nav.navigate(ViewId::Settings);
    
    assert_eq!(nav.current(), ViewId::Settings);
    assert_eq!(nav.stack_depth(), 2);
}

#[test]
fn test_navigate_back_pops_stack() {
    let mut nav = NavigationState::new();
    nav.navigate(ViewId::Settings);
    nav.navigate(ViewId::ProfileEditor { id: None });
    
    assert_eq!(nav.stack_depth(), 3);
    
    nav.navigate_back();
    assert_eq!(nav.current(), ViewId::Settings);
    assert_eq!(nav.stack_depth(), 2);
}

#[test]
fn test_navigate_back_at_root_stays_at_root() {
    let mut nav = NavigationState::new();
    nav.navigate_back(); // Already at Chat
    
    assert_eq!(nav.current(), ViewId::Chat);
    assert_eq!(nav.stack_depth(), 1);
}

#[test]
fn test_navigate_to_same_view_does_nothing() {
    let mut nav = NavigationState::new();
    nav.navigate(ViewId::Chat); // Already at Chat
    
    assert_eq!(nav.stack_depth(), 1);
}

#[test]
fn test_can_go_back_returns_false_at_root() {
    let nav = NavigationState::new();
    assert!(!nav.can_go_back());
}

#[test]
fn test_can_go_back_returns_true_when_stacked() {
    let mut nav = NavigationState::new();
    nav.navigate(ViewId::Settings);
    assert!(nav.can_go_back());
}
```

### MainPanel Tests

```rust
// tests/ui_gpui/main_panel_tests.rs

use personal_agent::ui_gpui::views::main_panel::MainPanel;
use personal_agent::ui_gpui::navigation::ViewId;
use personal_agent::presentation::view_command::ViewCommand;

#[gpui::test]
fn test_main_panel_starts_with_chat_view(cx: &mut TestAppContext) {
    let panel = cx.new(|cx| MainPanel::new(cx));
    
    panel.read(cx, |panel, _| {
        assert_eq!(panel.current_view(), ViewId::Chat);
    });
}

#[gpui::test]
fn test_main_panel_handles_navigate_command(cx: &mut TestAppContext) {
    let panel = cx.new(|cx| MainPanel::new(cx));
    
    panel.update(cx, |panel, cx| {
        panel.handle_command(ViewCommand::Navigate { to: ViewId::Settings }, cx);
    });
    
    panel.read(cx, |panel, _| {
        assert_eq!(panel.current_view(), ViewId::Settings);
    });
}

#[gpui::test]
fn test_main_panel_handles_navigate_back_command(cx: &mut TestAppContext) {
    let panel = cx.new(|cx| MainPanel::new(cx));
    
    panel.update(cx, |panel, cx| {
        panel.handle_command(ViewCommand::Navigate { to: ViewId::Settings }, cx);
        panel.handle_command(ViewCommand::NavigateBack, cx);
    });
    
    panel.read(cx, |panel, _| {
        assert_eq!(panel.current_view(), ViewId::Chat);
    });
}
```

### ViewCommand Receiver Tests

```rust
// tests/ui_gpui/command_receiver_tests.rs

use flume;
use personal_agent::ui_gpui::app::GpuiApp;
use personal_agent::presentation::view_command::ViewCommand;

#[gpui::test]
fn test_app_receives_view_commands(cx: &mut TestAppContext) {
    let (tx, rx) = flume::bounded::<ViewCommand>(16);
    let app = cx.new(|cx| GpuiApp::new(rx, cx));
    
    // Send a command through the channel
    tx.send(ViewCommand::Navigate { to: ViewId::History }).unwrap();
    
    // Run event loop briefly
    cx.run_until_parked();
    
    app.read(cx, |app, _| {
        assert_eq!(app.main_panel().current_view(), ViewId::History);
    });
}
```

## Implementation

After tests are written and failing, implement:

### 1. Navigation State (`src/ui_gpui/navigation.rs`)

```rust
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewId {
    Chat,
    History,
    Settings,
    ModelSelector,
    ProfileEditor { id: Option<Uuid> },
    McpAdd,
    McpConfigure { id: Option<Uuid> },
}

pub struct NavigationState {
    stack: Vec<ViewId>,
}

impl NavigationState {
    pub fn new() -> Self {
        Self {
            stack: vec![ViewId::Chat],
        }
    }

    pub fn current(&self) -> ViewId {
        self.stack.last().cloned().unwrap_or(ViewId::Chat)
    }

    pub fn stack_depth(&self) -> usize {
        self.stack.len()
    }

    pub fn can_go_back(&self) -> bool {
        self.stack.len() > 1
    }

    pub fn navigate(&mut self, to: ViewId) {
        if self.current() != to {
            self.stack.push(to);
        }
    }

    pub fn navigate_back(&mut self) -> bool {
        if self.stack.len() > 1 {
            self.stack.pop();
            true
        } else {
            false
        }
    }
}
```

### 2. MainPanel (`src/ui_gpui/views/main_panel.rs`)

```rust
use gpui::prelude::*;
use crate::ui_gpui::navigation::{NavigationState, ViewId};
use crate::presentation::view_command::ViewCommand;

pub struct MainPanel {
    navigation: NavigationState,
    // Child view entities will be added in later phases
}

impl MainPanel {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            navigation: NavigationState::new(),
        }
    }

    pub fn current_view(&self) -> ViewId {
        self.navigation.current()
    }

    pub fn handle_command(&mut self, cmd: ViewCommand, cx: &mut Context<Self>) {
        match cmd {
            ViewCommand::Navigate { to } => {
                self.navigation.navigate(to);
                cx.notify();
            }
            ViewCommand::NavigateBack => {
                self.navigation.navigate_back();
                cx.notify();
            }
            // Forward other commands to child views (implemented later)
            _ => {}
        }
    }
}

impl Render for MainPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Placeholder - actual view rendering added in later phases
        div()
            .size_full()
            .bg(gpui::rgb(0x121212))
            .child(format!("Current view: {:?}", self.navigation.current()))
    }
}
```

### 3. GpuiApp Update (`src/ui_gpui/app.rs`)

```rust
use gpui::prelude::*;
use flume::Receiver;
use crate::ui_gpui::views::main_panel::MainPanel;
use crate::presentation::view_command::ViewCommand;

pub struct GpuiApp {
    main_panel: Entity<MainPanel>,
    command_rx: Receiver<ViewCommand>,
}

impl GpuiApp {
    pub fn new(command_rx: Receiver<ViewCommand>, cx: &mut Context<Self>) -> Self {
        let main_panel = cx.new(|cx| MainPanel::new(cx));
        
        Self {
            main_panel,
            command_rx,
        }
    }

    pub fn main_panel(&self) -> &Entity<MainPanel> {
        &self.main_panel
    }

    /// Poll for ViewCommands and dispatch to MainPanel
    pub fn poll_commands(&mut self, cx: &mut Context<Self>) {
        while let Ok(cmd) = self.command_rx.try_recv() {
            self.main_panel.update(cx, |panel, cx| {
                panel.handle_command(cmd, cx);
            });
        }
    }
}

impl Render for GpuiApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Poll commands each render
        self.poll_commands(cx);
        
        div()
            .size_full()
            .child(self.main_panel.clone())
    }
}
```

## Verification Checklist

- [ ] `cargo test navigation` passes all 7 navigation tests
- [ ] `cargo test main_panel` passes all 3 MainPanel tests
- [ ] `cargo test command_receiver` passes command receiving test
- [ ] `cargo build --bin personal_agent_gpui` succeeds
- [ ] Navigation state correctly tracks view stack
- [ ] MainPanel renders placeholder for current view
- [ ] ViewCommand receiver polls and dispatches commands

## Files Created/Modified

| File | Action |
|------|--------|
| `src/ui_gpui/navigation.rs` | Create |
| `src/ui_gpui/views/main_panel.rs` | Rewrite |
| `src/ui_gpui/app.rs` | Rewrite |
| `src/ui_gpui/mod.rs` | Update exports |
| `tests/ui_gpui/navigation_tests.rs` | Create |
| `tests/ui_gpui/main_panel_tests.rs` | Create |
| `tests/ui_gpui/command_receiver_tests.rs` | Create |

## Notes

- ViewId enum must match what presenters expect (check `src/presentation/view_command.rs`)
- Command polling should be non-blocking (try_recv, not recv)
- Navigation stack never becomes empty (Chat is always at bottom)
- This phase is purely structural - no visual styling needed yet
