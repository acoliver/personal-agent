//! Add MCP view - enter URL or search registries
#![allow(unsafe_code)]
#![allow(unused_unsafe)]
#![allow(unused_variables)]
#![allow(clippy::single_char_pattern)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::option_map_or_none)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::explicit_iter_loop)]

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAppearanceCustomization, NSBezelStyle, NSButton, NSButtonType, NSFont,
    NSLayoutConstraintOrientation, NSPopUpButton, NSScrollView, NSStackView,
    NSStackViewDistribution, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
    NSViewController,
};
use objc2_core_graphics::CGColor;
use objc2_foundation::{
    NSEdgeInsets, NSNotificationCenter, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};

use crate::ui::mcp_add_helpers::{log_to_file, parse_mcp_url, ParsedMcp};
use crate::ui::Theme;
use personal_agent::config::Config;
use personal_agent::mcp::{
    registry::{McpRegistry, McpRegistryServerWrapper},
    McpAuthType, McpConfig,
};

mod registry_search;
use registry_search::{SearchContext, SearchResults, SEARCH_RESULTS};

// Thread-local storage for passing parsed MCP to configure view
thread_local! {
    pub static PARSED_MCP: RefCell<Option<ParsedMcp>> = const { RefCell::new(None) };
    pub static SELECTED_MCP_CONFIG: RefCell<Option<McpConfig>> = const { RefCell::new(None) };
}

pub struct McpAddViewIvars {
    search_field: RefCell<Option<Retained<NSTextField>>>,
    results_stack: RefCell<Option<Retained<NSStackView>>>,
    loading_label: RefCell<Option<Retained<NSTextField>>>,
    results_scroll: RefCell<Option<Retained<NSScrollView>>>,
    url_input: RefCell<Option<Retained<NSTextField>>>,
    next_button: RefCell<Option<Retained<NSButton>>>,
    registry_popup: RefCell<Option<Retained<NSPopUpButton>>>,
    search_helper: RefCell<Option<Retained<NSTextField>>>,
    selected_result_index: RefCell<Option<usize>>,
    key_input_container: RefCell<Option<Retained<NSView>>>,
    key_input_field: RefCell<Option<Retained<NSTextField>>>,
    search_button: RefCell<Option<Retained<NSButton>>>,
}

define_class!(
    #[unsafe(super(NSViewController))]
    #[thread_kind = MainThreadOnly]
    #[name = "McpAddViewController"]
    #[ivars = McpAddViewIvars]
    pub struct McpAddViewController;

    unsafe impl NSObjectProtocol for McpAddViewController {}

    impl McpAddViewController {
        #[unsafe(method(loadView))]
        fn load_view(&self) {
            log_to_file("loadView started");
            let mtm = MainThreadMarker::new().unwrap();

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
            let content = self.build_content(mtm);

            top_bar.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            content.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);

            main_stack.addArrangedSubview(&top_bar);
            main_stack.addArrangedSubview(&content);
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

            // Register for search complete notification
            // First remove any existing observer for this notification
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentMcpSearchComplete");
            unsafe {
                center.removeObserver_name_object(self, Some(&name), None);
                center.addObserver_selector_name_object(
                    self,
                    sel!(searchCompleteNotification:),
                    Some(&name),
                    None,
                );
            }

            log_to_file("loadView completed");
        }

        #[unsafe(method(searchCompleteNotification:))]
        fn search_complete_notification(&self, _notification: Option<&NSObject>) {
            log_to_file("Search complete notification received");
            self.update_search_results();
        }

        #[unsafe(method(backClicked:))]
        fn back_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("Back clicked");
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowSettingsView");
            unsafe { center.postNotificationName_object(&name, None); }
        }

        #[unsafe(method(nextClicked:))]
        fn next_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("Next clicked");

            let url = if let Some(field) = &*self.ivars().url_input.borrow() {
                field.stringValue().to_string()
            } else {
                String::new()
            };

            match parse_mcp_url(&url) {
                Ok(parsed) => {
                    log_to_file(&format!("Parsed MCP: {parsed:?}"));
                    PARSED_MCP.with(|cell| {
                        *cell.borrow_mut() = Some(parsed);
                    });

                    use objc2_foundation::NSNotificationCenter;
                    let center = NSNotificationCenter::defaultCenter();
                    let name = NSString::from_str("PersonalAgentShowConfigureMcp");
                    unsafe { center.postNotificationName_object(&name, None); }
                }
                Err(e) => {
                    log_to_file(&format!("Parse error: {e}"));

                    use objc2_app_kit::NSAlert;
                    let mtm = MainThreadMarker::new().unwrap();
                    let alert = NSAlert::new(mtm);
                    alert.setMessageText(&NSString::from_str("Invalid URL"));
                    alert.setInformativeText(&NSString::from_str(&e));
                    alert.addButtonWithTitle(&NSString::from_str("OK"));
                    unsafe { alert.runModal() };
                }
            }
        }

        #[unsafe(method(urlChanged:))]
        fn url_changed(&self, _sender: Option<&NSObject>) {
            // Enable Next button when URL is non-empty OR a result is selected
            let url = if let Some(field) = &*self.ivars().url_input.borrow() {
                field.stringValue().to_string()
            } else {
                String::new()
            };

            let has_url = !url.trim().is_empty();
            let has_selection = self.ivars().selected_result_index.borrow().is_some();
            let is_valid = has_url || has_selection;

            if let Some(btn) = &*self.ivars().next_button.borrow() {
                btn.setEnabled(is_valid);
            }
        }

        #[unsafe(method(registryChanged:))]
        fn registry_changed(&self, _sender: Option<&NSObject>) {
            log_to_file("Registry changed");

            let selected_index = if let Some(popup) = &*self.ivars().registry_popup.borrow() {
                popup.indexOfSelectedItem()
            } else {
                0
            };

            // 0 = "Select...", 1 = "Official", 2 = "Smithery"
            let is_smithery_selected = selected_index == 2;

            if is_smithery_selected {
                // Check if config has smithery_auth
                let has_key = if let Ok(config_path) = personal_agent::config::Config::default_path() {
                    if let Ok(config) = personal_agent::config::Config::load(&config_path) {
                        config.smithery_auth.as_ref().map(|s| !s.is_empty()).unwrap_or(false)
                    } else {
                        false
                    }
                } else {
                    false
                };

                if !has_key {
                    // Show key input and open browser
                    if let Some(container) = &*self.ivars().key_input_container.borrow() {
                        container.setHidden(false);
                    }

                    // Open Smithery keys page
                    use objc2_app_kit::NSWorkspace;
                    use objc2_foundation::NSURL;
                    let _mtm = MainThreadMarker::new().unwrap();
                    let url_str = NSString::from_str("https://smithery.ai/account/api-keys");
                    if let Some(url) = unsafe { NSURL::URLWithString(&url_str) } {
                        let workspace = NSWorkspace::sharedWorkspace();
                        unsafe { workspace.openURL(&url) };
                    }

                    // Disable search until key is saved
                    if let Some(field) = &*self.ivars().search_field.borrow() {
                        field.setEnabled(false);
                    }
                    if let Some(btn) = &*self.ivars().search_button.borrow() {
                        btn.setEnabled(false);
                    }
                    return;
                }
            }

            // Hide key input for non-Smithery or if key exists
            if let Some(container) = &*self.ivars().key_input_container.borrow() {
                container.setHidden(true);
            }

            let should_enable_search = selected_index > 0;

            if let Some(field) = &*self.ivars().search_field.borrow() {
                field.setEnabled(should_enable_search);
            }

            if let Some(btn) = &*self.ivars().search_button.borrow() {
                btn.setEnabled(should_enable_search);
            }

            if let Some(helper) = &*self.ivars().search_helper.borrow() {
                helper.setHidden(should_enable_search);
            }
        }

        #[unsafe(method(saveKeyClicked:))]
        fn save_key_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("Save key clicked");

            let key_or_path = if let Some(field) = &*self.ivars().key_input_field.borrow() {
                field.stringValue().to_string()
            } else {
                String::new()
            };

            if key_or_path.trim().is_empty() {
                return;
            }

            let config_path = match personal_agent::config::Config::default_path() {
                Ok(path) => path,
                Err(e) => {
                    log_to_file(&format!("ERROR: Failed to get config path: {e}"));
                    return;
                }
            };

            let mut config = match personal_agent::config::Config::load(&config_path) {
                Ok(c) => c,
                Err(e) => {
                    log_to_file(&format!("ERROR: Failed to load config: {e}"));
                    personal_agent::config::Config::default()
                }
            };

            config.smithery_auth = Some(key_or_path.clone());

            if let Err(e) = config.save(&config_path) {
                log_to_file(&format!("ERROR: Failed to save config: {e}"));

                use objc2_app_kit::NSAlert;
                let mtm = MainThreadMarker::new().unwrap();
                let alert = NSAlert::new(mtm);
                alert.setMessageText(&NSString::from_str("Failed to Save Key"));
                alert.setInformativeText(&NSString::from_str(&format!("Error: {e}")));
                alert.addButtonWithTitle(&NSString::from_str("OK"));
                unsafe { alert.runModal() };
            } else {
                log_to_file("Smithery key saved successfully");

                // Hide key input container
                if let Some(container) = &*self.ivars().key_input_container.borrow() {
                    container.setHidden(true);
                }

                // Enable search
                if let Some(field) = &*self.ivars().search_field.borrow() {
                    field.setEnabled(true);
                }
                if let Some(btn) = &*self.ivars().search_button.borrow() {
                    btn.setEnabled(true);
                }

                // Update dropdown text to remove "(requires API key)"
                if let Some(popup) = &*self.ivars().registry_popup.borrow() {
                    if let Some(item) = popup.itemAtIndex(2) {
                        item.setTitle(&NSString::from_str("Smithery"));
                    }
                }
            }
        }

        #[unsafe(method(searchFieldAction:))]
        fn search_field_action(&self, _sender: Option<&NSObject>) {
            let query = if let Some(field) = &*self.ivars().search_field.borrow() {
                field.stringValue().to_string()
            } else {
                String::new()
            };

            if query.trim().is_empty() {
                return;
            }

            log_to_file(&format!("Search triggered: {query}"));
            self.perform_search(query);
        }

        #[unsafe(method(searchButtonClicked:))]
        fn search_button_clicked(&self, _sender: Option<&NSObject>) {
            // Trigger the same search logic as pressing Enter
            let query = if let Some(field) = &*self.ivars().search_field.borrow() {
                field.stringValue().to_string()
            } else {
                String::new()
            };

            if query.trim().is_empty() {
                return;
            }

            log_to_file(&format!("Search button clicked: {query}"));
            self.perform_search(query);
        }

        #[unsafe(method(resultClicked:))]
        fn result_clicked(&self, sender: Option<&NSObject>) {
            if let Some(button) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                let tag = button.tag() as usize;
                log_to_file(&format!("Result clicked with tag: {tag}"));

                *self.ivars().selected_result_index.borrow_mut() = Some(tag);

                if let Ok(entries_guard) = SEARCH_RESULTS.lock() {
                    let entries = entries_guard.clone().unwrap_or_default();
                    self.handle_selected_entry(tag, &entries);
                }
            }
        }
    }
);

impl McpAddViewController {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let ivars = McpAddViewIvars {
            search_field: RefCell::new(None),
            results_stack: RefCell::new(None),
            loading_label: RefCell::new(None),
            results_scroll: RefCell::new(None),
            url_input: RefCell::new(None),
            next_button: RefCell::new(None),
            registry_popup: RefCell::new(None),
            search_helper: RefCell::new(None),
            selected_result_index: RefCell::new(None),
            key_input_container: RefCell::new(None),
            key_input_field: RefCell::new(None),
            search_button: RefCell::new(None),
        };
        let this = mtm.alloc::<Self>().set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }

    fn perform_search(&self, query: String) {
        log_to_file("perform_search started");

        let search_context = SearchContext::from_popup(&self.ivars().registry_popup);
        let smithery_key = search_context.load_smithery_key();

        log_to_file(&format!(
            "Searching registry: {:?}",
            search_context.registry_source
        ));

        self.show_loading(true);
        SearchResults::clear();

        search_context.spawn_search(query, smithery_key);
    }

    fn show_loading(&self, show: bool) {
        if let Some(label) = &*self.ivars().loading_label.borrow() {
            label.setHidden(!show);
        }
    }

    fn update_search_results(&self) {
        let mtm = MainThreadMarker::new().unwrap();

        self.show_loading(false);

        if let Some(stack) = &*self.ivars().results_stack.borrow() {
            Self::clear_results_stack(stack);

            if let Some(entries) = SearchResults::take() {
                log_to_file(&format!("Adding {} results to UI", entries.len()));
                if entries.is_empty() {
                    Self::add_no_results_label(stack, mtm);
                } else {
                    self.add_result_rows(stack, &entries, mtm);
                }
            }
        }
    }

    fn clear_results_stack(stack: &NSStackView) {
        let subviews = stack.arrangedSubviews();
        log_to_file(&format!("Clearing {} existing subviews", subviews.len()));
        for view in subviews.iter() {
            stack.removeArrangedSubview(&view);
            view.removeFromSuperview();
        }
    }

    fn add_no_results_label(stack: &NSStackView, mtm: MainThreadMarker) {
        let label = NSTextField::labelWithString(&NSString::from_str("No servers found."), mtm);
        label.setTextColor(Some(&Theme::text_secondary_color()));
        label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        stack.addArrangedSubview(&label);
    }

    fn add_result_rows(
        &self,
        stack: &NSStackView,
        entries: &[McpRegistryServerWrapper],
        mtm: MainThreadMarker,
    ) {
        for (index, wrapper) in entries.iter().enumerate() {
            let row = self.create_result_row(wrapper, index, mtm);
            stack.addArrangedSubview(&row);
        }
    }

    fn handle_selected_entry(&self, tag: usize, entries: &[McpRegistryServerWrapper]) {
        if tag >= entries.len() {
            return;
        }

        let wrapper = &entries[tag];

        match McpRegistry::entry_to_config(wrapper) {
            Ok(mcp_config) => {
                log_to_file(&format!("Converted to config: {}", mcp_config.name));

                let needs_config = !mcp_config.env_vars.is_empty()
                    || !mcp_config.package_args.is_empty()
                    || mcp_config.auth_type != McpAuthType::None;

                if needs_config {
                    log_to_file(&format!(
                        "Config needs setup - env_vars: {}, auth_type: {:?}",
                        mcp_config.env_vars.len(),
                        mcp_config.auth_type
                    ));

                    SELECTED_MCP_CONFIG.with(|cell| {
                        *cell.borrow_mut() = Some(mcp_config);
                    });

                    use objc2_foundation::NSNotificationCenter;
                    let center = NSNotificationCenter::defaultCenter();
                    let name = NSString::from_str("PersonalAgentShowConfigureMcp");
                    unsafe {
                        center.postNotificationName_object(&name, None);
                    }
                } else {
                    log_to_file("No config needed - saving directly");
                    self.save_config_and_return_to_settings(mcp_config);
                }
            }
            Err(e) => {
                log_to_file(&format!("Failed to convert entry: {e}"));
            }
        }
    }

    #[allow(clippy::unused_self)]
    fn save_config_and_return_to_settings(&self, mcp_config: McpConfig) {
        let _ = self;
        Self::save_config(mcp_config);
        let center = NSNotificationCenter::defaultCenter();
        let name = NSString::from_str("PersonalAgentShowSettingsView");
        unsafe {
            center.postNotificationName_object(&name, None);
        }
    }

    fn save_config(mcp_config: McpConfig) {
        let config_path = match Config::default_path() {
            Ok(path) => path,
            Err(e) => {
                log_to_file(&format!("ERROR: Failed to get config path: {e}"));
                return;
            }
        };

        let mut config = match Config::load(&config_path) {
            Ok(config) => config,
            Err(e) => {
                log_to_file(&format!("ERROR: Failed to load config: {e}"));
                return;
            }
        };

        let mut found = false;
        for existing_mcp in &mut config.mcps {
            if existing_mcp.id == mcp_config.id {
                *existing_mcp = mcp_config.clone();
                found = true;
                break;
            }
        }

        if !found {
            config.mcps.push(mcp_config);
        }

        if let Err(e) = config.save(&config_path) {
            log_to_file(&format!("ERROR: Failed to save config: {e}"));
            return;
        }

        log_to_file("MCP saved successfully");

        unsafe {
            let notification_center = NSNotificationCenter::defaultCenter();
            notification_center
                .postNotificationName_object(&NSString::from_str("MCPConfigSaved"), None);
        }
    }

    #[allow(dead_code)]
    fn reload_mcp_service() {
        std::thread::spawn(|| {
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async {
                    use personal_agent::mcp::McpService;
                    let service = McpService::global();
                    let mut guard = service.lock().await;
                    if let Err(e) = guard.reload().await {
                        eprintln!("Failed to reload MCPs: {e}");
                    }
                });
            }
        });
    }

    fn create_result_row(
        &self,
        wrapper: &McpRegistryServerWrapper,
        index: usize,
        mtm: MainThreadMarker,
    ) -> Retained<NSView> {
        let server = &wrapper.server;

        let button = NSButton::initWithFrame(
            NSButton::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(360.0, 50.0)),
        );
        button.setButtonType(NSButtonType::MomentaryLight);
        button.setBezelStyle(NSBezelStyle::SmallSquare);
        button.setBordered(false);
        button.setTitle(&NSString::from_str(""));
        button.setTag(index as isize);

        button.setWantsLayer(true);
        if let Some(layer) = button.layer() {
            let color = CGColor::new_generic_rgb(0.15, 0.15, 0.15, 1.0);
            layer.setBackgroundColor(Some(&color));
            layer.setCornerRadius(6.0);
        }

        unsafe {
            button.setTarget(Some(self));
            button.setAction(Some(sel!(resultClicked:)));
            button.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        let desc_text: String = server.description.chars().take(45).collect();
        let desc_text = if server.description.chars().count() > 45 {
            format!("{desc_text}...")
        } else {
            desc_text
        };

        let title = format!("{}\n {}", server.name, desc_text);
        button.setTitle(&NSString::from_str(&title));

        let width = button.widthAnchor().constraintEqualToConstant(360.0);
        let height = button.heightAnchor().constraintEqualToConstant(50.0);
        width.setActive(true);
        height.setActive(true);

        Retained::from(&*button as &NSView)
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
        top_bar.setDistribution(NSStackViewDistribution::Fill);

        top_bar.setWantsLayer(true);
        if let Some(layer) = top_bar.layer() {
            let color = CGColor::new_generic_rgb(0.13, 0.13, 0.13, 1.0);
            layer.setBackgroundColor(Some(&color));
        }

        let back_btn = self.create_top_bar_button("←", sel!(backClicked:), mtm);
        let title_label = NSTextField::labelWithString(&NSString::from_str("Add MCP"), mtm);
        title_label.setFont(Some(&NSFont::boldSystemFontOfSize(15.0)));
        title_label.setTextColor(Some(&Theme::text_primary()));

        let spacer = NSView::new(mtm);
        spacer.setTranslatesAutoresizingMaskIntoConstraints(false);
        spacer.setContentHuggingPriority_forOrientation(
            1.0,
            NSLayoutConstraintOrientation::Horizontal,
        );

        let refresh_btn = self.create_top_bar_button("⟳", sel!(refreshClicked:), mtm);

        top_bar.addArrangedSubview(&back_btn);
        top_bar.addArrangedSubview(&title_label);
        top_bar.addArrangedSubview(&spacer);
        top_bar.addArrangedSubview(&refresh_btn);

        Retained::from(&*top_bar as &NSView)
    }

    fn create_top_bar_button(
        &self,
        label: &str,
        action: objc2::runtime::Sel,
        mtm: MainThreadMarker,
    ) -> Retained<NSButton> {
        let button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str(label),
                Some(self),
                Some(action),
                mtm,
            )
        };
        button.setBezelStyle(NSBezelStyle::Automatic);
        unsafe {
            button.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width = button.widthAnchor().constraintEqualToConstant(32.0);
            width.setActive(true);
            let height = button.heightAnchor().constraintEqualToConstant(28.0);
            height.setActive(true);
        }
        button
    }

    fn build_content(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let content = NSStackView::new(mtm);
        content.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        content.setSpacing(12.0);
        content.setEdgeInsets(NSEdgeInsets {
            top: 16.0,
            left: 16.0,
            bottom: 16.0,
            right: 16.0,
        });
        content.setTranslatesAutoresizingMaskIntoConstraints(false);
        content.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);

        self.build_url_section(&content, mtm);
        self.build_registry_section(&content, mtm);
        self.build_key_section(&content, mtm);
        self.build_search_section(&content, mtm);
        self.build_results_section(&content, mtm);
        self.build_next_button(&content, mtm);

        Retained::from(&*content as &NSView)
    }

    fn build_url_section(&self, content: &NSStackView, mtm: MainThreadMarker) {
        let url_label = NSTextField::labelWithString(&NSString::from_str("URL:"), mtm);
        url_label.setTextColor(Some(&Theme::text_primary()));
        url_label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        content.addArrangedSubview(&url_label);

        let url_field = NSTextField::new(mtm);
        url_field.setPlaceholderString(Some(&NSString::from_str(
            "npx -y @modelcontextprotocol/server-filesystem",
        )));
        url_field.setTranslatesAutoresizingMaskIntoConstraints(false);
        unsafe {
            url_field.setTarget(Some(self));
            url_field.setAction(Some(sel!(urlChanged:)));
        }
        let width = url_field
            .widthAnchor()
            .constraintGreaterThanOrEqualToConstant(360.0);
        width.setActive(true);
        content.addArrangedSubview(&url_field);
        *self.ivars().url_input.borrow_mut() = Some(Retained::clone(&url_field));

        let divider_label =
            NSTextField::labelWithString(&NSString::from_str("-- or search registry --"), mtm);
        divider_label.setTextColor(Some(&Theme::text_secondary_color()));
        divider_label.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        divider_label.setAlignment(objc2_app_kit::NSTextAlignment::Center);
        unsafe {
            divider_label.setTranslatesAutoresizingMaskIntoConstraints(false);
            let div_width = divider_label.widthAnchor().constraintEqualToConstant(360.0);
            div_width.setActive(true);
        }
        content.addArrangedSubview(&divider_label);
    }

    fn build_registry_section(&self, content: &NSStackView, mtm: MainThreadMarker) {
        let registry_label = NSTextField::labelWithString(&NSString::from_str("Registry:"), mtm);
        registry_label.setTextColor(Some(&Theme::text_primary()));
        registry_label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        content.addArrangedSubview(&registry_label);

        let registry_popup = NSPopUpButton::new(mtm);
        registry_popup.addItemWithTitle(&NSString::from_str("Select..."));
        registry_popup.addItemWithTitle(&NSString::from_str("Official"));

        let has_smithery_key =
            if let Ok(config_path) = personal_agent::config::Config::default_path() {
                if let Ok(config) = personal_agent::config::Config::load(&config_path) {
                    config
                        .smithery_auth
                        .as_ref()
                        .map(|s| !s.is_empty())
                        .unwrap_or(false)
                } else {
                    false
                }
            } else {
                false
            };

        let smithery_text = if has_smithery_key {
            "Smithery"
        } else {
            "Smithery (requires API key)"
        };
        registry_popup.addItemWithTitle(&NSString::from_str(smithery_text));
        unsafe {
            registry_popup.setTarget(Some(self));
            registry_popup.setAction(Some(sel!(registryChanged:)));
            registry_popup.setTranslatesAutoresizingMaskIntoConstraints(false);
            let popup_width = registry_popup
                .widthAnchor()
                .constraintEqualToConstant(360.0);
            popup_width.setActive(true);
        }
        content.addArrangedSubview(&registry_popup);
        *self.ivars().registry_popup.borrow_mut() = Some(Retained::clone(&registry_popup));
    }

    fn build_key_section(&self, content: &NSStackView, mtm: MainThreadMarker) {
        let key_container = NSStackView::new(mtm);
        key_container.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        key_container.setSpacing(8.0);
        key_container.setTranslatesAutoresizingMaskIntoConstraints(false);
        key_container.setHidden(true);

        let key_label = NSTextField::labelWithString(&NSString::from_str("Key or File:"), mtm);
        key_label.setTextColor(Some(&Theme::text_primary()));
        key_label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        let label_width = key_label.widthAnchor().constraintEqualToConstant(90.0);
        label_width.setActive(true);
        key_container.addArrangedSubview(&key_label);

        let key_field = NSTextField::new(mtm);
        key_field.setPlaceholderString(Some(&NSString::from_str("API key or path to keyfile")));
        key_field.setTranslatesAutoresizingMaskIntoConstraints(false);
        key_container.addArrangedSubview(&key_field);
        *self.ivars().key_input_field.borrow_mut() = Some(Retained::clone(&key_field));

        let save_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Save"),
                Some(self),
                Some(sel!(saveKeyClicked:)),
                mtm,
            )
        };
        save_btn.setBezelStyle(NSBezelStyle::Automatic);
        let save_width = save_btn.widthAnchor().constraintEqualToConstant(60.0);
        save_width.setActive(true);
        key_container.addArrangedSubview(&save_btn);

        let container_width = key_container.widthAnchor().constraintEqualToConstant(360.0);
        container_width.setActive(true);

        content.addArrangedSubview(&key_container);
        *self.ivars().key_input_container.borrow_mut() =
            Some(Retained::from(&*key_container as &NSView));
    }

    fn build_search_section(&self, content: &NSStackView, mtm: MainThreadMarker) {
        let search_label = NSTextField::labelWithString(&NSString::from_str("Search:"), mtm);
        search_label.setTextColor(Some(&Theme::text_primary()));
        search_label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        content.addArrangedSubview(&search_label);

        let search_row = NSStackView::new(mtm);
        search_row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        search_row.setSpacing(8.0);
        search_row.setTranslatesAutoresizingMaskIntoConstraints(false);

        let search_field = NSTextField::new(mtm);
        search_field.setPlaceholderString(Some(&NSString::from_str("Enter search term...")));
        search_field.setTranslatesAutoresizingMaskIntoConstraints(false);
        search_field.setEnabled(false);
        unsafe {
            search_field.setTarget(Some(self));
            search_field.setAction(Some(sel!(searchFieldAction:)));
        }
        search_row.addArrangedSubview(&search_field);
        *self.ivars().search_field.borrow_mut() = Some(Retained::clone(&search_field));

        let search_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Search"),
                Some(self),
                Some(sel!(searchButtonClicked:)),
                mtm,
            )
        };
        search_btn.setBezelStyle(NSBezelStyle::Automatic);
        search_btn.setEnabled(false);
        let btn_width = search_btn.widthAnchor().constraintEqualToConstant(80.0);
        btn_width.setActive(true);
        search_row.addArrangedSubview(&search_btn);
        *self.ivars().search_button.borrow_mut() = Some(Retained::clone(&search_btn));

        let row_width = search_row.widthAnchor().constraintEqualToConstant(360.0);
        row_width.setActive(true);
        content.addArrangedSubview(&search_row);

        let helper_text =
            NSTextField::labelWithString(&NSString::from_str("(select registry first)"), mtm);
        helper_text.setTextColor(Some(&Theme::text_secondary_color()));
        helper_text.setFont(Some(&NSFont::systemFontOfSize(10.0)));
        helper_text.setHidden(false);
        content.addArrangedSubview(&helper_text);
        *self.ivars().search_helper.borrow_mut() = Some(Retained::clone(&helper_text));

        let loading_label = NSTextField::labelWithString(&NSString::from_str("Searching..."), mtm);
        loading_label.setTextColor(Some(&Theme::text_secondary_color()));
        loading_label.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        loading_label.setHidden(true);
        content.addArrangedSubview(&loading_label);
        *self.ivars().loading_label.borrow_mut() = Some(Retained::clone(&loading_label));
    }

    fn build_results_section(&self, content: &NSStackView, mtm: MainThreadMarker) {
        let results_scroll = NSScrollView::new(mtm);
        results_scroll.setHasVerticalScroller(true);
        results_scroll.setDrawsBackground(false);
        unsafe {
            results_scroll.setAutohidesScrollers(true);
            results_scroll.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        let results_stack = super::FlippedStackView::new(mtm);
        unsafe {
            results_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            results_stack.setSpacing(4.0);
            results_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
            results_stack.setDistribution(NSStackViewDistribution::Fill);
        }
        results_stack.setTranslatesAutoresizingMaskIntoConstraints(false);

        results_scroll.setDocumentView(Some(&results_stack));

        let scroll_height = results_scroll
            .heightAnchor()
            .constraintGreaterThanOrEqualToConstant(150.0);
        scroll_height.setActive(true);
        let scroll_width = results_scroll
            .widthAnchor()
            .constraintEqualToConstant(360.0);
        scroll_width.setActive(true);

        content.addArrangedSubview(&results_scroll);
        *self.ivars().results_stack.borrow_mut() =
            Some(Retained::from(&*results_stack as &NSStackView));
        *self.ivars().results_scroll.borrow_mut() = Some(Retained::clone(&results_scroll));

        let spacer = NSView::new(mtm);
        spacer
            .setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
        content.addArrangedSubview(&spacer);
    }

    fn build_next_button(&self, content: &NSStackView, mtm: MainThreadMarker) {
        let next_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Next"),
                Some(self),
                Some(sel!(nextClicked:)),
                mtm,
            )
        };
        next_btn.setBezelStyle(NSBezelStyle::Automatic);
        next_btn.setEnabled(false);
        let next_width = next_btn.widthAnchor().constraintEqualToConstant(80.0);
        next_width.setActive(true);
        content.addArrangedSubview(&next_btn);
        *self.ivars().next_button.borrow_mut() = Some(Retained::clone(&next_btn));
    }
}
