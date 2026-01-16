//! Profile editor view for creating and editing model profiles

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSButtonType, NSFont, NSLayoutConstraintOrientation, NSPopUpButton,
    NSScrollView, NSSlider, NSStackView, NSStackViewDistribution, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView, NSViewController,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::CALayer;
use uuid::Uuid;

use super::model_selector::{SELECTED_MODEL_ID, SELECTED_MODEL_PROVIDER};
use super::theme::Theme;
use personal_agent::config::Config;
use personal_agent::models::{AuthConfig, ModelParameters, ModelProfile};
use personal_agent::registry::{ModelRegistry, RegistryManager};

// ============================================================================
// Helper functions for CALayer operations
// ============================================================================

fn set_layer_background_color(layer: &CALayer, r: f64, g: f64, b: f64) {
    use objc2_core_graphics::CGColor;
    let color = CGColor::new_generic_rgb(r, g, b, 1.0);
    layer.setBackgroundColor(Some(&color));
}

fn set_layer_corner_radius(layer: &CALayer, radius: f64) {
    layer.setCornerRadius(radius);
}

// ============================================================================
// Profile Editor View Controller Ivars
// ============================================================================

pub struct ProfileEditorIvars {
    // Profile being edited (None = creating new)
    editing_profile_id: RefCell<Option<Uuid>>,

    // Pre-selected model from model selector (if any)
    preselected_provider: RefCell<Option<String>>,
    preselected_model: RefCell<Option<String>>,
    selected_model_label: RefCell<Option<Retained<NSTextField>>>,

    // Basic Info fields
    name_field: RefCell<Option<Retained<NSTextField>>>,
    provider_picker: RefCell<Option<Retained<NSView>>>, // Container for provider list
    model_picker: RefCell<Option<Retained<NSView>>>,    // Container for model list

    // Auth fields
    auth_type_popup: RefCell<Option<Retained<NSPopUpButton>>>,
    api_key_field: RefCell<Option<Retained<NSTextField>>>,
    key_file_field: RefCell<Option<Retained<NSTextField>>>,
    base_url_field: RefCell<Option<Retained<NSTextField>>>,

    // Parameter fields
    temperature_slider: RefCell<Option<Retained<NSSlider>>>,
    temperature_label: RefCell<Option<Retained<NSTextField>>>,
    top_p_slider: RefCell<Option<Retained<NSSlider>>>,
    top_p_label: RefCell<Option<Retained<NSTextField>>>,
    max_tokens_field: RefCell<Option<Retained<NSTextField>>>,
    thinking_budget_field: RefCell<Option<Retained<NSTextField>>>,
    enable_thinking_button: RefCell<Option<Retained<NSButton>>>,
    show_thinking_button: RefCell<Option<Retained<NSButton>>>,

    // Registry data
    registry: RefCell<Option<ModelRegistry>>,
    selected_provider: RefCell<Option<String>>,
    selected_model: RefCell<Option<String>>,

    // UI containers
    scroll_view: RefCell<Option<Retained<NSScrollView>>>,
    content_stack: RefCell<Option<Retained<NSStackView>>>,
    delete_button: RefCell<Option<Retained<NSButton>>>,
    title_label: RefCell<Option<Retained<NSTextField>>>,
}

// ============================================================================
// Profile Editor View Controller
// ============================================================================

define_class!(
    #[unsafe(super(NSViewController))]
    #[thread_kind = MainThreadOnly]
    #[name = "ProfileEditorViewController"]
    #[ivars = ProfileEditorIvars]
    pub struct ProfileEditorViewController;

    unsafe impl NSObjectProtocol for ProfileEditorViewController {}

    impl ProfileEditorViewController {
        #[unsafe(method(loadView))]
        fn load_view(&self) {
            let mtm = MainThreadMarker::new().unwrap();

            // Create main container
            let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 500.0));
            let main_view = NSView::initWithFrame(NSView::alloc(mtm), frame);
            main_view.setWantsLayer(true);
            if let Some(layer) = main_view.layer() {
                set_layer_background_color(&layer, Theme::BG_DARKEST.0, Theme::BG_DARKEST.1, Theme::BG_DARKEST.2);
            }

            // Create main vertical stack
            let main_stack = NSStackView::new(mtm);
            unsafe {
                main_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
                main_stack.setSpacing(0.0);
                main_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
                main_stack.setDistribution(objc2_app_kit::NSStackViewDistribution::Fill);
            }

            // Build the UI components
            let top_bar = self.build_top_bar_stack(mtm);
            let scroll_area = self.build_content_scroll_area(mtm);
            let bottom_bar = self.build_bottom_bar_stack(mtm);

            // Add to main stack
            unsafe {
                main_stack.addArrangedSubview(&top_bar);
                main_stack.addArrangedSubview(&scroll_area);
                main_stack.addArrangedSubview(&bottom_bar);
            }

            // Add stack to main view
            main_view.addSubview(&main_stack);

            // Set constraints to fill parent
            unsafe {
                let leading = main_stack.leadingAnchor().constraintEqualToAnchor(&main_view.leadingAnchor());
                leading.setActive(true);

                let trailing = main_stack.trailingAnchor().constraintEqualToAnchor(&main_view.trailingAnchor());
                trailing.setActive(true);

                let top = main_stack.topAnchor().constraintEqualToAnchor(&main_view.topAnchor());
                top.setActive(true);

                let bottom = main_stack.bottomAnchor().constraintEqualToAnchor(&main_view.bottomAnchor());
                bottom.setActive(true);
            }

            self.setView(&main_view);

            // Load registry and populate fields
            self.load_registry();

            // Scroll to top after view is loaded
            if let Some(scroll_view) = &*self.ivars().scroll_view.borrow() {
                let clip_view = scroll_view.contentView();
                clip_view.scrollToPoint(NSPoint::new(0.0, 0.0));
                scroll_view.reflectScrolledClipView(&clip_view);
            }
        }

        #[unsafe(method(providerSelected:))]
        fn provider_selected(&self, sender: Option<&NSObject>) {
            if let Some(button) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                let tag = button.tag() as usize;

                // Get provider ID from registry based on tag
                if let Some(registry) = &*self.ivars().registry.borrow() {
                    let provider_ids = registry.get_provider_ids();
                    if tag < provider_ids.len() {
                        let provider_id = provider_ids[tag].clone();
                        *self.ivars().selected_provider.borrow_mut() = Some(provider_id.clone());

                        // Clear selected model when provider changes
                        *self.ivars().selected_model.borrow_mut() = None;

                        // Populate model list for the selected provider
                        self.populate_model_list();

                        // Update provider button states
                        self.update_provider_button_states(&provider_id);
                    }
                }
            }
        }

        #[unsafe(method(modelSelected:))]
        fn model_selected(&self, sender: Option<&NSObject>) {
            if let Some(button) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                let tag = button.tag() as usize;

                // Get model ID from registry based on tag and selected provider
                if let Some(provider_id) = &*self.ivars().selected_provider.borrow() {
                    if let Some(registry) = &*self.ivars().registry.borrow() {
                        if let Some(models) = registry.get_models_for_provider(provider_id) {
                            if tag < models.len() {
                                let model_id = models[tag].id.clone();
                                *self.ivars().selected_model.borrow_mut() = Some(model_id.clone());

                                // Update model button states
                                self.update_model_button_states(&model_id);
                            }
                        }
                    }
                }
            }
        }

        #[unsafe(method(cancelButtonClicked:))]
        fn cancel_button_clicked(&self, _sender: Option<&NSObject>) {
            // Post notification to go back to settings
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowSettingsView");
            unsafe {
                center.postNotificationName_object(&name, None);
            }
        }

        #[unsafe(method(saveButtonClicked:))]
        fn save_button_clicked(&self, _sender: Option<&NSObject>) {
            // Validate and save profile
            if self.validate_and_save() {
                // Go back to settings
                use objc2_foundation::NSNotificationCenter;
                let center = NSNotificationCenter::defaultCenter();
                let name = NSString::from_str("PersonalAgentShowSettingsView");
                unsafe {
                    center.postNotificationName_object(&name, None);
                }
            }
        }

        #[unsafe(method(deleteButtonClicked:))]
        fn delete_button_clicked(&self, _sender: Option<&NSObject>) {
            if let Some(profile_id) = *self.ivars().editing_profile_id.borrow() {
                // Load config
                let config_path = match Config::default_path() {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("Failed to get config path: {e}");
                        return;
                    }
                };

                let mut config = match Config::load(&config_path) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Failed to load config: {e}");
                        return;
                    }
                };

                // Remove profile
                if let Err(e) = config.remove_profile(&profile_id) {
                    eprintln!("Failed to remove profile: {e}");
                    return;
                }

                // Save config
                if let Err(e) = config.save(&config_path) {
                    eprintln!("Failed to save config: {e}");
                    return;
                }

                println!("Profile deleted");

                // Go back to settings
                use objc2_foundation::NSNotificationCenter;
                let center = NSNotificationCenter::defaultCenter();
                let name = NSString::from_str("PersonalAgentShowSettingsView");
                unsafe {
                    center.postNotificationName_object(&name, None);
                }
            }
        }

        #[unsafe(method(authTypeChanged:))]
        fn auth_type_changed(&self, sender: Option<&NSObject>) {
            if let Some(popup) = sender.and_then(|s| s.downcast_ref::<NSPopUpButton>()) {
                let selected = popup.indexOfSelectedItem();
                // 0 = API Key, 1 = Key File, 2 = None
                if selected == 0 {
                    self.update_auth_fields_visibility(true);
                } else if selected == 1 {
                    self.update_auth_fields_visibility(false);
                } else {
                    // None - hide both fields
                    if let Some(api_key_field) = &*self.ivars().api_key_field.borrow() {
                        api_key_field.setHidden(true);
                    }
                    if let Some(key_file_field) = &*self.ivars().key_file_field.borrow() {
                        key_file_field.setHidden(true);
                    }
                }
            }
        }

        #[unsafe(method(temperatureChanged:))]
        fn temperature_changed(&self, sender: Option<&NSObject>) {
            if let Some(slider) = sender.and_then(|s| s.downcast_ref::<NSSlider>()) {
                let value = slider.doubleValue();
                if let Some(label) = &*self.ivars().temperature_label.borrow() {
                    label.setStringValue(&NSString::from_str(&format!("{value:.2}")));
                }
            }
        }

        #[unsafe(method(topPChanged:))]
        fn top_p_changed(&self, sender: Option<&NSObject>) {
            if let Some(slider) = sender.and_then(|s| s.downcast_ref::<NSSlider>()) {
                let value = slider.doubleValue();
                if let Some(label) = &*self.ivars().top_p_label.borrow() {
                    label.setStringValue(&NSString::from_str(&format!("{value:.2}")));
                }
            }
        }
    }
);

impl ProfileEditorViewController {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        // Check if there's a pre-selected model from model selector
        let preselected_provider = SELECTED_MODEL_PROVIDER.with(|cell| cell.take());
        let preselected_model = SELECTED_MODEL_ID.with(|cell| cell.take());

        let ivars = ProfileEditorIvars {
            editing_profile_id: RefCell::new(None),
            preselected_provider: RefCell::new(preselected_provider.clone()),
            preselected_model: RefCell::new(preselected_model.clone()),
            selected_model_label: RefCell::new(None),
            name_field: RefCell::new(None),
            provider_picker: RefCell::new(None),
            model_picker: RefCell::new(None),
            auth_type_popup: RefCell::new(None),
            api_key_field: RefCell::new(None),
            key_file_field: RefCell::new(None),
            base_url_field: RefCell::new(None),
            temperature_slider: RefCell::new(None),
            temperature_label: RefCell::new(None),
            top_p_slider: RefCell::new(None),
            top_p_label: RefCell::new(None),
            max_tokens_field: RefCell::new(None),
            thinking_budget_field: RefCell::new(None),
            enable_thinking_button: RefCell::new(None),
            show_thinking_button: RefCell::new(None),
            registry: RefCell::new(None),
            selected_provider: RefCell::new(preselected_provider),
            selected_model: RefCell::new(preselected_model),
            scroll_view: RefCell::new(None),
            content_stack: RefCell::new(None),
            delete_button: RefCell::new(None),
            title_label: RefCell::new(None),
        };

        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }

    /// Load an existing profile for editing
    pub fn load_profile(&self, profile: &ModelProfile) {
        *self.ivars().editing_profile_id.borrow_mut() = Some(profile.id);
        *self.ivars().selected_provider.borrow_mut() = Some(profile.provider_id.clone());
        *self.ivars().selected_model.borrow_mut() = Some(profile.model_id.clone());

        // Update title
        if let Some(title) = &*self.ivars().title_label.borrow() {
            title.setStringValue(&NSString::from_str("Edit Profile"));
        }

        // Set field values
        if let Some(field) = &*self.ivars().name_field.borrow() {
            field.setStringValue(&NSString::from_str(&profile.name));
        }

        if let Some(field) = &*self.ivars().base_url_field.borrow() {
            field.setStringValue(&NSString::from_str(&profile.base_url));
        }

        // Set auth fields based on type
        match &profile.auth {
            AuthConfig::Key { value } => {
                if let Some(popup) = &*self.ivars().auth_type_popup.borrow() {
                    popup.selectItemAtIndex(0);
                }
                if let Some(field) = &*self.ivars().api_key_field.borrow() {
                    field.setStringValue(&NSString::from_str(value));
                }
                self.update_auth_fields_visibility(true);
            }
            AuthConfig::Keyfile { path } => {
                if let Some(popup) = &*self.ivars().auth_type_popup.borrow() {
                    popup.selectItemAtIndex(1);
                }
                if let Some(field) = &*self.ivars().key_file_field.borrow() {
                    field.setStringValue(&NSString::from_str(path));
                }
                self.update_auth_fields_visibility(false);
            }
        }

        // Set parameter fields
        if let Some(slider) = &*self.ivars().temperature_slider.borrow() {
            slider.setDoubleValue(profile.parameters.temperature);
        }
        if let Some(label) = &*self.ivars().temperature_label.borrow() {
            label.setStringValue(&NSString::from_str(&format!(
                "{:.2}",
                profile.parameters.temperature
            )));
        }

        if let Some(slider) = &*self.ivars().top_p_slider.borrow() {
            slider.setDoubleValue(profile.parameters.top_p);
        }
        if let Some(label) = &*self.ivars().top_p_label.borrow() {
            label.setStringValue(&NSString::from_str(&format!(
                "{:.2}",
                profile.parameters.top_p
            )));
        }

        if let Some(field) = &*self.ivars().max_tokens_field.borrow() {
            field.setStringValue(&NSString::from_str(
                &profile.parameters.max_tokens.to_string(),
            ));
        }

        if let Some(field) = &*self.ivars().thinking_budget_field.borrow() {
            let value = profile
                .parameters
                .thinking_budget
                .map(|v| v.to_string())
                .unwrap_or_default();
            field.setStringValue(&NSString::from_str(&value));
        }

        if let Some(button) = &*self.ivars().enable_thinking_button.borrow() {
            button.setState(isize::from(profile.parameters.enable_thinking));
        }

        if let Some(button) = &*self.ivars().show_thinking_button.borrow() {
            button.setState(isize::from(profile.parameters.show_thinking));
        }
    }

    fn build_top_bar_stack(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        // Create horizontal stack for top bar (fixed height 44px per wireframe)
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
            set_layer_background_color(
                &layer,
                Theme::BG_DARK.0,
                Theme::BG_DARK.1,
                Theme::BG_DARK.2,
            );
        }

        // CRITICAL: Set fixed height and high content hugging priority
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

        // Cancel button (left, w=70 per wireframe)
        let cancel_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Cancel"),
                Some(self),
                Some(sel!(cancelButtonClicked:)),
                mtm,
            )
        };
        cancel_btn.setBezelStyle(NSBezelStyle::Rounded);
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

        // Spacer (flexible, centers title)
        let spacer1 = NSView::new(mtm);
        unsafe {
            spacer1.setContentHuggingPriority_forOrientation(
                1.0,
                NSLayoutConstraintOrientation::Horizontal,
            );
            top_bar.addArrangedSubview(&spacer1);
        }

        // Title (center)
        let title = NSTextField::labelWithString(&NSString::from_str("New Profile"), mtm);
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
        *self.ivars().title_label.borrow_mut() = Some(title);

        // Spacer (flexible, centers title)
        let spacer2 = NSView::new(mtm);
        unsafe {
            spacer2.setContentHuggingPriority_forOrientation(
                1.0,
                NSLayoutConstraintOrientation::Horizontal,
            );
            top_bar.addArrangedSubview(&spacer2);
        }

        // Save button (right, w=60 per wireframe)
        let save_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Save"),
                Some(self),
                Some(sel!(saveButtonClicked:)),
                mtm,
            )
        };
        save_btn.setBezelStyle(NSBezelStyle::Rounded);
        unsafe {
            save_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
            save_btn.setContentHuggingPriority_forOrientation(
                750.0,
                NSLayoutConstraintOrientation::Horizontal,
            );
            let width_constraint = save_btn.widthAnchor().constraintEqualToConstant(60.0);
            width_constraint.setActive(true);
        }
        unsafe {
            top_bar.addArrangedSubview(&save_btn);
        }

        Retained::from(&*top_bar as &NSView)
    }

    fn build_content_scroll_area(&self, mtm: MainThreadMarker) -> Retained<NSScrollView> {
        // Create scroll view (flexible - takes remaining space)
        let scroll_view = NSScrollView::new(mtm);
        scroll_view.setHasVerticalScroller(true);
        scroll_view.setDrawsBackground(false);
        unsafe {
            scroll_view.setAutohidesScrollers(true);
            scroll_view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        // CRITICAL: Set low content hugging priority so it expands to fill space
        unsafe {
            scroll_view.setContentHuggingPriority_forOrientation(
                1.0,
                NSLayoutConstraintOrientation::Vertical,
            );
            scroll_view.setContentCompressionResistancePriority_forOrientation(
                250.0,
                NSLayoutConstraintOrientation::Vertical,
            );

            // Add minimum height constraint to prevent collapse
            let min_height = scroll_view
                .heightAnchor()
                .constraintGreaterThanOrEqualToConstant(100.0);
            min_height.setActive(true);
        }

        // Create vertical stack for form sections inside scroll view
        let content_stack = NSStackView::new(mtm);
        unsafe {
            content_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            content_stack.setSpacing(16.0);
            content_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
            content_stack.setDistribution(NSStackViewDistribution::Fill);
            content_stack.setEdgeInsets(objc2_foundation::NSEdgeInsets {
                top: 16.0,
                left: 14.0,
                bottom: 16.0,
                right: 14.0,
            });
        }

        content_stack.setWantsLayer(true);
        if let Some(layer) = content_stack.layer() {
            set_layer_background_color(
                &layer,
                Theme::BG_DARKEST.0,
                Theme::BG_DARKEST.1,
                Theme::BG_DARKEST.2,
            );
        }

        // Build sections
        let profile_name_section = self.build_profile_name_section(mtm);
        let provider_section = self.build_provider_section(mtm);
        let model_section = self.build_model_section(mtm);
        let auth_section = self.build_auth_section(mtm);
        let params_section = self.build_parameters_section(mtm);

        unsafe {
            content_stack.addArrangedSubview(&profile_name_section);
            content_stack.addArrangedSubview(&provider_section);
            content_stack.addArrangedSubview(&model_section);
            content_stack.addArrangedSubview(&auth_section);
            content_stack.addArrangedSubview(&params_section);
        }

        scroll_view.setDocumentView(Some(&content_stack));

        // Store references
        *self.ivars().scroll_view.borrow_mut() = Some(scroll_view.clone());
        *self.ivars().content_stack.borrow_mut() = Some(content_stack);

        scroll_view
    }

    fn build_section_label(&self, text: &str, mtm: MainThreadMarker) -> Retained<NSTextField> {
        let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
        label.setTextColor(Some(&Theme::text_secondary_color()));
        label.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        label
    }

    fn build_profile_name_section(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let section = NSStackView::new(mtm);
        unsafe {
            section.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            section.setSpacing(6.0);
            section.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
        }

        // Show selected model info if available
        if let Some(provider_id) = &*self.ivars().preselected_provider.borrow() {
            if let Some(model_id) = &*self.ivars().preselected_model.borrow() {
                let selected_text = format!("Selected: {provider_id}:{model_id}");
                let selected_label =
                    NSTextField::labelWithString(&NSString::from_str(&selected_text), mtm);
                selected_label.setTextColor(Some(&Theme::text_primary()));
                selected_label.setFont(Some(&NSFont::boldSystemFontOfSize(12.0)));
                unsafe {
                    section.addArrangedSubview(&selected_label);
                }
                *self.ivars().selected_model_label.borrow_mut() = Some(selected_label);
            }
        }

        // Label
        let label = self.build_section_label("PROFILE NAME", mtm);
        unsafe {
            section.addArrangedSubview(&label);
        }

        // Text field
        let name_field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(372.0, 24.0)),
        );
        name_field.setPlaceholderString(Some(&NSString::from_str("My Profile")));
        name_field.setBackgroundColor(Some(&Theme::bg_darker()));
        name_field.setTextColor(Some(&Theme::text_primary()));
        name_field.setDrawsBackground(true);
        name_field.setBordered(true);
        unsafe {
            name_field.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = name_field.widthAnchor().constraintEqualToConstant(372.0);
            width_constraint.setActive(true);
        }
        unsafe {
            section.addArrangedSubview(&name_field);
        }
        *self.ivars().name_field.borrow_mut() = Some(name_field);

        Retained::from(&*section as &NSView)
    }

    fn build_provider_section(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let section = NSStackView::new(mtm);
        unsafe {
            section.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            section.setSpacing(6.0);
            section.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
        }

        // Label
        let label = self.build_section_label("PROVIDER", mtm);
        unsafe {
            section.addArrangedSubview(&label);
        }

        // Provider picker container (scrollable list of buttons)
        let provider_picker = NSScrollView::new(mtm);
        provider_picker.setHasVerticalScroller(true);
        provider_picker.setDrawsBackground(true);
        provider_picker.setBackgroundColor(&Theme::bg_darker());
        unsafe {
            provider_picker.setAutohidesScrollers(true);
            provider_picker.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = provider_picker
                .widthAnchor()
                .constraintEqualToConstant(372.0);
            let height_constraint = provider_picker
                .heightAnchor()
                .constraintEqualToConstant(120.0);
            width_constraint.setActive(true);
            height_constraint.setActive(true);
        }

        // Inner stack for provider buttons
        let provider_stack = NSStackView::new(mtm);
        unsafe {
            provider_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            provider_stack.setSpacing(4.0);
            provider_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
        }
        provider_stack.setWantsLayer(true);
        if let Some(layer) = provider_stack.layer() {
            set_layer_background_color(
                &layer,
                Theme::BG_DARKER.0,
                Theme::BG_DARKER.1,
                Theme::BG_DARKER.2,
            );
        }

        provider_picker.setDocumentView(Some(&provider_stack));
        *self.ivars().provider_picker.borrow_mut() =
            Some(Retained::from(&*provider_stack as &NSView));

        unsafe {
            section.addArrangedSubview(&provider_picker);
        }

        Retained::from(&*section as &NSView)
    }

    fn build_model_section(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let section = NSStackView::new(mtm);
        unsafe {
            section.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            section.setSpacing(6.0);
            section.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
        }

        // Label
        let label = self.build_section_label("MODEL", mtm);
        unsafe {
            section.addArrangedSubview(&label);
        }

        // Model picker container (scrollable list of buttons)
        let model_picker = NSScrollView::new(mtm);
        model_picker.setHasVerticalScroller(true);
        model_picker.setDrawsBackground(true);
        model_picker.setBackgroundColor(&Theme::bg_darker());
        unsafe {
            model_picker.setAutohidesScrollers(true);
            model_picker.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = model_picker.widthAnchor().constraintEqualToConstant(372.0);
            let height_constraint = model_picker.heightAnchor().constraintEqualToConstant(120.0);
            width_constraint.setActive(true);
            height_constraint.setActive(true);
        }

        // Inner stack for model buttons
        let model_stack = NSStackView::new(mtm);
        unsafe {
            model_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            model_stack.setSpacing(4.0);
            model_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
        }
        model_stack.setWantsLayer(true);
        if let Some(layer) = model_stack.layer() {
            set_layer_background_color(
                &layer,
                Theme::BG_DARKER.0,
                Theme::BG_DARKER.1,
                Theme::BG_DARKER.2,
            );
        }

        model_picker.setDocumentView(Some(&model_stack));
        *self.ivars().model_picker.borrow_mut() = Some(Retained::from(&*model_stack as &NSView));

        unsafe {
            section.addArrangedSubview(&model_picker);
        }

        Retained::from(&*section as &NSView)
    }

    fn build_auth_section(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let section = NSStackView::new(mtm);
        unsafe {
            section.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            section.setSpacing(10.0);
            section.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
        }

        // Label
        let label = self.build_section_label("AUTHENTICATION", mtm);
        unsafe {
            section.addArrangedSubview(&label);
        }

        // Auth type popup button (dropdown)
        let auth_popup_row = NSStackView::new(mtm);
        unsafe {
            auth_popup_row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            auth_popup_row.setSpacing(8.0);
        }

        let auth_popup = NSPopUpButton::new(mtm);
        auth_popup.addItemWithTitle(&NSString::from_str("API Key"));
        auth_popup.addItemWithTitle(&NSString::from_str("Key File"));
        auth_popup.addItemWithTitle(&NSString::from_str("None"));
        unsafe {
            auth_popup.setTarget(Some(self));
            auth_popup.setAction(Some(sel!(authTypeChanged:)));
            auth_popup.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = auth_popup.widthAnchor().constraintEqualToConstant(150.0);
            width_constraint.setActive(true);
        }
        unsafe {
            auth_popup_row.addArrangedSubview(&auth_popup);
        }
        *self.ivars().auth_type_popup.borrow_mut() = Some(auth_popup);

        unsafe {
            section.addArrangedSubview(&auth_popup_row);
        }

        // API Key field
        let api_key_field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(372.0, 24.0)),
        );
        api_key_field.setPlaceholderString(Some(&NSString::from_str("sk-...")));
        api_key_field.setBackgroundColor(Some(&Theme::bg_darker()));
        api_key_field.setTextColor(Some(&Theme::text_primary()));
        api_key_field.setDrawsBackground(true);
        api_key_field.setBordered(true);
        // Enable editing and selection for copy/paste
        api_key_field.setEditable(true);
        api_key_field.setSelectable(true);
        unsafe {
            api_key_field.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = api_key_field.widthAnchor().constraintEqualToConstant(372.0);
            width_constraint.setActive(true);
        }
        unsafe {
            section.addArrangedSubview(&api_key_field);
        }
        *self.ivars().api_key_field.borrow_mut() = Some(api_key_field);

        // Key File field (initially hidden)
        let key_file_field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(372.0, 24.0)),
        );
        key_file_field.setPlaceholderString(Some(&NSString::from_str("/path/to/keyfile")));
        key_file_field.setBackgroundColor(Some(&Theme::bg_darker()));
        key_file_field.setTextColor(Some(&Theme::text_primary()));
        key_file_field.setDrawsBackground(true);
        key_file_field.setBordered(true);
        key_file_field.setHidden(true);
        unsafe {
            key_file_field.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = key_file_field
                .widthAnchor()
                .constraintEqualToConstant(372.0);
            width_constraint.setActive(true);
        }
        unsafe {
            section.addArrangedSubview(&key_file_field);
        }
        *self.ivars().key_file_field.borrow_mut() = Some(key_file_field);

        // Base URL label
        let base_url_label = self.build_section_label("BASE URL (OPTIONAL)", mtm);
        unsafe {
            section.addArrangedSubview(&base_url_label);
        }

        // Base URL field
        let base_url_field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(372.0, 24.0)),
        );
        base_url_field
            .setPlaceholderString(Some(&NSString::from_str("https://api.example.com/v1")));
        base_url_field.setBackgroundColor(Some(&Theme::bg_darker()));
        base_url_field.setTextColor(Some(&Theme::text_primary()));
        base_url_field.setDrawsBackground(true);
        base_url_field.setBordered(true);
        unsafe {
            base_url_field.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = base_url_field
                .widthAnchor()
                .constraintEqualToConstant(372.0);
            width_constraint.setActive(true);
        }
        unsafe {
            section.addArrangedSubview(&base_url_field);
        }
        *self.ivars().base_url_field.borrow_mut() = Some(base_url_field);

        Retained::from(&*section as &NSView)
    }

    fn build_parameters_section(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let section = NSStackView::new(mtm);
        unsafe {
            section.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            section.setSpacing(12.0);
            section.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
        }

        // Label
        let label = self.build_section_label("PARAMETERS", mtm);
        unsafe {
            section.addArrangedSubview(&label);
        }

        // Temperature row
        let temp_row =
            self.build_slider_row("Temperature", 0.0, 2.0, 0.7, sel!(temperatureChanged:), mtm);
        unsafe {
            section.addArrangedSubview(&temp_row.0);
        }
        *self.ivars().temperature_slider.borrow_mut() = Some(temp_row.1);
        *self.ivars().temperature_label.borrow_mut() = Some(temp_row.2);

        // Top P row
        let top_p_row = self.build_slider_row("Top P", 0.0, 1.0, 0.95, sel!(topPChanged:), mtm);
        unsafe {
            section.addArrangedSubview(&top_p_row.0);
        }
        *self.ivars().top_p_slider.borrow_mut() = Some(top_p_row.1);
        *self.ivars().top_p_label.borrow_mut() = Some(top_p_row.2);

        // Max Tokens
        let max_tokens_row = NSStackView::new(mtm);
        unsafe {
            max_tokens_row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            max_tokens_row.setSpacing(10.0);
        }

        let max_tokens_label =
            NSTextField::labelWithString(&NSString::from_str("Max Output Tokens"), mtm);
        max_tokens_label.setTextColor(Some(&Theme::text_secondary_color()));
        max_tokens_label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        unsafe {
            max_tokens_label.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = max_tokens_label
                .widthAnchor()
                .constraintEqualToConstant(150.0);
            width_constraint.setActive(true);
        }
        unsafe {
            max_tokens_row.addArrangedSubview(&max_tokens_label);
        }

        let max_tokens_field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(100.0, 24.0)),
        );
        max_tokens_field.setPlaceholderString(Some(&NSString::from_str("4096")));
        max_tokens_field.setStringValue(&NSString::from_str("4096"));
        max_tokens_field.setBackgroundColor(Some(&Theme::bg_darker()));
        max_tokens_field.setTextColor(Some(&Theme::text_primary()));
        max_tokens_field.setDrawsBackground(true);
        max_tokens_field.setBordered(true);
        unsafe {
            max_tokens_field.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = max_tokens_field
                .widthAnchor()
                .constraintEqualToConstant(100.0);
            width_constraint.setActive(true);
        }
        unsafe {
            max_tokens_row.addArrangedSubview(&max_tokens_field);
        }
        *self.ivars().max_tokens_field.borrow_mut() = Some(max_tokens_field);

        unsafe {
            section.addArrangedSubview(&max_tokens_row);
        }

        // Thinking Budget
        let thinking_budget_row = NSStackView::new(mtm);
        unsafe {
            thinking_budget_row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            thinking_budget_row.setSpacing(10.0);
        }

        let thinking_budget_label =
            NSTextField::labelWithString(&NSString::from_str("Thinking Budget"), mtm);
        thinking_budget_label.setTextColor(Some(&Theme::text_secondary_color()));
        thinking_budget_label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        unsafe {
            thinking_budget_label.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = thinking_budget_label
                .widthAnchor()
                .constraintEqualToConstant(150.0);
            width_constraint.setActive(true);
        }
        unsafe {
            thinking_budget_row.addArrangedSubview(&thinking_budget_label);
        }

        let thinking_budget_field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(100.0, 24.0)),
        );
        thinking_budget_field.setPlaceholderString(Some(&NSString::from_str("10000")));
        thinking_budget_field.setBackgroundColor(Some(&Theme::bg_darker()));
        thinking_budget_field.setTextColor(Some(&Theme::text_primary()));
        thinking_budget_field.setDrawsBackground(true);
        thinking_budget_field.setBordered(true);
        unsafe {
            thinking_budget_field.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = thinking_budget_field
                .widthAnchor()
                .constraintEqualToConstant(100.0);
            width_constraint.setActive(true);
        }
        unsafe {
            thinking_budget_row.addArrangedSubview(&thinking_budget_field);
        }
        *self.ivars().thinking_budget_field.borrow_mut() = Some(thinking_budget_field);

        unsafe {
            section.addArrangedSubview(&thinking_budget_row);
        }

        // Enable Thinking checkbox
        let enable_thinking_button = NSButton::initWithFrame(
            NSButton::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(200.0, 20.0)),
        );
        enable_thinking_button.setButtonType(NSButtonType::Switch);
        enable_thinking_button.setTitle(&NSString::from_str("Enable Thinking"));
        enable_thinking_button.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        unsafe {
            section.addArrangedSubview(&enable_thinking_button);
        }
        *self.ivars().enable_thinking_button.borrow_mut() = Some(enable_thinking_button);

        // Show Thinking checkbox
        let show_thinking_button = NSButton::initWithFrame(
            NSButton::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(200.0, 20.0)),
        );
        show_thinking_button.setButtonType(NSButtonType::Switch);
        show_thinking_button.setTitle(&NSString::from_str("Show Thinking"));
        show_thinking_button.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        unsafe {
            section.addArrangedSubview(&show_thinking_button);
        }
        *self.ivars().show_thinking_button.borrow_mut() = Some(show_thinking_button);

        Retained::from(&*section as &NSView)
    }

    fn build_slider_row(
        &self,
        label_text: &str,
        min_value: f64,
        max_value: f64,
        initial_value: f64,
        action: objc2::runtime::Sel,
        mtm: MainThreadMarker,
    ) -> (Retained<NSView>, Retained<NSSlider>, Retained<NSTextField>) {
        let row = NSStackView::new(mtm);
        unsafe {
            row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            row.setSpacing(10.0);
        }

        // Label (fixed width for alignment)
        let label = NSTextField::labelWithString(&NSString::from_str(label_text), mtm);
        label.setTextColor(Some(&Theme::text_secondary_color()));
        label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        unsafe {
            label.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = label.widthAnchor().constraintEqualToConstant(100.0);
            width_constraint.setActive(true);
        }
        unsafe {
            row.addArrangedSubview(&label);
        }

        // Slider (flexible)
        let slider = NSSlider::initWithFrame(
            NSSlider::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(200.0, 20.0)),
        );
        slider.setMinValue(min_value);
        slider.setMaxValue(max_value);
        slider.setDoubleValue(initial_value);
        unsafe {
            slider.setTarget(Some(self));
            slider.setAction(Some(action));
            slider.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = slider.widthAnchor().constraintEqualToConstant(200.0);
            width_constraint.setActive(true);
        }
        unsafe {
            row.addArrangedSubview(&slider);
        }

        // Value label (fixed width)
        let value_label =
            NSTextField::labelWithString(&NSString::from_str(&format!("{initial_value:.2}")), mtm);
        value_label.setTextColor(Some(&Theme::text_primary()));
        value_label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        value_label.setAlignment(objc2_app_kit::NSTextAlignment::Right);
        unsafe {
            value_label.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = value_label.widthAnchor().constraintEqualToConstant(50.0);
            width_constraint.setActive(true);
        }
        unsafe {
            row.addArrangedSubview(&value_label);
        }

        (Retained::from(&*row as &NSView), slider, value_label)
    }

    fn build_bottom_bar_stack(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        // Profile editor doesn't have a bottom bar per wireframe
        // The Save button is in the top bar
        // Return an empty view with 0 height
        let empty = NSView::new(mtm);
        unsafe {
            empty.setTranslatesAutoresizingMaskIntoConstraints(false);
            let height_constraint = empty.heightAnchor().constraintEqualToConstant(0.0);
            height_constraint.setActive(true);
        }
        empty
    }

    fn load_registry(&self) {
        // Try to load registry from cache or fetch fresh
        let manager = match RegistryManager::new() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Failed to create registry manager: {e}");
                return;
            }
        };

        // Try to get cached registry first
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to create runtime: {e}");
                return;
            }
        };

        let registry = runtime.block_on(async { manager.get_registry().await });

        match registry {
            Ok(reg) => {
                *self.ivars().registry.borrow_mut() = Some(reg);
                self.populate_provider_list();

                // If there's a pre-selected model, populate the model list and auto-fill base URL
                if let Some(provider_id) = &*self.ivars().preselected_provider.borrow() {
                    self.populate_model_list();
                    self.update_provider_button_states(provider_id);

                    if let Some(model_id) = &*self.ivars().preselected_model.borrow() {
                        self.update_model_button_states(model_id);
                        self.auto_fill_base_url(provider_id);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to load registry: {e}");
                self.show_registry_error();
            }
        }
    }

    fn auto_fill_base_url(&self, provider_id: &str) {
        if let Some(base_url_field) = &*self.ivars().base_url_field.borrow() {
            // Check if base URL is already filled
            let current_url = base_url_field.stringValue().to_string();
            if !current_url.trim().is_empty() {
                return; // Don't overwrite existing URL
            }

            // Get provider from registry
            if let Some(registry) = &*self.ivars().registry.borrow() {
                if let Some(provider) = registry.get_provider(provider_id) {
                    // Use the api field from provider info if available
                    if let Some(api) = &provider.api {
                        base_url_field.setStringValue(&NSString::from_str(api));
                    } else {
                        // Fallback to default format
                        let default_url = format!("https://api.{provider_id}.com/v1");
                        base_url_field.setStringValue(&NSString::from_str(&default_url));
                    }
                }
            }
        }
    }

    fn populate_provider_list(&self) {
        let mtm = MainThreadMarker::new().unwrap();

        if let Some(container) = &*self.ivars().provider_picker.borrow() {
            // Clear existing subviews
            let subviews = container.subviews();
            for view in &subviews {
                if let Some(stack) = container.downcast_ref::<NSStackView>() {
                    unsafe {
                        stack.removeArrangedSubview(&view);
                    }
                }
                view.removeFromSuperview();
            }

            // Get provider IDs from registry
            if let Some(registry) = &*self.ivars().registry.borrow() {
                let provider_ids = registry.get_provider_ids();

                if provider_ids.is_empty() {
                    let label = NSTextField::labelWithString(
                        &NSString::from_str("No providers available"),
                        mtm,
                    );
                    label.setTextColor(Some(&Theme::text_secondary_color()));
                    label.setFont(Some(&NSFont::systemFontOfSize(11.0)));
                    container.addSubview(&label);
                } else {
                    // Add provider buttons to stack
                    if let Some(stack) = container.downcast_ref::<NSStackView>() {
                        for (i, provider_id) in provider_ids.iter().enumerate() {
                            if let Some(provider) = registry.get_provider(provider_id) {
                                let button = NSButton::initWithFrame(
                                    NSButton::alloc(mtm),
                                    NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(350.0, 28.0)),
                                );
                                button.setTitle(&NSString::from_str(&provider.name));
                                button.setBezelStyle(NSBezelStyle::Rounded);
                                button.setButtonType(NSButtonType::MomentaryPushIn);
                                button.setTag(i as isize);

                                unsafe {
                                    button.setTarget(Some(self));
                                    button.setAction(Some(sel!(providerSelected:)));
                                    button.setTranslatesAutoresizingMaskIntoConstraints(false);
                                    let width_constraint =
                                        button.widthAnchor().constraintEqualToConstant(350.0);
                                    width_constraint.setActive(true);
                                }

                                unsafe {
                                    stack.addArrangedSubview(&button);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn populate_model_list(&self) {
        let mtm = MainThreadMarker::new().unwrap();

        if let Some(container) = &*self.ivars().model_picker.borrow() {
            // Clear existing subviews
            let subviews = container.subviews();
            for view in &subviews {
                if let Some(stack) = container.downcast_ref::<NSStackView>() {
                    unsafe {
                        stack.removeArrangedSubview(&view);
                    }
                }
                view.removeFromSuperview();
            }

            // Get models for selected provider
            if let Some(provider_id) = &*self.ivars().selected_provider.borrow() {
                if let Some(registry) = &*self.ivars().registry.borrow() {
                    if let Some(models) = registry.get_models_for_provider(provider_id) {
                        if models.is_empty() {
                            let label = NSTextField::labelWithString(
                                &NSString::from_str("No models available"),
                                mtm,
                            );
                            label.setTextColor(Some(&Theme::text_secondary_color()));
                            label.setFont(Some(&NSFont::systemFontOfSize(11.0)));
                            container.addSubview(&label);
                        } else {
                            // Add model buttons to stack
                            if let Some(stack) = container.downcast_ref::<NSStackView>() {
                                for (i, model) in models.iter().enumerate() {
                                    let button = NSButton::initWithFrame(
                                        NSButton::alloc(mtm),
                                        NSRect::new(
                                            NSPoint::new(0.0, 0.0),
                                            NSSize::new(350.0, 28.0),
                                        ),
                                    );

                                    // Build button title with model info
                                    let mut title = model.name.clone();
                                    if model.tool_call {
                                        title.push_str(" [TC]");
                                    }
                                    if model.reasoning {
                                        title.push_str(" [R]");
                                    }
                                    if let Some(limit) = &model.limit {
                                        title.push_str(&format!(" ({}k)", limit.context / 1000));
                                    }

                                    button.setTitle(&NSString::from_str(&title));
                                    button.setBezelStyle(NSBezelStyle::Rounded);
                                    button.setButtonType(NSButtonType::MomentaryPushIn);
                                    button.setTag(i as isize);

                                    unsafe {
                                        button.setTarget(Some(self));
                                        button.setAction(Some(sel!(modelSelected:)));
                                        button.setTranslatesAutoresizingMaskIntoConstraints(false);
                                        let width_constraint =
                                            button.widthAnchor().constraintEqualToConstant(350.0);
                                        width_constraint.setActive(true);
                                    }

                                    unsafe {
                                        stack.addArrangedSubview(&button);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn update_provider_button_states(&self, selected_provider: &str) {
        if let Some(container) = &*self.ivars().provider_picker.borrow() {
            for view in &container.subviews() {
                if let Some(button) = view.downcast_ref::<NSButton>() {
                    let title = button.title().to_string();

                    // Check if this button's title matches the selected provider
                    if let Some(registry) = &*self.ivars().registry.borrow() {
                        if let Some(provider) = registry.get_provider(selected_provider) {
                            if title == provider.name {
                                button.setState(1);
                            } else {
                                button.setState(0);
                            }
                        }
                    }
                }
            }
        }
    }

    fn update_model_button_states(&self, selected_model: &str) {
        if let Some(container) = &*self.ivars().model_picker.borrow() {
            for view in &container.subviews() {
                if let Some(button) = view.downcast_ref::<NSButton>() {
                    let title = button.title().to_string();

                    // Check if this button's title starts with the selected model name
                    if title.starts_with(selected_model) {
                        button.setState(1);
                    } else {
                        button.setState(0);
                    }
                }
            }
        }
    }

    fn show_error_alert(&self, message: &str) {
        use objc2_app_kit::NSAlert;
        let mtm = MainThreadMarker::new().unwrap();

        let alert = NSAlert::new(mtm);
        alert.setMessageText(&NSString::from_str("Validation Error"));
        alert.setInformativeText(&NSString::from_str(message));
        alert.addButtonWithTitle(&NSString::from_str("OK"));

        unsafe {
            alert.runModal();
        }
    }

    fn show_registry_error(&self) {
        let mtm = MainThreadMarker::new().unwrap();

        if let Some(container) = &*self.ivars().provider_picker.borrow() {
            let label = NSTextField::labelWithString(
                &NSString::from_str("Failed to load models.\nCheck connection and try again."),
                mtm,
            );
            label.setTextColor(Some(&Theme::text_secondary_color()));
            label.setFont(Some(&NSFont::systemFontOfSize(11.0)));
            container.addSubview(&label);
        }
    }

    fn update_auth_fields_visibility(&self, show_api_key: bool) {
        if let Some(api_key_field) = &*self.ivars().api_key_field.borrow() {
            api_key_field.setHidden(!show_api_key);
        }

        if let Some(key_file_field) = &*self.ivars().key_file_field.borrow() {
            key_file_field.setHidden(show_api_key);
        }
    }

    fn validate_and_save(&self) -> bool {
        // Get field values
        let name = if let Some(field) = &*self.ivars().name_field.borrow() {
            field.stringValue().to_string()
        } else {
            String::new()
        };

        if name.trim().is_empty() {
            self.show_error_alert("Profile name is required");
            return false;
        }

        // Get selected provider and model
        let provider_id = if let Some(provider) = &*self.ivars().selected_provider.borrow() {
            provider.clone()
        } else {
            self.show_error_alert("Please select a provider");
            return false;
        };

        let model_id = if let Some(model) = &*self.ivars().selected_model.borrow() {
            model.clone()
        } else {
            self.show_error_alert("Please select a model");
            return false;
        };

        // Get auth config
        let auth = if let Some(popup) = &*self.ivars().auth_type_popup.borrow() {
            let selected = popup.indexOfSelectedItem();
            if selected == 0 {
                // API Key - trim whitespace and newlines
                let value = if let Some(field) = &*self.ivars().api_key_field.borrow() {
                    field.stringValue().to_string().trim().to_string()
                } else {
                    String::new()
                };
                AuthConfig::Key { value }
            } else if selected == 1 {
                // Key File - trim whitespace and newlines
                let path = if let Some(field) = &*self.ivars().key_file_field.borrow() {
                    field.stringValue().to_string().trim().to_string()
                } else {
                    String::new()
                };
                AuthConfig::Keyfile { path }
            } else {
                // None - use empty API key
                AuthConfig::Key {
                    value: String::new(),
                }
            }
        } else {
            AuthConfig::Key {
                value: String::new(),
            }
        };

        // Get base URL
        let base_url = if let Some(field) = &*self.ivars().base_url_field.borrow() {
            let url = field.stringValue().to_string();
            if url.trim().is_empty() {
                // Default based on provider
                format!("https://api.{provider_id}.com/v1")
            } else {
                url
            }
        } else {
            format!("https://api.{provider_id}.com/v1")
        };

        // Get parameters
        let temperature = if let Some(slider) = &*self.ivars().temperature_slider.borrow() {
            slider.doubleValue()
        } else {
            0.7
        };

        let top_p = if let Some(slider) = &*self.ivars().top_p_slider.borrow() {
            slider.doubleValue()
        } else {
            0.95
        };

        let max_tokens = if let Some(field) = &*self.ivars().max_tokens_field.borrow() {
            field
                .stringValue()
                .to_string()
                .parse::<u32>()
                .unwrap_or(4096)
        } else {
            4096
        };

        let thinking_budget = if let Some(field) = &*self.ivars().thinking_budget_field.borrow() {
            let value_str = field.stringValue().to_string();
            if value_str.trim().is_empty() {
                None
            } else {
                value_str.parse::<u32>().ok()
            }
        } else {
            None
        };

        let enable_thinking = if let Some(button) = &*self.ivars().enable_thinking_button.borrow() {
            button.state() == 1
        } else {
            false
        };

        let show_thinking = if let Some(button) = &*self.ivars().show_thinking_button.borrow() {
            button.state() == 1
        } else {
            false
        };

        let parameters = ModelParameters {
            temperature,
            top_p,
            max_tokens,
            thinking_budget,
            enable_thinking,
            show_thinking,
        };

        // Create or update profile
        let profile = if let Some(profile_id) = *self.ivars().editing_profile_id.borrow() {
            // Updating existing profile
            ModelProfile {
                id: profile_id,
                name,
                provider_id,
                model_id,
                base_url,
                auth,
                parameters,
                system_prompt:
                    "You are a helpful assistant, be direct and to the point. Respond in English."
                        .to_string(),
            }
        } else {
            // Creating new profile
            ModelProfile {
                id: Uuid::new_v4(),
                name,
                provider_id,
                model_id,
                base_url,
                auth,
                parameters,
                system_prompt:
                    "You are a helpful assistant, be direct and to the point. Respond in English."
                        .to_string(),
            }
        };

        // Save to config
        let config_path = match Config::default_path() {
            Ok(path) => path,
            Err(e) => {
                eprintln!("Failed to get config path: {e}");
                return false;
            }
        };

        let mut config = match Config::load(&config_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to load config: {e}");
                return false;
            }
        };

        if self.ivars().editing_profile_id.borrow().is_some() {
            // Update existing
            if let Err(e) = config.update_profile(profile) {
                eprintln!("Failed to update profile: {e}");
                return false;
            }
        } else {
            // Add new
            config.add_profile(profile.clone());

            // If this is the first profile, make it the default
            if config.profiles.len() == 1 {
                config.default_profile = Some(profile.id);
            }
        }

        // Save config
        if let Err(e) = config.save(&config_path) {
            eprintln!("Failed to save config: {e}");
            return false;
        }

        println!("Profile saved successfully");
        true
    }
}
