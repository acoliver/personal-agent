# Phase 06: Components Stub

## Phase ID

`PLAN-20250128-GPUI.P06`

## Prerequisites

- Phase 05a (Bridge Implementation Verification) completed with PASS
- Evidence file: `project-plans/gpui-migration/plan/.completed/P05A.md`
- **Bridge is working:** GpuiBridge, ViewCommandSink, UserEvent forwarder functional

---

## Requirements Implemented

### REQ-GPUI-002: Tab Navigation
- REQ-GPUI-002.1: Three tabs (Chat, History, Settings)

### REQ-GPUI-003: Chat View Components
- Message bubbles
- Input bar
- Toolbar buttons

---

## Implementation Tasks

### Files to Create

**1. `src/ui_gpui/components/tab_bar.rs`**
```rust
//! Tab bar component for view navigation
//! 
//! @plan PLAN-20250128-GPUI.P06
//! @requirement REQ-GPUI-002

use gpui::{div, prelude::*, px, IntoElement, Hsla};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Chat,
    History,
    Settings,
}

pub struct TabBar {
    active: Tab,
    on_select: Option<Box<dyn Fn(Tab) + Send + Sync + 'static>>,
}

impl TabBar {
    pub fn new(active: Tab) -> Self {
        Self { active, on_select: None }
    }
    
    pub fn on_select(mut self, f: impl Fn(Tab) + Send + Sync + 'static) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }
}

impl IntoElement for TabBar {
    type Element = gpui::Stateful<gpui::Div>;
    
    fn into_element(self) -> Self::Element {
        unimplemented!("Phase 08: TabBar render")
    }
}
```

**2. `src/ui_gpui/components/message_bubble.rs`**
```rust
//! Message bubble components for chat
//! 
//! @plan PLAN-20250128-GPUI.P06
//! @requirement REQ-GPUI-003

use gpui::{div, prelude::*, IntoElement};

pub struct UserBubble {
    content: String,
}

impl UserBubble {
    pub fn new(content: impl Into<String>) -> Self {
        Self { content: content.into() }
    }
}

impl IntoElement for UserBubble {
    type Element = gpui::Div;
    
    fn into_element(self) -> Self::Element {
        unimplemented!("Phase 08: UserBubble render")
    }
}

pub struct AssistantBubble {
    content: String,
    model_id: Option<String>,
    thinking: Option<String>,
    show_thinking: bool,
    is_streaming: bool,
}

impl AssistantBubble {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            model_id: None,
            thinking: None,
            show_thinking: false,
            is_streaming: false,
        }
    }
    
    pub fn model_id(mut self, id: impl Into<String>) -> Self {
        self.model_id = Some(id.into());
        self
    }
    
    pub fn thinking(mut self, thinking: impl Into<String>) -> Self {
        self.thinking = Some(thinking.into());
        self
    }
    
    pub fn show_thinking(mut self, show: bool) -> Self {
        self.show_thinking = show;
        self
    }
    
    pub fn streaming(mut self, is_streaming: bool) -> Self {
        self.is_streaming = is_streaming;
        self
    }
}

impl IntoElement for AssistantBubble {
    type Element = gpui::Div;
    
    fn into_element(self) -> Self::Element {
        unimplemented!("Phase 08: AssistantBubble render")
    }
}
```

**3. `src/ui_gpui/components/input_bar.rs`**
```rust
//! Input bar with text field and buttons
//! 
//! @plan PLAN-20250128-GPUI.P06
//! @requirement REQ-GPUI-003.4

use gpui::{div, prelude::*, IntoElement};

pub struct InputBar {
    text: String,
    is_streaming: bool,
    on_send: Option<Box<dyn Fn(String) + Send + Sync + 'static>>,
    on_stop: Option<Box<dyn Fn() + Send + Sync + 'static>>,
}

impl InputBar {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            is_streaming: false,
            on_send: None,
            on_stop: None,
        }
    }
    
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }
    
    pub fn is_streaming(mut self, streaming: bool) -> Self {
        self.is_streaming = streaming;
        self
    }
    
    pub fn on_send(mut self, f: impl Fn(String) + Send + Sync + 'static) -> Self {
        self.on_send = Some(Box::new(f));
        self
    }
    
    pub fn on_stop(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_stop = Some(Box::new(f));
        self
    }
}

impl IntoElement for InputBar {
    type Element = gpui::Div;
    
    fn into_element(self) -> Self::Element {
        unimplemented!("Phase 08: InputBar render")
    }
}
```

**4. `src/ui_gpui/components/button.rs`**
```rust
//! Button components
//! 
//! @plan PLAN-20250128-GPUI.P06
//! @requirement REQ-GPUI-003

use gpui::{div, prelude::*, IntoElement, Hsla};

pub struct Button {
    label: String,
    active: bool,
    disabled: bool,
    on_click: Option<Box<dyn Fn() + Send + Sync + 'static>>,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            active: false,
            disabled: false,
            on_click: None,
        }
    }
    
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
    
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    
    pub fn on_click(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl IntoElement for Button {
    type Element = gpui::Stateful<gpui::Div>;
    
    fn into_element(self) -> Self::Element {
        unimplemented!("Phase 08: Button render")
    }
}
```

**5. Update `src/ui_gpui/components/mod.rs`**
```rust
//! Reusable GPUI components
//! 
//! @plan PLAN-20250128-GPUI.P06

pub mod tab_bar;
pub mod message_bubble;
pub mod input_bar;
pub mod button;

pub use tab_bar::{Tab, TabBar};
pub use message_bubble::{UserBubble, AssistantBubble};
pub use input_bar::InputBar;
pub use button::Button;
```

---

## Verification Commands

### Files Exist

```bash
ls -la src/ui_gpui/components/
```

### Markers Present

```bash
grep -r "@plan PLAN-20250128-GPUI.P06" src/ui_gpui/components/
```

### Compiles

```bash
cargo build
```

---

## Success Criteria

- [ ] All component files created
- [ ] All files have @plan markers
- [ ] `cargo build` succeeds
- [ ] Components have builder pattern methods

---

## Evidence File

Create: `project-plans/gpui-migration/plan/.completed/P06.md`

---

## Next Phase

After P06 completes with PASS:
--> P06a: Components Stub Verification
