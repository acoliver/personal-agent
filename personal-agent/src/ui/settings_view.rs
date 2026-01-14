//! Settings view for managing model profiles and configuration

use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly, DefinedClass};
use objc2_foundation::{
    NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use objc2_app_kit::{
    NSView, NSViewController, NSTextField, NSButton, NSScrollView, NSFont, NSBezelStyle,
    NSStackView, NSUserInterfaceLayoutOrientation, NSStackViewDistribution, NSLayoutConstraintOrientation,
};
use objc2_quartz_core::CALayer;
use uuid::Uuid;

use super::theme::Theme;
use personal_agent::config::Config;
use personal_agent::models::AuthConfig;

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

// ============================================================================
// Settings callback protocol
// ============================================================================

/// Callback trait for settings view actions
pub trait SettingsDelegate {
    fn settings_did_select_profile(&self, profile_id: String);
    fn settings_did_request_refresh(&self);
    fn settings_did_close(&self);
}

// ============================================================================
// SettingsViewController ivars
// ============================================================================

pub struct SettingsViewIvars {
    profiles_container: RefCell<Option<Retained<NSView>>>,
    scroll_view: RefCell<Option<Retained<NSScrollView>>>,
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
                main_stack.setDistribution(objc2_app_kit::NSStackViewDistribution::Fill);
            }
            
            // Build the UI components
            let top_bar = self.build_top_bar_stack(mtm);
            let content_area = self.build_content_area_stack(mtm);
            let bottom_area = self.build_bottom_buttons_stack(mtm);
            
            // Add to stack
            unsafe {
                main_stack.addArrangedSubview(&top_bar);
                main_stack.addArrangedSubview(&content_area);
                main_stack.addArrangedSubview(&bottom_area);
            }
            
            // Add stack to main view
            main_view.addSubview(&main_stack);
            
            // Set constraints to fill parent
            unsafe {
                // Activate constraints individually
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
            
            // Load profiles
            self.load_profiles();
        }

        #[unsafe(method(backButtonClicked:))]
        fn back_button_clicked(&self, _sender: Option<&NSObject>) {
            // Post notification to switch back to chat view
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowChatView");
            unsafe {
                center.postNotificationName_object(&name, None);
            }
        }

        #[unsafe(method(refreshButtonClicked:))]
        fn refresh_button_clicked(&self, _sender: Option<&NSObject>) {
            println!("Refresh models requested - fetching from models.dev");
            
            // Spawn async task to refresh registry
            use personal_agent::registry::RegistryManager;
            
            std::thread::spawn(|| {
                let runtime = match tokio::runtime::Runtime::new() {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Failed to create runtime: {}", e);
                        return;
                    }
                };
                
                runtime.block_on(async {
                    match RegistryManager::new() {
                        Ok(manager) => {
                            match manager.refresh().await {
                                Ok(_) => println!("Registry refreshed successfully"),
                                Err(e) => eprintln!("Failed to refresh registry: {}", e),
                            }
                        }
                        Err(e) => eprintln!("Failed to create registry manager: {}", e),
                    }
                });
            });
            
            // Reload profiles (they'll pick up new registry next time editor is opened)
            self.load_profiles();
        }

        #[unsafe(method(addProfileClicked:))]
        fn add_profile_clicked(&self, _sender: Option<&NSObject>) {
            // Post notification to show profile editor
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowProfileEditor");
            unsafe {
                center.postNotificationName_object(&name, None);
            }
        }

        #[unsafe(method(profileEditClicked:))]
        fn profile_edit_clicked(&self, sender: Option<&NSObject>) {
            // Get the button's tag (profile index)
            if let Some(button) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                let tag = button.tag();
                println!("Profile edit clicked: tag {}", tag);
                
                // Load the config
                let config_path = match Config::default_path() {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("Failed to get config path: {}", e);
                        return;
                    }
                };
                
                let config = match Config::load(&config_path) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Failed to load config: {}", e);
                        return;
                    }
                };
                
                // Get the profile from the selected index
                if let Some(profile) = config.profiles.get(tag as usize) {
                    // Store profile ID in thread-local storage for profile editor
                    EDITING_PROFILE_ID.set(Some(profile.id));
                    
                    // Post notification to show profile editor
                    use objc2_foundation::NSNotificationCenter;
                    let center = NSNotificationCenter::defaultCenter();
                    let name = NSString::from_str("PersonalAgentShowProfileEditor");
                    unsafe {
                        center.postNotificationName_object(&name, None);
                    }
                }
            }
        }

        #[unsafe(method(profileSelected:))]
        fn profile_selected(&self, sender: Option<&NSObject>) {
            // Get the button's tag (profile index)
            if let Some(button) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                let tag = button.tag();
                println!("Profile selected: tag {}", tag);
                
                // Load the config
                let config_path = match Config::default_path() {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("Failed to get config path: {}", e);
                        return;
                    }
                };
                
                let mut config = match Config::load(&config_path) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Failed to load config: {}", e);
                        return;
                    }
                };
                
                // Get the profile ID from the selected index
                if let Some(profile) = config.profiles.get(tag as usize) {
                    let profile_id = profile.id;
                    
                    // Set as active profile
                    config.default_profile = Some(profile_id);
                    
                    // Save the config
                    if let Err(e) = config.save(&config_path) {
                        eprintln!("Failed to save config: {}", e);
                    } else {
                        println!("Active profile set to: {}", profile.name);
                    }
                }
                
                // Post notification to go back to chat
                use objc2_foundation::NSNotificationCenter;
                let center = NSNotificationCenter::defaultCenter();
                let name = NSString::from_str("PersonalAgentShowChatView");
                unsafe {
                    center.postNotificationName_object(&name, None);
                }
            }
        }

        #[unsafe(method(deleteProfile:))]
        fn delete_profile(&self, sender: Option<&NSObject>) {
            use objc2_app_kit::NSAlert;
            
            // Get the button's tag (profile index)
            if let Some(button) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                let tag = button.tag();
                println!("Delete profile requested: tag {}", tag);
                
                let mtm = MainThreadMarker::new().unwrap();
                
                // Show confirmation dialog
                let alert = NSAlert::new(mtm);
                alert.setMessageText(&NSString::from_str("Delete Profile?"));
                alert.setInformativeText(&NSString::from_str("This action cannot be undone."));
                alert.addButtonWithTitle(&NSString::from_str("Delete"));
                alert.addButtonWithTitle(&NSString::from_str("Cancel"));
                
                let response = unsafe { alert.runModal() };
                
                // NSAlertFirstButtonReturn = 1000
                if response == 1000 {
                    // Load the config
                    let config_path = match Config::default_path() {
                        Ok(path) => path,
                        Err(e) => {
                            eprintln!("Failed to get config path: {}", e);
                            return;
                        }
                    };
                    
                    let mut config = match Config::load(&config_path) {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("Failed to load config: {}", e);
                            return;
                        }
                    };
                    
                    // Get the profile ID from the selected index
                    if let Some(profile) = config.profiles.get(tag as usize) {
                        let profile_id = profile.id;
                        
                        // Remove the profile
                        if let Err(e) = config.remove_profile(&profile_id) {
                            eprintln!("Failed to remove profile: {}", e);
                            return;
                        }
                        
                        // Save the config
                        if let Err(e) = config.save(&config_path) {
                            eprintln!("Failed to save config: {}", e);
                            return;
                        }
                        
                        println!("Profile deleted successfully");
                        
                        // Reload profiles
                        self.load_profiles();
                    }
                }
            }
        }
    }
);

impl SettingsViewController {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let ivars = SettingsViewIvars {
            profiles_container: RefCell::new(None),
            scroll_view: RefCell::new(None),
        };
        
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
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
            set_layer_background_color(&layer, Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2);
        }
        
        // CRITICAL: Set fixed height and high content hugging priority
        unsafe {
            top_bar.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            top_bar.setContentCompressionResistancePriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            let height_constraint = top_bar.heightAnchor().constraintEqualToConstant(44.0);
            height_constraint.setActive(true);
        }

        // Back button (w=40 per wireframe)
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
            back_btn.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
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
            title.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
        }
        unsafe {
            top_bar.addArrangedSubview(&title);
        }
        
        // Spacer (flexible)
        let spacer = NSView::new(mtm);
        unsafe {
            spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
            top_bar.addArrangedSubview(&spacer);
        }
        
        // Refresh Models button (w=120 per wireframe)
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
            refresh_btn.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
            let width_constraint = refresh_btn.widthAnchor().constraintEqualToConstant(120.0);
            width_constraint.setActive(true);
        }
        unsafe {
            top_bar.addArrangedSubview(&refresh_btn);
        }

        Retained::from(&*top_bar as &NSView)
    }

    fn build_content_area_stack(&self, mtm: MainThreadMarker) -> Retained<NSScrollView> {
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
            scroll_view.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
            scroll_view.setContentCompressionResistancePriority_forOrientation(250.0, NSLayoutConstraintOrientation::Vertical);
            
            // Add minimum height constraint to prevent collapse
            let min_height = scroll_view.heightAnchor().constraintGreaterThanOrEqualToConstant(100.0);
            min_height.setActive(true);
        }

        // Create vertical stack for profiles inside scroll view
        let profiles_stack = NSStackView::new(mtm);
        unsafe {
            profiles_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            profiles_stack.setSpacing(8.0);
            profiles_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
            profiles_stack.setDistribution(NSStackViewDistribution::Fill);
        }
        
        profiles_stack.setWantsLayer(true);
        if let Some(layer) = profiles_stack.layer() {
            set_layer_background_color(&layer, Theme::BG_DARKEST.0, Theme::BG_DARKEST.1, Theme::BG_DARKEST.2);
        }

        scroll_view.setDocumentView(Some(&profiles_stack));

        // Store references
        *self.ivars().scroll_view.borrow_mut() = Some(scroll_view.clone());
        *self.ivars().profiles_container.borrow_mut() = Some(Retained::from(&*profiles_stack as &NSView));

        scroll_view
    }

    fn build_bottom_buttons_stack(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        // Create horizontal stack for bottom bar (fixed height 50px per wireframe)
        let bottom_stack = NSStackView::new(mtm);
        unsafe {
            bottom_stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            bottom_stack.setSpacing(8.0);
            bottom_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
            bottom_stack.setDistribution(NSStackViewDistribution::Fill);
            bottom_stack.setEdgeInsets(objc2_foundation::NSEdgeInsets {
                top: 10.0,
                left: 12.0,
                bottom: 10.0,
                right: 12.0,
            });
        }
        
        bottom_stack.setWantsLayer(true);
        if let Some(layer) = bottom_stack.layer() {
            set_layer_background_color(&layer, Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2);
        }
        
        // CRITICAL: Set fixed height and high content hugging priority
        unsafe {
            bottom_stack.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            bottom_stack.setContentCompressionResistancePriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            let height_constraint = bottom_stack.heightAnchor().constraintEqualToConstant(50.0);
            height_constraint.setActive(true);
        }

        // Spacer (flexible, pushes button to right)
        let spacer = NSView::new(mtm);
        unsafe {
            spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
            bottom_stack.addArrangedSubview(&spacer);
        }

        // Add Profile button (per wireframe)
        let add_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("+ Add Profile"),
                Some(self),
                Some(sel!(addProfileClicked:)),
                mtm,
            )
        };
        add_btn.setBezelStyle(NSBezelStyle::Rounded);
        unsafe {
            add_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
            add_btn.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
            let width_constraint = add_btn.widthAnchor().constraintGreaterThanOrEqualToConstant(100.0);
            width_constraint.setActive(true);
        }
        unsafe {
            bottom_stack.addArrangedSubview(&add_btn);
        }

        Retained::from(&*bottom_stack as &NSView)
    }

    fn load_profiles(&self) {
        let mtm = MainThreadMarker::new().unwrap();
        
        // Load config
        let config = Config::load(Config::default_path().unwrap()).unwrap_or_default();
        
        if let Some(container) = &*self.ivars().profiles_container.borrow() {
            // Clear existing subviews (for stack view, remove arranged subviews)
            let subviews = container.subviews();
            for view in subviews.iter() {
                if let Some(stack) = container.downcast_ref::<NSStackView>() {
                    unsafe {
                        stack.removeArrangedSubview(&view);
                    }
                }
                view.removeFromSuperview();
            }

            let profiles = &config.profiles;
            
            // For stack view, just add profile views - stack handles positioning
            if let Some(stack) = container.downcast_ref::<NSStackView>() {
                for (index, profile) in profiles.iter().enumerate() {
                    let profile_view = self.create_profile_card(
                        &profile.name,
                        &profile.provider_id,
                        &profile.model_id,
                        &profile.auth,
                        index,
                        mtm,
                    );
                    unsafe {
                        stack.addArrangedSubview(&profile_view);
                    }
                }
                
                // If no profiles, show a friendly message
                if profiles.is_empty() {
                    let message = NSTextField::labelWithString(
                        &NSString::from_str("No profiles yet.\n\nCreate your first one!"),
                        mtm,
                    );
                    message.setTextColor(Some(&Theme::text_secondary_color()));
                    message.setFont(Some(&NSFont::systemFontOfSize(13.0)));
                    unsafe {
                        stack.addArrangedSubview(&message);
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn create_profile_card(
        &self,
        name: &str,
        provider: &str,
        model: &str,
        auth: &AuthConfig,
        index: usize,
        mtm: MainThreadMarker,
    ) -> Retained<NSView> {
        let width = 380.0;
        let height = 80.0;

        // Create card container
        let card = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height)),
        );
        card.setWantsLayer(true);
        if let Some(layer) = card.layer() {
            set_layer_background_color(&layer, Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2);
            set_layer_corner_radius(&layer, 8.0);
        }
        
        // Set fixed height constraint
        unsafe {
            card.setTranslatesAutoresizingMaskIntoConstraints(false);
            let height_constraint = card.heightAnchor().constraintEqualToConstant(height);
            height_constraint.setActive(true);
        }

        // Left side: vertical stack for labels
        let labels_stack = NSStackView::new(mtm);
        labels_stack.setFrame(NSRect::new(
            NSPoint::new(12.0, 10.0),
            NSSize::new(260.0, 60.0),
        ));
        unsafe {
            labels_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            labels_stack.setSpacing(4.0);
            labels_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
        }

        // Profile name
        let name_label = NSTextField::labelWithString(&NSString::from_str(name), mtm);
        name_label.setTextColor(Some(&Theme::text_primary()));
        name_label.setFont(Some(&NSFont::boldSystemFontOfSize(14.0)));
        unsafe {
            labels_stack.addArrangedSubview(&name_label);
        }

        // Provider and model
        let model_text = format!("{}:{}", provider, model);
        let model_label = NSTextField::labelWithString(&NSString::from_str(&model_text), mtm);
        model_label.setTextColor(Some(&Theme::text_secondary_color()));
        model_label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        unsafe {
            labels_stack.addArrangedSubview(&model_label);
        }

        // API key status
        let key_status = match auth {
            AuthConfig::Key { value } => {
                if value.is_empty() {
                    "No API key"
                } else {
                    "API key configured"
                }
            }
            AuthConfig::Keyfile { path } => {
                if path.is_empty() {
                    "No keyfile"
                } else {
                    "Keyfile configured"
                }
            }
        };
        let status_label = NSTextField::labelWithString(&NSString::from_str(key_status), mtm);
        status_label.setTextColor(Some(&Theme::text_secondary_color()));
        status_label.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        unsafe {
            labels_stack.addArrangedSubview(&status_label);
        }
        
        card.addSubview(&labels_stack);

        // Right side: vertical stack for buttons
        let buttons_stack = NSStackView::new(mtm);
        buttons_stack.setFrame(NSRect::new(
            NSPoint::new(280.0, 15.0),
            NSSize::new(80.0, 60.0),
        ));
        unsafe {
            buttons_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            buttons_stack.setSpacing(5.0);
        }

        // Edit button
        let edit_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Edit"),
                Some(self),
                Some(sel!(profileEditClicked:)),
                mtm,
            )
        };
        edit_btn.setBezelStyle(NSBezelStyle::Rounded);
        edit_btn.setTag(index as isize);
        unsafe {
            buttons_stack.addArrangedSubview(&edit_btn);
        }

        // Select button
        let select_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Select"),
                Some(self),
                Some(sel!(profileSelected:)),
                mtm,
            )
        };
        select_btn.setBezelStyle(NSBezelStyle::Rounded);
        select_btn.setTag(index as isize);
        unsafe {
            buttons_stack.addArrangedSubview(&select_btn);
        }

        // Del button
        let del_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Del"),
                Some(self),
                Some(sel!(deleteProfile:)),
                mtm,
            )
        };
        del_btn.setBezelStyle(NSBezelStyle::Rounded);
        del_btn.setTag(index as isize);
        unsafe {
            buttons_stack.addArrangedSubview(&del_btn);
        }
        
        card.addSubview(&buttons_stack);

        card
    }
}
