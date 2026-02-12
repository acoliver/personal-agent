# Phase 02: Component Library

**Phase ID:** PLAN-20250130-GPUIREDUX.P02
**Depends On:** P01a
**Estimated Effort:** 6-8 hours

## Objective

Build reusable atomic components that match the mockup styling. These components will be used by all views.

## Test-First Requirements

### Button Tests

```rust
// tests/ui_gpui/components/button_tests.rs

#[gpui::test]
fn test_button_renders_label(cx: &mut TestAppContext) {
    let button = Button::new("Click Me");
    // Assert button contains "Click Me" text
}

#[gpui::test]
fn test_button_click_calls_handler(cx: &mut TestAppContext) {
    let clicked = Rc::new(Cell::new(false));
    let clicked_clone = clicked.clone();
    
    let button = Button::new("Click")
        .on_click(move |_, _| clicked_clone.set(true));
    
    // Simulate click
    // Assert clicked.get() == true
}

#[gpui::test]
fn test_button_disabled_ignores_click(cx: &mut TestAppContext) {
    let clicked = Rc::new(Cell::new(false));
    let clicked_clone = clicked.clone();
    
    let button = Button::new("Click")
        .disabled(true)
        .on_click(move |_, _| clicked_clone.set(true));
    
    // Simulate click
    // Assert clicked.get() == false
}

#[gpui::test]
fn test_icon_button_renders_28x28(cx: &mut TestAppContext) {
    let button = IconButton::new("T");
    // Assert size is 28x28
}
```

### TextField Tests

```rust
// tests/ui_gpui/components/text_field_tests.rs

#[gpui::test]
fn test_text_field_captures_input(cx: &mut TestAppContext) {
    let field = cx.new(|cx| TextField::new(cx));
    
    // Simulate typing "hello"
    field.update(cx, |f, cx| {
        f.handle_key_down("h", cx);
        f.handle_key_down("e", cx);
        f.handle_key_down("l", cx);
        f.handle_key_down("l", cx);
        f.handle_key_down("o", cx);
    });
    
    field.read(cx, |f, _| {
        assert_eq!(f.text(), "hello");
    });
}

#[gpui::test]
fn test_text_field_backspace_deletes(cx: &mut TestAppContext) {
    let field = cx.new(|cx| TextField::new(cx).with_text("hello"));
    
    field.update(cx, |f, cx| {
        f.handle_key_down("backspace", cx);
    });
    
    field.read(cx, |f, _| {
        assert_eq!(f.text(), "hell");
    });
}

#[gpui::test]
fn test_text_field_placeholder_shown_when_empty(cx: &mut TestAppContext) {
    let field = TextField::new(cx).placeholder("Enter text...");
    // Assert placeholder visible
}

#[gpui::test]
fn test_text_field_enter_calls_on_submit(cx: &mut TestAppContext) {
    let submitted = Rc::new(Cell::new(false));
    let submitted_clone = submitted.clone();
    
    let field = cx.new(|cx| {
        TextField::new(cx)
            .with_text("test")
            .on_submit(move |_, _| submitted_clone.set(true))
    });
    
    field.update(cx, |f, cx| {
        f.handle_key_down("enter", cx);
    });
    
    assert!(submitted.get());
}
```

### SecureTextField Tests

```rust
// tests/ui_gpui/components/secure_text_field_tests.rs

#[gpui::test]
fn test_secure_field_masks_text(cx: &mut TestAppContext) {
    let field = cx.new(|cx| SecureTextField::new(cx).with_text("secret"));
    
    // Assert displayed text is dots, not "secret"
    field.read(cx, |f, _| {
        assert_eq!(f.display_text(), "******");
        assert_eq!(f.actual_text(), "secret");
    });
}

#[gpui::test]
fn test_secure_field_toggle_reveals_text(cx: &mut TestAppContext) {
    let field = cx.new(|cx| SecureTextField::new(cx).with_text("secret"));
    
    field.update(cx, |f, cx| {
        f.toggle_mask(cx);
    });
    
    field.read(cx, |f, _| {
        assert_eq!(f.display_text(), "secret");
    });
}
```

### Dropdown Tests

```rust
// tests/ui_gpui/components/dropdown_tests.rs

#[gpui::test]
fn test_dropdown_shows_selected_value(cx: &mut TestAppContext) {
    let dropdown = Dropdown::new(vec!["A", "B", "C"]).selected(1);
    // Assert displays "B"
}

#[gpui::test]
fn test_dropdown_click_opens_overlay(cx: &mut TestAppContext) {
    let dropdown = cx.new(|cx| Dropdown::new(vec!["A", "B", "C"]));
    
    dropdown.update(cx, |d, cx| {
        d.handle_click(cx);
    });
    
    dropdown.read(cx, |d, _| {
        assert!(d.is_open());
    });
}

#[gpui::test]
fn test_dropdown_select_item_closes_overlay(cx: &mut TestAppContext) {
    let dropdown = cx.new(|cx| Dropdown::new(vec!["A", "B", "C"]));
    
    dropdown.update(cx, |d, cx| {
        d.handle_click(cx); // Open
        d.select_item(2, cx); // Select "C"
    });
    
    dropdown.read(cx, |d, _| {
        assert!(!d.is_open());
        assert_eq!(d.selected_index(), 2);
    });
}
```

### Toggle Tests

```rust
// tests/ui_gpui/components/toggle_tests.rs

#[gpui::test]
fn test_toggle_click_changes_state(cx: &mut TestAppContext) {
    let toggle = cx.new(|cx| Toggle::new(false));
    
    toggle.update(cx, |t, cx| {
        t.handle_click(cx);
    });
    
    toggle.read(cx, |t, _| {
        assert!(t.is_on());
    });
}

#[gpui::test]
fn test_toggle_calls_on_change(cx: &mut TestAppContext) {
    let changed_to = Rc::new(Cell::new(false));
    let changed_clone = changed_to.clone();
    
    let toggle = cx.new(|cx| {
        Toggle::new(false)
            .on_change(move |value, _| changed_clone.set(value))
    });
    
    toggle.update(cx, |t, cx| {
        t.handle_click(cx);
    });
    
    assert!(changed_to.get());
}
```

### List Tests

```rust
// tests/ui_gpui/components/list_tests.rs

#[gpui::test]
fn test_list_renders_items(cx: &mut TestAppContext) {
    let items = vec!["Item 1", "Item 2", "Item 3"];
    let list = List::new(items);
    // Assert 3 children rendered
}

#[gpui::test]
fn test_list_row_click_selects(cx: &mut TestAppContext) {
    let list = cx.new(|cx| List::new(vec!["A", "B", "C"]));
    
    list.update(cx, |l, cx| {
        l.select_row(1, cx);
    });
    
    list.read(cx, |l, _| {
        assert_eq!(l.selected_index(), Some(1));
    });
}

#[gpui::test]
fn test_list_row_click_calls_on_select(cx: &mut TestAppContext) {
    let selected = Rc::new(Cell::new(None));
    let selected_clone = selected.clone();
    
    let list = cx.new(|cx| {
        List::new(vec!["A", "B", "C"])
            .on_select(move |idx, _| selected_clone.set(Some(idx)))
    });
    
    list.update(cx, |l, cx| {
        l.select_row(2, cx);
    });
    
    assert_eq!(selected.get(), Some(2));
}
```

## Implementation

### Component Specifications

All components follow the mockup styling:

| Property | Value |
|----------|-------|
| Background (input) | #2a2a2a |
| Background (button) | #3a3a3a |
| Border | 1px #444444 |
| Border radius | 4-6px |
| Text color | #e5e5e5 |
| Muted text | #888888 |
| Placeholder | #666666 |
| Accent (active) | #2563eb |
| Danger | #ef4444 |

### Button Component

```rust
// src/ui_gpui/components/button.rs

pub struct Button {
    label: SharedString,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut WindowContext)>>,
    disabled: bool,
    variant: ButtonVariant,
}

pub enum ButtonVariant {
    Primary,    // #3a3a3a, main actions
    Secondary,  // Transparent, subtle
    Danger,     // Red tint, destructive
    Accent,     // Blue, prominent
}

impl Button {
    pub fn new(label: impl Into<SharedString>) -> Self { ... }
    pub fn on_click(self, handler: impl Fn(&ClickEvent, &mut WindowContext) + 'static) -> Self { ... }
    pub fn disabled(self, disabled: bool) -> Self { ... }
    pub fn variant(self, variant: ButtonVariant) -> Self { ... }
}

impl IntoElement for Button { ... }
```

### IconButton Component

```rust
// src/ui_gpui/components/icon_button.rs

pub struct IconButton {
    icon: SharedString,  // Single char like "T", "S", "H", "+"
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut WindowContext)>>,
    active: bool,
    tooltip: Option<SharedString>,
}

impl IconButton {
    pub fn new(icon: impl Into<SharedString>) -> Self { ... }
    // Size always 28x28 per mockup
}
```

### TextField Component

```rust
// src/ui_gpui/components/text_field.rs

pub struct TextField {
    text: String,
    placeholder: SharedString,
    focus_handle: FocusHandle,
    on_change: Option<Box<dyn Fn(&str, &mut WindowContext)>>,
    on_submit: Option<Box<dyn Fn(&str, &mut WindowContext)>>,
}

impl TextField {
    pub fn new(cx: &mut Context<Self>) -> Self { ... }
    pub fn with_text(self, text: impl Into<String>) -> Self { ... }
    pub fn placeholder(self, text: impl Into<SharedString>) -> Self { ... }
    pub fn on_change(self, handler: impl Fn(&str, &mut WindowContext) + 'static) -> Self { ... }
    pub fn on_submit(self, handler: impl Fn(&str, &mut WindowContext) + 'static) -> Self { ... }
    
    pub fn text(&self) -> &str { ... }
    pub fn set_text(&mut self, text: String, cx: &mut Context<Self>) { ... }
    pub fn handle_key_down(&mut self, key: &str, cx: &mut Context<Self>) { ... }
}

impl Focusable for TextField { ... }
impl Render for TextField { ... }
```

### SecureTextField Component

```rust
// src/ui_gpui/components/secure_text_field.rs

pub struct SecureTextField {
    inner: TextField,
    masked: bool,
}

impl SecureTextField {
    pub fn display_text(&self) -> String {
        if self.masked {
            "*".repeat(self.inner.text().len())
        } else {
            self.inner.text().to_string()
        }
    }
    
    pub fn actual_text(&self) -> &str {
        self.inner.text()
    }
    
    pub fn toggle_mask(&mut self, cx: &mut Context<Self>) {
        self.masked = !self.masked;
        cx.notify();
    }
}
```

### Dropdown Component

```rust
// src/ui_gpui/components/dropdown.rs

pub struct Dropdown {
    options: Vec<SharedString>,
    selected_index: usize,
    is_open: bool,
    on_select: Option<Box<dyn Fn(usize, &mut WindowContext)>>,
}

impl Dropdown {
    pub fn new(options: Vec<impl Into<SharedString>>) -> Self { ... }
    pub fn selected(self, index: usize) -> Self { ... }
    pub fn on_select(self, handler: impl Fn(usize, &mut WindowContext) + 'static) -> Self { ... }
}
```

### Toggle Component

```rust
// src/ui_gpui/components/toggle.rs

pub struct Toggle {
    is_on: bool,
    on_change: Option<Box<dyn Fn(bool, &mut WindowContext)>>,
}
```

### List Component

```rust
// src/ui_gpui/components/list.rs

pub struct List<T> {
    items: Vec<T>,
    selected_index: Option<usize>,
    render_item: Box<dyn Fn(&T, bool) -> impl IntoElement>,
    on_select: Option<Box<dyn Fn(usize, &mut WindowContext)>>,
}
```

## Component Module Export

```rust
// src/ui_gpui/components/mod.rs

mod button;
mod icon_button;
mod text_field;
mod secure_text_field;
mod text_area;
mod dropdown;
mod toggle;
mod checkbox;
mod stepper;
mod list;
mod list_row;
mod badge;
mod divider;
mod spinner;

pub use button::{Button, ButtonVariant};
pub use icon_button::IconButton;
pub use text_field::TextField;
pub use secure_text_field::SecureTextField;
pub use text_area::TextArea;
pub use dropdown::Dropdown;
pub use toggle::Toggle;
pub use checkbox::Checkbox;
pub use stepper::Stepper;
pub use list::List;
pub use list_row::ListRow;
pub use badge::Badge;
pub use divider::Divider;
pub use spinner::Spinner;
```

## Verification Checklist

- [ ] All Button tests pass
- [ ] All TextField tests pass
- [ ] All SecureTextField tests pass
- [ ] All Dropdown tests pass
- [ ] All Toggle tests pass
- [ ] All List tests pass
- [ ] Components use correct colors from Theme
- [ ] Components have correct sizing per mockup
- [ ] Focus handling works (TextField, SecureTextField)
- [ ] Click handlers fire correctly
- [ ] Disabled state prevents interaction

## Files Created

| File | Purpose |
|------|---------|
| `src/ui_gpui/components/mod.rs` | Module exports |
| `src/ui_gpui/components/button.rs` | Button + IconButton |
| `src/ui_gpui/components/text_field.rs` | Text input |
| `src/ui_gpui/components/secure_text_field.rs` | Masked input |
| `src/ui_gpui/components/text_area.rs` | Multi-line input |
| `src/ui_gpui/components/dropdown.rs` | Select dropdown |
| `src/ui_gpui/components/toggle.rs` | On/off switch |
| `src/ui_gpui/components/checkbox.rs` | Checkbox with label |
| `src/ui_gpui/components/stepper.rs` | Number +/- |
| `src/ui_gpui/components/list.rs` | Scrollable list |
| `src/ui_gpui/components/list_row.rs` | List item |
| `src/ui_gpui/components/badge.rs` | Colored pill |
| `src/ui_gpui/components/divider.rs` | Horizontal line |
| `src/ui_gpui/components/spinner.rs` | Loading indicator |
| `tests/ui_gpui/components/*.rs` | Component tests |
