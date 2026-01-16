//! Demo Profile Editor - validates the wireframe layout
//!
//! This is a standalone test view that implements the Profile Editor wireframe exactly.
//! Used to verify the layout works before integrating into the main app.

use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::Write;

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAppearanceCustomization, NSButton, NSControlStateValueOn, NSFont,
    NSLayoutConstraintOrientation, NSPopUpButton, NSScrollView, NSStackView,
    NSStackViewDistribution, NSStepper, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
    NSViewController,
};
use objc2_core_graphics::CGColor;
use objc2_foundation::{NSEdgeInsets, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use super::model_selector::{
    SELECTED_MODEL_BASE_URL, SELECTED_MODEL_CONTEXT, SELECTED_MODEL_ID, SELECTED_MODEL_PROVIDER,
};
use crate::ui::Theme;
use personal_agent::config::Config;
use personal_agent::models::{AuthConfig, ModelParameters, ModelProfile};
use uuid::Uuid;

/// Logging helper - writes to file
fn log_to_file(message: &str) {
    let log_path = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/PersonalAgent/debug.log");

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let _ = writeln!(file, "[{timestamp}] {message}");
    }
}

pub struct ProfileEditorDemoIvars {
    /// If editing an existing profile, this is its ID
    editing_profile_id: RefCell<Option<Uuid>>,
    preselected_provider: RefCell<Option<String>>,
    preselected_model: RefCell<Option<String>>,
    preselected_base_url: RefCell<Option<String>>,
    preselected_context: RefCell<Option<u64>>,
    selected_model_label: RefCell<Option<Retained<NSTextField>>>,
    name_input: RefCell<Option<Retained<NSTextField>>>,
    provider_popup: RefCell<Option<Retained<NSPopUpButton>>>,
    model_popup: RefCell<Option<Retained<NSPopUpButton>>>,
    base_url_input: RefCell<Option<Retained<NSTextField>>>,
    auth_type_popup: RefCell<Option<Retained<NSPopUpButton>>>,
    auth_value_input: RefCell<Option<Retained<NSTextField>>>,
    system_prompt_input: RefCell<Option<Retained<NSTextField>>>,
    temperature_stepper: RefCell<Option<Retained<NSStepper>>>,
    temperature_label: RefCell<Option<Retained<NSTextField>>>,
    max_tokens_input: RefCell<Option<Retained<NSTextField>>>,
    thinking_budget_input: RefCell<Option<Retained<NSTextField>>>,
    enable_thinking_checkbox: RefCell<Option<Retained<NSButton>>>,
    show_thinking_checkbox: RefCell<Option<Retained<NSButton>>>,
}

impl Default for ProfileEditorDemoIvars {
    fn default() -> Self {
        // Check for editing existing profile from settings view
        use super::settings_view::EDITING_PROFILE_ID;
        let editing_profile_id = EDITING_PROFILE_ID.with(std::cell::Cell::take);

        // Check for pre-selected model from model selector
        let preselected_provider = SELECTED_MODEL_PROVIDER.with(std::cell::Cell::take);
        let preselected_model = SELECTED_MODEL_ID.with(std::cell::Cell::take);
        let preselected_base_url = SELECTED_MODEL_BASE_URL.with(std::cell::Cell::take);
        let preselected_context = SELECTED_MODEL_CONTEXT.with(std::cell::Cell::take);

        Self {
            editing_profile_id: RefCell::new(editing_profile_id),
            preselected_provider: RefCell::new(preselected_provider),
            preselected_model: RefCell::new(preselected_model),
            preselected_base_url: RefCell::new(preselected_base_url),
            preselected_context: RefCell::new(preselected_context),
            selected_model_label: RefCell::new(None),
            name_input: RefCell::new(None),
            provider_popup: RefCell::new(None),
            model_popup: RefCell::new(None),
            base_url_input: RefCell::new(None),
            auth_type_popup: RefCell::new(None),
            auth_value_input: RefCell::new(None),
            system_prompt_input: RefCell::new(None),
            temperature_stepper: RefCell::new(None),
            temperature_label: RefCell::new(None),
            max_tokens_input: RefCell::new(None),
            thinking_budget_input: RefCell::new(None),
            enable_thinking_checkbox: RefCell::new(None),
            show_thinking_checkbox: RefCell::new(None),
        }
    }
}

define_class!(
    #[unsafe(super(NSViewController))]
    #[thread_kind = MainThreadOnly]
    #[name = "ProfileEditorDemoViewController"]
    #[ivars = ProfileEditorDemoIvars]
    pub struct ProfileEditorDemoViewController;

    unsafe impl NSObjectProtocol for ProfileEditorDemoViewController {}

    impl ProfileEditorDemoViewController {
        #[unsafe(method(loadView))]
        fn load_view(&self) {
            log_to_file("ProfileEditorDemo: loadView started");
            let mtm = MainThreadMarker::new().unwrap();

            let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 500.0));
            let root_view = NSView::initWithFrame(NSView::alloc(mtm), frame);

            // Force dark appearance on this view to prevent blue accent colors
            // SAFETY: NSAppearanceNameDarkAqua is a constant string provided by AppKit
            let dark_appearance_name = unsafe { objc2_app_kit::NSAppearanceNameDarkAqua };
            if let Some(dark_appearance) = objc2_app_kit::NSAppearance::appearanceNamed(dark_appearance_name) {
                root_view.setAppearance(Some(&dark_appearance));
            }

            root_view.setWantsLayer(true);
            if let Some(layer) = root_view.layer() {
                let color = CGColor::new_generic_rgb(
                    Theme::BG_DARKEST.0, Theme::BG_DARKEST.1, Theme::BG_DARKEST.2, 1.0,
                );
                layer.setBackgroundColor(Some(&color));
            }

            let main_stack = NSStackView::new(mtm);
            main_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            main_stack.setSpacing(0.0);
            main_stack.setDistribution(NSStackViewDistribution::Fill);
            main_stack.setTranslatesAutoresizingMaskIntoConstraints(false);

            let top_bar = self.build_top_bar(mtm);
            let form_scroll = self.build_form_scroll(mtm);

            top_bar.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            top_bar.setContentCompressionResistancePriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            form_scroll.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
            form_scroll.setContentCompressionResistancePriority_forOrientation(250.0, NSLayoutConstraintOrientation::Vertical);

            main_stack.addArrangedSubview(&top_bar);
            main_stack.addArrangedSubview(&form_scroll);
            root_view.addSubview(&main_stack);

            let leading = main_stack.leadingAnchor().constraintEqualToAnchor(&root_view.leadingAnchor());
            let trailing = main_stack.trailingAnchor().constraintEqualToAnchor(&root_view.trailingAnchor());
            let top = main_stack.topAnchor().constraintEqualToAnchor(&root_view.topAnchor());
            let bottom = main_stack.bottomAnchor().constraintEqualToAnchor(&root_view.bottomAnchor());
            leading.setActive(true);
            trailing.setActive(true);
            top.setActive(true);
            bottom.setActive(true);

            let top_height = top_bar.heightAnchor().constraintEqualToConstant(44.0);
            top_height.setActive(true);
            let form_min_height = form_scroll.heightAnchor().constraintGreaterThanOrEqualToConstant(100.0);
            form_min_height.setActive(true);

            self.setView(&root_view);
            log_to_file("ProfileEditorDemo: loadView completed");
        }

        #[unsafe(method(cancelClicked:))]
        fn cancel_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("ProfileEditorDemo: Cancel clicked");
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowSettingsView");
            unsafe { center.postNotificationName_object(&name, None); }
        }

        #[unsafe(method(saveClicked:))]
        fn save_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("ProfileEditorDemo: Save clicked");

            // Get name
            let name = if let Some(name_field) = &*self.ivars().name_input.borrow() {
                name_field.stringValue().to_string().trim().to_string()
            } else {
                String::new()
            };
            log_to_file(&format!("  Name: {name}"));

            if name.is_empty() {
                log_to_file("  ERROR: Name is empty");
                return;
            }

            // Get provider
            let provider_id = if let Some(popup) = &*self.ivars().provider_popup.borrow() {
                let index = popup.indexOfSelectedItem();
                log_to_file(&format!("  Provider index: {index}"));
                if let Some(item) = popup.itemAtIndex(index) {
                    item.title().to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            log_to_file(&format!("  Provider ID: {provider_id}"));

            // Get model
            let model_id = if let Some(popup) = &*self.ivars().model_popup.borrow() {
                let index = popup.indexOfSelectedItem();
                if let Some(item) = popup.itemAtIndex(index) {
                    item.title().to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            log_to_file(&format!("  Model ID: {model_id}"));

            // Get auth config
            let auth = if let Some(popup) = &*self.ivars().auth_type_popup.borrow() {
                let index = popup.indexOfSelectedItem();
                let value = if let Some(field) = &*self.ivars().auth_value_input.borrow() {
                    field.stringValue().to_string().trim().to_string()
                } else {
                    String::new()
                };

                if index == 0 {
                    AuthConfig::Key { value }
                } else if index == 1 {
                    AuthConfig::Keyfile { path: value }
                } else {
                    AuthConfig::Key { value: String::new() }
                }
            } else {
                AuthConfig::Key { value: String::new() }
            };
            log_to_file(&format!("  Auth: {auth:?}"));

            // Get parameters
            let temperature = if let Some(stepper) = &*self.ivars().temperature_stepper.borrow() {
                stepper.doubleValue()
            } else {
                0.7
            };

            let max_tokens = if let Some(field) = &*self.ivars().max_tokens_input.borrow() {
                field.stringValue().to_string().parse::<u32>().unwrap_or(4096)
            } else {
                4096
            };

            let enable_thinking = if let Some(checkbox) = &*self.ivars().enable_thinking_checkbox.borrow() {
                checkbox.state() == NSControlStateValueOn
            } else {
                false
            };

            let thinking_budget = if enable_thinking {
                if let Some(field) = &*self.ivars().thinking_budget_input.borrow() {
                    field.stringValue().to_string().parse::<u32>().ok()
                } else {
                    None
                }
            } else {
                None
            };

            let show_thinking = if let Some(checkbox) = &*self.ivars().show_thinking_checkbox.borrow() {
                checkbox.state() == NSControlStateValueOn
            } else {
                false
            };

            // Get system prompt
            let system_prompt = if let Some(field) = &*self.ivars().system_prompt_input.borrow() {
                field.stringValue().to_string().trim().to_string()
            } else {
                "You are a helpful assistant, be direct and to the point. Respond in English.".to_string()
            };

            let parameters = ModelParameters {
                temperature,
                top_p: 0.95,
                max_tokens,
                thinking_budget,
                enable_thinking,
                show_thinking,
            };

            // Get base URL from the input field - this is important for OpenAI-compatible providers
            let base_url = if let Some(field) = &*self.ivars().base_url_input.borrow() {
                field.stringValue().to_string().trim().to_string()
            } else {
                String::new()
            };
            log_to_file(&format!("  Base URL: {base_url}"));

            // Check if editing existing profile or creating new
            let editing_id = self.ivars().editing_profile_id.borrow().clone();

            let profile = ModelProfile {
                id: editing_id.unwrap_or_else(Uuid::new_v4),
                name,
                provider_id,
                model_id,
                base_url,
                auth,
                parameters,
                system_prompt,
            };
            log_to_file(&format!("  Profile ID: {:?} (editing: {})", profile.id, editing_id.is_some()));

            // Load and save config
            let config_path = match Config::default_path() {
                Ok(path) => path,
                Err(e) => {
                    log_to_file(&format!("  ERROR: Failed to get config path: {e}"));
                    return;
                }
            };

            let mut config = match Config::load(&config_path) {
                Ok(c) => c,
                Err(e) => {
                    log_to_file(&format!("  ERROR: Failed to load config: {e}"));
                    Config::default()
                }
            };

            // If editing, remove old profile first
            if editing_id.is_some() {
                config.profiles.retain(|p| p.id != profile.id);
                log_to_file(&format!("  Removed old profile, remaining: {}", config.profiles.len()));
            }

            // Add profile
            config.add_profile(profile.clone());

            // If first profile, make it default
            if config.profiles.len() == 1 {
                config.default_profile = Some(profile.id);
            }

            // Save
            if let Err(e) = config.save(&config_path) {
                log_to_file(&format!("  ERROR: Failed to save config: {e}"));
                return;
            }

            log_to_file(&format!("  Profile saved successfully! Total profiles: {}", config.profiles.len()));

            // Go back to settings
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowSettingsView");
            unsafe { center.postNotificationName_object(&name, None); }
        }

        #[unsafe(method(providerChanged:))]
        fn provider_changed(&self, _sender: Option<&NSObject>) {
            log_to_file("ProfileEditorDemo: Provider changed");
        }

        #[unsafe(method(authTypeChanged:))]
        fn auth_type_changed(&self, _sender: Option<&NSObject>) {
            log_to_file("ProfileEditorDemo: Auth type changed");
            if let Some(popup) = &*self.ivars().auth_type_popup.borrow() {
                let index = popup.indexOfSelectedItem();
                if let Some(auth_value) = &*self.ivars().auth_value_input.borrow() {
                    if index == 2 {
                        auth_value.setHidden(true);
                    } else {
                        auth_value.setHidden(false);
                        if index == 0 {
                            auth_value.setPlaceholderString(Some(&NSString::from_str("sk-...")));
                        } else {
                            auth_value.setPlaceholderString(Some(&NSString::from_str("/path/to/keyfile")));
                        }
                    }
                }
            }
        }

        #[unsafe(method(temperatureChanged:))]
        fn temperature_changed(&self, _sender: Option<&NSObject>) {
            if let Some(stepper) = &*self.ivars().temperature_stepper.borrow() {
                let value = stepper.doubleValue();
                if let Some(label) = &*self.ivars().temperature_label.borrow() {
                    label.setStringValue(&NSString::from_str(&format!("{value:.2}")));
                }
            }
        }

        #[unsafe(method(enableThinkingChanged:))]
        fn enable_thinking_changed(&self, _sender: Option<&NSObject>) {
            log_to_file("ProfileEditorDemo: Enable thinking changed");
            if let Some(checkbox) = &*self.ivars().enable_thinking_checkbox.borrow() {
                let enabled = checkbox.state() == NSControlStateValueOn;
                if let Some(budget) = &*self.ivars().thinking_budget_input.borrow() {
                    budget.setHidden(!enabled);
                }
                if let Some(show_cb) = &*self.ivars().show_thinking_checkbox.borrow() {
                    show_cb.setHidden(!enabled);
                    show_cb.setEnabled(enabled);
                }
            }
        }
    }
);

impl ProfileEditorDemoViewController {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm
            .alloc::<Self>()
            .set_ivars(ProfileEditorDemoIvars::default());
        unsafe { msg_send![super(this), init] }
    }

    fn build_top_bar(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let stack = NSStackView::new(mtm);
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        stack.setSpacing(8.0);
        stack.setEdgeInsets(NSEdgeInsets {
            top: 8.0,
            left: 12.0,
            bottom: 8.0,
            right: 12.0,
        });
        stack.setTranslatesAutoresizingMaskIntoConstraints(false);

        stack.setWantsLayer(true);
        if let Some(layer) = stack.layer() {
            let color =
                CGColor::new_generic_rgb(Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2, 1.0);
            layer.setBackgroundColor(Some(&color));
        }

        let cancel_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Cancel"),
                Some(self),
                Some(sel!(cancelClicked:)),
                mtm,
            )
        };
        let cancel_width = cancel_btn.widthAnchor().constraintEqualToConstant(70.0);
        cancel_width.setActive(true);
        stack.addArrangedSubview(&cancel_btn);

        let spacer = NSView::new(mtm);
        spacer.setTranslatesAutoresizingMaskIntoConstraints(false);
        spacer.setContentHuggingPriority_forOrientation(
            1.0,
            NSLayoutConstraintOrientation::Horizontal,
        );
        stack.addArrangedSubview(&spacer);

        let save_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Save"),
                Some(self),
                Some(sel!(saveClicked:)),
                mtm,
            )
        };
        let save_width = save_btn.widthAnchor().constraintEqualToConstant(60.0);
        save_width.setActive(true);
        stack.addArrangedSubview(&save_btn);

        Retained::from(&*stack as &NSView)
    }

    fn build_form_scroll(&self, mtm: MainThreadMarker) -> Retained<NSScrollView> {
        let scroll_view = NSScrollView::new(mtm);
        scroll_view.setHasVerticalScroller(true);
        scroll_view.setTranslatesAutoresizingMaskIntoConstraints(false);
        scroll_view.setDrawsBackground(false);

        // Use FlippedStackView so form starts at TOP (not bottom)
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

        // Check if editing existing profile
        let editing_profile = if let Some(editing_id) = &*self.ivars().editing_profile_id.borrow() {
            // Load profile from config
            if let Ok(config_path) = Config::default_path() {
                if let Ok(config) = Config::load(&config_path) {
                    config.profiles.into_iter().find(|p| p.id == *editing_id)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Get preselected values (from model selector OR editing profile)
        let preselected_provider = if let Some(ref profile) = editing_profile {
            Some(profile.provider_id.clone())
        } else {
            self.ivars().preselected_provider.borrow().clone()
        };
        let preselected_model = if let Some(ref profile) = editing_profile {
            Some(profile.model_id.clone())
        } else {
            self.ivars().preselected_model.borrow().clone()
        };

        // Profile Name - use editing profile name, or default to model name
        let default_name = if let Some(ref profile) = editing_profile {
            profile.name.clone()
        } else if let Some(ref model_id) = preselected_model {
            model_id.clone()
        } else {
            "My Profile".to_string()
        };
        let name_section = self.build_form_field("Profile Name", &default_name, mtm);
        form_stack.addArrangedSubview(&name_section.0);
        *self.ivars().name_input.borrow_mut() = Some(name_section.1);

        // Provider popup - include the preselected provider if not in default list
        let mut providers = vec![
            "anthropic",
            "openai",
            "google",
            "groq",
            "mistral",
            "ollama",
            "custom",
        ];
        let provider_to_select = preselected_provider.as_deref();

        // Add preselected provider to list if not already there
        if let Some(ref provider) = preselected_provider {
            if !providers.contains(&provider.as_str()) {
                providers.insert(0, Box::leak(provider.clone().into_boxed_str()));
            }
        }

        let provider_section = self.build_popup_field("Provider", &providers, mtm);
        form_stack.addArrangedSubview(&provider_section.0);

        // Select the preselected provider in the popup
        if let Some(provider) = provider_to_select {
            let provider_popup = &provider_section.1;
            // Find and select the matching item
            if let Some(idx) = providers.iter().position(|p| *p == provider) {
                provider_popup.selectItemAtIndex(idx as isize);
            }
        }
        *self.ivars().provider_popup.borrow_mut() = Some(provider_section.1);

        // Model popup - show the preselected model
        let model_items: Vec<&str> = if let Some(ref model_id) = preselected_model {
            vec![model_id.as_str()]
        } else {
            vec!["(select a model)"]
        };

        let model_section = self.build_popup_field("Model", &model_items, mtm);
        form_stack.addArrangedSubview(&model_section.0);
        // First item is already selected by default
        *self.ivars().model_popup.borrow_mut() = Some(model_section.1);

        if preselected_provider.is_some() && preselected_model.is_some() {
            log_to_file(&format!(
                "Profile editor configured with: {}:{}",
                preselected_provider.as_deref().unwrap_or("?"),
                preselected_model.as_deref().unwrap_or("?")
            ));
        }

        // Base URL - use editing profile, preselected, or default
        let default_base_url = if let Some(ref profile) = editing_profile {
            if profile.base_url.is_empty() {
                "https://api.anthropic.com/v1".to_string()
            } else {
                profile.base_url.clone()
            }
        } else {
            self.ivars()
                .preselected_base_url
                .borrow()
                .clone()
                .unwrap_or_else(|| "https://api.anthropic.com/v1".to_string())
        };
        let url_section = self.build_form_field("Base URL", &default_base_url, mtm);
        form_stack.addArrangedSubview(&url_section.0);
        *self.ivars().base_url_input.borrow_mut() = Some(url_section.1);

        // Authentication - use editing profile auth if available
        let auth_section = self.build_auth_section_with_profile(mtm, editing_profile.as_ref());
        form_stack.addArrangedSubview(&auth_section);

        // System Prompt - use editing profile or default
        let default_system_prompt = if let Some(ref profile) = editing_profile {
            profile.system_prompt.clone()
        } else {
            "You are a helpful assistant, be direct and to the point. Respond in English."
                .to_string()
        };
        let system_prompt_section =
            self.build_multiline_field("System Prompt", &default_system_prompt, mtm);
        form_stack.addArrangedSubview(&system_prompt_section.0);
        *self.ivars().system_prompt_input.borrow_mut() = Some(system_prompt_section.1);

        // Parameters - pass editing profile for loading saved values
        let params_section =
            self.build_parameters_section_with_profile(mtm, editing_profile.as_ref());
        form_stack.addArrangedSubview(&params_section);

        scroll_view.setDocumentView(Some(&form_stack));

        let form_width = form_stack
            .widthAnchor()
            .constraintEqualToAnchor_constant(&scroll_view.contentView().widthAnchor(), 0.0);
        form_width.setActive(true);

        // Scroll to top after view is set up
        let clip_view = scroll_view.contentView();
        clip_view.scrollToPoint(NSPoint::new(0.0, 0.0));
        scroll_view.reflectScrolledClipView(&clip_view);

        scroll_view
    }

    fn build_form_field(
        &self,
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
        // Set both placeholder and string value so field shows content immediately
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

    /// Build a multiline text field (for system prompt)
    fn build_multiline_field(
        &self,
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

        // Create a text field that allows multiple lines
        let input = NSTextField::new(mtm);
        input.setStringValue(&NSString::from_str(default_text));
        input.setTranslatesAutoresizingMaskIntoConstraints(false);
        unsafe {
            input.setEditable(true);
            input.setSelectable(true);
        }

        // Set width and height constraints
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

    fn build_popup_field(
        &self,
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

    fn build_auth_section(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        self.build_auth_section_with_profile(mtm, None)
    }

    fn build_auth_section_with_profile(
        &self,
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
            auth_popup.setTarget(Some(self));
            auth_popup.setAction(Some(sel!(authTypeChanged:)));
        }

        // Set auth type from profile
        let (auth_index, auth_value_str) = if let Some(profile) = profile {
            match &profile.auth {
                AuthConfig::Key { value } => (0, value.clone()),
                AuthConfig::Keyfile { path } => (1, path.clone()),
            }
        } else {
            (0, String::new())
        };
        auth_popup.selectItemAtIndex(auth_index);

        let width = auth_popup
            .widthAnchor()
            .constraintGreaterThanOrEqualToConstant(350.0);
        width.setActive(true);
        container.addArrangedSubview(&auth_popup);
        *self.ivars().auth_type_popup.borrow_mut() = Some(auth_popup);

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
        *self.ivars().auth_value_input.borrow_mut() = Some(auth_value);

        Retained::from(&*container as &NSView)
    }

    fn build_parameters_section(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        self.build_parameters_section_with_profile(mtm, None)
    }

    fn build_parameters_section_with_profile(
        &self,
        mtm: MainThreadMarker,
        profile: Option<&ModelProfile>,
    ) -> Retained<NSView> {
        let container = NSStackView::new(mtm);
        container.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        container.setSpacing(12.0);
        container.setTranslatesAutoresizingMaskIntoConstraints(false);

        let header = NSTextField::labelWithString(&NSString::from_str("Parameters"), mtm);
        header.setTextColor(Some(&Theme::text_primary()));
        header.setFont(Some(&NSFont::boldSystemFontOfSize(13.0)));
        container.addArrangedSubview(&header);

        // Get defaults from profile or fallbacks
        let default_temp = profile.map(|p| p.parameters.temperature).unwrap_or(1.0);
        let default_max_tokens = profile
            .map(|p| p.parameters.max_tokens.to_string())
            .or_else(|| {
                self.ivars()
                    .preselected_context
                    .borrow()
                    .map(|c| c.to_string())
            })
            .unwrap_or_else(|| "4096".to_string());
        let default_thinking_budget = profile
            .and_then(|p| p.parameters.thinking_budget)
            .map(|b| b.to_string())
            .unwrap_or_else(|| "10000".to_string());
        let default_enable_thinking = profile.is_some_and(|p| p.parameters.enable_thinking);
        let default_show_thinking = profile.is_some_and(|p| p.parameters.show_thinking);

        // Temperature with stepper (up/down arrows)
        let temp_container = NSStackView::new(mtm);
        temp_container.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        temp_container.setSpacing(8.0);

        let temp_label = NSTextField::labelWithString(&NSString::from_str("Temperature"), mtm);
        temp_label.setTextColor(Some(&Theme::text_primary()));
        temp_label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        let label_width = temp_label.widthAnchor().constraintEqualToConstant(100.0);
        label_width.setActive(true);
        temp_container.addArrangedSubview(&temp_label);

        // Value display
        let value_label =
            NSTextField::labelWithString(&NSString::from_str(&format!("{default_temp:.2}")), mtm);
        value_label.setTextColor(Some(&Theme::text_primary()));
        value_label.setFont(Some(&NSFont::monospacedDigitSystemFontOfSize_weight(
            13.0, 0.0,
        )));
        let value_width = value_label.widthAnchor().constraintEqualToConstant(45.0);
        value_width.setActive(true);
        temp_container.addArrangedSubview(&value_label);
        *self.ivars().temperature_label.borrow_mut() = Some(value_label);

        // Stepper (up/down arrows)
        let stepper = NSStepper::new(mtm);
        stepper.setMinValue(0.0);
        stepper.setMaxValue(2.0);
        stepper.setDoubleValue(default_temp);
        stepper.setIncrement(0.1);
        stepper.setValueWraps(false);
        unsafe {
            stepper.setTarget(Some(self));
            stepper.setAction(Some(sel!(temperatureChanged:)));
        }
        temp_container.addArrangedSubview(&stepper);
        *self.ivars().temperature_stepper.borrow_mut() = Some(stepper);

        // Range hint
        let range_label = NSTextField::labelWithString(&NSString::from_str("(0.0 - 2.0)"), mtm);
        range_label.setTextColor(Some(&Theme::text_secondary_color()));
        range_label.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        temp_container.addArrangedSubview(&range_label);

        container.addArrangedSubview(&temp_container);

        // Max Tokens
        let (tokens_view, tokens_input) =
            self.build_form_field("Max Tokens", &default_max_tokens, mtm);
        container.addArrangedSubview(&tokens_view);
        *self.ivars().max_tokens_input.borrow_mut() = Some(tokens_input);

        // Thinking Budget
        let (budget_view, budget_input) =
            self.build_form_field("Thinking Budget", &default_thinking_budget, mtm);
        container.addArrangedSubview(&budget_view);
        *self.ivars().thinking_budget_input.borrow_mut() = Some(budget_input);

        // Enable Thinking checkbox
        let enable_cb = unsafe {
            NSButton::checkboxWithTitle_target_action(
                &NSString::from_str("Enable Thinking"),
                Some(self),
                Some(sel!(enableThinkingChanged:)),
                mtm,
            )
        };
        if default_enable_thinking {
            enable_cb.setState(NSControlStateValueOn);
        }
        container.addArrangedSubview(&enable_cb);
        *self.ivars().enable_thinking_checkbox.borrow_mut() = Some(enable_cb);

        // Show Thinking checkbox
        let show_cb = unsafe {
            NSButton::checkboxWithTitle_target_action(
                &NSString::from_str("Show Thinking"),
                None,
                None,
                mtm,
            )
        };
        if default_show_thinking {
            show_cb.setState(NSControlStateValueOn);
        }
        container.addArrangedSubview(&show_cb);
        *self.ivars().show_thinking_checkbox.borrow_mut() = Some(show_cb);

        Retained::from(&*container as &NSView)
    }
}
