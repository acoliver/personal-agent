//! Main-thread GUI integration tests for `PopupWindow`, `TrayBridge`, and `GpuiApp`.
//!
//! A custom harness (`harness = false`) is used so that `fn main()` — which IS the
//! macOS main thread — runs the test bodies directly. In CI the binary exits 0.
#![allow(clippy::too_many_lines)]

#[cfg(ci)]
fn main() {
    println!("gui_main_thread: skipped in CI (no window server)");
}

#[cfg(not(ci))]
fn main() {
    let mut passed = 0u32;
    let mut failed = 0u32;

    run_test(
        "popup_window::creation_and_visibility",
        &mut passed,
        &mut failed,
        test_popup_creation,
    );
    run_test(
        "popup_window::esc_and_resign_key_hide",
        &mut passed,
        &mut failed,
        test_popup_esc_resign,
    );
    run_test(
        "popup_window::bridge_is_shared",
        &mut passed,
        &mut failed,
        test_popup_bridge_shared,
    );
    run_test(
        "tray_bridge::creation_and_visibility",
        &mut passed,
        &mut failed,
        test_tray_creation,
    );
    run_test(
        "tray_bridge::toggle_popup_flips",
        &mut passed,
        &mut failed,
        test_tray_toggle,
    );
    run_test(
        "tray_bridge::click_outside_hides",
        &mut passed,
        &mut failed,
        test_tray_click_outside,
    );
    run_test(
        "tray_bridge::set_popup_window",
        &mut passed,
        &mut failed,
        test_tray_set_popup,
    );
    run_test(
        "gpui_app::init_and_event_forwarding",
        &mut passed,
        &mut failed,
        test_app_init,
    );
    run_test(
        "gpui_app::toggle_shutdown",
        &mut passed,
        &mut failed,
        test_app_toggle_shutdown,
    );

    println!("\ntest result: {passed} passed; {failed} failed");
    if failed > 0 {
        std::process::exit(1);
    }
}

#[cfg(not(ci))]
fn run_test(name: &str, passed: &mut u32, failed: &mut u32, f: fn()) {
    print!("test {name} ... ");
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_ok() {
        println!("ok");
        *passed += 1;
    } else {
        println!("FAILED");
        *failed += 1;
    }
}

// ── Helpers ──────────────────────────────────────────────────────

#[cfg(not(ci))]
fn make_bridge() -> std::sync::Arc<personal_agent::ui_gpui::bridge::GpuiBridge> {
    let (user_tx, _) = flume::unbounded();
    let (_, view_rx) = flume::unbounded();
    std::sync::Arc::new(personal_agent::ui_gpui::bridge::GpuiBridge::new(
        user_tx, view_rx,
    ))
}

// ── PopupWindow tests ────────────────────────────────────────────

#[cfg(not(ci))]
fn test_popup_creation() {
    use personal_agent::ui_gpui::popup_window::PopupWindow;
    let mut popup = PopupWindow::new(make_bridge()).expect("PopupWindow::new");
    assert!(!popup.is_visible(), "should start hidden");
    popup.show();
    assert!(popup.is_visible(), "show makes visible");
    popup.hide();
    assert!(!popup.is_visible(), "hide makes invisible");
}

#[cfg(not(ci))]
fn test_popup_esc_resign() {
    use personal_agent::ui_gpui::popup_window::PopupWindow;
    let mut popup = PopupWindow::new(make_bridge()).expect("PopupWindow::new");
    popup.show();
    popup.handle_esc_key();
    assert!(!popup.is_visible(), "esc hides");
    popup.show();
    popup.handle_resign_key();
    assert!(!popup.is_visible(), "resign hides");
}

#[cfg(not(ci))]
fn test_popup_bridge_shared() {
    use personal_agent::ui_gpui::popup_window::PopupWindow;
    use std::sync::Arc;
    let bridge = make_bridge();
    let popup = PopupWindow::new(Arc::clone(&bridge)).expect("PopupWindow::new");
    assert!(Arc::ptr_eq(&popup.gpui_bridge(), &bridge));
}

// ── TrayBridge tests ─────────────────────────────────────────────

#[cfg(not(ci))]
fn test_tray_creation() {
    use personal_agent::ui_gpui::tray_bridge::TrayBridge;
    let tray = TrayBridge::new(make_bridge()).expect("TrayBridge::new");
    assert!(!tray.is_popup_visible());
}

#[cfg(not(ci))]
fn test_tray_toggle() {
    use personal_agent::ui_gpui::tray_bridge::TrayBridge;
    let tray = TrayBridge::new(make_bridge()).expect("TrayBridge::new");
    assert!(!tray.is_popup_visible());
    tray.toggle_popup();
    assert!(tray.is_popup_visible());
    tray.toggle_popup();
    assert!(!tray.is_popup_visible());
}

#[cfg(not(ci))]
fn test_tray_click_outside() {
    use personal_agent::ui_gpui::tray_bridge::TrayBridge;
    let tray = TrayBridge::new(make_bridge()).expect("TrayBridge::new");
    tray.toggle_popup();
    assert!(tray.is_popup_visible());
    tray.handle_click_outside();
    assert!(!tray.is_popup_visible());
}

#[cfg(not(ci))]
fn test_tray_set_popup() {
    use personal_agent::ui_gpui::popup_window::PopupWindow;
    use personal_agent::ui_gpui::tray_bridge::TrayBridge;
    use std::sync::Arc;
    let bridge = make_bridge();
    let tray = TrayBridge::new(Arc::clone(&bridge)).expect("TrayBridge::new");
    let popup = PopupWindow::new(Arc::clone(&bridge)).expect("PopupWindow::new");
    tray.set_popup_window(popup);
    assert!(!tray.is_popup_visible());
}

// ── GpuiApp tests ────────────────────────────────────────────────

#[cfg(not(ci))]
fn test_app_init() {
    use personal_agent::events::EventBus;
    use personal_agent::ui_gpui::app::GpuiApp;
    use std::sync::Arc;
    let mut app = GpuiApp::new(Arc::new(EventBus::new(100))).expect("GpuiApp::new");
    app.initialize().expect("initialize");
    assert!(!app.is_popup_visible());
    app.start_event_forwarding()
        .expect("start_event_forwarding");
}

#[cfg(not(ci))]
fn test_app_toggle_shutdown() {
    use personal_agent::events::EventBus;
    use personal_agent::ui_gpui::app::GpuiApp;
    use std::sync::Arc;
    let mut app = GpuiApp::new(Arc::new(EventBus::new(100))).expect("GpuiApp::new");
    app.initialize().expect("initialize");
    assert!(!app.is_popup_visible());
    app.toggle_popup();
    app.shutdown();
}
