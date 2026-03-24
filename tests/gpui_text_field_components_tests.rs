#![allow(clippy::future_not_send, clippy::unused_async)]

use gpui::{AppContext, Focusable, Render, TestAppContext};
use personal_agent::ui_gpui::components::{SecureTextField, TextField};
use std::sync::{Arc, Mutex};

#[gpui::test]
async fn text_field_tracks_text_placeholder_and_change_callback(cx: &mut TestAppContext) {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let observed_for_callback = Arc::clone(&observed);

    let field = cx.new(|cx| {
        TextField::new(cx)
            .placeholder("Search")
            .with_text("initial")
            .on_change(move |text, _cx| {
                observed_for_callback
                    .lock()
                    .expect("callback lock")
                    .push(text.to_string());
            })
    });

    assert_eq!(
        field.read_with(cx, |field: &TextField, _| field.text()),
        "initial"
    );

    field.update(cx, |field: &mut TextField, cx| {
        field.set_text("updated", cx);
    });
    cx.run_until_parked();

    assert_eq!(
        field.read_with(cx, |field: &TextField, _| field.text()),
        "updated"
    );
    assert_eq!(
        observed.lock().expect("observed lock").as_slice(),
        ["updated"]
    );

    field.read_with(cx, |field: &TextField, app| {
        let _ = Focusable::focus_handle(field, app);
    });
}

#[gpui::test]
async fn secure_text_field_tracks_masking_text_and_change_callback(cx: &mut TestAppContext) {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let observed_for_callback = Arc::clone(&observed);

    let field = cx.new(|cx| {
        SecureTextField::new(cx)
            .placeholder("API key")
            .with_text("secret")
            .on_change(move |text, _cx| {
                observed_for_callback
                    .lock()
                    .expect("callback lock")
                    .push(text.to_string());
            })
    });

    assert_eq!(
        field.read_with(cx, |field: &SecureTextField, _| field.actual_text()),
        "secret"
    );
    assert_eq!(
        field.read_with(cx, |field: &SecureTextField, _| field.display_text()),
        "******"
    );

    field.update(cx, |field: &mut SecureTextField, cx| {
        field.toggle_mask(cx);
    });
    cx.run_until_parked();

    assert_eq!(
        field.read_with(cx, |field: &SecureTextField, _| field.display_text()),
        "secret"
    );

    field.update(cx, |field: &mut SecureTextField, cx| {
        field.set_text("changed", cx);
    });
    cx.run_until_parked();

    assert_eq!(
        field.read_with(cx, |field: &SecureTextField, _| field.actual_text()),
        "changed"
    );
    assert_eq!(
        field.read_with(cx, |field: &SecureTextField, _| field.display_text()),
        "changed"
    );
    assert_eq!(
        observed.lock().expect("observed lock").as_slice(),
        ["changed"]
    );

    field.update(cx, |field: &mut SecureTextField, cx| {
        field.toggle_mask(cx);
    });
    cx.run_until_parked();

    assert_eq!(
        field.read_with(cx, |field: &SecureTextField, _| field.display_text()),
        "*******"
    );

    field.read_with(cx, |field: &SecureTextField, app| {
        let _ = Focusable::focus_handle(field, app);
    });
}

#[gpui::test]
async fn text_field_and_secure_text_field_render_without_panicking(cx: &mut TestAppContext) {
    let text_field = cx.new(|cx| TextField::new(cx).placeholder("Name").with_text("Alice"));
    let secure_field = cx.new(|cx| {
        SecureTextField::new(cx)
            .placeholder("Token")
            .with_text("secret-token")
    });

    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|window, app| {
        text_field.update(app, |field: &mut TextField, cx| {
            let _ = field.render(window, cx);
        });
        secure_field.update(app, |field: &mut SecureTextField, cx| {
            let _ = field.render(window, cx);
        });
    });
}
