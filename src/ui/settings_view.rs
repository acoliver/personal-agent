//! Settings view for managing model profiles and MCP configuration

use std::cell::{Cell, RefCell};
use std::fs::OpenOptions;
use std::io::Write;

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly, DefinedClass};
use objc2_foundation::{
    NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use objc2_app_kit::{
    NSView, NSViewController, NSTextField, NSButton, NSScrollView, NSFont, NSBezelStyle, NSButtonType,
    NSStackView, NSUserInterfaceLayoutOrientation, NSStackViewDistribution, NSLayoutConstraintOrientation,
    NSSwitch, NSControlStateValueOn, NSControlStateValueOff,
};
use objc2_quartz_core::CALayer;
use uuid::Uuid;

use super::theme::Theme;
use personal_agent::config::Config;
use personal_agent::mcp::McpConfig;

fn log_to_file(message: &str) {
    let log_path = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/PersonalAgent/debug.log");
    
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let _ = writeln!(file, "[{timestamp}] SettingsView: {message}");
    }
}

// Thread-local storage for passing profile ID to profile editor
thread_local! {
    pub static EDITING_PROFILE_ID: Cell<Option<Uuid>> = const { Cell::new(None) };
}

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

fn set_layer_border(layer: &CALayer, width: f64, r: f64, g: f64, b: f64) {
    use objc2_core_graphics::CGColor;
    let color = CGColor::new_generic_rgb(r, g, b, 1.0);
    layer.setBorderColor(Some(&color));
    layer.setBorderWidth(width);
}

// ============================================================================
// SettingsViewController ivars
// ============================================================================

pub struct SettingsViewIvars {
    scroll_view: RefCell<Option<Retained<NSScrollView>>>,
    profiles_list: RefCell<Option<Retained<super::FlippedStackView>>>,
    profiles_toolbar: RefCell<Option<Retained<NSView>>>,
    mcps_list: RefCell<Option<Retained<super::FlippedStackView>>>,
    mcps_toolbar: RefCell<Option<Retained<NSView>>>,
    hotkey_field: RefCell<Option<Retained<NSTextField>>>,
    selected_profile_id: RefCell<Option<Uuid>>,
    selected_mcp_id: RefCell<Option<Uuid>>,
    // Store buttons for enable/disable control
    profile_delete_btn: RefCell<Option<Retained<NSButton>>>,
    profile_edit_btn: RefCell<Option<Retained<NSButton>>>,
    mcp_delete_btn: RefCell<Option<Retained<NSButton>>>,
    mcp_edit_btn: RefCell<Option<Retained<NSButton>>>,
    // Maps to track UUID to index for tags
    profile_uuid_map: RefCell<Vec<Uuid>>,
    mcp_uuid_map: RefCell<Vec<Uuid>>,
}

// ============================================================================
// SettingsViewController - settings view controller
// ============================================================================

define_class!(
    #[unsafe(super(NSViewController))]
    #[thread_kind = MainThreadOnly]
    #[name = "SettingsViewController"]
    #[ivars = SettingsViewIvars]
    pub struct SettingsViewController;

    unsafe impl NSObjectProtocol for SettingsViewController {}

    impl SettingsViewController {
        #[unsafe(method(loadView))]
        fn load_view(&self) {
            let mtm = MainThreadMarker::new().unwrap();

            // Create main container (400x500 for popover content)
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
                main_stack.setDistribution(NSStackViewDistribution::Fill);
            }
            
            // Build the UI components
            let top_bar = self.build_top_bar(mtm);
            let content_scroll = self.build_content_area(mtm);
            
            // Add to stack
            unsafe {
                main_stack.addArrangedSubview(&top_bar);
                main_stack.addArrangedSubview(&content_scroll);
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
            
            // Load data
            self.load_profiles();
            self.load_mcps();
            self.load_hotkey();
        }

        // ========================================================================
        // Action handlers
        // ========================================================================

        #[unsafe(method(backButtonClicked:))]
        fn back_button_clicked(&self, _sender: Option<&NSObject>) {
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowChatView");
            unsafe {
                center.postNotificationName_object(&name, None);
            }
        }

        #[unsafe(method(refreshButtonClicked:))]
        fn refresh_button_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("Refresh models requested");
            
            use personal_agent::registry::RegistryManager;
            
            std::thread::spawn(|| {
                let runtime = match tokio::runtime::Runtime::new() {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Failed to create runtime: {e}");
                        return;
                    }
                };
                
                runtime.block_on(async {
                    match RegistryManager::new() {
                        Ok(manager) => {
                            match manager.refresh().await {
                                Ok(_) => println!("Registry refreshed successfully"),
                                Err(e) => eprintln!("Failed to refresh registry: {e}"),
                            }
                        }
                        Err(e) => eprintln!("Failed to create registry manager: {e}"),
                    }
                });
            });
            
            self.load_profiles();
        }

        #[unsafe(method(profileRowClicked:))]
        fn profile_row_clicked(&self, sender: Option<&NSObject>) {
            if let Some(button) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                // Get profile UUID from tag
                let tag = button.tag() as usize;
                // Must drop borrow before calling select_profile (which calls load_profiles)
                let uuid = {
                    let uuid_map = self.ivars().profile_uuid_map.borrow();
                    uuid_map.get(tag).copied()
                };
                if let Some(uuid) = uuid {
                    log_to_file(&format!("Profile row clicked: {uuid}"));
                    self.select_profile(uuid);
                }
            }
        }

        #[unsafe(method(addProfileClicked:))]
        fn add_profile_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("Add profile clicked");
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowModelSelector");
            unsafe {
                center.postNotificationName_object(&name, None);
            }
        }

        #[unsafe(method(editProfileClicked:))]
        fn edit_profile_clicked(&self, _sender: Option<&NSObject>) {
            if let Some(profile_id) = *self.ivars().selected_profile_id.borrow() {
                log_to_file(&format!("Edit profile clicked: {profile_id}"));
                
                EDITING_PROFILE_ID.set(Some(profile_id));
                
                use objc2_foundation::NSNotificationCenter;
                let center = NSNotificationCenter::defaultCenter();
                let name = NSString::from_str("PersonalAgentShowProfileEditor");
                unsafe {
                    center.postNotificationName_object(&name, None);
                }
            }
        }

        #[unsafe(method(deleteProfileClicked:))]
        fn delete_profile_clicked(&self, _sender: Option<&NSObject>) {
            if let Some(profile_id) = *self.ivars().selected_profile_id.borrow() {
                log_to_file(&format!("Delete profile clicked: {profile_id}"));
                
                use objc2_app_kit::NSAlert;
                let mtm = MainThreadMarker::new().unwrap();
                
                let alert = NSAlert::new(mtm);
                alert.setMessageText(&NSString::from_str("Delete Profile?"));
                alert.setInformativeText(&NSString::from_str("This action cannot be undone."));
                alert.addButtonWithTitle(&NSString::from_str("Delete"));
                alert.addButtonWithTitle(&NSString::from_str("Cancel"));
                
                let response = unsafe { alert.runModal() };
                
                // NSAlertFirstButtonReturn = 1000
                if response == 1000 {
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
                    
                    if let Err(e) = config.remove_profile(&profile_id) {
                        eprintln!("Failed to remove profile: {e}");
                        return;
                    }
                    
                    // If deleted profile was default, set new default
                    if config.default_profile == Some(profile_id) {
                        config.default_profile = config.profiles.first().map(|p| p.id);
                    }
                    
                    if let Err(e) = config.save(&config_path) {
                        eprintln!("Failed to save config: {e}");
                        return;
                    }
                    
                    log_to_file("Profile deleted successfully");
                    self.load_profiles();
                }
            }
        }

        #[unsafe(method(mcpRowClicked:))]
        fn mcp_row_clicked(&self, sender: Option<&NSObject>) {
            if let Some(button) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                // Get MCP UUID from tag
                let tag = button.tag() as usize;
                let uuid_map = self.ivars().mcp_uuid_map.borrow();
                if let Some(&uuid) = uuid_map.get(tag) {
                    log_to_file(&format!("MCP row clicked: {uuid}"));
                    self.select_mcp(uuid);
                }
            }
        }

        #[unsafe(method(mcpToggled:))]
        fn mcp_toggled(&self, sender: Option<&NSObject>) {
            if let Some(switch) = sender.and_then(|s| s.downcast_ref::<NSSwitch>()) {
                // Get MCP UUID from tag
                let tag = switch.tag() as usize;
                let uuid_map = self.ivars().mcp_uuid_map.borrow();
                if let Some(&uuid) = uuid_map.get(tag) {
                    let is_on = switch.state() == NSControlStateValueOn;
                    log_to_file(&format!("MCP toggled: {uuid}, enabled: {is_on}"));
                    
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
                    
                    // Find and update the MCP
                    if let Some(mcp) = config.mcps.iter_mut().find(|m| m.id == uuid) {
                        mcp.enabled = is_on;
                        
                        if let Err(e) = config.save(&config_path) {
                            eprintln!("Failed to save config: {e}");
                        }
                    }
                }
            }
        }

        #[unsafe(method(addMcpClicked:))]
        fn add_mcp_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("Add MCP clicked");
            
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowAddMcp");
            unsafe {
                center.postNotificationName_object(&name, None);
            }
        }

        #[unsafe(method(editMcpClicked:))]
        fn edit_mcp_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("Edit MCP clicked");
            
            // TODO: Implement edit flow - similar to add but pre-populate with existing MCP data
            use objc2_app_kit::NSAlert;
            let mtm = MainThreadMarker::new().unwrap();
            
            let alert = NSAlert::new(mtm);
            alert.setMessageText(&NSString::from_str("Edit MCP"));
            alert.setInformativeText(&NSString::from_str("Edit MCP not yet implemented."));
            alert.addButtonWithTitle(&NSString::from_str("OK"));
            unsafe { alert.runModal() };
        }

        #[unsafe(method(deleteMcpClicked:))]
        fn delete_mcp_clicked(&self, _sender: Option<&NSObject>) {
            if let Some(mcp_id) = *self.ivars().selected_mcp_id.borrow() {
                log_to_file(&format!("Delete MCP clicked: {mcp_id}"));
                
                use objc2_app_kit::NSAlert;
                let mtm = MainThreadMarker::new().unwrap();
                
                let alert = NSAlert::new(mtm);
                alert.setMessageText(&NSString::from_str("Delete MCP?"));
                alert.setInformativeText(&NSString::from_str("This action cannot be undone."));
                alert.addButtonWithTitle(&NSString::from_str("Delete"));
                alert.addButtonWithTitle(&NSString::from_str("Cancel"));
                
                let response = unsafe { alert.runModal() };
                
                if response == 1000 {
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
                    
                    if let Err(e) = config.remove_mcp(&mcp_id) {
                        eprintln!("Failed to remove MCP: {e}");
                        return;
                    }
                    
                    if let Err(e) = config.save(&config_path) {
                        eprintln!("Failed to save config: {e}");
                        return;
                    }
                    
                    log_to_file("MCP deleted successfully");
                    self.load_mcps();
                }
            }
        }

        #[unsafe(method(hotkeyChanged:))]
        fn hotkey_changed(&self, sender: Option<&NSObject>) {
            if let Some(field) = sender.and_then(|s| s.downcast_ref::<NSTextField>()) {
                let text = field.stringValue().to_string();
                log_to_file(&format!("Hotkey changed: {text}"));
                
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
                
                config.global_hotkey = text;
                
                if let Err(e) = config.save(&config_path) {
                    eprintln!("Failed to save config: {e}");
                }
            }
        }
    }
);

impl SettingsViewController {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let ivars = SettingsViewIvars {
            scroll_view: RefCell::new(None),
            profiles_list: RefCell::new(None),
            profiles_toolbar: RefCell::new(None),
            mcps_list: RefCell::new(None),
            mcps_toolbar: RefCell::new(None),
            hotkey_field: RefCell::new(None),
            selected_profile_id: RefCell::new(None),
            selected_mcp_id: RefCell::new(None),
            profile_delete_btn: RefCell::new(None),
            profile_edit_btn: RefCell::new(None),
            mcp_delete_btn: RefCell::new(None),
            mcp_edit_btn: RefCell::new(None),
            profile_uuid_map: RefCell::new(Vec::new()),
            mcp_uuid_map: RefCell::new(Vec::new()),
        };
        
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }
    
    /// Reload profiles from config - called when returning from profile editor
    pub fn reload_profiles(&self) {
        self.load_profiles();
    }

    // ========================================================================
    // UI building methods
    // ========================================================================

    fn build_top_bar(&self, mtm: MainThreadMarker) -> Retained<NSView> {
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
            top_bar.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            let height_constraint = top_bar.heightAnchor().constraintEqualToConstant(44.0);
            height_constraint.setActive(true);
        }

        // Back button
        let back_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("<"),
                Some(self),
                Some(sel!(backButtonClicked:)),
                mtm,
            )
        };
        back_btn.setBezelStyle(NSBezelStyle::Rounded);
        unsafe {
            back_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = back_btn.widthAnchor().constraintEqualToConstant(40.0);
            width_constraint.setActive(true);
        }
        unsafe {
            top_bar.addArrangedSubview(&back_btn);
        }

        // Title
        let title = NSTextField::labelWithString(&NSString::from_str("Settings"), mtm);
        title.setTextColor(Some(&Theme::text_primary()));
        title.setFont(Some(&NSFont::boldSystemFontOfSize(14.0)));
        unsafe {
            top_bar.addArrangedSubview(&title);
        }
        
        // Spacer
        let spacer = NSView::new(mtm);
        unsafe {
            spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
            top_bar.addArrangedSubview(&spacer);
        }
        
        // Refresh Models button
        let refresh_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Refresh Models"),
                Some(self),
                Some(sel!(refreshButtonClicked:)),
                mtm,
            )
        };
        refresh_btn.setBezelStyle(NSBezelStyle::Rounded);
        unsafe {
            refresh_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = refresh_btn.widthAnchor().constraintEqualToConstant(120.0);
            width_constraint.setActive(true);
        }
        unsafe {
            top_bar.addArrangedSubview(&refresh_btn);
        }

        Retained::from(&*top_bar as &NSView)
    }

    fn build_content_area(&self, mtm: MainThreadMarker) -> Retained<NSScrollView> {
        let scroll_view = NSScrollView::new(mtm);
        scroll_view.setHasVerticalScroller(true);
        scroll_view.setDrawsBackground(false);
        unsafe {
            scroll_view.setAutohidesScrollers(true);
            scroll_view.setTranslatesAutoresizingMaskIntoConstraints(false);
            scroll_view.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
        }

        // Create vertical stack for content - use FlippedStackView so content starts at TOP
        let content_stack = super::FlippedStackView::new(mtm);
        unsafe {
            content_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            content_stack.setSpacing(8.0);
            content_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Width);
            content_stack.setDistribution(NSStackViewDistribution::Fill);
            content_stack.setEdgeInsets(objc2_foundation::NSEdgeInsets {
                top: 4.0,
                left: 8.0,
                bottom: 8.0,
                right: 8.0,
            });
        }
        
        content_stack.setWantsLayer(true);
        if let Some(layer) = content_stack.layer() {
            set_layer_background_color(&layer, Theme::BG_DARKEST.0, Theme::BG_DARKEST.1, Theme::BG_DARKEST.2);
        }
        
        content_stack.setTranslatesAutoresizingMaskIntoConstraints(false);

        // Build sections
        let profiles_section = self.build_profiles_section(mtm);
        let separator1 = self.build_separator(mtm);
        let mcps_section = self.build_mcps_section(mtm);
        let separator2 = self.build_separator(mtm);
        let hotkey_section = self.build_hotkey_section(mtm);
        
        unsafe {
            content_stack.addArrangedSubview(&profiles_section);
            content_stack.addArrangedSubview(&separator1);
            content_stack.addArrangedSubview(&mcps_section);
            content_stack.addArrangedSubview(&separator2);
            content_stack.addArrangedSubview(&hotkey_section);
        }

        scroll_view.setDocumentView(Some(&content_stack));
        
        let content_view = scroll_view.contentView();
        let width_constraint = content_stack.widthAnchor().constraintEqualToAnchor(&content_view.widthAnchor());
        width_constraint.setActive(true);
        
        // Force sections to match content_stack width
        unsafe {
            let profiles_width = profiles_section.widthAnchor().constraintEqualToAnchor_constant(&content_stack.widthAnchor(), -16.0);
            profiles_width.setActive(true);
            let mcps_width = mcps_section.widthAnchor().constraintEqualToAnchor_constant(&content_stack.widthAnchor(), -16.0);
            mcps_width.setActive(true);
        }

        *self.ivars().scroll_view.borrow_mut() = Some(scroll_view.clone());

        scroll_view
    }

    fn build_profiles_section(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        // Use plain NSView with manual constraints for precise control
        let section = NSView::new(mtm);
        section.setTranslatesAutoresizingMaskIntoConstraints(false);
        
        // Section label
        let label = NSTextField::labelWithString(&NSString::from_str("Profiles"), mtm);
        label.setFont(Some(&NSFont::boldSystemFontOfSize(12.0)));
        label.setTextColor(Some(&Theme::text_primary()));
        label.setTranslatesAutoresizingMaskIntoConstraints(false);
        section.addSubview(&label);
        
        // List box container
        let (list_container, list_stack, toolbar) = self.build_list_box(120.0, mtm);
        section.addSubview(&list_container);
        
        // Constraints
        unsafe {
            // Label at top, left aligned
            let label_top = label.topAnchor().constraintEqualToAnchor(&section.topAnchor());
            label_top.setActive(true);
            let label_left = label.leadingAnchor().constraintEqualToAnchor(&section.leadingAnchor());
            label_left.setActive(true);
            
            // List container below label, full width
            let lc_top = list_container.topAnchor().constraintEqualToAnchor_constant(&label.bottomAnchor(), 4.0);
            lc_top.setActive(true);
            let lc_left = list_container.leadingAnchor().constraintEqualToAnchor(&section.leadingAnchor());
            lc_left.setActive(true);
            let lc_right = list_container.trailingAnchor().constraintEqualToAnchor(&section.trailingAnchor());
            lc_right.setActive(true);
            let lc_bottom = list_container.bottomAnchor().constraintEqualToAnchor(&section.bottomAnchor());
            lc_bottom.setActive(true);
        }
        
        // Store references
        *self.ivars().profiles_list.borrow_mut() = Some(list_stack);
        *self.ivars().profiles_toolbar.borrow_mut() = Some(toolbar.clone());
        
        // Add toolbar buttons
        self.setup_profiles_toolbar(&toolbar, mtm);
        
        section
    }

    fn build_mcps_section(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        // Use plain NSView with manual constraints for precise control
        let section = NSView::new(mtm);
        section.setTranslatesAutoresizingMaskIntoConstraints(false);
        
        // Section label
        let label = NSTextField::labelWithString(&NSString::from_str("MCPs"), mtm);
        label.setFont(Some(&NSFont::boldSystemFontOfSize(12.0)));
        label.setTextColor(Some(&Theme::text_primary()));
        label.setTranslatesAutoresizingMaskIntoConstraints(false);
        section.addSubview(&label);
        
        // List box container - same height as Profiles
        let (list_container, list_stack, toolbar) = self.build_list_box(120.0, mtm);
        section.addSubview(&list_container);
        
        // Constraints
        unsafe {
            // Label at top, left aligned
            let label_top = label.topAnchor().constraintEqualToAnchor(&section.topAnchor());
            label_top.setActive(true);
            let label_left = label.leadingAnchor().constraintEqualToAnchor(&section.leadingAnchor());
            label_left.setActive(true);
            
            // List container below label, full width
            let lc_top = list_container.topAnchor().constraintEqualToAnchor_constant(&label.bottomAnchor(), 4.0);
            lc_top.setActive(true);
            let lc_left = list_container.leadingAnchor().constraintEqualToAnchor(&section.leadingAnchor());
            lc_left.setActive(true);
            let lc_right = list_container.trailingAnchor().constraintEqualToAnchor(&section.trailingAnchor());
            lc_right.setActive(true);
            let lc_bottom = list_container.bottomAnchor().constraintEqualToAnchor(&section.bottomAnchor());
            lc_bottom.setActive(true);
        }
        
        // Store references
        *self.ivars().mcps_list.borrow_mut() = Some(list_stack);
        *self.ivars().mcps_toolbar.borrow_mut() = Some(toolbar.clone());
        
        // Add toolbar buttons
        self.setup_mcps_toolbar(&toolbar, mtm);
        
        section
    }

    fn build_hotkey_section(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let section = NSStackView::new(mtm);
        unsafe {
            section.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            section.setSpacing(4.0);
            section.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        
        // Label
        let label = NSTextField::labelWithString(&NSString::from_str("Global Hotkey:"), mtm);
        label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        label.setTextColor(Some(&Theme::text_primary()));
        unsafe {
            label.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
            section.addArrangedSubview(&label);
        }
        
        // Text field
        let field = NSTextField::new(mtm);
        field.setBackgroundColor(Some(&Theme::bg_darker()));
        field.setTextColor(Some(&Theme::text_primary()));
        field.setDrawsBackground(true);
        field.setBordered(true);
        field.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        unsafe {
            field.setTarget(Some(self));
            field.setAction(Some(sel!(hotkeyChanged:)));
            field.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
            section.addArrangedSubview(&field);
        }
        
        *self.ivars().hotkey_field.borrow_mut() = Some(field);
        
        Retained::from(&*section as &NSView)
    }

    fn build_list_box(&self, height: f64, mtm: MainThreadMarker) -> (Retained<NSView>, Retained<super::FlippedStackView>, Retained<NSView>) {
        // Container with border - use NSView not NSStackView so we can control layout more precisely
        let container = NSView::new(mtm);
        container.setTranslatesAutoresizingMaskIntoConstraints(false);
        
        container.setWantsLayer(true);
        if let Some(layer) = container.layer() {
            set_layer_background_color(&layer, Theme::BG_DARKER.0, Theme::BG_DARKER.1, Theme::BG_DARKER.2);
            set_layer_corner_radius(&layer, 4.0);
            set_layer_border(&layer, 1.0, 0.3, 0.3, 0.3);
        }
        
        // Inner scroll view for list items
        let scroll_view = NSScrollView::new(mtm);
        scroll_view.setHasVerticalScroller(true);
        scroll_view.setDrawsBackground(false);
        scroll_view.setTranslatesAutoresizingMaskIntoConstraints(false);
        unsafe {
            scroll_view.setAutohidesScrollers(true);
        }
        
        // List stack - use FlippedStackView so items start at TOP (not bottom)
        let list_stack = super::FlippedStackView::new(mtm);
        unsafe {
            list_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            list_stack.setSpacing(1.0);
            list_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Width);
            list_stack.setDistribution(NSStackViewDistribution::Fill);
            list_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        
        scroll_view.setDocumentView(Some(&list_stack));
        
        // Toolbar
        let toolbar = NSStackView::new(mtm);
        unsafe {
            toolbar.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            toolbar.setSpacing(4.0);
            toolbar.setTranslatesAutoresizingMaskIntoConstraints(false);
            toolbar.setEdgeInsets(objc2_foundation::NSEdgeInsets {
                top: 4.0,
                left: 4.0,
                bottom: 4.0,
                right: 4.0,
            });
        }
        
        toolbar.setWantsLayer(true);
        if let Some(layer) = toolbar.layer() {
            set_layer_background_color(&layer, Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2);
        }
        
        // Add subviews to container
        container.addSubview(&scroll_view);
        container.addSubview(&toolbar);
        
        // Set up constraints manually for precise control
        unsafe {
            // Container height = scroll_view height + toolbar height
            let container_height = container.heightAnchor().constraintEqualToConstant(height + 28.0);
            container_height.setActive(true);
            
            // Scroll view: top, left, right of container, fixed height
            let sv_top = scroll_view.topAnchor().constraintEqualToAnchor(&container.topAnchor());
            sv_top.setActive(true);
            let sv_left = scroll_view.leadingAnchor().constraintEqualToAnchor(&container.leadingAnchor());
            sv_left.setActive(true);
            let sv_right = scroll_view.trailingAnchor().constraintEqualToAnchor(&container.trailingAnchor());
            sv_right.setActive(true);
            let sv_height = scroll_view.heightAnchor().constraintEqualToConstant(height);
            sv_height.setActive(true);
            
            // Toolbar: bottom, left, right of container, fixed height
            let tb_bottom = toolbar.bottomAnchor().constraintEqualToAnchor(&container.bottomAnchor());
            tb_bottom.setActive(true);
            let tb_left = toolbar.leadingAnchor().constraintEqualToAnchor(&container.leadingAnchor());
            tb_left.setActive(true);
            let tb_right = toolbar.trailingAnchor().constraintEqualToAnchor(&container.trailingAnchor());
            tb_right.setActive(true);
            let tb_height = toolbar.heightAnchor().constraintEqualToConstant(28.0);
            tb_height.setActive(true);
            
            // List stack width = scroll view content width
            let content_view = scroll_view.contentView();
            let ls_width = list_stack.widthAnchor().constraintEqualToAnchor(&content_view.widthAnchor());
            ls_width.setActive(true);
        }
        
        (container, list_stack, Retained::from(&*toolbar as &NSView))
    }

    fn build_separator(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let separator = NSView::new(mtm);
        separator.setWantsLayer(true);
        if let Some(layer) = separator.layer() {
            set_layer_background_color(&layer, 0.3, 0.3, 0.3);
        }
        unsafe {
            separator.setTranslatesAutoresizingMaskIntoConstraints(false);
            let height_constraint = separator.heightAnchor().constraintEqualToConstant(1.0);
            height_constraint.setActive(true);
        }
        separator
    }

    fn setup_profiles_toolbar(&self, toolbar: &NSView, mtm: MainThreadMarker) {
        if let Some(toolbar_stack) = toolbar.downcast_ref::<NSStackView>() {
            // Delete button
            let delete_btn = unsafe {
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str("−"),
                    Some(self),
                    Some(sel!(deleteProfileClicked:)),
                    mtm,
                )
            };
            delete_btn.setBezelStyle(NSBezelStyle::Inline);
            delete_btn.setEnabled(false);
            unsafe {
                delete_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
                let width_constraint = delete_btn.widthAnchor().constraintEqualToConstant(24.0);
                width_constraint.setActive(true);
            }
            
            // Add button
            let add_btn = unsafe {
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str("+"),
                    Some(self),
                    Some(sel!(addProfileClicked:)),
                    mtm,
                )
            };
            add_btn.setBezelStyle(NSBezelStyle::Inline);
            unsafe {
                add_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
                let width_constraint = add_btn.widthAnchor().constraintEqualToConstant(24.0);
                width_constraint.setActive(true);
            }
            
            // Spacer
            let spacer = NSView::new(mtm);
            unsafe {
                spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
            }
            
            // Edit button
            let edit_btn = unsafe {
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str("Edit"),
                    Some(self),
                    Some(sel!(editProfileClicked:)),
                    mtm,
                )
            };
            edit_btn.setBezelStyle(NSBezelStyle::Inline);
            edit_btn.setEnabled(false);
            unsafe {
                edit_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
                let width_constraint = edit_btn.widthAnchor().constraintEqualToConstant(40.0);
                width_constraint.setActive(true);
            }
            
            unsafe {
                toolbar_stack.addArrangedSubview(&delete_btn);
                toolbar_stack.addArrangedSubview(&add_btn);
                toolbar_stack.addArrangedSubview(&spacer);
                toolbar_stack.addArrangedSubview(&edit_btn);
            }
            
            *self.ivars().profile_delete_btn.borrow_mut() = Some(delete_btn);
            *self.ivars().profile_edit_btn.borrow_mut() = Some(edit_btn);
        }
    }

    fn setup_mcps_toolbar(&self, toolbar: &NSView, mtm: MainThreadMarker) {
        if let Some(toolbar_stack) = toolbar.downcast_ref::<NSStackView>() {
            // Delete button
            let delete_btn = unsafe {
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str("−"),
                    Some(self),
                    Some(sel!(deleteMcpClicked:)),
                    mtm,
                )
            };
            delete_btn.setBezelStyle(NSBezelStyle::Inline);
            delete_btn.setEnabled(false);
            unsafe {
                delete_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
                let width_constraint = delete_btn.widthAnchor().constraintEqualToConstant(24.0);
                width_constraint.setActive(true);
            }
            
            // Add button
            let add_btn = unsafe {
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str("+"),
                    Some(self),
                    Some(sel!(addMcpClicked:)),
                    mtm,
                )
            };
            add_btn.setBezelStyle(NSBezelStyle::Inline);
            unsafe {
                add_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
                let width_constraint = add_btn.widthAnchor().constraintEqualToConstant(24.0);
                width_constraint.setActive(true);
            }
            
            // Spacer
            let spacer = NSView::new(mtm);
            unsafe {
                spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
            }
            
            // Edit button
            let edit_btn = unsafe {
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str("Edit"),
                    Some(self),
                    Some(sel!(editMcpClicked:)),
                    mtm,
                )
            };
            edit_btn.setBezelStyle(NSBezelStyle::Inline);
            edit_btn.setEnabled(false);
            unsafe {
                edit_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
                let width_constraint = edit_btn.widthAnchor().constraintEqualToConstant(40.0);
                width_constraint.setActive(true);
            }
            
            unsafe {
                toolbar_stack.addArrangedSubview(&delete_btn);
                toolbar_stack.addArrangedSubview(&add_btn);
                toolbar_stack.addArrangedSubview(&spacer);
                toolbar_stack.addArrangedSubview(&edit_btn);
            }
            
            *self.ivars().mcp_delete_btn.borrow_mut() = Some(delete_btn);
            *self.ivars().mcp_edit_btn.borrow_mut() = Some(edit_btn);
        }
    }

    // ========================================================================
    // Data loading and management
    // ========================================================================

    fn load_profiles(&self) {
        let mtm = MainThreadMarker::new().unwrap();
        
        log_to_file("load_profiles called");
        
        let config = Config::load(Config::default_path().unwrap()).unwrap_or_default();
        log_to_file(&format!("Config has {} profiles", config.profiles.len()));
        
        // Clear UUID map
        self.ivars().profile_uuid_map.borrow_mut().clear();
        
        if let Some(list_stack) = &*self.ivars().profiles_list.borrow() {
            // Clear existing rows
            let subviews = list_stack.subviews();
            for view in &subviews {
                unsafe {
                    list_stack.removeArrangedSubview(&view);
                }
                view.removeFromSuperview();
            }
            
            if config.profiles.is_empty() {
                // Show empty state
                let message = NSTextField::labelWithString(
                    &NSString::from_str("No profiles yet. Click + to add one."),
                    mtm,
                );
                message.setTextColor(Some(&Theme::text_secondary_color()));
                message.setFont(Some(&NSFont::systemFontOfSize(12.0)));
                message.setAlignment(objc2_app_kit::NSTextAlignment::Center);
                unsafe {
                    list_stack.addArrangedSubview(&message);
                }
            } else {
                // Add profile rows
                for (index, profile) in config.profiles.iter().enumerate() {
                    let is_selected = Some(profile.id) == config.default_profile;
                    let row = self.create_profile_row(profile, is_selected, index, mtm);
                    unsafe {
                        list_stack.addArrangedSubview(&row);
                    }
                    
                    // Store UUID in map
                    self.ivars().profile_uuid_map.borrow_mut().push(profile.id);
                    
                    // Set selected profile
                    if is_selected {
                        *self.ivars().selected_profile_id.borrow_mut() = Some(profile.id);
                    }
                }
            }
        }
        
        // Update button states
        self.update_profile_button_states();
    }

    fn load_mcps(&self) {
        let mtm = MainThreadMarker::new().unwrap();
        
        log_to_file("load_mcps called");
        
        let config = Config::load(Config::default_path().unwrap()).unwrap_or_default();
        log_to_file(&format!("Config has {} MCPs", config.mcps.len()));
        
        // Clear UUID map
        self.ivars().mcp_uuid_map.borrow_mut().clear();
        
        if let Some(list_stack) = &*self.ivars().mcps_list.borrow() {
            // Clear existing rows
            let subviews = list_stack.subviews();
            for view in &subviews {
                unsafe {
                    list_stack.removeArrangedSubview(&view);
                }
                view.removeFromSuperview();
            }
            
            if config.mcps.is_empty() {
                // Show empty state
                let message = NSTextField::labelWithString(
                    &NSString::from_str("No MCPs configured."),
                    mtm,
                );
                message.setTextColor(Some(&Theme::text_secondary_color()));
                message.setFont(Some(&NSFont::systemFontOfSize(12.0)));
                message.setAlignment(objc2_app_kit::NSTextAlignment::Center);
                unsafe {
                    list_stack.addArrangedSubview(&message);
                }
            } else {
                // Add MCP rows
                for (index, mcp) in config.mcps.iter().enumerate() {
                    let row = self.create_mcp_row(mcp, index, mtm);
                    unsafe {
                        list_stack.addArrangedSubview(&row);
                    }
                    
                    // Store UUID in map
                    self.ivars().mcp_uuid_map.borrow_mut().push(mcp.id);
                }
            }
        }
        
        // Update button states
        self.update_mcp_button_states();
    }

    fn load_hotkey(&self) {
        let config = Config::load(Config::default_path().unwrap()).unwrap_or_default();
        
        if let Some(field) = &*self.ivars().hotkey_field.borrow() {
            field.setStringValue(&NSString::from_str(&config.global_hotkey));
        }
    }

    fn create_profile_row(
        &self,
        profile: &personal_agent::models::ModelProfile,
        is_selected: bool,
        index: usize,
        mtm: MainThreadMarker,
    ) -> Retained<NSView> {
        // Row as clickable button
        let row_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str(""),
                Some(self),
                Some(sel!(profileRowClicked:)),
                mtm,
            )
        };
        row_btn.setBezelStyle(NSBezelStyle::Inline);
        row_btn.setBordered(false);
        row_btn.setTag(index as isize);
        
        // Create row content stack
        let row = NSStackView::new(mtm);
        unsafe {
            row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            row.setSpacing(8.0);
            row.setTranslatesAutoresizingMaskIntoConstraints(false);
            row.setEdgeInsets(objc2_foundation::NSEdgeInsets {
                top: 4.0,
                left: 8.0,
                bottom: 4.0,
                right: 8.0,
            });
        }
        
        row.setWantsLayer(true);
        if let Some(layer) = row.layer() {
            if is_selected {
                // Highlight selected row
                set_layer_background_color(&layer, 0.2, 0.4, 0.6);
            } else {
                set_layer_background_color(&layer, Theme::BG_DARKER.0, Theme::BG_DARKER.1, Theme::BG_DARKER.2);
            }
        }
        
        unsafe {
            let height_constraint = row.heightAnchor().constraintEqualToConstant(24.0);
            height_constraint.setActive(true);
        }
        
        // Indicator
        let indicator = if is_selected { "▶ " } else { "  " };
        let text = format!("{}{} ({}:{})", indicator, profile.name, profile.provider_id, profile.model_id);
        
        let label = NSTextField::labelWithString(&NSString::from_str(&text), mtm);
        label.setTextColor(Some(&Theme::text_primary()));
        label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        unsafe {
            label.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
            row.addArrangedSubview(&label);
        }
        
        // Add row to button
        row_btn.addSubview(&row);
        
        // Constrain row to fill button
        unsafe {
            let leading = row.leadingAnchor().constraintEqualToAnchor(&row_btn.leadingAnchor());
            leading.setActive(true);
            let trailing = row.trailingAnchor().constraintEqualToAnchor(&row_btn.trailingAnchor());
            trailing.setActive(true);
            let top = row.topAnchor().constraintEqualToAnchor(&row_btn.topAnchor());
            top.setActive(true);
            let bottom = row.bottomAnchor().constraintEqualToAnchor(&row_btn.bottomAnchor());
            bottom.setActive(true);
        }
        
        Retained::from(&*row_btn as &NSView)
    }

    fn create_mcp_row(
        &self,
        mcp: &McpConfig,
        index: usize,
        mtm: MainThreadMarker,
    ) -> Retained<NSView> {
        // Use a button as the row container for click handling
        let row_btn = NSButton::initWithFrame(
            NSButton::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(360.0, 32.0)),
        );
        row_btn.setButtonType(NSButtonType::MomentaryLight);
        row_btn.setBezelStyle(NSBezelStyle::SmallSquare);
        row_btn.setBordered(false);
        row_btn.setTitle(&NSString::from_str(""));
        row_btn.setTag(index as isize);
        
        unsafe {
            row_btn.setTarget(Some(self));
            row_btn.setAction(Some(sel!(mcpRowClicked:)));
            row_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        
        row_btn.setWantsLayer(true);
        if let Some(layer) = row_btn.layer() {
            set_layer_background_color(&layer, Theme::BG_DARKER.0, Theme::BG_DARKER.1, Theme::BG_DARKER.2);
        }
        
        // Row content container
        let container = NSStackView::new(mtm);
        container.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        container.setSpacing(8.0);
        container.setTranslatesAutoresizingMaskIntoConstraints(false);
        container.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: 4.0,
            left: 8.0,
            bottom: 4.0,
            right: 8.0,
        });
        
        // Status indicator (colored dot)
        let status_view = NSView::new(mtm);
        status_view.setWantsLayer(true);
        unsafe {
            status_view.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width = status_view.widthAnchor().constraintEqualToConstant(8.0);
            width.setActive(true);
            let height = status_view.heightAnchor().constraintEqualToConstant(8.0);
            height.setActive(true);
        }
        
        if let Some(layer) = status_view.layer() {
            // For now, show green if enabled, gray if disabled
            // TODO: Connect to McpRuntime to get actual status
            let (r, g, b) = if mcp.enabled {
                (0.0, 0.8, 0.0) // Green
            } else {
                (0.5, 0.5, 0.5) // Gray
            };
            set_layer_background_color(&layer, r, g, b);
            set_layer_corner_radius(&layer, 4.0);
        }
        
        unsafe {
            container.addArrangedSubview(&status_view);
        }
        
        // Label
        // Show MCP name and source type
        let source_type = match &mcp.source {
            personal_agent::mcp::McpSource::Official { name, version } => {
                format!("Official: {} v{}", name, version)
            }
            personal_agent::mcp::McpSource::Smithery { qualified_name } => {
                format!("Smithery: {}", qualified_name)
            }
            personal_agent::mcp::McpSource::Manual { url } => {
                format!("Manual: {}", url)
            }
        };
        let text = format!("{} - {}", mcp.name, source_type);
        let label = NSTextField::labelWithString(&NSString::from_str(&text), mtm);
        label.setTextColor(Some(&Theme::text_primary()));
        label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        unsafe {
            label.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
            container.addArrangedSubview(&label);
        }
        
        // Toggle switch
        let toggle = NSSwitch::new(mtm);
        toggle.setState(if mcp.enabled { NSControlStateValueOn } else { NSControlStateValueOff });
        toggle.setTag(index as isize);
        unsafe {
            toggle.setTarget(Some(self));
            toggle.setAction(Some(sel!(mcpToggled:)));
            toggle.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
            container.addArrangedSubview(&toggle);
        }
        
        // Add container to button
        row_btn.addSubview(&container);
        
        // Constrain container to fill button
        let leading = container.leadingAnchor().constraintEqualToAnchor(&row_btn.leadingAnchor());
        let trailing = container.trailingAnchor().constraintEqualToAnchor(&row_btn.trailingAnchor());
        let top = container.topAnchor().constraintEqualToAnchor(&row_btn.topAnchor());
        let bottom = container.bottomAnchor().constraintEqualToAnchor(&row_btn.bottomAnchor());
        leading.setActive(true);
        trailing.setActive(true);
        top.setActive(true);
        bottom.setActive(true);
        
        // Button size
        let height = row_btn.heightAnchor().constraintEqualToConstant(32.0);
        height.setActive(true);
        
        Retained::from(&*row_btn as &NSView)
    }

    fn select_profile(&self, profile_id: Uuid) {
        log_to_file(&format!("Selecting profile: {profile_id}"));
        
        // Update config
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
        
        config.default_profile = Some(profile_id);
        
        if let Err(e) = config.save(&config_path) {
            eprintln!("Failed to save config: {e}");
            return;
        }
        
        // Update UI
        *self.ivars().selected_profile_id.borrow_mut() = Some(profile_id);
        self.load_profiles();
    }

    fn select_mcp(&self, mcp_id: Uuid) {
        log_to_file(&format!("Selecting MCP: {mcp_id}"));
        *self.ivars().selected_mcp_id.borrow_mut() = Some(mcp_id);
        self.update_mcp_button_states();
    }

    fn update_profile_button_states(&self) {
        let has_selection = self.ivars().selected_profile_id.borrow().is_some();
        
        if let Some(btn) = &*self.ivars().profile_delete_btn.borrow() {
            btn.setEnabled(has_selection);
        }
        if let Some(btn) = &*self.ivars().profile_edit_btn.borrow() {
            btn.setEnabled(has_selection);
        }
    }

    fn update_mcp_button_states(&self) {
        let has_selection = self.ivars().selected_mcp_id.borrow().is_some();
        
        if let Some(btn) = &*self.ivars().mcp_delete_btn.borrow() {
            btn.setEnabled(has_selection);
        }
        if let Some(btn) = &*self.ivars().mcp_edit_btn.borrow() {
            btn.setEnabled(has_selection);
        }
    }
}
