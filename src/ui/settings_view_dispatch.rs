use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSLayoutConstraintOrientation, NSScrollView, NSStackView, NSTextField, NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use uuid::Uuid;

use super::settings_view::SettingsViewController;
use super::Theme;
use crate::ui::settings_view_helpers::{build_list_box, create_mcp_row, create_profile_row};

pub fn build_content_area(
    controller: &SettingsViewController,
    mtm: MainThreadMarker,
) -> Retained<NSScrollView> {
    let scroll_view = build_scroll_view(mtm);
    let content_stack = build_content_stack(mtm);

    let profiles_section = build_profiles_section(controller, mtm);
    let mcps_section = build_mcps_section(controller, mtm);
    let hotkey_section = build_hotkey_section(controller, mtm);

    unsafe {
        content_stack.addArrangedSubview(&profiles_section);
        content_stack.addArrangedSubview(&mcps_section);
        content_stack.addArrangedSubview(&hotkey_section);
    }

    scroll_view.setDocumentView(Some(&content_stack));

    *controller.ivars().scroll_view.borrow_mut() = Some(scroll_view.clone());
    scroll_view
}

fn build_scroll_view(mtm: MainThreadMarker) -> Retained<NSScrollView> {
    let scroll_view = NSScrollView::new(mtm);
    scroll_view.setHasVerticalScroller(true);
    scroll_view.setDrawsBackground(false);
    unsafe {
        scroll_view.setAutohidesScrollers(true);
        scroll_view.setTranslatesAutoresizingMaskIntoConstraints(false);
        scroll_view
            .setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
        scroll_view.setContentCompressionResistancePriority_forOrientation(
            250.0,
            NSLayoutConstraintOrientation::Vertical,
        );
    }
    scroll_view
}

fn build_content_stack(mtm: MainThreadMarker) -> Retained<NSStackView> {
    let content_stack = NSStackView::new(mtm);
    unsafe {
        content_stack.setOrientation(objc2_app_kit::NSUserInterfaceLayoutOrientation::Vertical);
        content_stack.setSpacing(16.0);
        content_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
        content_stack.setDistribution(objc2_app_kit::NSStackViewDistribution::Fill);
        content_stack.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: 16.0,
            left: 14.0,
            bottom: 16.0,
            right: 14.0,
        });
    }

    content_stack.setWantsLayer(true);
    if let Some(layer) = content_stack.layer() {
        crate::ui::settings_view::set_layer_background_color(
            &layer,
            Theme::BG_DARKEST.0,
            Theme::BG_DARKEST.1,
            Theme::BG_DARKEST.2,
        );
    }

    content_stack
}

fn build_profiles_section(
    controller: &SettingsViewController,
    mtm: MainThreadMarker,
) -> Retained<NSView> {
    let section = NSStackView::new(mtm);
    unsafe {
        section.setOrientation(objc2_app_kit::NSUserInterfaceLayoutOrientation::Vertical);
        section.setSpacing(8.0);
        section.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
    }

    let header = build_section_header("PROFILES", mtm);
    unsafe {
        section.addArrangedSubview(&header);
    }

    let (list_container, list_stack, toolbar) = build_list_box(controller, 120.0, mtm);
    controller.setup_profiles_toolbar(&toolbar, mtm);

    unsafe {
        section.addArrangedSubview(&list_container);
        section.addArrangedSubview(&toolbar);
    }

    *controller.ivars().profiles_list.borrow_mut() = Some(list_stack);
    *controller.ivars().profiles_toolbar.borrow_mut() = Some(toolbar);

    Retained::from(&*section as &NSView)
}

fn build_mcps_section(
    controller: &SettingsViewController,
    mtm: MainThreadMarker,
) -> Retained<NSView> {
    let section = NSStackView::new(mtm);
    unsafe {
        section.setOrientation(objc2_app_kit::NSUserInterfaceLayoutOrientation::Vertical);
        section.setSpacing(8.0);
        section.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
    }

    let header = build_section_header("MCP TOOLS", mtm);
    unsafe {
        section.addArrangedSubview(&header);
    }

    let (list_container, list_stack, toolbar) = build_list_box(controller, 120.0, mtm);
    controller.setup_mcps_toolbar(&toolbar, mtm);

    unsafe {
        section.addArrangedSubview(&list_container);
        section.addArrangedSubview(&toolbar);
    }

    *controller.ivars().mcps_list.borrow_mut() = Some(list_stack);
    *controller.ivars().mcps_toolbar.borrow_mut() = Some(toolbar);

    Retained::from(&*section as &NSView)
}

fn build_hotkey_section(
    controller: &SettingsViewController,
    mtm: MainThreadMarker,
) -> Retained<NSView> {
    let section = NSStackView::new(mtm);
    unsafe {
        section.setOrientation(objc2_app_kit::NSUserInterfaceLayoutOrientation::Vertical);
        section.setSpacing(6.0);
        section.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
    }

    let header = build_section_header("GLOBAL HOTKEY", mtm);
    unsafe {
        section.addArrangedSubview(&header);
    }

    let field = build_hotkey_field(controller, mtm);
    unsafe {
        section.addArrangedSubview(&field);
    }

    Retained::from(&*section as &NSView)
}

fn build_section_header(text: &str, mtm: MainThreadMarker) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&objc2_foundation::NSString::from_str(text), mtm);
    label.setTextColor(Some(&Theme::text_secondary_color()));
    label.setFont(Some(&objc2_app_kit::NSFont::systemFontOfSize(11.0)));
    label
}

fn build_hotkey_field(
    controller: &SettingsViewController,
    mtm: MainThreadMarker,
) -> Retained<NSView> {
    let field = NSTextField::initWithFrame(
        NSTextField::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(372.0, 24.0)),
    );
    field.setPlaceholderString(Some(&objc2_foundation::NSString::from_str("Cmd+Shift+P")));
    field.setBackgroundColor(Some(&Theme::bg_darker()));
    field.setTextColor(Some(&Theme::text_primary()));
    field.setDrawsBackground(true);
    field.setBordered(true);
    unsafe {
        field.setTranslatesAutoresizingMaskIntoConstraints(false);
        let width_constraint = field.widthAnchor().constraintEqualToConstant(372.0);
        width_constraint.setActive(true);
    }

    *controller.ivars().hotkey_field.borrow_mut() = Some(field.clone());

    Retained::from(&*field as &NSView)
}

pub fn build_profile_rows(
    controller: &SettingsViewController,
    config_profiles: &[personal_agent::models::ModelProfile],
    mtm: MainThreadMarker,
) -> Vec<Retained<NSView>> {
    let mut rows = Vec::new();
    for (index, profile) in config_profiles.iter().enumerate() {
        rows.push(create_profile_row(controller, profile, index, mtm));
    }
    rows
}

pub fn build_mcp_rows(
    controller: &SettingsViewController,
    config_mcps: &[personal_agent::mcp::McpConfig],
    mtm: MainThreadMarker,
) -> Vec<Retained<NSView>> {
    let mut rows = Vec::new();
    for (index, mcp) in config_mcps.iter().enumerate() {
        rows.push(create_mcp_row(controller, mcp, index, mtm));
    }
    rows
}

pub fn sync_profile_selection(controller: &SettingsViewController, profile_ids: &[Uuid]) {
    *controller.ivars().profile_uuid_map.borrow_mut() = profile_ids.to_vec();
    *controller.ivars().selected_profile_id.borrow_mut() = None;
}

pub fn sync_mcp_selection(controller: &SettingsViewController, mcp_ids: &[Uuid]) {
    *controller.ivars().mcp_uuid_map.borrow_mut() = mcp_ids.to_vec();
    *controller.ivars().selected_mcp_id.borrow_mut() = None;
}
