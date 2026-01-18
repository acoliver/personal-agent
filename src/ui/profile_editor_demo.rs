//! Demo Profile Editor - validates the wireframe layout
//!
//! This is a standalone test view that implements the Profile Editor wireframe exactly.
//! Used to verify the layout works before integrating into the main app.
#![allow(unsafe_code)]
#![allow(unused_unsafe)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::single_char_pattern)]
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::unused_self)]

use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::Write;

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAppearanceCustomization, NSButton, NSControlStateValueOn, NSLayoutConstraintOrientation,
    NSPopUpButton, NSScrollView, NSStackView, NSStackViewDistribution, NSStepper, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView, NSViewController,
};
use objc2_core_graphics::CGColor;
use objc2_foundation::{NSEdgeInsets, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use super::model_selector::{
    SELECTED_MODEL_BASE_URL, SELECTED_MODEL_CONTEXT, SELECTED_MODEL_ID, SELECTED_MODEL_PROVIDER,
};
use crate::ui::Theme;
use personal_agent::config::Config;
use personal_agent::models::{AuthConfig, ModelParameters, ModelProfile};

use super::profile_editor_demo_sections::{
    attach_form_width, build_auth_section, build_base_url_section, build_form_stack,
    build_multiline_field, build_parameters_section, build_profile_name_section,
    build_provider_section, derive_profile_defaults, log_selected_model, scroll_to_top,
    EditingProfileDefaults,
};

use uuid::Uuid;

/// Logging helper - writes to file
pub(super) fn log_to_file(message: &str) {
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
    pub(super) editing_profile_id: RefCell<Option<Uuid>>,
    pub(super) preselected_provider: RefCell<Option<String>>,
    pub(super) preselected_model: RefCell<Option<String>>,
    pub(super) preselected_base_url: RefCell<Option<String>>,
    pub(super) preselected_context: RefCell<Option<u64>>,
    pub(super) name_input: RefCell<Option<Retained<NSTextField>>>,
    pub(super) provider_popup: RefCell<Option<Retained<NSPopUpButton>>>,
    pub(super) model_popup: RefCell<Option<Retained<NSPopUpButton>>>,
    pub(super) base_url_input: RefCell<Option<Retained<NSTextField>>>,
    pub(super) auth_type_popup: RefCell<Option<Retained<NSPopUpButton>>>,
    pub(super) auth_value_input: RefCell<Option<Retained<NSTextField>>>,
    pub(super) system_prompt_input: RefCell<Option<Retained<NSTextField>>>,
    pub(super) temperature_stepper: RefCell<Option<Retained<NSStepper>>>,
    pub(super) temperature_label: RefCell<Option<Retained<NSTextField>>>,
    pub(super) max_tokens_input: RefCell<Option<Retained<NSTextField>>>,
    pub(super) thinking_budget_input: RefCell<Option<Retained<NSTextField>>>,
    pub(super) enable_thinking_checkbox: RefCell<Option<Retained<NSButton>>>,
    pub(super) show_thinking_checkbox: RefCell<Option<Retained<NSButton>>>,
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

    #[allow(clippy::unused_self)]
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
            if let Some(dark_appearance) =
                objc2_app_kit::NSAppearance::appearanceNamed(dark_appearance_name)
            {
                root_view.setAppearance(Some(&dark_appearance));
            }

            root_view.setWantsLayer(true);
            if let Some(layer) = root_view.layer() {
                let color = CGColor::new_generic_rgb(
                    Theme::BG_DARKEST.0,
                    Theme::BG_DARKEST.1,
                    Theme::BG_DARKEST.2,
                    1.0,
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

            top_bar.setContentHuggingPriority_forOrientation(
                750.0,
                NSLayoutConstraintOrientation::Vertical,
            );
            top_bar.setContentCompressionResistancePriority_forOrientation(
                750.0,
                NSLayoutConstraintOrientation::Vertical,
            );
            form_scroll.setContentHuggingPriority_forOrientation(
                1.0,
                NSLayoutConstraintOrientation::Vertical,
            );
            form_scroll.setContentCompressionResistancePriority_forOrientation(
                250.0,
                NSLayoutConstraintOrientation::Vertical,
            );

            main_stack.addArrangedSubview(&top_bar);
            main_stack.addArrangedSubview(&form_scroll);
            root_view.addSubview(&main_stack);

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

            let top_height = top_bar.heightAnchor().constraintEqualToConstant(44.0);
            top_height.setActive(true);
            let form_min_height = form_scroll
                .heightAnchor()
                .constraintGreaterThanOrEqualToConstant(100.0);
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
            unsafe {
                center.postNotificationName_object(&name, None);
            }
        }

        #[unsafe(method(saveClicked:))]
        fn save_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("ProfileEditorDemo: Save clicked");

            let name = self.profile_name();
            if name.is_empty() {
                log_to_file("  ERROR: Name is empty");
                return;
            }

            let provider_id = self.selected_provider_id();
            let model_id = self.selected_model_id();
            let auth = self.current_auth_config();
            let parameters = self.current_parameters();
            let system_prompt = self.current_system_prompt();
            let base_url = self.current_base_url();
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
            log_to_file(&format!(
                "  Profile ID: {:?} (editing: {})",
                profile.id,
                editing_id.is_some()
            ));

            if self
                .persist_profile(&profile, editing_id.is_some())
                .is_none()
            {
                return;
            }

            self.return_to_settings();
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
                            auth_value.setPlaceholderString(Some(&NSString::from_str(
                                "/path/to/keyfile",
                            )));
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

    fn profile_name(&self) -> String {
        let name = if let Some(name_field) = &*self.ivars().name_input.borrow() {
            name_field.stringValue().to_string().trim().to_string()
        } else {
            String::new()
        };
        log_to_file(&format!("  Name: {name}"));
        name
    }

    fn selected_provider_id(&self) -> String {
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
        provider_id
    }

    fn selected_model_id(&self) -> String {
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
        model_id
    }

    fn current_auth_config(&self) -> AuthConfig {
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
                AuthConfig::Key {
                    value: String::new(),
                }
            }
        } else {
            AuthConfig::Key {
                value: String::new(),
            }
        };
        log_to_file(&format!("  Auth: {auth:?}"));
        auth
    }

    fn current_parameters(&self) -> ModelParameters {
        let temperature = if let Some(stepper) = &*self.ivars().temperature_stepper.borrow() {
            stepper.doubleValue()
        } else {
            0.7
        };

        let max_tokens = if let Some(field) = &*self.ivars().max_tokens_input.borrow() {
            field
                .stringValue()
                .to_string()
                .parse::<u32>()
                .unwrap_or(4096)
        } else {
            4096
        };

        let enable_thinking =
            if let Some(checkbox) = &*self.ivars().enable_thinking_checkbox.borrow() {
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

        ModelParameters {
            temperature,
            top_p: 0.95,
            max_tokens,
            thinking_budget,
            enable_thinking,
            show_thinking,
        }
    }

    fn current_system_prompt(&self) -> String {
        if let Some(field) = &*self.ivars().system_prompt_input.borrow() {
            field.stringValue().to_string().trim().to_string()
        } else {
            "You are a helpful assistant, be direct and to the point. Respond in English."
                .to_string()
        }
    }

    fn current_base_url(&self) -> String {
        let base_url = if let Some(field) = &*self.ivars().base_url_input.borrow() {
            field.stringValue().to_string().trim().to_string()
        } else {
            String::new()
        };
        log_to_file(&format!("  Base URL: {base_url}"));
        base_url
    }

    fn load_profile_defaults(&self) -> EditingProfileDefaults {
        let editing_profile = if let Some(editing_id) = &*self.ivars().editing_profile_id.borrow() {
            if let Ok(config_path) = Config::default_path() {
                Config::load(&config_path)
                    .ok()
                    .and_then(|config| config.profiles.into_iter().find(|p| p.id == *editing_id))
            } else {
                None
            }
        } else {
            None
        };

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

        derive_profile_defaults(
            editing_profile,
            preselected_provider,
            preselected_model,
            self.ivars().preselected_base_url.borrow().clone(),
        )
    }

    fn populate_form_sections(
        &self,
        form_stack: &NSStackView,
        defaults: &EditingProfileDefaults,
        mtm: MainThreadMarker,
    ) {
        let name_section =
            build_profile_name_section(self, "Profile Name", &defaults.default_name, mtm);
        form_stack.addArrangedSubview(&name_section.0);
        *self.ivars().name_input.borrow_mut() = Some(name_section.1);

        let mut providers = vec![
            "anthropic",
            "openai",
            "google",
            "groq",
            "mistral",
            "ollama",
            "custom",
        ];
        let provider_to_select = defaults.preselected_provider.as_deref();

        if let Some(ref provider) = defaults.preselected_provider {
            if !providers.contains(&provider.as_str()) {
                providers.insert(0, Box::leak(provider.clone().into_boxed_str()));
            }
        }

        let provider_section = build_provider_section(self, "Provider", providers.as_slice(), mtm);
        form_stack.addArrangedSubview(&provider_section.0);

        if let Some(provider) = provider_to_select {
            let provider_popup = &provider_section.1;
            if let Some(idx) = providers.iter().position(|p| *p == provider) {
                provider_popup.selectItemAtIndex(idx as isize);
            }
        }
        *self.ivars().provider_popup.borrow_mut() = Some(provider_section.1);

        let model_items: Vec<&str> = defaults
            .preselected_model
            .as_deref()
            .map_or_else(|| vec!["(select a model)"], |model_id| vec![model_id]);

        let model_section = build_provider_section(self, "Model", model_items.as_slice(), mtm);
        form_stack.addArrangedSubview(&model_section.0);
        *self.ivars().model_popup.borrow_mut() = Some(model_section.1);

        log_selected_model(provider_to_select, defaults.preselected_model.as_deref());

        let url_section = build_base_url_section(self, &defaults.default_base_url, mtm);
        form_stack.addArrangedSubview(&url_section.0);
        *self.ivars().base_url_input.borrow_mut() = Some(url_section.1);

        let auth_section = build_auth_section(self, mtm, defaults.editing_profile.as_ref());
        form_stack.addArrangedSubview(&auth_section);

        let system_prompt_section =
            build_multiline_field(self, "System Prompt", &defaults.default_system_prompt, mtm);
        form_stack.addArrangedSubview(&system_prompt_section.0);
        *self.ivars().system_prompt_input.borrow_mut() = Some(system_prompt_section.1);

        let params_section = build_parameters_section(self, mtm, defaults.editing_profile.as_ref());
        form_stack.addArrangedSubview(&params_section);
    }

    fn persist_profile(&self, profile: &ModelProfile, is_editing: bool) -> Option<()> {
        let config_path = match Config::default_path() {
            Ok(path) => path,
            Err(e) => {
                log_to_file(&format!("  ERROR: Failed to get config path: {e}"));
                return None;
            }
        };

        let mut config = match Config::load(&config_path) {
            Ok(c) => c,
            Err(e) => {
                log_to_file(&format!("  ERROR: Failed to load config: {e}"));
                Config::default()
            }
        };

        if is_editing {
            config.profiles.retain(|p| p.id != profile.id);
            log_to_file(&format!(
                "  Removed old profile, remaining: {}",
                config.profiles.len()
            ));
        }

        config.add_profile(profile.clone());

        if config.profiles.len() == 1 {
            config.default_profile = Some(profile.id);
        }

        if let Err(e) = config.save(&config_path) {
            log_to_file(&format!("  ERROR: Failed to save config: {e}"));
            return None;
        }

        log_to_file(&format!(
            "  Profile saved successfully! Total profiles: {}",
            config.profiles.len()
        ));

        Some(())
    }

    fn return_to_settings(&self) {
        use objc2_foundation::NSNotificationCenter;
        let center = NSNotificationCenter::defaultCenter();
        let name = NSString::from_str("PersonalAgentShowSettingsView");
        unsafe {
            center.postNotificationName_object(&name, None);
        }
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

        let form_stack = build_form_stack(mtm);
        let defaults = self.load_profile_defaults();

        self.populate_form_sections(&form_stack, &defaults, mtm);

        scroll_view.setDocumentView(Some(&form_stack));
        attach_form_width(&form_stack, &scroll_view);
        scroll_to_top(&scroll_view);

        scroll_view
    }
}
