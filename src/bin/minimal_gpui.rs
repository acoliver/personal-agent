//! Minimal GPUI button test - directly from GPUI window.rs example pattern
//! 

use gpui::*;

struct MinimalDemo;

fn button(text: &'static str) -> impl IntoElement {
    div()
        .id(text)  // Critical: ID is required for on_click
        .flex_none()
        .px_2()
        .py_1()
        .bg(rgb(0x444444))
        .active(|this| this.bg(rgb(0x666666)))
        .hover(|this| this.bg(rgb(0x555555)))
        .border_1()
        .border_color(rgb(0x666666))
        .rounded_sm()
        .cursor_pointer()
        .text_color(rgb(0xffffff))
        .child(text)
        .on_click(move |_, window, _cx| {
            // Use window to print - this is how GPUI examples do it
            println!(">>> BUTTON CLICKED: {} <<<", text);
            // Force refresh
            window.refresh();
        })
}

impl Render for MinimalDemo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .p_4()
            .gap_4()
            .bg(rgb(0x1a1a1a))
            .size_full()
            .child(
                div()
                    .text_xl()
                    .text_color(rgb(0xffffff))
                    .child("GPUI Click Test")
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(button("Button A"))
                    .child(button("Button B"))
                    .child(button("History"))
                    .child(button("Settings"))
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x888888))
                    .child("Click buttons - watch terminal for output")
            )
    }
}

fn main() {
    println!("Starting GPUI button test...");
    println!("This uses the exact same pattern as GPUI examples");
    
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(400.0), px(200.0)), cx);
        
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                show: true,
                ..Default::default()
            },
            |_window, cx| {
                cx.new(|_| MinimalDemo)
            },
        )
        .unwrap();
        
        // This is critical - activate the app
        cx.activate(true);
        
        println!("Window opened - try clicking buttons now!");
    });
}
