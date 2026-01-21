use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSLayoutConstraintOrientation, NSPopUpButton, NSScrollView,
    NSStackView, NSStackViewDistribution, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSEdgeInsets, NSPoint, NSRect, NSSize, NSString};

use crate::ui::{set_layer_background_color, Theme};

use super::helpers::{rebuild_messages, update_thinking_button_state, update_title_and_model};
use crate::ui::chat_view::log_to_file;
use crate::ui::ChatViewController;

pub fn build_top_bar_stack(
    controller: &ChatViewController,
    mtm: MainThreadMarker,
) -> Retained<NSView> {
    let top_bar = build_top_bar_container(mtm);
    let default_title = chrono::Local::now().format("%Y%m%d%H%M%S%3f").to_string();

    let title_popup = build_title_popup(controller, mtm);
    let title_edit = build_title_edit_field(controller, &default_title, mtm);
    let rename_btn = build_inline_button("R", sel!(renameConversation:), controller, mtm, 28.0);
    let new_btn = build_inline_button("+", sel!(newConversation:), controller, mtm, 28.0);

    super::helpers::populate_title_popup(&title_popup, &default_title);

    unsafe {
        top_bar.addArrangedSubview(&title_popup);
        top_bar.addArrangedSubview(&title_edit);
        top_bar.addArrangedSubview(&rename_btn);
        top_bar.addArrangedSubview(&new_btn);
    }
    controller.set_title_popup(title_popup);
    controller.set_title_edit_field(title_edit);
    controller.set_rename_button(rename_btn);

    let spacer = NSView::new(mtm);
    unsafe {
        spacer.setTranslatesAutoresizingMaskIntoConstraints(false);
        spacer.setContentHuggingPriority_forOrientation(
            1.0,
            NSLayoutConstraintOrientation::Horizontal,
        );
        top_bar.addArrangedSubview(&spacer);
    }

    for &(label, action) in &[("T", sel!(toggleThinking:)), ("H", sel!(showHistory:))] {
        let btn = create_icon_button_for_stack(controller, label, action, mtm);
        if label == "T" {
            controller.set_thinking_button(btn.clone());
            update_thinking_button_state(controller);
        }
        unsafe {
            top_bar.addArrangedSubview(&btn);
        }
    }

    let gear_btn = create_symbol_button(controller, "gearshape", sel!(showSettings:), mtm);
    unsafe {
        top_bar.addArrangedSubview(&gear_btn);
    }

    let power_btn = create_symbol_button(controller, "power", sel!(quitApp:), mtm);
    unsafe {
        top_bar.addArrangedSubview(&power_btn);
    }

    Retained::from(&*top_bar as &NSView)
}

fn build_top_bar_container(mtm: MainThreadMarker) -> Retained<NSStackView> {
    let top_bar = NSStackView::new(mtm);
    unsafe {
        top_bar.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        top_bar.setSpacing(8.0);
        top_bar.setTranslatesAutoresizingMaskIntoConstraints(false);
        top_bar.setDistribution(NSStackViewDistribution::Fill);
        top_bar.setEdgeInsets(NSEdgeInsets {
            top: 8.0,
            left: 12.0,
            bottom: 8.0,
            right: 12.0,
        });
    }

    top_bar.setWantsLayer(true);
    if let Some(layer) = top_bar.layer() {
        set_layer_background_color(&layer, Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2);
    }

    unsafe {
        top_bar.setContentHuggingPriority_forOrientation(
            750.0,
            NSLayoutConstraintOrientation::Vertical,
        );
        top_bar.setContentCompressionResistancePriority_forOrientation(
            750.0,
            NSLayoutConstraintOrientation::Vertical,
        );
        let height_constraint = top_bar.heightAnchor().constraintEqualToConstant(44.0);
        height_constraint.setActive(true);
    }

    top_bar
}

fn build_title_popup(
    controller: &ChatViewController,
    mtm: MainThreadMarker,
) -> Retained<NSPopUpButton> {
    let title_popup = unsafe { NSPopUpButton::new(mtm) };
    title_popup.addItemWithTitle(&NSString::from_str("New Conversation"));
    unsafe {
        title_popup.setTarget(Some(controller));
        title_popup.setAction(Some(sel!(titlePopupChanged:)));
        title_popup.setTranslatesAutoresizingMaskIntoConstraints(false);
    }

    let width_constraint = title_popup
        .widthAnchor()
        .constraintGreaterThanOrEqualToConstant(200.0);
    width_constraint.setActive(true);

    title_popup
}

fn build_title_edit_field(
    controller: &ChatViewController,
    default_title: &str,
    mtm: MainThreadMarker,
) -> Retained<NSTextField> {
    let title_edit = NSTextField::new(mtm);
    title_edit.setStringValue(&NSString::from_str(default_title));
    // Start hidden - only shown when renaming or creating new conversation
    title_edit.setHidden(true);
    unsafe {
        title_edit.setTarget(Some(controller));
        title_edit.setAction(Some(sel!(titleEditDone:)));
        title_edit.setTranslatesAutoresizingMaskIntoConstraints(false);
    }

    let width_constraint = title_edit
        .widthAnchor()
        .constraintGreaterThanOrEqualToConstant(180.0);
    width_constraint.setActive(true);

    title_edit
}

fn build_inline_button(
    label: &str,
    action: Sel,
    controller: &ChatViewController,
    mtm: MainThreadMarker,
    width: f64,
) -> Retained<NSButton> {
    let btn = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str(label),
            Some(controller),
            Some(action),
            mtm,
        )
    };
    btn.setBezelStyle(NSBezelStyle::Automatic);
    unsafe {
        btn.setTranslatesAutoresizingMaskIntoConstraints(false);
        let width_constraint = btn.widthAnchor().constraintEqualToConstant(width);
        width_constraint.setActive(true);
    }
    btn
}

pub fn build_input_area_stack(
    controller: &ChatViewController,
    mtm: MainThreadMarker,
) -> Retained<NSView> {
    let container = build_input_stack_container(mtm);
    let input_field = build_input_field(controller, mtm);
    let stop_btn = build_stop_button(controller, mtm);
    let send_btn = build_send_button(controller, mtm);

    unsafe {
        container.addArrangedSubview(&input_field);
        container.addArrangedSubview(&stop_btn);
        container.addArrangedSubview(&send_btn);
    }

    controller.set_input_field(input_field);
    controller.set_stop_button(stop_btn);

    Retained::from(&*container as &NSView)
}

pub fn create_icon_button_for_stack(
    controller: &ChatViewController,
    label: &str,
    action: Sel,
    mtm: MainThreadMarker,
) -> Retained<NSButton> {
    let button = build_inline_button(label, action, controller, mtm, 28.0);
    button.setBezelStyle(NSBezelStyle::Automatic);
    button
}

pub fn create_symbol_button(
    controller: &ChatViewController,
    symbol_name: &str,
    action: Sel,
    mtm: MainThreadMarker,
) -> Retained<NSButton> {
    let button = NSButton::new(mtm);
    button.setBezelStyle(NSBezelStyle::Automatic);
    if let Some(image) = objc2_app_kit::NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(symbol_name),
        None,
    ) {
        button.setImage(Some(&image));
    }
    unsafe {
        button.setTarget(Some(controller));
        button.setAction(Some(action));
        button.setTranslatesAutoresizingMaskIntoConstraints(false);
    }
    let width_constraint = button.widthAnchor().constraintEqualToConstant(28.0);
    width_constraint.setActive(true);
    button
}

fn build_input_stack_container(mtm: MainThreadMarker) -> Retained<NSStackView> {
    let container = NSStackView::new(mtm);
    unsafe {
        container.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        container.setSpacing(8.0);
        container.setTranslatesAutoresizingMaskIntoConstraints(false);
        container.setDistribution(NSStackViewDistribution::Fill);
        container.setEdgeInsets(NSEdgeInsets {
            top: 8.0,
            left: 12.0,
            bottom: 8.0,
            right: 12.0,
        });
    }

    container.setWantsLayer(true);
    if let Some(layer) = container.layer() {
        set_layer_background_color(&layer, Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2);
    }

    unsafe {
        container.setContentHuggingPriority_forOrientation(
            750.0,
            NSLayoutConstraintOrientation::Vertical,
        );
        container.setContentCompressionResistancePriority_forOrientation(
            750.0,
            NSLayoutConstraintOrientation::Vertical,
        );
        let height_constraint = container.heightAnchor().constraintEqualToConstant(44.0);
        height_constraint.setActive(true);
    }

    container
}

fn build_input_field(
    controller: &ChatViewController,
    mtm: MainThreadMarker,
) -> Retained<NSTextField> {
    let input_field = NSTextField::new(mtm);
    input_field.setPlaceholderString(Some(&NSString::from_str("Type a message...")));
    unsafe {
        input_field.setTarget(Some(controller));
        input_field.setAction(Some(sel!(sendMessage:)));
        input_field.setTranslatesAutoresizingMaskIntoConstraints(false);
    }
    input_field
}

fn build_stop_button(controller: &ChatViewController, mtm: MainThreadMarker) -> Retained<NSButton> {
    let btn = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Stop"),
            Some(controller),
            Some(sel!(stopStreamingClicked:)),
            mtm,
        )
    };
    btn.setBezelStyle(NSBezelStyle::Automatic);
    btn
}

fn build_send_button(controller: &ChatViewController, mtm: MainThreadMarker) -> Retained<NSButton> {
    let btn = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Send"),
            Some(controller),
            Some(sel!(sendMessage:)),
            mtm,
        )
    };
    btn.setBezelStyle(NSBezelStyle::Automatic);
    btn
}

pub fn load_view_layout(controller: &ChatViewController, mtm: MainThreadMarker) {
    let main_stack = NSStackView::new(mtm);
    unsafe {
        main_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        main_stack.setSpacing(0.0);
        main_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        main_stack.setDistribution(NSStackViewDistribution::Fill);
    }

    let top_bar = build_top_bar_stack(controller, mtm);
    let chat_area = build_chat_area_stack(controller, mtm);
    let input_area = build_input_area_stack(controller, mtm);

    unsafe {
        main_stack.addArrangedSubview(&top_bar);
        main_stack.addArrangedSubview(&chat_area);
        main_stack.addArrangedSubview(&input_area);
    }

    let root_view = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 500.0)),
    );
    root_view.setWantsLayer(true);
    if let Some(layer) = root_view.layer() {
        set_layer_background_color(
            &layer,
            Theme::BG_DARKEST.0,
            Theme::BG_DARKEST.1,
            Theme::BG_DARKEST.2,
        );
    }

    root_view.addSubview(&main_stack);
    unsafe {
        let leading = main_stack
            .leadingAnchor()
            .constraintEqualToAnchor(&root_view.leadingAnchor());
        let trailing = main_stack
            .trailingAnchor()
            .constraintEqualToAnchor(&root_view.trailingAnchor());
        let top = main_stack
            .topAnchor()
            .constraintEqualToAnchor(&root_view.topAnchor());
        let bottom = main_stack
            .bottomAnchor()
            .constraintEqualToAnchor(&root_view.bottomAnchor());
        leading.setActive(true);
        trailing.setActive(true);
        top.setActive(true);
        bottom.setActive(true);
    }

    controller.setView(&root_view);

    log_view_frames(&root_view, &main_stack, &top_bar, &chat_area, &input_area);
}

fn log_view_frames(
    root_view: &NSView,
    main_stack: &NSStackView,
    top_bar: &NSView,
    chat_area: &NSView,
    input_area: &NSView,
) {
    println!("\n=== Chat View Layout Debug ===");
    println!("  root_view: {:?}", root_view.frame());
    println!("  main_stack: {:?}", main_stack.frame());
    println!("  top_bar: {:?}", top_bar.frame());
    println!("  chat_area: {:?}", chat_area.frame());
    println!("  input_area: {:?}", input_area.frame());
    println!("=====================================\n");
}

pub fn load_initial_messages(controller: &ChatViewController) {
    if let Some(conversation) = &*controller.ivars().conversation.borrow() {
        log_to_file(&format!(
            "Loading {} messages from conversation",
            conversation.messages.len()
        ));
        for msg in &conversation.messages {
            let is_user = matches!(msg.role, personal_agent::models::MessageRole::User);
            controller.add_message_to_store(&msg.content, is_user);
        }
    }
    rebuild_messages(controller);
    update_title_and_model(controller);
}

pub fn build_chat_area_stack(
    controller: &ChatViewController,
    mtm: MainThreadMarker,
) -> Retained<NSScrollView> {
    let scroll_view = NSScrollView::new(mtm);
    scroll_view.setHasVerticalScroller(true);
    scroll_view.setDrawsBackground(false);
    unsafe {
        scroll_view.setAutohidesScrollers(true);
        scroll_view.setTranslatesAutoresizingMaskIntoConstraints(false);
    }

    unsafe {
        scroll_view
            .setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
        scroll_view.setContentCompressionResistancePriority_forOrientation(
            250.0,
            NSLayoutConstraintOrientation::Vertical,
        );

        let min_height = scroll_view
            .heightAnchor()
            .constraintGreaterThanOrEqualToConstant(100.0);
        min_height.setActive(true);
    }

    let messages_stack = NSStackView::new(mtm);
    unsafe {
        messages_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        messages_stack.setSpacing(12.0);
        messages_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
        messages_stack.setDistribution(NSStackViewDistribution::Fill);
    }

    messages_stack.setWantsLayer(true);
    if let Some(layer) = messages_stack.layer() {
        set_layer_background_color(
            &layer,
            Theme::BG_DARKEST.0,
            Theme::BG_DARKEST.1,
            Theme::BG_DARKEST.2,
        );
    }

    messages_stack.setTranslatesAutoresizingMaskIntoConstraints(false);

    scroll_view.setDocumentView(Some(&messages_stack));

    let content_view = scroll_view.contentView();
    let width_constraint = messages_stack
        .widthAnchor()
        .constraintEqualToAnchor(&content_view.widthAnchor());
    width_constraint.setActive(true);

    controller.set_scroll_view(scroll_view.clone());
    controller.set_messages_container(Retained::from(&*messages_stack as &NSView));

    scroll_view
}
