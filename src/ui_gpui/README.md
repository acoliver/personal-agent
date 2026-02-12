# GPUI UI Module

The `ui_gpui` module provides a GPUI-based user interface for PersonalAgent. It is designed to replace the legacy Cocoa-based UI system while maintaining compatibility with the existing presenter and service architecture.

## Purpose

The `ui_gpui` module serves as a modern, high-performance UI layer built on GPUI that:

1. Provides a responsive, native-like macOS interface
2. Maintains clean separation between UI and business logic
3. Enables smooth animations and interactive elements
4. Supports the full range of PersonalAgent functionality

## Architecture Overview

The module is organized into several key sections:

- **Bridge Layer**: Communication between GPUI (smol-based) and tokio
- **Components**: Reusable UI components
- **Views**: Main application views
- **Integration**: System integration components

## How to Use the Components

### Components

All UI components are in the `components` module and follow a consistent pattern:

```rust
use personal_agent::ui_gpui::components::{TabBar, MessageBubble, InputBar, Button};

// In a view's render method:
fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
    div()
        .child(
            TabBar::new("tabs")
                .tabs(self.tabs.clone())
                .on_click(cx.listener(|this, tab_id, _cx| {
                    // Handle tab selection
                }))
        )
        .child(
            MessageBubble::assistant("AI response content")
        )
        .child(
            InputBar::new("input")
                .placeholder("Type a message...")
                .on_submit(cx.listener(|this, text, _cx| {
                    // Handle message submission
                }))
        )
        .child(
            Button::new("send")
                .label("Send")
                .on_click(cx.listener(|this, _event, _cx| {
                    // Handle button click
                }))
        )
}
```

### Views

Views provide the main application interfaces:

```rust
use personal_agent::ui_gpui::views::{ChatView, MainPanel, HistoryView, SettingsView};

// In the main app:
new_entity(|cx| {
    MainPanel::new(cx)
})

// MainPanel manages switching between views
```

## Event Flow

The event flow follows a strict unidirectional pattern:

```
UserEvent → Bridge → EventBus → Presenter → ViewCommand → Bridge → UI Update
```

### Creating User Events

Components create `UserEvent` instances to capture user interactions:

```rust
// In a component's event handler:
let event = UserEvent::SendMessage(text, self.current_conversation_id);
self.bridge.send_user_event(cx, event);
```

### Handling View Commands

Views receive updates through `ViewCommand` instances:

```rust
// In a view's implementation:
fn update_messages(&mut self, messages: Vec<ChatMessage>) {
    self.messages = messages;
    self.notify(ViewCommand::Render);
}
```

### Bridge Communication

The bridge layer provides two-way communication:

```rust
// Sending user events to the presenter:
self.bridge.send_user_event(cx, user_event);

// Receiving view commands from the presenter:
// Handled automatically by ViewCommandSink
```

## Integration Points

### Tray Integration

The `TrayBridge` handles system tray integration:

```rust
let tray_bridge = TrayBridge::new(cx);
tray_bridge.show popup_window();
```

### Popup Window

The `PopupWindow` manages the main application window:

```rust
let popup = PopupWindow::new(cx);
popup.set_position(x, y);
popup.show();
```

### Application Lifecycle

The `GpuiApp` manages the application lifecycle:

```rust
let app = GpuiApp::new(cx);
app.activate();
app.run();
```

## State Management

State in the GPUI UI is managed through a combination of:

1. **Local State**: Component-specific state in Rust structs
2. **Global State**: Application state managed through bridges
3. **Derived State**: Computed state calculated from other state

### Local State Pattern

```rust
pub struct MyComponent {
    text: String,
    is_active: bool,
}

impl MyComponent {
    pub fn new(cx: &mut ViewContext<Self>) -> Self {
        Self {
            text: String::new(),
            is_active: false,
        }
    }
}
```

## Styling and Theming

Components support theming through the `theme` module:

```rust
use personal_agent::ui_gpui::theme::{Appearance,Theme};

// Apply a theme to a component:
div()
    .bg(rgb(0x1e1e1e))
    .text_color(rgb(0xffffff))
```

## Best Practices

1. **Separate UI from Business Logic**: Keep all business logic in the presenter layer
2. **Use the Bridge Pattern**: Always go through the bridge for communication
3. **Follow Unidirectional Flow**: Avoid direct callbacks or two-way bindings
4. **Handle Events Asynchronously**: Use the bridge for async operations
5. **Keep Components Focused**: Each component should have a single responsibility

## Example: Creating a Custom Component

```rust
use gpui::*;
use personal_agent::ui_gpui::bridge::GpuiBridge;

pub struct CustomButton {
    label: String,
    on_click: Option<Box<dyn Fn(&mut Self, &mut ViewContext<Self>)>>,
}

impl CustomButton {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            on_click: None,
        }
    }
    
    pub fn on_click<F>(mut self, f: F) -> Self 
    where
        F: Fn(&mut Self, &mut ViewContext<Self>) + 'static
    {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl Render for CustomButton {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        div()
            .bg(rgb(0x2d2d30))
            .border_radius(4.0)
            .py_2()
            .px_4()
            .hover(|style| style.bg(rgb(0x3e3e42)))
            .cursor_pointer()
            .when_some(self.on_click.as_ref(), |this, on_click| {
                this.on_click(cx.listener(|this, _event, cx| {
                    on_click(this, cx);
                }))
            })
            .child(Label::new(self.label.clone()))
    }
}
```

## Performance Considerations

1. **Minimize Re-renders**: Only trigger re-renders when state actually changes
2. **Use Efficient Layouts**: Prefer flex layouts over nested divs
3. **Optimize Event Handling**: Debounce rapid user events like text input
4. **Handle Large Lists**: Use virtual scrolling for large conversation histories

## Migration from UI Module

When migrating from the legacy `ui` module:

1. **Identify UI Components**: Map legacy components to GPUI equivalents
2. **Refactor Event Handling**: Replace direct method calls with the bridge pattern
3. **Replace View Logic**: Convert view logic to GPUI render methods
4. **Update State Management**: Move state management to the appropriate layer
5. **Test Functionality**: Ensure all user interactions work correctly

## Testing

Components can be tested using GPUI's testing utilities:

```rust
#[test]
fn test_button_click() {
    let mut cx = TestAppContext::new();
    let mut view = cx.build_view(Button::new("test"));
    assert_eq!(view.label(), "test");
    
    view.dispatch_click_event(&mut cx);
    assert!(view.was_clicked());
}
```

## Further Reading

- [GPUI Documentation](https://github.com/zed-industries/zed/tree/main/crates/gpui)
- [Architecture Documentation](../../../dev-docs/architecture/gpui-architecture.md)
- [Event System Documentation](../../../dev-docs/architecture/architecture_improvements.md)