//! Settings view for managing model profiles and MCP configuration
#![allow(unsafe_code)]
#![allow(unused_unsafe)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::unused_self)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::branches_sharing_code)]
#![allow(clippy::if_not_else)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::map_unwrap_or)]

use std::cell::{Cell, RefCell};
use std::fs::OpenOptions;
use std::io::Write;

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSControlStateValueOn, NSFont, NSLayoutConstraintOrientation,
    NSScrollView, NSStackView, NSStackViewDistribution, NSSwitch, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView, NSViewController,
};

use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use super::settings_view_dispatch::{
    build_content_area, build_mcp_rows, build_profile_rows, sync_mcp_selection,
    sync_profile_selection,
};
use super::settings_view_helpers::{create_toolbar_button, create_toolbar_spacer};

use objc2_quartz_core::CALayer;
use uuid::Uuid;

use super::theme::Theme;
use personal_agent::config::Config;

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

pub fn set_layer_background_color(layer: &CALayer, r: f64, g: f64, b: f64) {
    use objc2_core_graphics::CGColor;
    let color = CGColor::new_generic_rgb(r, g, b, 1.0);
    layer.setBackgroundColor(Some(&color));
}

pub fn set_layer_corner_radius(layer: &CALayer, radius: f64) {
    layer.setCornerRadius(radius);
}

pub fn set_layer_border(layer: &CALayer, width: f64, r: f64, g: f64, b: f64) {
    use objc2_core_graphics::CGColor;
    let color = CGColor::new_generic_rgb(r, g, b, 1.0);
    layer.setBorderColor(Some(&color));
    layer.setBorderWidth(width);
}

// ============================================================================
// SettingsViewController ivars
// ============================================================================

pub struct SettingsViewIvars {
    pub(super) scroll_view: RefCell<Option<Retained<NSScrollView>>>,
    pub(super) profiles_list: RefCell<Option<Retained<super::FlippedStackView>>>,
    pub(super) profiles_toolbar: RefCell<Option<Retained<NSView>>>,
    pub(super) mcps_list: RefCell<Option<Retained<super::FlippedStackView>>>,
    pub(super) mcps_toolbar: RefCell<Option<Retained<NSView>>>,
    pub(super) hotkey_field: RefCell<Option<Retained<NSTextField>>>,
    pub(super) selected_profile_id: RefCell<Option<Uuid>>,
    pub(super) selected_mcp_id: RefCell<Option<Uuid>>,
    // Store buttons for enable/disable control
    pub(super) profile_delete_btn: RefCell<Option<Retained<NSButton>>>,
    pub(super) profile_edit_btn: RefCell<Option<Retained<NSButton>>>,
    pub(super) mcp_delete_btn: RefCell<Option<Retained<NSButton>>>,
    pub(super) mcp_edit_btn: RefCell<Option<Retained<NSButton>>>,
    // Maps to track UUID to index for tags
    pub(super) profile_uuid_map: RefCell<Vec<Uuid>>,
    pub(super) mcp_uuid_map: RefCell<Vec<Uuid>>,
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
            let content_scroll = build_content_area(self, mtm);

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
                    self.apply_selected_profile(uuid);
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
            let selected_mcp_id = {
                let selected = self.ivars().selected_mcp_id.borrow();
                *selected
            };

            if let Some(mcp_id) = selected_mcp_id {
                log_to_file(&format!("Delete MCP clicked: {mcp_id}"));

                // Delete directly without confirmation
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

                // Clear selection
                *self.ivars().selected_mcp_id.borrow_mut() = None;
            }

            // Reload outside the borrow scope
            self.ivars().mcp_uuid_map.borrow_mut().clear();
            self.load_mcps();
            self.update_mcp_button_states();
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
        self.load_mcps();
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
            set_layer_background_color(
                &layer,
                Theme::BG_DARK.0,
                Theme::BG_DARK.1,
                Theme::BG_DARK.2,
            );
        }

        unsafe {
            top_bar.setContentHuggingPriority_forOrientation(
                750.0,
                NSLayoutConstraintOrientation::Vertical,
            );
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
        back_btn.setBezelStyle(NSBezelStyle::Automatic);
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
            spacer.setContentHuggingPriority_forOrientation(
                1.0,
                NSLayoutConstraintOrientation::Horizontal,
            );
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
        refresh_btn.setBezelStyle(NSBezelStyle::Automatic);
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

    pub(super) fn setup_profiles_toolbar(&self, toolbar: &NSView, mtm: MainThreadMarker) {
        if let Some(toolbar_stack) = toolbar.downcast_ref::<NSStackView>() {
            let delete_btn =
                create_toolbar_button("−", sel!(deleteProfileClicked:), self, mtm, false, 24.0);
            let add_btn =
                create_toolbar_button("+", sel!(addProfileClicked:), self, mtm, true, 24.0);
            let spacer = create_toolbar_spacer(mtm);
            let edit_btn =
                create_toolbar_button("Edit", sel!(editProfileClicked:), self, mtm, false, 40.0);

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

    pub(super) fn setup_mcps_toolbar(&self, toolbar: &NSView, mtm: MainThreadMarker) {
        if let Some(toolbar_stack) = toolbar.downcast_ref::<NSStackView>() {
            let delete_btn =
                create_toolbar_button("−", sel!(deleteMcpClicked:), self, mtm, false, 24.0);
            let add_btn = create_toolbar_button("+", sel!(addMcpClicked:), self, mtm, true, 24.0);
            let spacer = create_toolbar_spacer(mtm);
            let edit_btn =
                create_toolbar_button("Edit", sel!(editMcpClicked:), self, mtm, false, 40.0);

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

        if let Some(list_stack) = &*self.ivars().profiles_list.borrow() {
            let subviews: Vec<_> = list_stack.subviews().to_vec();
            for view in subviews {
                unsafe {
                    list_stack.removeArrangedSubview(&view);
                }
                view.removeFromSuperview();
            }

            if config.profiles.is_empty() {
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
                sync_profile_selection(self, &[]);
            } else {
                let rows = build_profile_rows(self, &config.profiles, mtm);
                for row in rows {
                    unsafe {
                        list_stack.addArrangedSubview(&row);
                    }
                }
                let profile_ids: Vec<Uuid> =
                    config.profiles.iter().map(|profile| profile.id).collect();
                sync_profile_selection(self, &profile_ids);
                if let Some(default_id) = config.default_profile {
                    *self.ivars().selected_profile_id.borrow_mut() = Some(default_id);
                }
            }
        }

        self.update_profile_button_states();
    }

    fn load_mcps(&self) {
        let mtm = MainThreadMarker::new().unwrap();

        log_to_file("load_mcps called");

        let config = Config::load(Config::default_path().unwrap()).unwrap_or_default();
        log_to_file(&format!("Config has {} MCPs", config.mcps.len()));

        if let Some(list_stack) = &*self.ivars().mcps_list.borrow() {
            let subviews: Vec<_> = list_stack.subviews().to_vec();
            for view in subviews {
                unsafe {
                    list_stack.removeArrangedSubview(&view);
                }
                view.removeFromSuperview();
            }

            if config.mcps.is_empty() {
                let message =
                    NSTextField::labelWithString(&NSString::from_str("No MCPs configured."), mtm);
                message.setTextColor(Some(&Theme::text_secondary_color()));
                message.setFont(Some(&NSFont::systemFontOfSize(12.0)));
                message.setAlignment(objc2_app_kit::NSTextAlignment::Center);
                unsafe {
                    list_stack.addArrangedSubview(&message);
                }
                sync_mcp_selection(self, &[]);
            } else {
                let rows = build_mcp_rows(self, &config.mcps, mtm);
                for row in rows {
                    unsafe {
                        list_stack.addArrangedSubview(&row);
                    }
                }
                let mcp_ids: Vec<Uuid> = config.mcps.iter().map(|mcp| mcp.id).collect();
                sync_mcp_selection(self, &mcp_ids);
            }
        }

        self.update_mcp_button_states();
    }

    fn load_hotkey(&self) {
        let config = Config::load(Config::default_path().unwrap()).unwrap_or_default();

        if let Some(field) = &*self.ivars().hotkey_field.borrow() {
            field.setStringValue(&NSString::from_str(&config.global_hotkey));
        }
    }

    fn apply_selected_profile(&self, profile_id: Uuid) {
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

        *self.ivars().selected_profile_id.borrow_mut() = Some(profile_id);
        self.load_profiles();
    }

    fn select_mcp(&self, mcp_id: Uuid) {
        log_to_file(&format!("Selecting MCP: {mcp_id}"));
        *self.ivars().selected_mcp_id.borrow_mut() = Some(mcp_id);
        self.update_mcp_button_states();
        self.highlight_selected_mcp();
    }

    fn highlight_selected_mcp(&self) {
        let selected_id = *self.ivars().selected_mcp_id.borrow();
        let uuid_map = self.ivars().mcp_uuid_map.borrow();

        if let Some(list_stack) = &*self.ivars().mcps_list.borrow() {
            let subviews = list_stack.arrangedSubviews();
            for (index, view) in subviews.iter().enumerate() {
                let is_selected = uuid_map
                    .get(index)
                    .map(|&id| Some(id) == selected_id)
                    .unwrap_or(false);

                if let Some(layer) = view.layer() {
                    if is_selected {
                        // Highlight color (blue-ish)
                        set_layer_background_color(&layer, 0.2, 0.4, 0.8);
                    } else {
                        // Normal color
                        set_layer_background_color(
                            &layer,
                            Theme::BG_DARKER.0,
                            Theme::BG_DARKER.1,
                            Theme::BG_DARKER.2,
                        );
                    }
                }
            }
        }
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
        log_to_file(&format!(
            "update_mcp_button_states: has_selection={}",
            has_selection
        ));

        if let Some(btn) = &*self.ivars().mcp_delete_btn.borrow() {
            log_to_file(&format!("Enabling delete button: {}", has_selection));
            btn.setEnabled(has_selection);
        } else {
            log_to_file("mcp_delete_btn is None!");
        }
        if let Some(btn) = &*self.ivars().mcp_edit_btn.borrow() {
            btn.setEnabled(has_selection);
        } else {
            log_to_file("mcp_edit_btn is None!");
        }
    }
}
