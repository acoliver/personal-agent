#![cfg(target_os = "linux")]

//! Linux-only compile/link sanity checks for issue #43.

#[test]
fn linux_build_sanity() {
    // This intentionally exercises no runtime behavior.
    // The value is in guaranteeing this target-specific test binary
    // compiles and links on Linux in CI.
}

#[test]
fn popup_creation_does_not_force_delayed_focus() {
    let source = include_str!("../src/main_gpui/system_tray.rs");
    let start = source.find("fn open_popup").expect("open_popup must exist");
    let end = source[start..]
        .find("fn open_popout")
        .map_or(source.len(), |offset| start + offset);
    let open_popup = &source[start..end];

    assert!(open_popup.contains("focus: false"));
    assert!(!open_popup.contains("cx.activate(true)"));
    assert!(!open_popup.contains("window.activate_window()"));
    assert!(open_popup.contains("kind: WindowKind::PopUp"));
}
