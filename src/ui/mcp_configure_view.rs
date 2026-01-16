//! Configure MCP view - set name, auth, etc.

use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::Write;

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAppearanceCustomization, NSButton, NSBezelStyle, NSFont, NSLayoutConstraintOrientation, NSPopUpButton, NSScrollView,
    NSStackView, NSStackViewDistribution, NSTextField, NSUserInterfaceLayoutOrientation, NSView, NSViewController,
};
use objc2_foundation::{NSEdgeInsets, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use objc2_core_graphics::CGColor;

use crate::ui::Theme;
use personal_agent::config::Config;
use personal_agent::mcp::{EnvVarConfig, McpAuthType, McpConfig, McpPackage, McpPackageType, McpSource, McpTransport};
use personal_agent::mcp::secrets::SecretsManager;
use uuid::Uuid;

use super::mcp_add_view::{ParsedMcp, PARSED_MCP, SELECTED_MCP_CONFIG};

fn log_to_file(message: &str) {
    let log_path = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/PersonalAgent/debug.log");
    
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let _ = writeln!(file, "[{timestamp}] McpConfigureView: {message}");
    }
}

pub struct McpConfigureViewIvars {
    parsed_mcp: RefCell<Option<ParsedMcp>>,
    selected_config: RefCell<Option<McpConfig>>,
    name_input: RefCell<Option<Retained<NSTextField>>>,
    auth_type_popup: RefCell<Option<Retained<NSPopUpButton>>>,
    api_key_input: RefCell<Option<Retained<NSTextField>>>,
    keyfile_input: RefCell<Option<Retained<NSTextField>>>,
    form_stack: RefCell<Option<Retained<super::FlippedStackView>>>,
    env_var_inputs: RefCell<Vec<(String, Retained<NSTextField>)>>,
    auth_section: RefCell<Option<Retained<NSView>>>,
    oauth_button: RefCell<Option<Retained<NSButton>>>,
    oauth_status_label: RefCell<Option<Retained<NSTextField>>>,
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
            
            // Validate and get name
            let name = if let Some(field) = &*self.ivars().name_input.borrow() {
                field.stringValue().to_string().trim().to_string()
            } else {
                String::new()
            };
            
            if name.is_empty() {
                log_to_file("ERROR: Name is empty");
                self.show_error("Validation Error", "MCP name is required");
                return;
            }
            
            // Get default secrets directory
            let secrets_dir = dirs::home_dir()
                .unwrap_or_default()
                .join("Library/Application Support/PersonalAgent/secrets");
            let secrets_manager = SecretsManager::new(secrets_dir);
            
            // Build MCP config - either from selected registry config or parsed manual entry
            let mcp_config = if let Some(base_config) = &*self.ivars().selected_config.borrow() {
                log_to_file("Building from selected registry config");
                
                // Clone and update with user input
                let mut config = base_config.clone();
                config.name = name; // User may have edited the name
                
                // Collect env var values from dynamic fields
                let mut env_values = std::collections::HashMap::new();
                for (var_name, field) in self.ivars().env_var_inputs.borrow().iter() {
                    let value = field.stringValue().to_string().trim().to_string();
                    if !value.is_empty() {
                        env_values.insert(var_name.clone(), value.clone());
                        
                        // Store secrets
                        let var_name_lower = var_name.to_lowercase();
                        let is_secret = var_name_lower.contains("key") 
                            || var_name_lower.contains("secret")
                            || var_name_lower.contains("token")
                            || var_name_lower.contains("password")
                            || var_name_lower.contains("pat");
                        
                        if is_secret {
                            log_to_file(&format!("Storing secret for {}", var_name));
                            if let Err(e) = secrets_manager.store_api_key_named(config.id, var_name, &value) {
                                log_to_file(&format!("ERROR: Failed to store secret {}: {}", var_name, e));
                                self.show_error("Failed to store secret", &format!("{}: {}", var_name, e));
                                return;
                            }
                        }
                    }
                }
                
                // Validate required env vars
                for env_var in &config.env_vars {
                    if env_var.required && !env_values.contains_key(&env_var.name) {
                        log_to_file(&format!("ERROR: Required env var missing: {}", env_var.name));
                        self.show_error("Validation Error", &format!("Required field '{}' is empty", env_var.name));
                        return;
                    }
                }
                
                // Store env values in config.config JSON
                config.config = serde_json::to_value(&env_values).unwrap_or_default();
                
                config
            } else {
                log_to_file("Building from manual parsed MCP");
                
                // Get auth type from popup
                let auth_type = if let Some(popup) = &*self.ivars().auth_type_popup.borrow() {
                    let index = popup.indexOfSelectedItem();
                    match index {
                        0 => McpAuthType::ApiKey,
                        1 => McpAuthType::Keyfile,
                        _ => McpAuthType::None,
                    }
                } else {
                    McpAuthType::None
                };
                
                // Get auth values
                let api_key = if let Some(field) = &*self.ivars().api_key_input.borrow() {
                    field.stringValue().to_string().trim().to_string()
                } else {
                    String::new()
                };
                
                let keyfile_path = if let Some(field) = &*self.ivars().keyfile_input.borrow() {
                    let path_str = field.stringValue().to_string().trim().to_string();
                    if path_str.is_empty() {
                        None
                    } else {
                        Some(std::path::PathBuf::from(path_str))
                    }
                } else {
                    None
                };
                
                // Build MCP config from parsed data
                let parsed = self.ivars().parsed_mcp.borrow();
                let Some(ref parsed) = *parsed else {
                    log_to_file("ERROR: No parsed MCP data and no selected config");
                    self.show_error("Configuration Error", "No MCP data to save");
                    return;
                };
                
                let (package, source) = match parsed {
                    ParsedMcp::Npm { identifier, runtime_hint } => {
                        let package = McpPackage {
                            package_type: McpPackageType::Npm,
                            identifier: identifier.clone(),
                            runtime_hint: Some(runtime_hint.clone()),
                        };
                        let source = McpSource::Manual {
                            url: format!("npx {}", identifier),
                        };
                        (package, source)
                    }
                    ParsedMcp::Docker { image } => {
                        let package = McpPackage {
                            package_type: McpPackageType::Docker,
                            identifier: image.clone(),
                            runtime_hint: None,
                        };
                        let source = McpSource::Manual {
                            url: format!("docker run {}", image),
                        };
                        (package, source)
                    }
                    ParsedMcp::Http { url } => {
                        let package = McpPackage {
                            package_type: McpPackageType::Http,
                            identifier: url.clone(),
                            runtime_hint: None,
                        };
                        let source = McpSource::Manual {
                            url: url.clone(),
                        };
                        (package, source)
                    }
                };
                
                let transport = match package.package_type {
                    McpPackageType::Http => McpTransport::Http,
                    _ => McpTransport::Stdio,
                };
                
                let mcp_id = Uuid::new_v4();
                
                // Store API key in secrets if provided
                if auth_type == McpAuthType::ApiKey && !api_key.is_empty() {
                    if let Err(e) = secrets_manager.store_api_key(mcp_id, &api_key) {
                        log_to_file(&format!("ERROR: Failed to store API key: {e}"));
                        self.show_error("Failed to store API key", &format!("{e}"));
                        return;
                    }
                }
                
                McpConfig {
                    id: mcp_id,
                    name: name.clone(),
                    enabled: true,
                    source,
                    package,
                    transport,
                    auth_type: auth_type.clone(),
                    env_vars: vec![],
                    keyfile_path: keyfile_path.clone(),
                    config: serde_json::json!({}),
                    oauth_token: None,
                }
            };
            
            log_to_file(&format!("MCP config: {mcp_config:?}"));
            
            // Save to config
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
            
            // Go back to settings
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowSettingsView");
            unsafe { center.postNotificationName_object(&name, None); }
        }

        #[unsafe(method(authTypeChanged:))]
        fn auth_type_changed(&self, _sender: Option<&NSObject>) {
            log_to_file("Auth type changed");
            if let Some(popup) = &*self.ivars().auth_type_popup.borrow() {
                let index = popup.indexOfSelectedItem();
                
                // Show/hide appropriate auth fields
                if let Some(api_key_field) = &*self.ivars().api_key_input.borrow() {
                    api_key_field.setHidden(index != 0);
                }
                if let Some(keyfile_field) = &*self.ivars().keyfile_input.borrow() {
                    keyfile_field.setHidden(index != 1);
                }
            }
        }

        #[unsafe(method(connectSmitheryClicked:))]
        fn connect_smithery_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("Connect with Smithery clicked");
            
            // Get the selected config
            let selected = self.ivars().selected_config.borrow();
            let Some(config) = selected.as_ref() else {
                log_to_file("ERROR: No selected config");
                return;
            };
            
            // Extract qualified name from package identifier
            // For Smithery: identifier is like "https://server.smithery.ai/@owner/server-name"
            let qualified_name = if let McpSource::Smithery { qualified_name } = &config.source {
                qualified_name.clone()
            } else {
                // Try to extract from package identifier as fallback
                config.package.identifier
                    .strip_prefix("https://server.smithery.ai/")
                    .unwrap_or(&config.name)
                    .to_string()
            };
            
            let config_id = config.id;
            
            log_to_file(&format!("Starting OAuth flow for: {}", qualified_name));
            
            // Update UI to show in-progress state
            if let Some(status_label) = &*self.ivars().oauth_status_label.borrow() {
                status_label.setStringValue(&NSString::from_str("Connecting..."));
            }
            if let Some(button) = &*self.ivars().oauth_button.borrow() {
                button.setEnabled(false);
            }
            
            // Start OAuth flow in background thread
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    log_to_file("OAuth: Starting callback server");
                    
                    // Start callback server
                    match personal_agent::mcp::start_oauth_callback_server().await {
                        Ok((port, receiver)) => {
                            log_to_file(&format!("OAuth: Callback server started on port {}", port));
                            
                            // Generate OAuth URL
                            let oauth_config = personal_agent::mcp::SmitheryOAuthConfig {
                                server_qualified_name: qualified_name.clone(),
                                redirect_uri: format!("http://localhost:{}", port),
                            };
                            
                            let oauth_url = personal_agent::mcp::generate_smithery_oauth_url(&oauth_config);
                            log_to_file(&format!("OAuth: Generated URL: {}", oauth_url));
                            
                            // Open browser
                            #[cfg(target_os = "macos")]
                            {
                                use std::process::Command;
                                if let Err(e) = Command::new("open").arg(&oauth_url).spawn() {
                                    log_to_file(&format!("OAuth: Failed to open browser: {}", e));
                                }
                            }
                            
                            // Wait for callback (with timeout)
                            log_to_file("OAuth: Waiting for callback (5 min timeout)");
                            match tokio::time::timeout(
                                std::time::Duration::from_secs(300), // 5 min timeout
                                receiver
                            ).await {
                                Ok(Ok(result)) => {
                                    if let Some(ref token) = result.token {
                                        log_to_file(&format!("OAuth: Got token (length: {})", token.len()));
                                        
                                        // Save token
                                        if let Err(e) = save_oauth_token(config_id, token) {
                                            log_to_file(&format!("OAuth: Failed to save token: {}", e));
                                        } else {
                                            log_to_file("OAuth: Token saved successfully");
                                            
                                            // Update UI on main thread
                                            use objc2::rc::autoreleasepool;
                                            autoreleasepool(|_| {
                                                use objc2_foundation::NSNotificationCenter;
                                                let center = NSNotificationCenter::defaultCenter();
                                                let name = NSString::from_str("PersonalAgentOAuthSuccess");
                                                unsafe { center.postNotificationName_object(&name, None); }
                                            });
                                        }
                                    } else if let Some(ref error) = result.error {
                                        log_to_file(&format!("OAuth: Error from callback: {}", error));
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
                            log_to_file(&format!("OAuth: Failed to start callback server: {}", e));
                        }
                    }
                });
            });
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

/// Helper function to save OAuth token to config
fn save_oauth_token(config_id: Uuid, token: &str) -> Result<(), String> {
    let config_path = Config::default_path()
        .map_err(|e| format!("Failed to get config path: {}", e))?;
    
    let mut config = Config::load(&config_path)
        .map_err(|e| format!("Failed to load config: {}", e))?;
    
    if let Some(mcp) = config.mcps.iter_mut().find(|m| m.id == config_id) {
        mcp.oauth_token = Some(token.to_string());
    } else {
        return Err("MCP config not found".to_string());
    }
    
    config.save(&config_path)
        .map_err(|e| format!("Failed to save config: {}", e))?;
    
    Ok(())
}

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

    fn build_top_bar(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let top_bar = NSStackView::new(mtm);
        top_bar.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        top_bar.setSpacing(8.0);
        top_bar.setEdgeInsets(NSEdgeInsets { top: 8.0, left: 12.0, bottom: 8.0, right: 12.0 });
        top_bar.setTranslatesAutoresizingMaskIntoConstraints(false);

        top_bar.setWantsLayer(true);
        if let Some(layer) = top_bar.layer() {
            let color = CGColor::new_generic_rgb(Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2, 1.0);
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
        back_btn.setBezelStyle(NSBezelStyle::Rounded);
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
        spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
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
        cancel_btn.setBezelStyle(NSBezelStyle::Rounded);
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
        save_btn.setBezelStyle(NSBezelStyle::Rounded);
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

        let form_stack = super::FlippedStackView::new(mtm);
        form_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        form_stack.setSpacing(16.0);
        form_stack.setEdgeInsets(NSEdgeInsets { top: 16.0, left: 16.0, bottom: 16.0, right: 16.0 });
        form_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        form_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
        
        // Store form_stack for adding dynamic fields later
        *self.ivars().form_stack.borrow_mut() = Some(Retained::clone(&form_stack));

        // Get default name - either from selected config or parsed MCP
        let default_name = if let Some(ref config) = *self.ivars().selected_config.borrow() {
            config.name.clone()
        } else if let Some(ref parsed) = *self.ivars().parsed_mcp.borrow() {
            match parsed {
                ParsedMcp::Npm { identifier, .. } => {
                    // Extract package name from @org/package
                    identifier.split('/').last().unwrap_or(identifier).to_string()
                }
                ParsedMcp::Docker { image } => {
                    // Extract image name from org/image:tag
                    image.split(':').next().unwrap_or(image)
                        .split('/').last().unwrap_or(image).to_string()
                }
                ParsedMcp::Http { url } => {
                    // Extract last path segment
                    url.split('/').last().unwrap_or("mcp").to_string()
                }
            }
        } else {
            "My MCP".to_string()
        };

        // Name field
        let name_section = self.build_form_field("Name", &default_name, mtm);
        form_stack.addArrangedSubview(&name_section.0);
        *self.ivars().name_input.borrow_mut() = Some(name_section.1);
        
        // If we have a selected config, check auth type
        if let Some(ref config) = *self.ivars().selected_config.borrow() {
            if config.auth_type == McpAuthType::OAuth {
                log_to_file("OAuth auth type detected - showing OAuth section");
                // Show OAuth section
                let oauth_section = self.build_oauth_section(mtm);
                form_stack.addArrangedSubview(&oauth_section);
            } else if !config.env_vars.is_empty() {
                log_to_file(&format!("Building {} dynamic env var fields", config.env_vars.len()));
                self.build_env_var_fields(&config.env_vars, mtm);
            } else {
                log_to_file("No env_vars in selected config - showing auth section");
                // No env vars, show standard auth section
                let auth_section = self.build_auth_section(mtm);
                form_stack.addArrangedSubview(&auth_section);
                *self.ivars().auth_section.borrow_mut() = Some(auth_section);
            }
        } else {
            // Manual entry - show standard auth section
            let auth_section = self.build_auth_section(mtm);
            form_stack.addArrangedSubview(&auth_section);
            *self.ivars().auth_section.borrow_mut() = Some(auth_section);
        }

        scroll_view.setDocumentView(Some(&form_stack));

        let form_width = form_stack.widthAnchor().constraintEqualToAnchor_constant(
            &scroll_view.contentView().widthAnchor(), 0.0,
        );
        form_width.setActive(true);

        scroll_view
    }

    fn build_form_field(&self, label: &str, default_value: &str, mtm: MainThreadMarker) -> (Retained<NSView>, Retained<NSTextField>) {
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
        let width = input.widthAnchor().constraintGreaterThanOrEqualToConstant(350.0);
        width.setActive(true);
        container.addArrangedSubview(&input);

        (Retained::from(&*container as &NSView), input)
    }

    fn build_env_var_fields(&self, env_vars: &[EnvVarConfig], mtm: MainThreadMarker) {
        log_to_file(&format!("build_env_var_fields called with {} vars", env_vars.len()));
        
        // Get the form stack - need to clone to avoid borrow issues
        let form_stack_opt = self.ivars().form_stack.borrow().clone();
        let form_stack = match form_stack_opt {
            Some(ref stack) => stack,
            None => {
                log_to_file("ERROR: form_stack not available");
                return;
            }
        };
        
        for env_var in env_vars {
            // Label with required indicator
            let label_text = if env_var.required {
                format!("{} (required)", env_var.name)
            } else {
                format!("{} (optional)", env_var.name)
            };
            
            let label = NSTextField::labelWithString(&NSString::from_str(&label_text), mtm);
            label.setTextColor(Some(&Theme::text_primary()));
            label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
            form_stack.addArrangedSubview(&label);
            
            // Input field
            let input = NSTextField::new(mtm);
            input.setPlaceholderString(Some(&NSString::from_str(&env_var.name)));
            input.setTranslatesAutoresizingMaskIntoConstraints(false);
            
            // Make it look like a secure field if it's likely a secret
            let var_name_lower = env_var.name.to_lowercase();
            let is_secret = var_name_lower.contains("key") 
                || var_name_lower.contains("secret")
                || var_name_lower.contains("token")
                || var_name_lower.contains("password")
                || var_name_lower.contains("pat");
            
            if is_secret {
                // Use a more muted placeholder for secrets
                input.setPlaceholderString(Some(&NSString::from_str(&format!("Enter {}", env_var.name))));
            }
            
            let width = input.widthAnchor().constraintGreaterThanOrEqualToConstant(350.0);
            width.setActive(true);
            form_stack.addArrangedSubview(&input);
            
            // Store reference
            self.ivars().env_var_inputs.borrow_mut().push((env_var.name.clone(), input));
            
            log_to_file(&format!("Added field for {}", env_var.name));
        }
    }
    
    fn build_auth_section(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let container = NSStackView::new(mtm);
        container.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        container.setSpacing(8.0);
        container.setTranslatesAutoresizingMaskIntoConstraints(false);

        let label = NSTextField::labelWithString(&NSString::from_str("Authentication"), mtm);
        label.setTextColor(Some(&Theme::text_primary()));
        label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        container.addArrangedSubview(&label);

        // Auth type popup
        let auth_popup = unsafe { NSPopUpButton::new(mtm) };
        auth_popup.addItemWithTitle(&NSString::from_str("API Key"));
        auth_popup.addItemWithTitle(&NSString::from_str("Key File"));
        auth_popup.addItemWithTitle(&NSString::from_str("None"));
        unsafe {
            auth_popup.setTarget(Some(self));
            auth_popup.setAction(Some(sel!(authTypeChanged:)));
        }
        auth_popup.selectItemAtIndex(2); // Default to None
        let width = auth_popup.widthAnchor().constraintGreaterThanOrEqualToConstant(350.0);
        width.setActive(true);
        container.addArrangedSubview(&auth_popup);
        *self.ivars().auth_type_popup.borrow_mut() = Some(auth_popup);

        // API Key input (hidden by default)
        let api_key_field = NSTextField::new(mtm);
        api_key_field.setPlaceholderString(Some(&NSString::from_str("Enter API key")));
        api_key_field.setHidden(true);
        let api_key_width = api_key_field.widthAnchor().constraintGreaterThanOrEqualToConstant(350.0);
        api_key_width.setActive(true);
        container.addArrangedSubview(&api_key_field);
        *self.ivars().api_key_input.borrow_mut() = Some(api_key_field);

        // Keyfile path input (hidden by default)
        let keyfile_field = NSTextField::new(mtm);
        keyfile_field.setPlaceholderString(Some(&NSString::from_str("/path/to/keyfile")));
        keyfile_field.setHidden(true);
        let keyfile_width = keyfile_field.widthAnchor().constraintGreaterThanOrEqualToConstant(350.0);
        keyfile_width.setActive(true);
        container.addArrangedSubview(&keyfile_field);
        *self.ivars().keyfile_input.borrow_mut() = Some(keyfile_field);

        Retained::from(&*container as &NSView)
    }

    fn build_oauth_section(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let container = NSStackView::new(mtm);
        container.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        container.setSpacing(12.0);
        container.setTranslatesAutoresizingMaskIntoConstraints(false);

        // Section label
        let label = NSTextField::labelWithString(&NSString::from_str("Authentication"), mtm);
        label.setTextColor(Some(&Theme::text_primary()));
        label.setFont(Some(&NSFont::boldSystemFontOfSize(12.0)));
        container.addArrangedSubview(&label);

        // Check if already connected
        let is_connected = self.ivars().selected_config.borrow()
            .as_ref()
            .and_then(|c| c.oauth_token.as_ref())
            .is_some();

        // Status label
        let status_text = if is_connected {
            "Connected [OK]"
        } else {
            "Not connected"
        };
        let status = NSTextField::labelWithString(&NSString::from_str(status_text), mtm);
        status.setTextColor(Some(&Theme::text_primary()));
        status.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        container.addArrangedSubview(&status);
        *self.ivars().oauth_status_label.borrow_mut() = Some(status);

        // Connect button
        let btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Connect with Smithery"),
                Some(self),
                Some(sel!(connectSmitheryClicked:)),
                mtm,
            )
        };
        btn.setBezelStyle(NSBezelStyle::Rounded);
        btn.setEnabled(!is_connected);
        let btn_width = btn.widthAnchor().constraintGreaterThanOrEqualToConstant(350.0);
        btn_width.setActive(true);
        container.addArrangedSubview(&btn);
        *self.ivars().oauth_button.borrow_mut() = Some(btn);

        // Info text
        let info = NSTextField::labelWithString(
            &NSString::from_str("Click to authorize this application with Smithery"), 
            mtm
        );
        info.setTextColor(Some(&Theme::text_secondary_color()));
        info.setFont(Some(&NSFont::systemFontOfSize(10.0)));
        container.addArrangedSubview(&info);

        Retained::from(&*container as &NSView)
    }
}
