//! Pure macOS menu bar app with `NSPopover` - like `BarTranslate`
//!
//! This is a minimal implementation that uses native macOS APIs directly
//! without trying to wrap an eframe window.

use std::cell::Cell;

use objc2::rc::Retained;
use objc2::runtime::{NSObject, ProtocolObject};
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_foundation::{
    NSNotification, NSObjectProtocol, NSRectEdge, NSSize, NSString,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSImage, NSMenu, NSMenuItem, NSPopover, NSPopoverBehavior, NSStatusBar, NSStatusItem,
    NSVariableStatusItemLength,
};

mod ui;
use ui::{ChatViewController, HistoryViewController, ModelSelectorViewController, ProfileEditorDemoViewController, SettingsViewController};
use ui::history_view::LOADED_CONVERSATION_JSON;

// Thread-local storage for selected model from model selector
thread_local! {
    pub static SELECTED_MODEL: Cell<Option<(String, String)>> = const { Cell::new(None) };
}

// Thread-local storage for popover state and view controllers
thread_local! {
    static POPOVER: Cell<Option<Retained<NSPopover>>> = const { Cell::new(None) };
    static STATUS_ITEM: Cell<Option<Retained<NSStatusItem>>> = const { Cell::new(None) };
    static CHAT_VIEW_CONTROLLER: Cell<Option<Retained<ChatViewController>>> = const { Cell::new(None) };
    static HISTORY_VIEW_CONTROLLER: Cell<Option<Retained<HistoryViewController>>> = const { Cell::new(None) };
    static SETTINGS_VIEW_CONTROLLER: Cell<Option<Retained<SettingsViewController>>> = const { Cell::new(None) };
    static MODEL_SELECTOR_VIEW_CONTROLLER: Cell<Option<Retained<ModelSelectorViewController>>> = const { Cell::new(None) };
    static PROFILE_EDITOR_VIEW_CONTROLLER: Cell<Option<Retained<ProfileEditorDemoViewController>>> = const { Cell::new(None) };
}

// PopoverContentViewController is now replaced by ChatViewController from ui module

/// Load PNG data as an `NSImage` (for menu bar icons)
/// NOT a template - we want to keep the original colors (red eye)
fn load_image(png_data: &[u8]) -> Option<Retained<NSImage>> {
    use objc2::AllocAnyThread;
    use objc2_foundation::NSData;
    
    let data = NSData::with_bytes(png_data);
    let image = NSImage::initWithData(NSImage::alloc(), &data)?;
    
    // Do NOT set as template - we want the original red color
    // image.setTemplate(true);  
    
    Some(image)
}

// ============================================================================
// AppDelegate - handles app lifecycle
// ============================================================================

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "PersonalAgentAppDelegate"]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            let mtm = MainThreadMarker::new().unwrap();
            
            // Force dark appearance for the entire app to avoid blue accents
            let app = NSApplication::sharedApplication(mtm);
            // SAFETY: NSAppearanceNameDarkAqua is a constant string provided by AppKit
            let dark_appearance_name = unsafe { objc2_app_kit::NSAppearanceNameDarkAqua };
            if let Some(dark_appearance) = objc2_app_kit::NSAppearance::appearanceNamed(dark_appearance_name) {
                app.setAppearance(Some(&dark_appearance));
            }

            // Create status bar item
            let status_bar = NSStatusBar::systemStatusBar();
            let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);

            // Configure status item button with icon
            if let Some(button) = status_item.button(mtm) {
                // Load the colored icon (red eye on transparent background)
                // Use 32px for retina, 16px for standard - macOS will pick appropriately
                let icon_data = include_bytes!("../assets/MenuBarIcon.imageset/icon-32.png");
                if let Some(image) = load_image(icon_data) {
                    // Set the size to 17.6x17.6 points (10% larger than standard 16x16)
                    image.setSize(NSSize::new(17.6, 17.6));
                    button.setImage(Some(&image));
                } else {
                    // Fallback to text if image fails
                    button.setTitle(&NSString::from_str("PA"));
                }
                // SAFETY: Setting action/target for event handling is standard Cocoa practice
                unsafe {
                    button.setAction(Some(sel!(togglePopover:)));
                    button.setTarget(Some(self));
                }
            }

            // Create popover
            let popover = NSPopover::new(mtm);
            // Use ApplicationDefined so the popover stays open during LLM requests
            // (Transient would close it when we block the main thread)
            popover.setBehavior(NSPopoverBehavior::ApplicationDefined);
            popover.setAnimates(true);
            popover.setContentSize(NSSize::new(400.0, 500.0));
            
            // Force dark appearance on the popover itself
            // SAFETY: NSAppearanceNameDarkAqua is a constant string provided by AppKit
            let dark_name = unsafe { objc2_app_kit::NSAppearanceNameDarkAqua };
            if let Some(dark_appearance) = objc2_app_kit::NSAppearance::appearanceNamed(dark_name) {
                popover.setAppearance(Some(&dark_appearance));
            }

            // Create view controllers once and store them
            let chat_view = ChatViewController::new(mtm);
            let history_view = HistoryViewController::new(mtm);
            let settings_view = SettingsViewController::new(mtm);
            let model_selector_view = ModelSelectorViewController::new(mtm);
            let profile_editor_view = ProfileEditorDemoViewController::new(mtm);
            
            // Set initial content to chat view
            popover.setContentViewController(Some(&chat_view));

            // Store references
            STATUS_ITEM.set(Some(status_item));
            POPOVER.set(Some(popover));
            CHAT_VIEW_CONTROLLER.set(Some(chat_view));
            HISTORY_VIEW_CONTROLLER.set(Some(history_view));
            SETTINGS_VIEW_CONTROLLER.set(Some(settings_view));
            MODEL_SELECTOR_VIEW_CONTROLLER.set(Some(model_selector_view));
            PROFILE_EDITOR_VIEW_CONTROLLER.set(Some(profile_editor_view));

            // Make this an accessory app (no dock icon, no main menu)
            let app = NSApplication::sharedApplication(mtm);
            app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

            // Register for view switching notifications
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            
            unsafe {
                center.addObserver_selector_name_object(
                    self,
                    sel!(showChatView:),
                    Some(&NSString::from_str("PersonalAgentShowChatView")),
                    None,
                );
                center.addObserver_selector_name_object(
                    self,
                    sel!(showSettingsView:),
                    Some(&NSString::from_str("PersonalAgentShowSettingsView")),
                    None,
                );
                center.addObserver_selector_name_object(
                    self,
                    sel!(showHistoryView:),
                    Some(&NSString::from_str("PersonalAgentShowHistoryView")),
                    None,
                );
                center.addObserver_selector_name_object(
                    self,
                    sel!(showModelSelector:),
                    Some(&NSString::from_str("PersonalAgentShowModelSelector")),
                    None,
                );
                center.addObserver_selector_name_object(
                    self,
                    sel!(showProfileEditor:),
                    Some(&NSString::from_str("PersonalAgentShowProfileEditor")),
                    None,
                );
                center.addObserver_selector_name_object(
                    self,
                    sel!(modelSelected:),
                    Some(&NSString::from_str("PersonalAgentModelSelected")),
                    None,
                );
                center.addObserver_selector_name_object(
                    self,
                    sel!(loadConversation:),
                    Some(&NSString::from_str("PersonalAgentLoadConversation")),
                    None,
                );
            }

            println!("PersonalAgent started - click 'PA' in menu bar");
        }
    }

    impl AppDelegate {
        #[unsafe(method(togglePopover:))]
        fn toggle_popover(&self, _sender: Option<&NSObject>) {
            let mtm = MainThreadMarker::new().unwrap();
            let popover = POPOVER.take();
            let status_item = STATUS_ITEM.take();

            if let (Some(ref popover), Some(ref status_item)) = (&popover, &status_item) {
                unsafe {
                    if popover.isShown() {
                        popover.performClose(None);
                    } else if let Some(button) = status_item.button(mtm) {
                        popover.showRelativeToRect_ofView_preferredEdge(
                            button.bounds(),
                            &button,
                            NSRectEdge::MinY,
                        );
                    }
                }
            }

            // Put them back
            POPOVER.set(popover);
            STATUS_ITEM.set(status_item);
        }

        #[unsafe(method(showChatView:))]
        fn show_chat_view(&self, _notification: &NSNotification) {
            let popover = POPOVER.take();
            let chat_view = CHAT_VIEW_CONTROLLER.take();
            
            if let (Some(ref popover), Some(ref chat_view)) = (&popover, &chat_view) {
                popover.setContentViewController(Some(chat_view));
            }
            
            POPOVER.set(popover);
            CHAT_VIEW_CONTROLLER.set(chat_view);
        }

        #[unsafe(method(showSettingsView:))]
        fn show_settings_view(&self, _notification: &NSNotification) {
            let popover = POPOVER.take();
            let settings_view = SETTINGS_VIEW_CONTROLLER.take();
            
            if let (Some(ref popover), Some(ref settings_view)) = (&popover, &settings_view) {
                popover.setContentViewController(Some(settings_view));
                // Reload profiles when settings view becomes visible
                settings_view.reload_profiles();
            }
            
            POPOVER.set(popover);
            SETTINGS_VIEW_CONTROLLER.set(settings_view);
        }

        #[unsafe(method(showHistoryView:))]
        fn show_history_view(&self, _notification: &NSNotification) {
            let popover = POPOVER.take();
            let history_view = HISTORY_VIEW_CONTROLLER.take();
            
            if let (Some(ref popover), Some(ref history_view)) = (&popover, &history_view) {
                // Refresh the history list before showing
                history_view.reload_conversations();
                popover.setContentViewController(Some(history_view));
            }
            
            POPOVER.set(popover);
            HISTORY_VIEW_CONTROLLER.set(history_view);
        }

        #[unsafe(method(showModelSelector:))]
        fn show_model_selector(&self, _notification: &NSNotification) {
            let popover = POPOVER.take();
            let model_selector_view = MODEL_SELECTOR_VIEW_CONTROLLER.take();
            
            if let (Some(ref popover), Some(ref model_selector_view)) = (&popover, &model_selector_view) {
                popover.setContentViewController(Some(model_selector_view));
            }
            
            POPOVER.set(popover);
            MODEL_SELECTOR_VIEW_CONTROLLER.set(model_selector_view);
        }

        #[unsafe(method(showProfileEditor:))]
        fn show_profile_editor(&self, _notification: &NSNotification) {
            let popover = POPOVER.take();
            let profile_editor_view = PROFILE_EDITOR_VIEW_CONTROLLER.take();
            
            if let (Some(ref popover), Some(ref profile_editor_view)) = (&popover, &profile_editor_view) {
                // Check if coming from model selector with pre-selected model
                // For now, just show editor with default values
                // TODO: Pass selected model to editor
                popover.setContentViewController(Some(profile_editor_view));
            }
            
            POPOVER.set(popover);
            PROFILE_EDITOR_VIEW_CONTROLLER.set(profile_editor_view);
        }

        #[unsafe(method(modelSelected:))]
        fn model_selected(&self, _notification: &NSNotification) {
            // Model was selected in model selector
            // Create a NEW profile editor to pick up the selected model from thread-local storage
            let mtm = MainThreadMarker::new().unwrap();
            
            println!("Model selected, creating new profile editor with selection");
            
            // Create fresh profile editor - it will read SELECTED_MODEL_PROVIDER/ID from thread-local
            let new_profile_editor = ProfileEditorDemoViewController::new(mtm);
            
            let popover = POPOVER.take();
            
            if let Some(ref popover) = popover {
                popover.setContentViewController(Some(&new_profile_editor));
            }
            
            POPOVER.set(popover);
            // Update the stored reference
            PROFILE_EDITOR_VIEW_CONTROLLER.set(Some(new_profile_editor));
        }
        
        #[unsafe(method(loadConversation:))]
        fn load_conversation(&self, _notification: &NSNotification) {
            // Get conversation JSON from thread-local storage
            let json_opt = LOADED_CONVERSATION_JSON.with(std::cell::Cell::take);
            
            if let Some(json) = json_opt {
                // Deserialize conversation
                match serde_json::from_str::<personal_agent::models::Conversation>(&json) {
                    Ok(conversation) => {
                        // Get chat view controller and load conversation
                        let chat_view = CHAT_VIEW_CONTROLLER.take();
                        if let Some(ref chat_view) = chat_view {
                            chat_view.load_conversation(conversation);
                        }
                        CHAT_VIEW_CONTROLLER.set(chat_view);
                        
                        // Switch to chat view
                        let popover = POPOVER.take();
                        let chat_view = CHAT_VIEW_CONTROLLER.take();
                        
                        if let (Some(ref popover), Some(ref chat_view)) = (&popover, &chat_view) {
                            popover.setContentViewController(Some(chat_view));
                        }
                        
                        POPOVER.set(popover);
                        CHAT_VIEW_CONTROLLER.set(chat_view);
                    }
                    Err(e) => {
                        eprintln!("Failed to deserialize conversation: {e}");
                    }
                }
            }
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        unsafe { msg_send![mtm.alloc::<Self>(), init] }
    }
}

// ============================================================================
// Main
// ============================================================================

/// Create the application's main menu with Edit menu for copy/paste
fn setup_main_menu(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    
    // Create main menu bar
    let main_menu = NSMenu::new(mtm);
    
    // Create Edit menu
    let edit_menu = NSMenu::initWithTitle(mtm.alloc(), &NSString::from_str("Edit"));
    
    // Add standard edit items
    unsafe {
        // Cut
        let cut_item = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Cut"),
            Some(sel!(cut:)),
            &NSString::from_str("x"),
        );
        edit_menu.addItem(&cut_item);
        
        // Copy
        let copy_item = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Copy"),
            Some(sel!(copy:)),
            &NSString::from_str("c"),
        );
        edit_menu.addItem(&copy_item);
        
        // Paste
        let paste_item = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Paste"),
            Some(sel!(paste:)),
            &NSString::from_str("v"),
        );
        edit_menu.addItem(&paste_item);
        
        // Select All
        let select_all_item = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Select All"),
            Some(sel!(selectAll:)),
            &NSString::from_str("a"),
        );
        edit_menu.addItem(&select_all_item);
    }
    
    // Create Edit menu item for menu bar
    let edit_menu_item = NSMenuItem::new(mtm);
    edit_menu_item.setSubmenu(Some(&edit_menu));
    
    // Add to main menu
    unsafe {
        main_menu.addItem(&edit_menu_item);
    }
    
    // Set as app's main menu
    app.setMainMenu(Some(&main_menu));
}

fn main() {
    let mtm = MainThreadMarker::new().expect("Must run on main thread");

    let app = NSApplication::sharedApplication(mtm);
    
    // Set up Edit menu for copy/paste to work
    setup_main_menu(mtm);
    
    let delegate = AppDelegate::new(mtm);

    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

    app.run();
}
