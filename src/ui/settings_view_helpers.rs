use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSButtonType, NSControlStateValueOff, NSControlStateValueOn, NSFont,
    NSLayoutConstraintOrientation, NSScrollView, NSStackView, NSStackViewDistribution, NSSwitch,
    NSTextField, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSEdgeInsets, NSPoint, NSRect, NSSize, NSString};
use std::convert::TryFrom;

use crate::ui::{set_layer_background_color, set_layer_border, set_layer_corner_radius, Theme};

use personal_agent::mcp::{McpConfig, McpService, McpSource, McpStatus};
use personal_agent::models::ModelProfile;

use super::SettingsViewController;

pub fn build_list_box(
    _controller: &SettingsViewController,
    height: f64,
    mtm: MainThreadMarker,
) -> (
    Retained<NSView>,
    Retained<super::FlippedStackView>,
    Retained<NSView>,
) {
    let container = build_list_container(mtm);
    let scroll_view = build_list_scroll_view(mtm);
    let list_stack = build_list_stack(mtm);
    scroll_view.setDocumentView(Some(&list_stack));

    let toolbar = build_toolbar_view(mtm);

    container.addSubview(&scroll_view);
    container.addSubview(&toolbar);

    apply_list_box_constraints(&container, &scroll_view, &list_stack, &toolbar, height);

    (container, list_stack, Retained::from(&*toolbar as &NSView))
}

fn index_tag(index: usize) -> isize {
    isize::try_from(index).unwrap_or(isize::MAX)
}

fn build_list_container(mtm: MainThreadMarker) -> Retained<NSView> {
    let container = NSView::new(mtm);
    container.setTranslatesAutoresizingMaskIntoConstraints(false);

    container.setWantsLayer(true);
    if let Some(layer) = container.layer() {
        set_layer_background_color(
            &layer,
            Theme::BG_DARKER.0,
            Theme::BG_DARKER.1,
            Theme::BG_DARKER.2,
        );
        set_layer_corner_radius(&layer, 4.0);
        set_layer_border(&layer, 1.0, 0.3, 0.3, 0.3);
    }

    container
}

fn build_list_scroll_view(mtm: MainThreadMarker) -> Retained<NSScrollView> {
    let scroll_view = NSScrollView::new(mtm);
    scroll_view.setHasVerticalScroller(true);
    scroll_view.setDrawsBackground(false);
    scroll_view.setTranslatesAutoresizingMaskIntoConstraints(false);
    unsafe {
        scroll_view.setAutohidesScrollers(true);
    }
    scroll_view
}

fn build_list_stack(mtm: MainThreadMarker) -> Retained<super::FlippedStackView> {
    let list_stack = super::FlippedStackView::new(mtm);
    unsafe {
        list_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        list_stack.setSpacing(1.0);
        list_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Width);
        list_stack.setDistribution(NSStackViewDistribution::Fill);
        list_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
    }
    list_stack
}

fn build_toolbar_view(mtm: MainThreadMarker) -> Retained<NSStackView> {
    let toolbar = NSStackView::new(mtm);
    unsafe {
        toolbar.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        toolbar.setSpacing(4.0);
        toolbar.setTranslatesAutoresizingMaskIntoConstraints(false);
        toolbar.setEdgeInsets(NSEdgeInsets {
            top: 4.0,
            left: 4.0,
            bottom: 4.0,
            right: 4.0,
        });
    }

    toolbar.setWantsLayer(true);
    if let Some(layer) = toolbar.layer() {
        set_layer_background_color(&layer, Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2);
    }

    toolbar
}

fn apply_list_box_constraints(
    container: &NSView,
    scroll_view: &NSScrollView,
    list_stack: &super::FlippedStackView,
    toolbar: &NSStackView,
    height: f64,
) {
    unsafe {
        let container_height = container
            .heightAnchor()
            .constraintEqualToConstant(height + 28.0);
        container_height.setActive(true);

        let sv_top = scroll_view
            .topAnchor()
            .constraintEqualToAnchor(&container.topAnchor());
        sv_top.setActive(true);
        let sv_left = scroll_view
            .leadingAnchor()
            .constraintEqualToAnchor(&container.leadingAnchor());
        sv_left.setActive(true);
        let sv_right = scroll_view
            .trailingAnchor()
            .constraintEqualToAnchor(&container.trailingAnchor());
        sv_right.setActive(true);
        let sv_height = scroll_view.heightAnchor().constraintEqualToConstant(height);
        sv_height.setActive(true);

        let tb_bottom = toolbar
            .bottomAnchor()
            .constraintEqualToAnchor(&container.bottomAnchor());
        tb_bottom.setActive(true);
        let tb_left = toolbar
            .leadingAnchor()
            .constraintEqualToAnchor(&container.leadingAnchor());
        tb_left.setActive(true);
        let tb_right = toolbar
            .trailingAnchor()
            .constraintEqualToAnchor(&container.trailingAnchor());
        tb_right.setActive(true);
        let tb_height = toolbar.heightAnchor().constraintEqualToConstant(28.0);
        tb_height.setActive(true);

        let content_view = scroll_view.contentView();
        let ls_width = list_stack
            .widthAnchor()
            .constraintEqualToAnchor(&content_view.widthAnchor());
        ls_width.setActive(true);
    }
}

pub fn create_profile_row(
    controller: &SettingsViewController,
    profile: &ModelProfile,
    index: usize,
    mtm: MainThreadMarker,
) -> Retained<NSView> {
    let row_btn = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str(""),
            Some(controller),
            Some(sel!(profileRowClicked:)),
            mtm,
        )
    };
    row_btn.setBezelStyle(NSBezelStyle::Automatic);
    row_btn.setBordered(false);
    row_btn.setTag(index_tag(index));

    let row = NSStackView::new(mtm);
    unsafe {
        row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        row.setSpacing(8.0);
        row.setTranslatesAutoresizingMaskIntoConstraints(false);
        row.setEdgeInsets(NSEdgeInsets {
            top: 4.0,
            left: 8.0,
            bottom: 4.0,
            right: 8.0,
        });
    }

    row.setWantsLayer(true);
    if let Some(layer) = row.layer() {
        set_layer_background_color(
            &layer,
            Theme::BG_DARKER.0,
            Theme::BG_DARKER.1,
            Theme::BG_DARKER.2,
        );
    }

    unsafe {
        let height_constraint = row.heightAnchor().constraintEqualToConstant(24.0);
        height_constraint.setActive(true);
    }

    let text = format!(
        "{} ({}:{})",
        profile.name, profile.provider_id, profile.model_id
    );
    let label = NSTextField::labelWithString(&NSString::from_str(&text), mtm);
    label.setTextColor(Some(&Theme::text_primary()));
    label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
    unsafe {
        label.setContentHuggingPriority_forOrientation(
            1.0,
            NSLayoutConstraintOrientation::Horizontal,
        );
        row.addArrangedSubview(&label);
    }

    row_btn.addSubview(&row);
    constrain_container_to_button(&row_btn, &row);
    set_row_button_height(&row_btn);

    Retained::from(&*row_btn as &NSView)
}

pub fn create_mcp_row(
    controller: &SettingsViewController,
    mcp: &McpConfig,
    index: usize,
    mtm: MainThreadMarker,
) -> Retained<NSView> {
    let row_btn = build_mcp_row_button(controller, index, mtm);
    apply_row_button_background(&row_btn);

    let container = build_mcp_row_container(mtm);
    let status_view = build_mcp_status_indicator(mtm);
    apply_mcp_status_color(&status_view, mcp);
    unsafe {
        container.addArrangedSubview(&status_view);
    }

    let label = build_mcp_row_label(mcp, mtm);
    unsafe {
        label.setContentHuggingPriority_forOrientation(
            1.0,
            NSLayoutConstraintOrientation::Horizontal,
        );
        container.addArrangedSubview(&label);
    }

    let toggle = build_mcp_toggle(controller, mcp, index, mtm);
    unsafe {
        container.addArrangedSubview(&toggle);
    }

    row_btn.addSubview(&container);
    constrain_container_to_button(&row_btn, &container);
    set_row_button_height(&row_btn);

    Retained::from(&*row_btn as &NSView)
}

fn build_mcp_row_button(
    controller: &SettingsViewController,
    index: usize,
    mtm: MainThreadMarker,
) -> Retained<NSButton> {
    let row_btn = NSButton::initWithFrame(
        NSButton::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(360.0, 32.0)),
    );
    row_btn.setButtonType(NSButtonType::MomentaryLight);
    row_btn.setBezelStyle(NSBezelStyle::SmallSquare);
    row_btn.setBordered(false);
    row_btn.setTitle(&NSString::from_str(""));
    row_btn.setTag(index_tag(index));

    unsafe {
        row_btn.setTarget(Some(controller));
        row_btn.setAction(Some(sel!(mcpRowClicked:)));
        row_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
    }

    row_btn
}

fn apply_row_button_background(row_btn: &NSButton) {
    row_btn.setWantsLayer(true);
    if let Some(layer) = row_btn.layer() {
        set_layer_background_color(
            &layer,
            Theme::BG_DARKER.0,
            Theme::BG_DARKER.1,
            Theme::BG_DARKER.2,
        );
    }
}

fn build_mcp_row_container(mtm: MainThreadMarker) -> Retained<NSStackView> {
    let container = NSStackView::new(mtm);
    container.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    container.setSpacing(8.0);
    container.setTranslatesAutoresizingMaskIntoConstraints(false);
    container.setEdgeInsets(NSEdgeInsets {
        top: 4.0,
        left: 8.0,
        bottom: 4.0,
        right: 8.0,
    });
    container
}

fn build_mcp_status_indicator(mtm: MainThreadMarker) -> Retained<NSView> {
    let status_view = NSView::new(mtm);
    status_view.setWantsLayer(true);
    unsafe {
        status_view.setTranslatesAutoresizingMaskIntoConstraints(false);
        let width = status_view.widthAnchor().constraintEqualToConstant(8.0);
        width.setActive(true);
        let height = status_view.heightAnchor().constraintEqualToConstant(8.0);
        height.setActive(true);
    }
    status_view
}

fn apply_mcp_status_color(status_view: &NSView, mcp: &McpConfig) {
    if let Some(layer) = status_view.layer() {
        let status: Option<McpStatus> = {
            let service = McpService::global();
            service
                .try_lock()
                .ok()
                .and_then(|guard| guard.get_status(&mcp.id))
        };

        if !mcp.enabled {
            set_layer_background_color(&layer, 0.0, 0.0, 0.0);
            set_layer_border(&layer, 1.0, 0.5, 0.5, 0.5);
            set_layer_corner_radius(&layer, 4.0);
            return;
        }

        let (r, g, b) = match status {
            Some(McpStatus::Running) => (0.0, 0.8, 0.0),
            Some(McpStatus::Starting) => (1.0, 0.8, 0.0),
            Some(McpStatus::Error(_)) => (0.8, 0.0, 0.0),
            Some(McpStatus::Disabled) => (0.3, 0.3, 0.3),
            Some(McpStatus::Stopped | McpStatus::Restarting) | None => (0.5, 0.5, 0.5),
        };
        set_layer_background_color(&layer, r, g, b);
        set_layer_corner_radius(&layer, 4.0);
    }
}

fn build_mcp_row_label(mcp: &McpConfig, mtm: MainThreadMarker) -> Retained<NSTextField> {
    let source_type = match &mcp.source {
        McpSource::Official { name, version } => format!("Official: {name} v{version}"),
        McpSource::Smithery { qualified_name } => format!("Smithery: {qualified_name}"),
        McpSource::Manual { url } => format!("Manual: {url}"),
    };
    let text = format!("{} - {}", mcp.name, source_type);
    let label = NSTextField::labelWithString(&NSString::from_str(&text), mtm);
    label.setTextColor(Some(&Theme::text_primary()));
    label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
    label
}

fn build_mcp_toggle(
    controller: &SettingsViewController,
    mcp: &McpConfig,
    index: usize,
    mtm: MainThreadMarker,
) -> Retained<NSSwitch> {
    let toggle = NSSwitch::new(mtm);
    toggle.setState(if mcp.enabled {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    });
    toggle.setTag(index_tag(index));
    unsafe {
        toggle.setTarget(Some(controller));
        toggle.setAction(Some(sel!(mcpToggled:)));
        toggle.setContentHuggingPriority_forOrientation(
            750.0,
            NSLayoutConstraintOrientation::Horizontal,
        );
    }
    toggle
}

fn constrain_container_to_button(row_btn: &NSButton, container: &NSStackView) {
    let leading = container
        .leadingAnchor()
        .constraintEqualToAnchor(&row_btn.leadingAnchor());
    let trailing = container
        .trailingAnchor()
        .constraintEqualToAnchor(&row_btn.trailingAnchor());
    let top = container
        .topAnchor()
        .constraintEqualToAnchor(&row_btn.topAnchor());
    let bottom = container
        .bottomAnchor()
        .constraintEqualToAnchor(&row_btn.bottomAnchor());
    leading.setActive(true);
    trailing.setActive(true);
    top.setActive(true);
    bottom.setActive(true);
}

fn set_row_button_height(row_btn: &NSButton) {
    let height = row_btn.heightAnchor().constraintEqualToConstant(32.0);
    height.setActive(true);
}

pub fn create_toolbar_button(
    title: &str,
    action: Sel,
    controller: &SettingsViewController,
    mtm: MainThreadMarker,
    enabled: bool,
    width: f64,
) -> Retained<NSButton> {
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str(title),
            Some(controller),
            Some(action),
            mtm,
        )
    };
    button.setBezelStyle(NSBezelStyle::Automatic);
    button.setEnabled(enabled);
    unsafe {
        button.setTranslatesAutoresizingMaskIntoConstraints(false);
        let width_constraint = button.widthAnchor().constraintEqualToConstant(width);
        width_constraint.setActive(true);
    }
    button
}

pub fn create_toolbar_spacer(mtm: MainThreadMarker) -> Retained<NSView> {
    let spacer = NSView::new(mtm);
    unsafe {
        spacer.setContentHuggingPriority_forOrientation(
            1.0,
            NSLayoutConstraintOrientation::Horizontal,
        );
    }
    spacer
}
