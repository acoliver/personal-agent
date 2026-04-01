use gpui::{hsla, IntoElement};
use personal_agent::ui_gpui::components::{
    Divider, Dropdown, IconButton, Toggle, TopBar, TopBarButton,
};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn divider_supports_default_and_custom_color() {
    let _ = Divider::new().into_element();
    let _ = Divider::default().into_element();
    let _ = Divider::new()
        .color(hsla(0.5, 0.4, 0.3, 1.0))
        .into_element();
}

#[test]
fn dropdown_tracks_selection_toggle_and_callback() {
    let selected = Rc::new(RefCell::new(None));
    let selected_for_callback = Rc::clone(&selected);

    let dropdown = Dropdown::new(vec!["Alpha".into(), "Beta".into(), "Gamma".into()])
        .selected(99)
        .on_select(move |index| {
            *selected_for_callback.borrow_mut() = Some(index);
        });

    assert_eq!(dropdown.selected_index(), 2);
    assert!(!dropdown.is_open());

    dropdown.handle_click();
    assert!(dropdown.is_open());

    dropdown.select_item(1);
    assert_eq!(dropdown.selected_index(), 1);
    assert!(!dropdown.is_open());
    assert_eq!(*selected.borrow(), Some(1));

    let _ = dropdown.into_element();
}

#[test]
fn dropdown_renders_closed_and_open_states() {
    let closed = Dropdown::new(vec!["One".into(), "Two".into()]).selected(0);
    let _ = closed.into_element();

    let open = Dropdown::new(vec!["One".into(), "Two".into()]).selected(1);
    open.handle_click();
    assert!(open.is_open());
    let _ = open.into_element();
}

#[test]
fn icon_button_supports_builder_variants() {
    let _ = IconButton::new("+").into_element();
    let _ = IconButton::new("+").active(true).into_element();
    let _ = IconButton::new("+")
        .tooltip("Add item")
        .on_click(|| {})
        .into_element();
}

#[test]
fn toggle_flips_state_and_notifies_callback() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let observed_for_callback = Rc::clone(&observed);

    let toggle = Toggle::new(false).on_change(move |value| {
        observed_for_callback.borrow_mut().push(value);
    });

    assert!(!toggle.is_on());
    let _ = Toggle::new(true).into_element();

    toggle.handle_click();
    assert!(toggle.is_on());
    toggle.handle_click();
    assert!(!toggle.is_on());
    assert_eq!(&*observed.borrow(), &[true, false]);

    let _ = toggle.into_element();
}

#[test]
fn top_bar_supports_optional_left_and_multiple_right_buttons() {
    let _ = TopBar::new("Settings").into_element();

    let top_bar = TopBar::new("Profiles")
        .left_button(TopBarButton::new("Back").on_click(|| {}))
        .right_button(TopBarButton::new("Add").on_click(|| {}))
        .right_buttons(vec![
            TopBarButton::new("Save").on_click(|| {}),
            TopBarButton::new("Close"),
        ]);

    let _ = top_bar.into_element();
}
