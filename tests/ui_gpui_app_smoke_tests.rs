use personal_agent::events::EventBus;
use personal_agent::ui_gpui::app::GpuiApp;
use std::sync::Arc;

#[test]
fn gpui_app_non_initialized_controls_are_safe_noops() {
    let event_bus = Arc::new(EventBus::new(16));
    let mut app = GpuiApp::new(Arc::clone(&event_bus)).expect("gpui app should construct");

    assert!(!app.is_popup_visible());

    app.toggle_popup();
    app.show_popup();
    app.hide_popup();
    app.shutdown();

    assert!(!app.is_popup_visible());
    assert!(Arc::strong_count(&app.gpui_bridge()) >= 2);
    app.start_event_forwarding()
        .expect("event forwarding should be a no-op success");
}
