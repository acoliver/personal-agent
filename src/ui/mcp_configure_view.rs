//! Configure MCP view - set name, auth, etc.
#![allow(unsafe_code)]
#![allow(unused_unsafe)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::assigning_clones)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::option_map_or_none)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::single_char_pattern)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::similar_names)]
#![allow(clippy::if_then_some_else_none)]
#![allow(clippy::ref_option)]
#![allow(clippy::unused_self)]
#![allow(clippy::double_ended_iterator_last)]
#![allow(clippy::if_not_else)]
#![allow(clippy::single_match_else)]

use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::Write;

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAppearanceCustomization, NSBezelStyle, NSButton, NSFont, NSLayoutConstraintOrientation,
    NSPopUpButton, NSScrollView, NSStackView, NSStackViewDistribution, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView, NSViewController,
};
use objc2_core_graphics::CGColor;
use objc2_foundation::{NSEdgeInsets, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use crate::ui::Theme;
use personal_agent::config::Config;
use personal_agent::mcp::{McpAuthType, McpConfig, McpSource, SecretsManager};

use super::mcp_configure_helpers::{
    build_auth_section, build_env_values, build_env_var_fields, build_form_stack,
    build_manual_mcp_config, build_oauth_section, build_package_arg_values,
    build_package_args_fields, form_default_name, merge_config_values, parse_manual_auth_inputs,
    save_oauth_token, validate_required_env_vars, validate_required_package_args,
};

use uuid::Uuid;

use super::mcp_add_view::{PARSED_MCP, SELECTED_MCP_CONFIG};
use crate::ui::mcp_add_helpers::ParsedMcp;

pub(super) fn log_to_file(message: &str) {
    let log_path = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/PersonalAgent/debug.log");

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let _ = writeln!(file, "[{timestamp}] McpConfigureView: {message}");
    }
}

pub struct McpConfigureViewIvars {
    pub(super) parsed_mcp: RefCell<Option<ParsedMcp>>,
    pub(super) selected_config: RefCell<Option<McpConfig>>,
    pub(super) name_input: RefCell<Option<Retained<NSTextField>>>,
    pub(super) auth_type_popup: RefCell<Option<Retained<NSPopUpButton>>>,
    pub(super) api_key_input: RefCell<Option<Retained<NSTextField>>>,
    pub(super) keyfile_input: RefCell<Option<Retained<NSTextField>>>,
    pub(super) form_stack: RefCell<Option<Retained<super::FlippedStackView>>>,
    pub(super) env_var_inputs: RefCell<Vec<(String, Retained<NSTextField>)>>,
    pub(super) package_arg_inputs: RefCell<Vec<(String, Retained<NSTextField>)>>,
    pub(super) auth_section: RefCell<Option<Retained<NSView>>>,
    pub(super) oauth_button: RefCell<Option<Retained<NSButton>>>,
    pub(super) oauth_status_label: RefCell<Option<Retained<NSTextField>>>,
}

define_class!(
    #[unsafe(super(NSViewController))]
    #[thread_kind = MainThreadOnly]
    #[name = "McpConfigureViewController"]
    #[ivars = McpConfigureViewIvars]
    pub struct McpConfigureViewController;

    unsafe impl NSObjectProtocol for McpConfigureViewController {}

    impl McpConfigureViewController {
        #[unsafe(method(loadView))]
        fn load_view(&self) {
            log_to_file("loadView started");
            let mtm = MainThreadMarker::new().unwrap();

            // Register for OAuth success notifications
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentOAuthSuccess");
            unsafe {
                center.addObserver_selector_name_object(
                    self,
                    sel!(oauthSuccessNotification:),
                    Some(&name),
                    None,
                );
            }

            // Check if we have a selected config from registry search
            let selected_config = SELECTED_MCP_CONFIG.with(|cell| cell.borrow_mut().take());

            if let Some(mcp_config) = selected_config {
                log_to_file(&format!("Using selected registry config: {} with {} env_vars",
                    mcp_config.name, mcp_config.env_vars.len()));

                // Store it for use in configure view
                *self.ivars().selected_config.borrow_mut() = Some(mcp_config.clone());
            } else {
                // Get parsed MCP from thread-local (manual URL entry)
                let parsed_mcp = PARSED_MCP.with(|cell| cell.borrow().clone());
                *self.ivars().parsed_mcp.borrow_mut() = parsed_mcp.clone();
            }

            let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 500.0));
            let root_view = NSView::initWithFrame(NSView::alloc(mtm), frame);

            // Force dark appearance
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
            form_scroll.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);

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

            self.setView(&root_view);
            log_to_file("loadView completed");
        }

        #[unsafe(method(backClicked:))]
        fn back_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("Back clicked");
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowAddMcp");
            unsafe { center.postNotificationName_object(&name, None); }
        }

        #[unsafe(method(cancelClicked:))]
        fn cancel_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("Cancel clicked");
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowSettingsView");
            unsafe { center.postNotificationName_object(&name, None); }
        }

        #[unsafe(method(saveClicked:))]
        fn save_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("Save clicked");

            let name = self.read_name_value();
            if name.is_empty() {
                log_to_file("ERROR: Name is empty");
                self.show_error("Validation Error", "MCP name is required");
                return;
            }

            let secrets_manager = self.create_secrets_manager();
            let mcp_config = if let Some(base_config) = &*self.ivars().selected_config.borrow() {
                log_to_file("Building from selected registry config");
                let mut config = base_config.clone();
                config.name = name;

                let env_values = match build_env_values(
                    &self.ivars().env_var_inputs.borrow(),
                    &secrets_manager,
                    config.id,
                ) {
                    Ok(values) => values,
                    Err(err) => {
                        log_to_file(&format!("ERROR: {err}"));
                        self.show_error("Failed to store secret", &err);
                        return;
                    }
                };

                let package_arg_values = build_package_arg_values(&self.ivars().package_arg_inputs.borrow());

                if let Some(missing) = validate_required_env_vars(&config.env_vars, &env_values) {
                    log_to_file(&format!("ERROR: Required env var missing: {missing}"));
                    self.show_error(
                        "Validation Error",
                        &format!("Required field '{missing}' is empty"),
                    );
                    return;
                }

                if let Some(missing) =
                    validate_required_package_args(&config.package_args, &package_arg_values)
                {
                    log_to_file(&format!("ERROR: Required package arg missing: {missing}"));
                    self.show_error(
                        "Validation Error",
                        &format!("Required field '{missing}' is empty"),
                    );
                    return;
                }

                merge_config_values(&mut config, &env_values, &package_arg_values);
                config
            } else {
                log_to_file("Building from manual parsed MCP");
                let parsed = self.ivars().parsed_mcp.borrow();
                let Some(ref parsed) = *parsed else {
                    log_to_file("ERROR: No parsed MCP data and no selected config");
                    self.show_error("Configuration Error", "No MCP data to save");
                    return;
                };

                let auth_inputs = parse_manual_auth_inputs(
                    self.ivars().auth_type_popup.borrow().as_ref(),
                    self.ivars().api_key_input.borrow().as_ref(),
                    self.ivars().keyfile_input.borrow().as_ref(),
                );

                let mcp_config = build_manual_mcp_config(parsed, name.clone(), auth_inputs);
                self.save_manual_api_key(&secrets_manager, &mcp_config);
                mcp_config
            };

            log_to_file(&format!("MCP config: {mcp_config:?}"));
            self.persist_config(mcp_config);
        }

        #[unsafe(method(authTypeChanged:))]
        fn auth_type_changed(&self, _sender: Option<&NSObject>) {
            log_to_file("Auth type changed");
            let Some(popup) = &*self.ivars().auth_type_popup.borrow() else {
                return;
            };

            let index = popup.indexOfSelectedItem();
            if let Some(api_key_field) = &*self.ivars().api_key_input.borrow() {
                api_key_field.setHidden(index != 0);
            }
            if let Some(keyfile_field) = &*self.ivars().keyfile_input.borrow() {
                keyfile_field.setHidden(index != 1);
            }
        }

        #[unsafe(method(connectSmitheryClicked:))]
        fn connect_smithery_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("Connect with Smithery clicked");

            let selected = self.ivars().selected_config.borrow();
            let Some(config) = selected.as_ref() else {
                log_to_file("ERROR: No selected config");
                return;
            };

            let qualified_name = if let McpSource::Smithery { qualified_name } = &config.source {
                qualified_name.clone()
            } else {
                config
                    .package
                    .identifier
                    .strip_prefix("https://server.smithery.ai/")
                    .unwrap_or(&config.name)
                    .to_string()
            };

            let config_id = config.id;
            log_to_file(&format!("Starting OAuth flow for: {qualified_name}"));

            self.update_oauth_ui(true);
            self.launch_oauth_flow(config_id, qualified_name);
        }

        #[unsafe(method(oauthSuccessNotification:))]
        fn oauth_success_notification(&self, _notification: &NSObject) {
            log_to_file("OAuth success notification received");

            // Update UI
            if let Some(status_label) = &*self.ivars().oauth_status_label.borrow() {
                status_label.setStringValue(&NSString::from_str("Connected"));
            }
            if let Some(button) = &*self.ivars().oauth_button.borrow() {
                button.setEnabled(false);
                button.setTitle(&NSString::from_str("Already Connected"));
            }
        }
    }
);

impl McpConfigureViewController {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let ivars = McpConfigureViewIvars {
            parsed_mcp: RefCell::new(None),
            selected_config: RefCell::new(None),
            name_input: RefCell::new(None),
            auth_type_popup: RefCell::new(None),
            api_key_input: RefCell::new(None),
            keyfile_input: RefCell::new(None),
            form_stack: RefCell::new(None),
            env_var_inputs: RefCell::new(Vec::new()),
            package_arg_inputs: RefCell::new(Vec::new()),
            auth_section: RefCell::new(None),
            oauth_button: RefCell::new(None),
            oauth_status_label: RefCell::new(None),
        };
        let this = mtm.alloc::<Self>().set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }

    fn show_error(&self, title: &str, message: &str) {
        use objc2_app_kit::NSAlert;
        let mtm = MainThreadMarker::new().unwrap();
        let alert = NSAlert::new(mtm);
        alert.setMessageText(&NSString::from_str(title));
        alert.setInformativeText(&NSString::from_str(message));
        alert.addButtonWithTitle(&NSString::from_str("OK"));
        unsafe { alert.runModal() };
    }

    fn read_name_value(&self) -> String {
        self.ivars()
            .name_input
            .borrow()
            .as_ref()
            .map(|field| field.stringValue().to_string().trim().to_string())
            .unwrap_or_default()
    }

    fn create_secrets_manager(&self) -> SecretsManager {
        let secrets_dir = dirs::home_dir()
            .unwrap_or_default()
            .join("Library/Application Support/PersonalAgent/secrets");
        SecretsManager::new(secrets_dir)
    }

    fn save_manual_api_key(&self, secrets_manager: &SecretsManager, config: &McpConfig) {
        if config.auth_type != McpAuthType::ApiKey {
            return;
        }

        let api_key = self
            .ivars()
            .api_key_input
            .borrow()
            .as_ref()
            .map(|field| field.stringValue().to_string().trim().to_string())
            .unwrap_or_default();

        if api_key.is_empty() {
            return;
        }

        if let Err(e) = secrets_manager.store_api_key(config.id, &api_key) {
            log_to_file(&format!("ERROR: Failed to store API key: {e}"));
            self.show_error("Failed to store API key", &format!("{e}"));
        }
    }

    fn persist_config(&self, mcp_config: McpConfig) {
        let config_path = match Config::default_path() {
            Ok(path) => path,
            Err(e) => {
                log_to_file(&format!("ERROR: Failed to get config path: {e}"));
                return;
            }
        };

        let mut config = match Config::load(&config_path) {
            Ok(c) => c,
            Err(e) => {
                log_to_file(&format!("ERROR: Failed to load config: {e}"));
                Config::default()
            }
        };

        config.mcps.push(mcp_config);

        if let Err(e) = config.save(&config_path) {
            log_to_file(&format!("ERROR: Failed to save config: {e}"));
            self.show_error("Failed to save configuration", &format!("{e}"));
            return;
        }

        log_to_file("MCP saved successfully");

        use objc2_foundation::NSNotificationCenter;
        let center = NSNotificationCenter::defaultCenter();
        let name = NSString::from_str("PersonalAgentShowSettingsView");
        unsafe {
            center.postNotificationName_object(&name, None);
        }
    }

    fn update_oauth_ui(&self, connecting: bool) {
        if let Some(status_label) = &*self.ivars().oauth_status_label.borrow() {
            let text = if connecting {
                "Connecting..."
            } else {
                "Connected"
            };
            status_label.setStringValue(&NSString::from_str(text));
        }
        if let Some(button) = &*self.ivars().oauth_button.borrow() {
            button.setEnabled(!connecting);
            if !connecting {
                button.setTitle(&NSString::from_str("Already Connected"));
            }
        }
    }

    fn launch_oauth_flow(&self, config_id: Uuid, qualified_name: String) {
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                log_to_file("OAuth: Starting callback server");

                match personal_agent::mcp::start_oauth_callback_server().await {
                    Ok((port, receiver)) => {
                        log_to_file(&format!("OAuth: Callback server started on port {port}"));

                        let oauth_config = personal_agent::mcp::SmitheryOAuthConfig {
                            server_qualified_name: qualified_name.clone(),
                            redirect_uri: format!("http://localhost:{port}"),
                        };

                        let oauth_url =
                            personal_agent::mcp::generate_smithery_oauth_url(&oauth_config);
                        log_to_file(&format!("OAuth: Generated URL: {oauth_url}"));

                        #[cfg(target_os = "macos")]
                        {
                            use std::process::Command;
                            if let Err(e) = Command::new("open").arg(&oauth_url).spawn() {
                                log_to_file(&format!("OAuth: Failed to open browser: {e}"));
                            }
                        }

                        log_to_file("OAuth: Waiting for callback (5 min timeout)");
                        match tokio::time::timeout(std::time::Duration::from_secs(300), receiver)
                            .await
                        {
                            Ok(Ok(result)) => {
                                if let Some(ref token) = result.token {
                                    log_to_file(&format!(
                                        "OAuth: Got token (length: {})",
                                        token.len()
                                    ));

                                    if let Err(e) = save_oauth_token(config_id, token) {
                                        log_to_file(&format!("OAuth: Failed to save token: {e}"));
                                    } else {
                                        log_to_file("OAuth: Token saved successfully");
                                        use objc2::rc::autoreleasepool;
                                        autoreleasepool(|_| {
                                            use objc2_foundation::NSNotificationCenter;
                                            let center = NSNotificationCenter::defaultCenter();
                                            let name =
                                                NSString::from_str("PersonalAgentOAuthSuccess");
                                            unsafe {
                                                center.postNotificationName_object(&name, None);
                                            }
                                        });
                                    }
                                } else if let Some(ref error) = result.error {
                                    log_to_file(&format!("OAuth: Error from callback: {error}"));
                                }
                            }
                            Ok(Err(_)) => {
                                log_to_file("OAuth: Callback channel closed");
                            }
                            Err(_) => {
                                log_to_file("OAuth: Timeout waiting for callback");
                            }
                        }
                    }
                    Err(e) => {
                        log_to_file(&format!("OAuth: Failed to start callback server: {e}"));
                    }
                }
            });
        });
    }

    fn add_auth_form_sections(&self, form_stack: &NSStackView, mtm: MainThreadMarker) {
        if let Some(ref config) = *self.ivars().selected_config.borrow() {
            self.add_registry_auth_sections(form_stack, config, mtm);
        } else {
            self.add_manual_auth_section(form_stack, mtm);
        }
    }

    fn add_registry_auth_sections(
        &self,
        form_stack: &NSStackView,
        config: &McpConfig,
        mtm: MainThreadMarker,
    ) {
        if config.auth_type == McpAuthType::OAuth {
            log_to_file("OAuth auth type detected - showing OAuth section");
            let oauth_section = build_oauth_section(self, mtm);
            form_stack.addArrangedSubview(&oauth_section);
            return;
        }

        self.add_package_arg_fields(config, mtm);
        self.add_env_or_auth_fields(form_stack, config, mtm);
    }

    fn add_package_arg_fields(&self, config: &McpConfig, mtm: MainThreadMarker) {
        if config.package_args.is_empty() {
            return;
        }

        log_to_file(&format!(
            "Building {} package args fields",
            config.package_args.len()
        ));
        build_package_args_fields(self, &config.package_args, mtm);
    }

    fn add_env_or_auth_fields(
        &self,
        form_stack: &NSStackView,
        config: &McpConfig,
        mtm: MainThreadMarker,
    ) {
        if !config.env_vars.is_empty() {
            log_to_file(&format!(
                "Building {} dynamic env var fields",
                config.env_vars.len()
            ));
            build_env_var_fields(self, &config.env_vars, mtm);
            return;
        }

        log_to_file("No env_vars in selected config - showing auth section");
        let auth_section = build_auth_section(self, mtm);
        form_stack.addArrangedSubview(&auth_section);
        *self.ivars().auth_section.borrow_mut() = Some(auth_section);
    }

    fn add_manual_auth_section(&self, form_stack: &NSStackView, mtm: MainThreadMarker) {
        let auth_section = build_auth_section(self, mtm);
        form_stack.addArrangedSubview(&auth_section);
        *self.ivars().auth_section.borrow_mut() = Some(auth_section);
    }

    fn build_top_bar(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let top_bar = NSStackView::new(mtm);
        top_bar.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        top_bar.setSpacing(8.0);
        top_bar.setEdgeInsets(NSEdgeInsets {
            top: 8.0,
            left: 12.0,
            bottom: 8.0,
            right: 12.0,
        });
        top_bar.setTranslatesAutoresizingMaskIntoConstraints(false);

        top_bar.setWantsLayer(true);
        if let Some(layer) = top_bar.layer() {
            let color =
                CGColor::new_generic_rgb(Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2, 1.0);
            layer.setBackgroundColor(Some(&color));
        }

        // Back button
        let back_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("<"),
                Some(self),
                Some(sel!(backClicked:)),
                mtm,
            )
        };
        back_btn.setBezelStyle(NSBezelStyle::Automatic);
        let back_width = back_btn.widthAnchor().constraintEqualToConstant(40.0);
        back_width.setActive(true);
        top_bar.addArrangedSubview(&back_btn);

        // Title
        let title = NSTextField::labelWithString(&NSString::from_str("Configure MCP"), mtm);
        title.setTextColor(Some(&Theme::text_primary()));
        title.setFont(Some(&NSFont::boldSystemFontOfSize(14.0)));
        top_bar.addArrangedSubview(&title);

        // Spacer
        let spacer = NSView::new(mtm);
        spacer.setContentHuggingPriority_forOrientation(
            1.0,
            NSLayoutConstraintOrientation::Horizontal,
        );
        top_bar.addArrangedSubview(&spacer);

        // Cancel button
        let cancel_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Cancel"),
                Some(self),
                Some(sel!(cancelClicked:)),
                mtm,
            )
        };
        cancel_btn.setBezelStyle(NSBezelStyle::Automatic);
        let cancel_width = cancel_btn.widthAnchor().constraintEqualToConstant(70.0);
        cancel_width.setActive(true);
        top_bar.addArrangedSubview(&cancel_btn);

        // Save button
        let save_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Save"),
                Some(self),
                Some(sel!(saveClicked:)),
                mtm,
            )
        };
        save_btn.setBezelStyle(NSBezelStyle::Automatic);
        let save_width = save_btn.widthAnchor().constraintEqualToConstant(60.0);
        save_width.setActive(true);
        top_bar.addArrangedSubview(&save_btn);

        Retained::from(&*top_bar as &NSView)
    }

    fn build_form_scroll(&self, mtm: MainThreadMarker) -> Retained<NSScrollView> {
        let scroll_view = NSScrollView::new(mtm);
        scroll_view.setHasVerticalScroller(true);
        scroll_view.setTranslatesAutoresizingMaskIntoConstraints(false);
        scroll_view.setDrawsBackground(false);

        let form_stack = build_form_stack(mtm);
        *self.ivars().form_stack.borrow_mut() = Some(Retained::clone(&form_stack));

        let default_name = form_default_name(
            self.ivars().selected_config.borrow().as_ref(),
            self.ivars().parsed_mcp.borrow().as_ref(),
        );
        let name_section = self.build_form_field("Name", &default_name, mtm);
        form_stack.addArrangedSubview(&name_section.0);
        *self.ivars().name_input.borrow_mut() = Some(name_section.1);

        self.add_auth_form_sections(&form_stack, mtm);

        scroll_view.setDocumentView(Some(&form_stack));

        let form_width = form_stack
            .widthAnchor()
            .constraintEqualToAnchor_constant(&scroll_view.contentView().widthAnchor(), 0.0);
        form_width.setActive(true);

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
}
