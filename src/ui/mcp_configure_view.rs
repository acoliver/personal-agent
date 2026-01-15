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
use personal_agent::mcp::{McpAuthType, McpConfig, McpPackage, McpPackageType, McpSource, McpTransport};
use personal_agent::mcp::secrets::SecretsManager;
use uuid::Uuid;

use super::mcp_add_view::{ParsedMcp, PARSED_MCP};

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
    name_input: RefCell<Option<Retained<NSTextField>>>,
    auth_type_popup: RefCell<Option<Retained<NSPopUpButton>>>,
    api_key_input: RefCell<Option<Retained<NSTextField>>>,
    keyfile_input: RefCell<Option<Retained<NSTextField>>>,
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

            // Get parsed MCP from thread-local
            let parsed_mcp = PARSED_MCP.with(|cell| cell.borrow().clone());
            *self.ivars().parsed_mcp.borrow_mut() = parsed_mcp.clone();

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
            
            // Get auth type
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
                log_to_file("ERROR: No parsed MCP data");
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
            let mcp_config = McpConfig {
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
            };
            
            log_to_file(&format!("MCP config: {mcp_config:?}"));
            
            // Store API key in secrets if provided
            if auth_type == McpAuthType::ApiKey && !api_key.is_empty() {
                // Get default secrets directory
                let secrets_dir = dirs::home_dir()
                    .unwrap_or_default()
                    .join("Library/Application Support/PersonalAgent/secrets");
                
                let secrets_manager = SecretsManager::new(secrets_dir);
                
                if let Err(e) = secrets_manager.store_api_key(mcp_id, &api_key) {
                    log_to_file(&format!("ERROR: Failed to store API key: {e}"));
                    self.show_error("Failed to store API key", &format!("{e}"));
                    return;
                }
            }
            
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
    }
);

impl McpConfigureViewController {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let ivars = McpConfigureViewIvars {
            parsed_mcp: RefCell::new(None),
            name_input: RefCell::new(None),
            auth_type_popup: RefCell::new(None),
            api_key_input: RefCell::new(None),
            keyfile_input: RefCell::new(None),
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

        // Get default name from parsed MCP
        let default_name = if let Some(ref parsed) = *self.ivars().parsed_mcp.borrow() {
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

        // Auth type popup
        let auth_section = self.build_auth_section(mtm);
        form_stack.addArrangedSubview(&auth_section);

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
}
