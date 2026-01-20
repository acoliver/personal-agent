use personal_agent::main_utils::{
    create_dark_style, create_native_options, load_icon_from_bytes, PANEL_HEIGHT, PANEL_WIDTH,
};

#[test]
fn native_options_keep_panel_hidden() {
    let options = create_native_options();
    assert_eq!(options.viewport.resizable, Some(false));
    assert_eq!(options.viewport.visible, Some(false));
    assert_eq!(
        options.viewport.inner_size,
        Some(eframe::egui::Vec2::new(PANEL_WIDTH, PANEL_HEIGHT))
    );
}

#[test]
fn dark_style_sets_background() {
    let style = create_dark_style();
    assert_eq!(
        style.visuals.window_fill,
        eframe::egui::Color32::from_rgb(13, 13, 13)
    );
}

#[test]
fn icon_bytes_create_valid_icon() {
    let icon = load_icon_from_bytes(include_bytes!("../assets/MenuIcon.imageset/icon-32.png"));
    assert!(icon.is_ok());
}
