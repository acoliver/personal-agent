use objc2::rc::Retained;
use objc2::{sel, DefinedClass, MainThreadMarker};
use objc2_app_kit::{
    NSButton, NSControlStateValueOn, NSFont, NSPopUpButton, NSScrollView, NSStackView,
    NSStackViewDistribution, NSStepper, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
};

use objc2_foundation::{NSEdgeInsets, NSPoint, NSString};

use crate::ui::Theme;
use personal_agent::models::{AuthConfig, ModelProfile};

use super::profile_editor_demo::ProfileEditorDemoViewController;

pub struct EditingProfileDefaults {
    pub editing_profile: Option<ModelProfile>,
    pub preselected_provider: Option<String>,
    pub preselected_model: Option<String>,
    pub default_name: String,
    pub default_base_url: String,
    pub default_system_prompt: String,
}

struct ParameterDefaults {
    temperature: f64,
    max_tokens: String,
    thinking_budget: String,
    enable_thinking: bool,
    show_thinking: bool,
}

pub fn build_form_stack(mtm: MainThreadMarker) -> Retained<super::FlippedStackView> {
    let form_stack = super::FlippedStackView::new(mtm);
    form_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    form_stack.setSpacing(16.0);
    form_stack.setEdgeInsets(NSEdgeInsets {
        top: 16.0,
        left: 16.0,
        bottom: 16.0,
        right: 16.0,
    });
    form_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
    form_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
    form_stack
}

pub fn derive_profile_defaults(
    editing_profile: Option<ModelProfile>,
    preselected_provider: Option<String>,
    preselected_model: Option<String>,
    preselected_base_url: Option<String>,
) -> EditingProfileDefaults {
    let default_name = editing_profile.as_ref().map_or_else(
        || {
            preselected_model
                .clone()
                .unwrap_or_else(|| "My Profile".to_string())
        },
        |profile| profile.name.clone(),
    );

    let default_base_url = editing_profile.as_ref().map_or_else(
        || preselected_base_url.unwrap_or_else(|| "https://api.anthropic.com/v1".to_string()),
        |profile| {
            if profile.base_url.is_empty() {
                "https://api.anthropic.com/v1".to_string()
            } else {
                profile.base_url.clone()
            }
        },
    );

    let default_system_prompt = editing_profile.as_ref().map_or_else(
        || {
            "You are a helpful assistant, be direct and to the point. Respond in English."
                .to_string()
        },
        |profile| profile.system_prompt.clone(),
    );

    EditingProfileDefaults {
        editing_profile,
        preselected_provider,
        preselected_model,
        default_name,
        default_base_url,
        default_system_prompt,
    }
}

pub fn log_selected_model(provider: Option<&str>, model: Option<&str>) {
    if let (Some(provider), Some(model)) = (provider, model) {
        crate::ui::profile_editor_demo::log_to_file(&format!(
            "Profile editor configured with: {provider}:{model}"
        ));
    }
}

pub fn attach_form_width(form_stack: &NSStackView, scroll_view: &NSScrollView) {
    let form_width = form_stack
        .widthAnchor()
        .constraintEqualToAnchor_constant(&scroll_view.contentView().widthAnchor(), 0.0);
    form_width.setActive(true);
}

pub fn scroll_to_top(scroll_view: &NSScrollView) {
    let clip_view = scroll_view.contentView();
    clip_view.scrollToPoint(NSPoint::new(0.0, 0.0));
    scroll_view.reflectScrolledClipView(&clip_view);
}

pub fn build_profile_name_section(
    #[allow(clippy::unused_self)] _controller: &ProfileEditorDemoViewController,
    label: &str,
    default_value: &str,
    mtm: MainThreadMarker,
) -> (Retained<NSView>, Retained<NSTextField>) {
    let container = NSStackView::new(mtm);
    container.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    container.setSpacing(4.0);
    container.setTranslatesAutoresizingMaskIntoConstraints(false);

    let label_field = NSTextField::labelWithString(&NSString::from_str(label), mtm);
    label_field.setTextColor(Some(&Theme::text_primary()));
    label_field.setFont(Some(&NSFont::systemFontOfSize(12.0)));
    container.addArrangedSubview(&label_field);

    let input = NSTextField::new(mtm);
    input.setPlaceholderString(Some(&NSString::from_str(default_value)));
    input.setStringValue(&NSString::from_str(default_value));
    input.setTranslatesAutoresizingMaskIntoConstraints(false);
    let width = input
        .widthAnchor()
        .constraintGreaterThanOrEqualToConstant(350.0);
    width.setActive(true);
    container.addArrangedSubview(&input);

    (Retained::from(&*container as &NSView), input)
}

pub fn build_base_url_section(
    controller: &ProfileEditorDemoViewController,
    default_value: &str,
    mtm: MainThreadMarker,
) -> (Retained<NSView>, Retained<NSTextField>) {
    build_profile_name_section(controller, "Base URL", default_value, mtm)
}

pub fn build_multiline_field(
    #[allow(clippy::unused_self)] _controller: &ProfileEditorDemoViewController,
    label: &str,
    default_text: &str,
    mtm: MainThreadMarker,
) -> (Retained<NSView>, Retained<NSTextField>) {
    let container = NSStackView::new(mtm);
    container.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    container.setSpacing(4.0);
    container.setTranslatesAutoresizingMaskIntoConstraints(false);

    let label_field = NSTextField::labelWithString(&NSString::from_str(label), mtm);
    label_field.setTextColor(Some(&Theme::text_primary()));
    label_field.setFont(Some(&NSFont::systemFontOfSize(12.0)));
    container.addArrangedSubview(&label_field);

    let input = NSTextField::new(mtm);
    input.setStringValue(&NSString::from_str(default_text));
    input.setTranslatesAutoresizingMaskIntoConstraints(false);
    unsafe {
        input.setEditable(true);
        input.setSelectable(true);
    }

    let width = input
        .widthAnchor()
        .constraintGreaterThanOrEqualToConstant(350.0);
    width.setActive(true);
    let height = input
        .heightAnchor()
        .constraintGreaterThanOrEqualToConstant(60.0);
    height.setActive(true);

    container.addArrangedSubview(&input);

    (Retained::from(&*container as &NSView), input)
}

pub fn build_provider_section(
    #[allow(clippy::unused_self)] _controller: &ProfileEditorDemoViewController,
    label: &str,
    items: &[&str],
    mtm: MainThreadMarker,
) -> (Retained<NSView>, Retained<NSPopUpButton>) {
    let container = NSStackView::new(mtm);
    container.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    container.setSpacing(4.0);
    container.setTranslatesAutoresizingMaskIntoConstraints(false);

    let label_field = NSTextField::labelWithString(&NSString::from_str(label), mtm);
    label_field.setTextColor(Some(&Theme::text_primary()));
    label_field.setFont(Some(&NSFont::systemFontOfSize(12.0)));
    container.addArrangedSubview(&label_field);

    let popup = unsafe { NSPopUpButton::new(mtm) };
    for item in items {
        popup.addItemWithTitle(&NSString::from_str(item));
    }
    let width = popup
        .widthAnchor()
        .constraintGreaterThanOrEqualToConstant(350.0);
    width.setActive(true);
    container.addArrangedSubview(&popup);

    (Retained::from(&*container as &NSView), popup)
}

pub fn build_auth_section(
    controller: &ProfileEditorDemoViewController,
    mtm: MainThreadMarker,
    profile: Option<&ModelProfile>,
) -> Retained<NSView> {
    let container = NSStackView::new(mtm);
    container.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    container.setSpacing(8.0);
    container.setTranslatesAutoresizingMaskIntoConstraints(false);

    let label = NSTextField::labelWithString(&NSString::from_str("Authentication"), mtm);
    label.setTextColor(Some(&Theme::text_primary()));
    label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
    container.addArrangedSubview(&label);

    let auth_popup = unsafe { NSPopUpButton::new(mtm) };
    auth_popup.addItemWithTitle(&NSString::from_str("API Key"));
    auth_popup.addItemWithTitle(&NSString::from_str("Key File"));
    auth_popup.addItemWithTitle(&NSString::from_str("None"));
    unsafe {
        auth_popup.setTarget(Some(controller));
        auth_popup.setAction(Some(sel!(authTypeChanged:)));
    }

    let (auth_index, auth_value_str) = profile.map_or_else(
        || (0, String::new()),
        |profile| match &profile.auth {
            AuthConfig::Key { value } => (0, value.clone()),
            AuthConfig::Keyfile { path } => (1, path.clone()),
        },
    );
    auth_popup.selectItemAtIndex(auth_index);

    let width = auth_popup
        .widthAnchor()
        .constraintGreaterThanOrEqualToConstant(350.0);
    width.setActive(true);
    container.addArrangedSubview(&auth_popup);
    *controller.ivars().auth_type_popup.borrow_mut() = Some(auth_popup);

    let auth_value = NSTextField::new(mtm);
    auth_value.setPlaceholderString(Some(&NSString::from_str("sk-...")));
    if !auth_value_str.is_empty() {
        auth_value.setStringValue(&NSString::from_str(&auth_value_str));
    }
    let value_width = auth_value
        .widthAnchor()
        .constraintGreaterThanOrEqualToConstant(350.0);
    value_width.setActive(true);
    container.addArrangedSubview(&auth_value);
    *controller.ivars().auth_value_input.borrow_mut() = Some(auth_value);

    Retained::from(&*container as &NSView)
}

pub fn build_parameters_section(
    controller: &ProfileEditorDemoViewController,
    mtm: MainThreadMarker,
    profile: Option<&ModelProfile>,
) -> Retained<NSView> {
    let defaults = parameter_defaults(controller, profile);
    let container = build_parameters_container(mtm);

    add_temperature_section(controller, &container, &defaults, mtm);
    add_token_sections(controller, &container, &defaults, mtm);
    add_thinking_checkboxes(controller, &container, &defaults, mtm);

    Retained::from(&*container as &NSView)
}

fn parameter_defaults(
    controller: &ProfileEditorDemoViewController,
    profile: Option<&ModelProfile>,
) -> ParameterDefaults {
    let temperature = profile.map_or(1.0, |p| p.parameters.temperature);
    let max_tokens = profile
        .map(|p| p.parameters.max_tokens.to_string())
        .or_else(|| {
            controller
                .ivars()
                .preselected_context
                .borrow()
                .as_ref()
                .map(std::string::ToString::to_string)
        })
        .unwrap_or_else(|| "4096".to_string());
    let thinking_budget = profile
        .and_then(|p| p.parameters.thinking_budget)
        .map_or_else(|| "10000".to_string(), |b| b.to_string());
    let enable_thinking = profile.is_some_and(|p| p.parameters.enable_thinking);
    let show_thinking = profile.is_some_and(|p| p.parameters.show_thinking);

    ParameterDefaults {
        temperature,
        max_tokens,
        thinking_budget,
        enable_thinking,
        show_thinking,
    }
}

fn build_parameters_container(mtm: MainThreadMarker) -> Retained<NSStackView> {
    let container = NSStackView::new(mtm);
    container.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    container.setSpacing(12.0);
    container.setTranslatesAutoresizingMaskIntoConstraints(false);
    container.setDistribution(NSStackViewDistribution::Fill);

    let header = NSTextField::labelWithString(&NSString::from_str("Parameters"), mtm);
    header.setTextColor(Some(&Theme::text_primary()));
    header.setFont(Some(&NSFont::boldSystemFontOfSize(13.0)));
    container.addArrangedSubview(&header);

    container
}

fn add_temperature_section(
    controller: &ProfileEditorDemoViewController,
    container: &NSStackView,
    defaults: &ParameterDefaults,
    mtm: MainThreadMarker,
) {
    let temp_container = NSStackView::new(mtm);
    temp_container.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    temp_container.setSpacing(8.0);

    let temp_label = NSTextField::labelWithString(&NSString::from_str("Temperature"), mtm);
    temp_label.setTextColor(Some(&Theme::text_primary()));
    temp_label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
    let label_width = temp_label.widthAnchor().constraintEqualToConstant(100.0);
    label_width.setActive(true);
    temp_container.addArrangedSubview(&temp_label);

    let value_label = NSTextField::labelWithString(
        &NSString::from_str(&format!("{:.2}", defaults.temperature)),
        mtm,
    );
    value_label.setTextColor(Some(&Theme::text_primary()));
    value_label.setFont(Some(&NSFont::monospacedDigitSystemFontOfSize_weight(
        13.0, 0.0,
    )));
    let value_width = value_label.widthAnchor().constraintEqualToConstant(45.0);
    value_width.setActive(true);
    temp_container.addArrangedSubview(&value_label);
    *controller.ivars().temperature_label.borrow_mut() = Some(value_label);

    let stepper = NSStepper::new(mtm);
    stepper.setMinValue(0.0);
    stepper.setMaxValue(2.0);
    stepper.setDoubleValue(defaults.temperature);
    stepper.setIncrement(0.1);
    stepper.setValueWraps(false);
    unsafe {
        stepper.setTarget(Some(controller));
        stepper.setAction(Some(sel!(temperatureChanged:)));
    }
    temp_container.addArrangedSubview(&stepper);
    *controller.ivars().temperature_stepper.borrow_mut() = Some(stepper);

    let range_label = NSTextField::labelWithString(&NSString::from_str("(0.0 - 2.0)"), mtm);
    range_label.setTextColor(Some(&Theme::text_secondary_color()));
    range_label.setFont(Some(&NSFont::systemFontOfSize(11.0)));
    temp_container.addArrangedSubview(&range_label);

    container.addArrangedSubview(&temp_container);
}

fn add_token_sections(
    controller: &ProfileEditorDemoViewController,
    container: &NSStackView,
    defaults: &ParameterDefaults,
    mtm: MainThreadMarker,
) {
    let (tokens_view, tokens_input) =
        build_profile_name_section(controller, "Max Tokens", &defaults.max_tokens, mtm);
    container.addArrangedSubview(&tokens_view);
    *controller.ivars().max_tokens_input.borrow_mut() = Some(tokens_input);

    let (budget_view, budget_input) = build_profile_name_section(
        controller,
        "Thinking Budget",
        &defaults.thinking_budget,
        mtm,
    );
    container.addArrangedSubview(&budget_view);
    *controller.ivars().thinking_budget_input.borrow_mut() = Some(budget_input);
}

fn add_thinking_checkboxes(
    controller: &ProfileEditorDemoViewController,
    container: &NSStackView,
    defaults: &ParameterDefaults,
    mtm: MainThreadMarker,
) {
    let enable_cb = unsafe {
        NSButton::checkboxWithTitle_target_action(
            &NSString::from_str("Enable Thinking"),
            Some(controller),
            Some(sel!(enableThinkingChanged:)),
            mtm,
        )
    };
    if defaults.enable_thinking {
        enable_cb.setState(NSControlStateValueOn);
    }
    container.addArrangedSubview(&enable_cb);
    *controller.ivars().enable_thinking_checkbox.borrow_mut() = Some(enable_cb);

    let show_cb = unsafe {
        NSButton::checkboxWithTitle_target_action(
            &NSString::from_str("Show Thinking"),
            None,
            None,
            mtm,
        )
    };
    if defaults.show_thinking {
        show_cb.setState(NSControlStateValueOn);
    }
    container.addArrangedSubview(&show_cb);
    *controller.ivars().show_thinking_checkbox.borrow_mut() = Some(show_cb);

    let spacer = NSView::new(mtm);
    spacer.setTranslatesAutoresizingMaskIntoConstraints(false);
    container.addArrangedSubview(&spacer);
}
