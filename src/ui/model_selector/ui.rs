use objc2::rc::Retained;
use objc2::{sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSButtonType, NSFont, NSLayoutConstraintOrientation, NSPopUpButton,
    NSScrollView, NSSearchField, NSStackView, NSStackViewDistribution, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::ui::model_selector_rows::has_vision;
use crate::ui::theme::Theme;
use crate::ui::{FlippedStackView, ModelSelectorViewController};

use super::ModelSelectorViewController as Controller;
use personal_agent::registry::ModelInfo;

pub fn create_provider_header(
    _controller: &Controller,
    provider_name: &str,
    mtm: MainThreadMarker,
) -> Retained<NSView> {
    crate::ui::model_selector_rows::ModelSelectorRowHelper::create_provider_header(
        provider_name,
        mtm,
    )
}

pub fn create_model_row(
    controller: &Controller,
    model: &ModelInfo,
    provider_index: usize,
    model_index: usize,
    mtm: MainThreadMarker,
) -> Retained<NSView> {
    let button = NSButton::initWithFrame(
        NSButton::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(376.0, 28.0)),
    );
    button.setButtonType(NSButtonType::MomentaryPushIn);
    button.setBezelStyle(NSBezelStyle::Automatic);
    button.setBordered(true);

    let mut title_parts = vec![model.id.clone()];

    if let Some(limit) = &model.limit {
        title_parts.push(format_context_window(limit.context));
    }

    let mut caps = String::new();
    if model.reasoning {
        caps.push('R');
    }
    if has_vision(model) {
        if !caps.is_empty() {
            caps.push(' ');
        }
        caps.push('V');
    }
    if !caps.is_empty() {
        title_parts.push(caps);
    }

    if let Some(cost) = &model.cost {
        title_parts.push(format_cost(cost.input, cost.output));
    }

    let title = title_parts.join(" | ");
    button.setTitle(&NSString::from_str(&title));
    button.setAlignment(objc2_app_kit::NSTextAlignment::Left);
    button.setFont(Some(&NSFont::systemFontOfSize(11.0)));

    let tag = (provider_index * 1000 + model_index) as isize;
    button.setTag(tag);

    unsafe {
        button.setTarget(Some(controller));
        button.setAction(Some(sel!(modelSelected:)));
        button.setTranslatesAutoresizingMaskIntoConstraints(false);
        let width = button.widthAnchor().constraintEqualToConstant(376.0);
        let height = button.heightAnchor().constraintEqualToConstant(28.0);
        width.setActive(true);
        height.setActive(true);
    }

    Retained::from(&*button as &NSView)
}

pub fn format_context_window(context: u64) -> String {
    if context >= 1_000_000 {
        format!("{}M", context / 1_000_000)
    } else if context >= 1_000 {
        format!("{}K", context / 1_000)
    } else {
        context.to_string()
    }
}

pub fn format_cost(input: f64, output: f64) -> String {
    if input == 0.0 && output == 0.0 {
        "free".to_string()
    } else {
        let input_per_m = input * 1_000_000.0;
        let output_per_m = output * 1_000_000.0;

        let input_str = format!("{input_per_m:.1}")
            .trim_end_matches(".0")
            .to_string();
        let output_str = format!("{output_per_m:.1}")
            .trim_end_matches(".0")
            .to_string();

        format!("{input_str}/{output_str}")
    }
}

pub fn show_empty_state(controller: &ModelSelectorViewController) {
    let mtm = MainThreadMarker::new().unwrap();

    if let Some(container) = &*controller.ivars().models_container.borrow() {
        let Some(stack) = container.downcast_ref::<NSStackView>() else {
            return;
        };
        let message = NSTextField::labelWithString(
            &NSString::from_str(
                "No models match your filters.\n\nTry adjusting the capability\nfilters or search term.",
            ),
            mtm,
        );
        message.setTextColor(Some(&Theme::text_secondary_color()));
        message.setFont(Some(&NSFont::systemFontOfSize(13.0)));
        message.setAlignment(objc2_app_kit::NSTextAlignment::Center);
        unsafe {
            stack.addArrangedSubview(&message);
        }
    }

    if let Some(status_label) = &*controller.ivars().status_label.borrow() {
        status_label.setStringValue(&NSString::from_str("0 models from 0 providers"));
    }
}

pub fn show_error(controller: &ModelSelectorViewController, message: &str) {
    let mtm = MainThreadMarker::new().unwrap();

    if let Some(container) = &*controller.ivars().models_container.borrow() {
        let Some(stack) = container.downcast_ref::<NSStackView>() else {
            return;
        };
        let label = NSTextField::labelWithString(&NSString::from_str(message), mtm);
        label.setTextColor(Some(&Theme::text_secondary_color()));
        label.setFont(Some(&NSFont::systemFontOfSize(13.0)));
        unsafe {
            stack.addArrangedSubview(&label);
        }
    }
}

pub fn post_model_selected_notification(
    controller: &ModelSelectorViewController,
    provider_id: &str,
    model_id: &str,
) {
    use objc2_foundation::NSNotificationCenter;

    let center = NSNotificationCenter::defaultCenter();
    let name = NSString::from_str("PersonalAgentModelSelected");

    super::SELECTED_MODEL_PROVIDER.with(|cell| {
        cell.set(Some(provider_id.to_string()));
    });
    super::SELECTED_MODEL_ID.with(|cell| {
        cell.set(Some(model_id.to_string()));
    });

    if let Some(registry) = &*controller.ivars().registry.borrow() {
        if let Some(provider) = registry.get_provider(provider_id) {
            super::SELECTED_MODEL_BASE_URL.with(|cell| {
                cell.set(provider.api.clone());
            });
        }

        if let Some(models) = registry.get_models_for_provider(provider_id) {
            if let Some(model) = models.iter().find(|m| m.id == model_id) {
                if let Some(limit) = &model.limit {
                    super::SELECTED_MODEL_CONTEXT.with(|cell| {
                        cell.set(Some(limit.context));
                    });
                }
            }
        }
    }

    unsafe {
        center.postNotificationName_object(&name, None);
    }
}

pub fn build_top_bar(controller: &Controller, mtm: MainThreadMarker) -> Retained<NSView> {
    let top_bar = NSStackView::new(mtm);
    unsafe {
        top_bar.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        top_bar.setSpacing(8.0);
        top_bar.setTranslatesAutoresizingMaskIntoConstraints(false);
        top_bar.setDistribution(NSStackViewDistribution::Fill);
        top_bar.setEdgeInsets(objc2_foundation::NSEdgeInsets {
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
        let height_constraint = top_bar.heightAnchor().constraintEqualToConstant(44.0);
        height_constraint.setActive(true);
    }

    let cancel_btn = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Cancel"),
            Some(controller),
            Some(sel!(cancelButtonClicked:)),
            mtm,
        )
    };
    cancel_btn.setBezelStyle(NSBezelStyle::Automatic);
    unsafe {
        cancel_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
        cancel_btn.setContentHuggingPriority_forOrientation(
            750.0,
            NSLayoutConstraintOrientation::Horizontal,
        );
        let width_constraint = cancel_btn.widthAnchor().constraintEqualToConstant(70.0);
        width_constraint.setActive(true);
    }
    unsafe {
        top_bar.addArrangedSubview(&cancel_btn);
    }

    let spacer1 = NSView::new(mtm);
    unsafe {
        spacer1.setContentHuggingPriority_forOrientation(
            1.0,
            NSLayoutConstraintOrientation::Horizontal,
        );
        top_bar.addArrangedSubview(&spacer1);
    }

    let title = NSTextField::labelWithString(&NSString::from_str("Select Model"), mtm);
    title.setTextColor(Some(&Theme::text_primary()));
    title.setFont(Some(&NSFont::boldSystemFontOfSize(14.0)));
    title.setAlignment(objc2_app_kit::NSTextAlignment::Center);
    unsafe {
        title.setContentHuggingPriority_forOrientation(
            750.0,
            NSLayoutConstraintOrientation::Horizontal,
        );
        top_bar.addArrangedSubview(&title);
    }

    let spacer2 = NSView::new(mtm);
    unsafe {
        spacer2.setContentHuggingPriority_forOrientation(
            1.0,
            NSLayoutConstraintOrientation::Horizontal,
        );
        top_bar.addArrangedSubview(&spacer2);
    }

    Retained::from(&*top_bar as &NSView)
}

pub fn build_filter_bar(controller: &Controller, mtm: MainThreadMarker) -> Retained<NSView> {
    let filter_bar = NSStackView::new(mtm);
    unsafe {
        filter_bar.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        filter_bar.setSpacing(8.0);
        filter_bar.setTranslatesAutoresizingMaskIntoConstraints(false);
        filter_bar.setDistribution(NSStackViewDistribution::Fill);
        filter_bar.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: 8.0,
            left: 12.0,
            bottom: 8.0,
            right: 12.0,
        });
    }

    filter_bar.setWantsLayer(true);
    if let Some(layer) = filter_bar.layer() {
        set_layer_background_color(
            &layer,
            Theme::BG_DARKEST.0,
            Theme::BG_DARKEST.1,
            Theme::BG_DARKEST.2,
        );
    }

    unsafe {
        filter_bar.setContentHuggingPriority_forOrientation(
            750.0,
            NSLayoutConstraintOrientation::Vertical,
        );
        let height_constraint = filter_bar.heightAnchor().constraintEqualToConstant(36.0);
        height_constraint.setActive(true);
    }

    let search_field = NSSearchField::initWithFrame(
        NSSearchField::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(240.0, 24.0)),
    );
    search_field.setPlaceholderString(Some(&NSString::from_str("Search models...")));
    search_field.setBackgroundColor(Some(&Theme::bg_darker()));
    search_field.setTextColor(Some(&Theme::text_primary()));
    unsafe {
        search_field.setTarget(Some(controller));
        search_field.setAction(Some(sel!(searchFieldChanged:)));
        search_field.setTranslatesAutoresizingMaskIntoConstraints(false);
        search_field.setContentHuggingPriority_forOrientation(
            1.0,
            NSLayoutConstraintOrientation::Horizontal,
        );
    }
    unsafe {
        filter_bar.addArrangedSubview(&search_field);
    }
    *controller.ivars().search_field.borrow_mut() = Some(search_field);

    let provider_label = NSTextField::labelWithString(&NSString::from_str("Provider:"), mtm);
    provider_label.setTextColor(Some(&Theme::text_secondary_color()));
    provider_label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
    unsafe {
        provider_label.setContentHuggingPriority_forOrientation(
            750.0,
            NSLayoutConstraintOrientation::Horizontal,
        );
        filter_bar.addArrangedSubview(&provider_label);
    }

    let provider_popup = NSPopUpButton::new(mtm);
    provider_popup.addItemWithTitle(&NSString::from_str("All"));
    unsafe {
        provider_popup.setTarget(Some(controller));
        provider_popup.setAction(Some(sel!(providerPopupChanged:)));
        provider_popup.setTranslatesAutoresizingMaskIntoConstraints(false);
        provider_popup.setContentHuggingPriority_forOrientation(
            750.0,
            NSLayoutConstraintOrientation::Horizontal,
        );
        let width_constraint = provider_popup
            .widthAnchor()
            .constraintEqualToConstant(100.0);
        width_constraint.setActive(true);
    }
    unsafe {
        filter_bar.addArrangedSubview(&provider_popup);
    }
    *controller.ivars().provider_popup.borrow_mut() = Some(provider_popup);

    Retained::from(&*filter_bar as &NSView)
}

pub fn build_capability_toggles(
    controller: &Controller,
    mtm: MainThreadMarker,
) -> Retained<NSView> {
    let toggles_bar = NSStackView::new(mtm);
    unsafe {
        toggles_bar.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        toggles_bar.setSpacing(12.0);
        toggles_bar.setTranslatesAutoresizingMaskIntoConstraints(false);
        toggles_bar.setDistribution(NSStackViewDistribution::Fill);
        toggles_bar.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: 6.0,
            left: 12.0,
            bottom: 6.0,
            right: 12.0,
        });
    }

    toggles_bar.setWantsLayer(true);
    if let Some(layer) = toggles_bar.layer() {
        set_layer_background_color(
            &layer,
            Theme::BG_DARKEST.0,
            Theme::BG_DARKEST.1,
            Theme::BG_DARKEST.2,
        );
    }

    unsafe {
        toggles_bar.setContentHuggingPriority_forOrientation(
            750.0,
            NSLayoutConstraintOrientation::Vertical,
        );
        let height_constraint = toggles_bar.heightAnchor().constraintEqualToConstant(28.0);
        height_constraint.setActive(true);
    }

    let reasoning_checkbox = NSButton::initWithFrame(
        NSButton::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(100.0, 20.0)),
    );
    reasoning_checkbox.setButtonType(NSButtonType::Switch);
    reasoning_checkbox.setTitle(&NSString::from_str("Reasoning"));
    reasoning_checkbox.setFont(Some(&NSFont::systemFontOfSize(12.0)));
    unsafe {
        reasoning_checkbox.setTarget(Some(controller));
        reasoning_checkbox.setAction(Some(sel!(reasoningCheckboxToggled:)));
        toggles_bar.addArrangedSubview(&reasoning_checkbox);
    }
    *controller.ivars().reasoning_checkbox.borrow_mut() = Some(reasoning_checkbox);

    let vision_checkbox = NSButton::initWithFrame(
        NSButton::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(80.0, 20.0)),
    );
    vision_checkbox.setButtonType(NSButtonType::Switch);
    vision_checkbox.setTitle(&NSString::from_str("Vision"));
    vision_checkbox.setFont(Some(&NSFont::systemFontOfSize(12.0)));
    unsafe {
        vision_checkbox.setTarget(Some(controller));
        vision_checkbox.setAction(Some(sel!(visionCheckboxToggled:)));
        toggles_bar.addArrangedSubview(&vision_checkbox);
    }
    *controller.ivars().vision_checkbox.borrow_mut() = Some(vision_checkbox);

    let spacer = NSView::new(mtm);
    unsafe {
        spacer.setContentHuggingPriority_forOrientation(
            1.0,
            NSLayoutConstraintOrientation::Horizontal,
        );
        toggles_bar.addArrangedSubview(&spacer);
    }

    Retained::from(&*toggles_bar as &NSView)
}

pub fn build_model_list(controller: &Controller, mtm: MainThreadMarker) -> Retained<NSScrollView> {
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

    let models_stack = FlippedStackView::new(mtm);
    unsafe {
        models_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        models_stack.setSpacing(0.0);
        models_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Width);
        models_stack.setDistribution(NSStackViewDistribution::Fill);
        models_stack.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: 0.0,
            left: 0.0,
            bottom: 0.0,
            right: 0.0,
        });
    }

    models_stack.setWantsLayer(true);
    if let Some(layer) = models_stack.layer() {
        set_layer_background_color(
            &layer,
            Theme::BG_DARKEST.0,
            Theme::BG_DARKEST.1,
            Theme::BG_DARKEST.2,
        );
    }

    models_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
    scroll_view.setDocumentView(Some(&models_stack));

    let content_view = scroll_view.contentView();
    let width_constraint = models_stack
        .widthAnchor()
        .constraintEqualToAnchor_constant(&content_view.widthAnchor(), -24.0);
    width_constraint.setActive(true);

    *controller.ivars().scroll_view.borrow_mut() = Some(scroll_view.clone());
    *controller.ivars().models_container.borrow_mut() =
        Some(Retained::from(&*models_stack as &NSView));

    scroll_view
}

pub fn build_status_bar(controller: &Controller, mtm: MainThreadMarker) -> Retained<NSView> {
    let status_bar = NSStackView::new(mtm);
    unsafe {
        status_bar.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        status_bar.setTranslatesAutoresizingMaskIntoConstraints(false);
        status_bar.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: 6.0,
            left: 12.0,
            bottom: 6.0,
            right: 12.0,
        });
    }

    status_bar.setWantsLayer(true);
    if let Some(layer) = status_bar.layer() {
        set_layer_background_color(&layer, Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2);
    }

    unsafe {
        status_bar.setContentHuggingPriority_forOrientation(
            750.0,
            NSLayoutConstraintOrientation::Vertical,
        );
        let height_constraint = status_bar.heightAnchor().constraintEqualToConstant(24.0);
        height_constraint.setActive(true);
    }

    let status_label = NSTextField::labelWithString(&NSString::from_str(""), mtm);
    status_label.setTextColor(Some(&Theme::text_secondary_color()));
    status_label.setFont(Some(&NSFont::systemFontOfSize(11.0)));
    unsafe {
        status_bar.addArrangedSubview(&status_label);
    }
    *controller.ivars().status_label.borrow_mut() = Some(status_label);

    Retained::from(&*status_bar as &NSView)
}

fn set_layer_background_color(layer: &objc2_quartz_core::CALayer, r: f64, g: f64, b: f64) {
    use objc2_core_graphics::CGColor;
    let color = CGColor::new_generic_rgb(r, g, b, 1.0);
    layer.setBackgroundColor(Some(&color));
}
