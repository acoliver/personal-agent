//! Pure macOS menu bar app with `NSPopover` - like `BarTranslate`
//!
//! This is a minimal implementation that uses native macOS APIs directly
//! without trying to wrap an eframe window.
#![allow(unsafe_code)]
#![allow(unused_unsafe)]
#![allow(clippy::items_after_statements)]

use std::cell::Cell;

use objc2::rc::Retained;
use objc2::runtime::{NSObject, ProtocolObject};
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSImage, NSMenu,
    NSMenuItem, NSPopover, NSPopoverBehavior, NSStatusBar, NSStatusItem,
    NSVariableStatusItemLength,
};
use objc2_foundation::{NSNotification, NSObjectProtocol, NSRectEdge, NSSize, NSString};

mod ui;
use ui::history_view::LOADED_CONVERSATION_JSON;
use ui::{
    ChatViewController, HistoryViewController, McpAddViewController, McpConfigureViewController,
    ModelSelectorViewController, ProfileEditorDemoViewController, SettingsViewController,
};

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
    static MCP_ADD_VIEW_CONTROLLER: Cell<Option<Retained<McpAddViewController>>> = const { Cell::new(None) };
    static MCP_CONFIGURE_VIEW_CONTROLLER: Cell<Option<Retained<McpConfigureViewController>>> = const { Cell::new(None) };
}

// PopoverContentViewController is now replaced by ChatViewController from ui module

/// Load PNG data as an `NSImage` (for menu bar icons)
/// NOT a template - we want to keep the original colors (red eye)
fn load_image(png_data: &[u8]) -> Option<Retained<NSImage>> {
    load_image_data(png_data)
}

fn load_image_data(png_data: &[u8]) -> Option<Retained<NSImage>> {
    use objc2::AllocAnyThread;
    use objc2_foundation::NSData;

    let data = NSData::with_bytes(png_data);
    NSImage::initWithData(NSImage::alloc(), &data)
}

struct ViewControllers {
    chat: Retained<ChatViewController>,
    history: Retained<HistoryViewController>,
    settings: Retained<SettingsViewController>,
    model_selector: Retained<ModelSelectorViewController>,
    profile_editor: Retained<ProfileEditorDemoViewController>,
    mcp_add: Retained<McpAddViewController>,
    mcp_configure: Retained<McpConfigureViewController>,
}

fn configure_app_appearance(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    let dark_appearance_name = unsafe { objc2_app_kit::NSAppearanceNameDarkAqua };
    if let Some(dark_appearance) =
        objc2_app_kit::NSAppearance::appearanceNamed(dark_appearance_name)
    {
        app.setAppearance(Some(&dark_appearance));
    }
}

fn create_status_item(self_ref: &AppDelegate, mtm: MainThreadMarker) -> Retained<NSStatusItem> {
    let status_bar = NSStatusBar::systemStatusBar();
    let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);

    if let Some(button) = status_item.button(mtm) {
        let icon_data = include_bytes!("../assets/MenuBarIcon.imageset/icon-32.png");
        if let Some(image) = load_image(icon_data) {
            image.setSize(NSSize::new(17.6, 17.6));
            button.setImage(Some(&image));
        } else {
            button.setTitle(&NSString::from_str("PA"));
        }
        unsafe {
            button.setAction(Some(sel!(togglePopover:)));
            button.setTarget(Some(self_ref));
        }
    }

    status_item
}

fn create_popover(mtm: MainThreadMarker) -> Retained<NSPopover> {
    let popover = NSPopover::new(mtm);
    popover.setBehavior(NSPopoverBehavior::ApplicationDefined);
    popover.setAnimates(true);
    popover.setContentSize(NSSize::new(400.0, 500.0));

    let dark_name = unsafe { objc2_app_kit::NSAppearanceNameDarkAqua };
    if let Some(dark_appearance) = objc2_app_kit::NSAppearance::appearanceNamed(dark_name) {
        popover.setAppearance(Some(&dark_appearance));
    }

    popover
}

fn create_view_controllers(mtm: MainThreadMarker) -> ViewControllers {
    ViewControllers {
        chat: ChatViewController::new(mtm),
        history: HistoryViewController::new(mtm),
        settings: SettingsViewController::new(mtm),
        model_selector: ModelSelectorViewController::new(mtm),
        profile_editor: ProfileEditorDemoViewController::new(mtm),
        mcp_add: McpAddViewController::new(mtm),
        mcp_configure: McpConfigureViewController::new(mtm),
    }
}

fn store_app_state(
    status_item: Retained<NSStatusItem>,
    popover: Retained<NSPopover>,
    controllers: ViewControllers,
) {
    STATUS_ITEM.set(Some(status_item));
    POPOVER.set(Some(popover));
    CHAT_VIEW_CONTROLLER.set(Some(controllers.chat));
    HISTORY_VIEW_CONTROLLER.set(Some(controllers.history));
    SETTINGS_VIEW_CONTROLLER.set(Some(controllers.settings));
    MODEL_SELECTOR_VIEW_CONTROLLER.set(Some(controllers.model_selector));
    PROFILE_EDITOR_VIEW_CONTROLLER.set(Some(controllers.profile_editor));
    MCP_ADD_VIEW_CONTROLLER.set(Some(controllers.mcp_add));
    MCP_CONFIGURE_VIEW_CONTROLLER.set(Some(controllers.mcp_configure));
}

fn register_view_notifications(self_ref: &AppDelegate) {
    use objc2_foundation::NSNotificationCenter;

    let center = NSNotificationCenter::defaultCenter();
    unsafe {
        center.addObserver_selector_name_object(
            self_ref,
            sel!(showChatView:),
            Some(&NSString::from_str("PersonalAgentShowChatView")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(showSettingsView:),
            Some(&NSString::from_str("PersonalAgentShowSettingsView")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(showHistoryView:),
            Some(&NSString::from_str("PersonalAgentShowHistoryView")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(showModelSelector:),
            Some(&NSString::from_str("PersonalAgentShowModelSelector")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(showProfileEditor:),
            Some(&NSString::from_str("PersonalAgentShowProfileEditor")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(modelSelected:),
            Some(&NSString::from_str("PersonalAgentModelSelected")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(loadConversation:),
            Some(&NSString::from_str("PersonalAgentLoadConversation")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(showAddMcp:),
            Some(&NSString::from_str("PersonalAgentShowAddMcp")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(showConfigureMcp:),
            Some(&NSString::from_str("PersonalAgentShowConfigureMcp")),
            None,
        );
        
        // Keyboard shortcut notifications
        center.addObserver_selector_name_object(
            self_ref,
            sel!(closePopover:),
            Some(&NSString::from_str("PersonalAgentClosePopover")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(newConversation:),
            Some(&NSString::from_str("PersonalAgentNewConversation")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(renameConversation:),
            Some(&NSString::from_str("PersonalAgentRenameConversation")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(toggleThinking:),
            Some(&NSString::from_str("PersonalAgentToggleThinking")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(showHistoryView:),
            Some(&NSString::from_str("PersonalAgentShowHistory")),
            None,
        );
        
        // Settings view shortcuts - forward to settings view
        center.addObserver_selector_name_object(
            self_ref,
            sel!(focusProfiles:),
            Some(&NSString::from_str("PersonalAgentFocusProfiles")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(focusMcps:),
            Some(&NSString::from_str("PersonalAgentFocusMcps")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(addItem:),
            Some(&NSString::from_str("PersonalAgentAddItem")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(deleteItem:),
            Some(&NSString::from_str("PersonalAgentDeleteItem")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(editItem:),
            Some(&NSString::from_str("PersonalAgentEditItem")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(toggleMcp:),
            Some(&NSString::from_str("PersonalAgentToggleMcp")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(moveUp:),
            Some(&NSString::from_str("PersonalAgentMoveUp")),
            None,
        );
        center.addObserver_selector_name_object(
            self_ref,
            sel!(moveDown:),
            Some(&NSString::from_str("PersonalAgentMoveDown")),
            None,
        );
    }
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

            configure_app_appearance(mtm);

            let status_item = create_status_item(self, mtm);
            let popover = create_popover(mtm);
            let controllers = create_view_controllers(mtm);

            popover.setContentViewController(Some(&controllers.chat));
            store_app_state(status_item, popover, controllers);

            let app = NSApplication::sharedApplication(mtm);
            app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

            register_view_notifications(self);

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
            eprintln!("showSettingsView: called");
            let popover = POPOVER.take();
            let settings_view = SETTINGS_VIEW_CONTROLLER.take();

            if let (Some(ref popover), Some(ref settings_view)) = (&popover, &settings_view) {
                eprintln!("showSettingsView: setting content view controller");
                popover.setContentViewController(Some(settings_view));
                // Reload profiles when settings view becomes visible
                eprintln!("showSettingsView: calling reload_profiles");
                settings_view.reload_profiles();
                
                // Force view to update
                let view = settings_view.view();
                eprintln!("showSettingsView: got view, calling setNeedsDisplay");
                view.setNeedsDisplay(true);
            } else {
                eprintln!("showSettingsView: ERROR - popover or settings_view is None");
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
            // Create a NEW profile editor instance to pick up EDITING_PROFILE_ID from thread-local
            // (The settings view sets EDITING_PROFILE_ID before posting this notification)
            let mtm = MainThreadMarker::new().unwrap();
            let new_profile_editor = ProfileEditorDemoViewController::new(mtm);

            let popover = POPOVER.take();

            if let Some(ref popover) = popover {
                popover.setContentViewController(Some(&new_profile_editor));
            }

            POPOVER.set(popover);
            // Update the stored reference
            PROFILE_EDITOR_VIEW_CONTROLLER.set(Some(new_profile_editor));
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

        #[unsafe(method(showAddMcp:))]
        fn show_add_mcp(&self, _notification: &NSNotification) {
            let mtm = MainThreadMarker::new().unwrap();

            let popover = POPOVER.take();
            let existing = MCP_ADD_VIEW_CONTROLLER.take();

            // Reuse existing controller or create new one
            let mcp_add_view = existing.unwrap_or_else(|| McpAddViewController::new(mtm));

            if let Some(ref popover) = popover {
                popover.setContentViewController(Some(&mcp_add_view));
            }

            POPOVER.set(popover);
            MCP_ADD_VIEW_CONTROLLER.set(Some(mcp_add_view));
        }

        #[unsafe(method(showConfigureMcp:))]
        fn show_configure_mcp(&self, _notification: &NSNotification) {
            // Create a NEW MCP configure view instance (picks up PARSED_MCP from thread-local)
            let mtm = MainThreadMarker::new().unwrap();
            let new_mcp_configure_view = McpConfigureViewController::new(mtm);

            let popover = POPOVER.take();

            if let Some(ref popover) = popover {
                popover.setContentViewController(Some(&new_mcp_configure_view));
            }

            POPOVER.set(popover);
            MCP_CONFIGURE_VIEW_CONTROLLER.set(Some(new_mcp_configure_view));
        }
        
        // Keyboard shortcut handlers
        #[unsafe(method(closePopover:))]
        fn close_popover(&self, _notification: &NSNotification) {
            let popover = POPOVER.take();
            if let Some(ref popover) = popover {
                unsafe {
                    popover.performClose(None);
                }
            }
            POPOVER.set(popover);
        }
        
        #[unsafe(method(newConversation:))]
        fn new_conversation(&self, _notification: &NSNotification) {
            // Forward to chat view
            if let Some(chat_view) = CHAT_VIEW_CONTROLLER.take() {
                unsafe {
                    let _: () = msg_send![&*chat_view, newConversation:std::ptr::null::<NSObject>()];
                }
                CHAT_VIEW_CONTROLLER.set(Some(chat_view));
            }
        }
        
        #[unsafe(method(renameConversation:))]
        fn rename_conversation(&self, _notification: &NSNotification) {
            // Forward to chat view
            if let Some(chat_view) = CHAT_VIEW_CONTROLLER.take() {
                unsafe {
                    let _: () = msg_send![&*chat_view, renameConversation:std::ptr::null::<NSObject>()];
                }
                CHAT_VIEW_CONTROLLER.set(Some(chat_view));
            }
        }
        
        #[unsafe(method(toggleThinking:))]
        fn toggle_thinking(&self, _notification: &NSNotification) {
            // Forward to chat view
            if let Some(chat_view) = CHAT_VIEW_CONTROLLER.take() {
                unsafe {
                    let _: () = msg_send![&*chat_view, toggleThinking:std::ptr::null::<NSObject>()];
                }
                CHAT_VIEW_CONTROLLER.set(Some(chat_view));
            }
        }
        
        // Settings view shortcut handlers
        #[unsafe(method(focusProfiles:))]
        fn focus_profiles(&self, _notification: &NSNotification) {
            if let Some(settings_view) = SETTINGS_VIEW_CONTROLLER.take() {
                unsafe {
                    let _: () = msg_send![&*settings_view, focusProfilesShortcut:std::ptr::null::<NSObject>()];
                }
                SETTINGS_VIEW_CONTROLLER.set(Some(settings_view));
            }
        }
        
        #[unsafe(method(focusMcps:))]
        fn focus_mcps(&self, _notification: &NSNotification) {
            if let Some(settings_view) = SETTINGS_VIEW_CONTROLLER.take() {
                unsafe {
                    let _: () = msg_send![&*settings_view, focusMcpsShortcut:std::ptr::null::<NSObject>()];
                }
                SETTINGS_VIEW_CONTROLLER.set(Some(settings_view));
            }
        }
        
        #[unsafe(method(addItem:))]
        fn add_item(&self, _notification: &NSNotification) {
            if let Some(settings_view) = SETTINGS_VIEW_CONTROLLER.take() {
                unsafe {
                    let _: () = msg_send![&*settings_view, addItemShortcut:std::ptr::null::<NSObject>()];
                }
                SETTINGS_VIEW_CONTROLLER.set(Some(settings_view));
            }
        }
        
        #[unsafe(method(deleteItem:))]
        fn delete_item(&self, _notification: &NSNotification) {
            if let Some(settings_view) = SETTINGS_VIEW_CONTROLLER.take() {
                unsafe {
                    let _: () = msg_send![&*settings_view, deleteItemShortcut:std::ptr::null::<NSObject>()];
                }
                SETTINGS_VIEW_CONTROLLER.set(Some(settings_view));
            }
        }
        
        #[unsafe(method(editItem:))]
        fn edit_item(&self, _notification: &NSNotification) {
            if let Some(settings_view) = SETTINGS_VIEW_CONTROLLER.take() {
                unsafe {
                    let _: () = msg_send![&*settings_view, editItemShortcut:std::ptr::null::<NSObject>()];
                }
                SETTINGS_VIEW_CONTROLLER.set(Some(settings_view));
            }
        }
        
        #[unsafe(method(toggleMcp:))]
        fn toggle_mcp(&self, _notification: &NSNotification) {
            if let Some(settings_view) = SETTINGS_VIEW_CONTROLLER.take() {
                unsafe {
                    let _: () = msg_send![&*settings_view, toggleMcpShortcut:std::ptr::null::<NSObject>()];
                }
                SETTINGS_VIEW_CONTROLLER.set(Some(settings_view));
            }
        }
        
        #[unsafe(method(moveUp:))]
        fn move_up(&self, _notification: &NSNotification) {
            if let Some(settings_view) = SETTINGS_VIEW_CONTROLLER.take() {
                unsafe {
                    let _: () = msg_send![&*settings_view, moveSelectionUp:std::ptr::null::<NSObject>()];
                }
                SETTINGS_VIEW_CONTROLLER.set(Some(settings_view));
            }
        }
        
        #[unsafe(method(moveDown:))]
        fn move_down(&self, _notification: &NSNotification) {
            if let Some(settings_view) = SETTINGS_VIEW_CONTROLLER.take() {
                unsafe {
                    let _: () = msg_send![&*settings_view, moveSelectionDown:std::ptr::null::<NSObject>()];
                }
                SETTINGS_VIEW_CONTROLLER.set(Some(settings_view));
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
    
    // Create Actions menu with keyboard shortcuts
    let actions_menu = NSMenu::initWithTitle(mtm.alloc(), &NSString::from_str("Actions"));
    
    unsafe {
        use objc2_app_kit::NSEventModifierFlags;
        
        // Chat View shortcuts (Cmd+Shift+...)
        let new_conv = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("New Conversation"),
            Some(sel!(newConversationShortcut:)),
            &NSString::from_str("n"),
        );
        new_conv.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift
        );
        actions_menu.addItem(&new_conv);
        
        let rename = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Rename Conversation"),
            Some(sel!(renameConversationShortcut:)),
            &NSString::from_str("r"),
        );
        rename.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift
        );
        actions_menu.addItem(&rename);
        
        let toggle_think = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Toggle Thinking"),
            Some(sel!(toggleThinkingShortcut:)),
            &NSString::from_str("t"),
        );
        toggle_think.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift
        );
        actions_menu.addItem(&toggle_think);
        
        let history = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Show History"),
            Some(sel!(showHistoryShortcut:)),
            &NSString::from_str("h"),
        );
        history.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift
        );
        actions_menu.addItem(&history);
        
        let settings = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Show Settings"),
            Some(sel!(showSettingsShortcut:)),
            &NSString::from_str("s"),
        );
        settings.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift
        );
        actions_menu.addItem(&settings);
        
        actions_menu.addItem(&NSMenuItem::separatorItem(mtm));
        
        // Settings View shortcuts
        let focus_profiles = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Focus Profiles"),
            Some(sel!(focusProfilesShortcut:)),
            &NSString::from_str("p"),
        );
        focus_profiles.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift
        );
        actions_menu.addItem(&focus_profiles);
        
        let focus_mcps = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Focus MCPs"),
            Some(sel!(focusMcpsShortcut:)),
            &NSString::from_str("m"),
        );
        focus_mcps.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift
        );
        actions_menu.addItem(&focus_mcps);
        
        let add_item = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Add Item"),
            Some(sel!(addItemShortcut:)),
            &NSString::from_str("="),
        );
        add_item.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift
        );
        actions_menu.addItem(&add_item);
        
        let delete_item = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Delete Item"),
            Some(sel!(deleteItemShortcut:)),
            &NSString::from_str("-"),
        );
        delete_item.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift
        );
        actions_menu.addItem(&delete_item);
        
        let edit_item = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Edit Item"),
            Some(sel!(editItemShortcut:)),
            &NSString::from_str("e"),
        );
        edit_item.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift
        );
        actions_menu.addItem(&edit_item);
        
        actions_menu.addItem(&NSMenuItem::separatorItem(mtm));
        
        // Arrow key navigation for settings lists
        let move_up = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Move Selection Up"),
            Some(sel!(moveSelectionUp:)),
            &NSString::from_str("\u{F700}"), // Up arrow
        );
        move_up.setKeyEquivalentModifierMask(NSEventModifierFlags::empty());
        actions_menu.addItem(&move_up);
        
        let move_down = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Move Selection Down"),
            Some(sel!(moveSelectionDown:)),
            &NSString::from_str("\u{F701}"), // Down arrow
        );
        move_down.setKeyEquivalentModifierMask(NSEventModifierFlags::empty());
        actions_menu.addItem(&move_down);
        
        actions_menu.addItem(&NSMenuItem::separatorItem(mtm));
        
        // Navigation
        let back = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("Close Popover"),
            Some(sel!(backShortcut:)),
            &NSString::from_str("\u{1b}"), // Escape
        );
        back.setKeyEquivalentModifierMask(NSEventModifierFlags::empty());
        actions_menu.addItem(&back);
    }
    
    let actions_menu_item = NSMenuItem::new(mtm);
    actions_menu_item.setSubmenu(Some(&actions_menu));
    
    unsafe {
        main_menu.addItem(&actions_menu_item);
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
