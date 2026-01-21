//! Keyboard shortcuts for the application
//!
//! Chat View shortcuts (Cmd+Shift+...):
//! - R: Rename conversation
//! - N: New conversation  
//! - T: Toggle thinking display
//! - H: Show history
//! - S: Show settings/config
//! - Q: Close/quit popover
//!
//! Settings View shortcuts (Cmd+Shift+...):
//! - P: Focus profiles list
//! - M: Focus MCPs list
//! - Up/Down arrows: Navigate list items
//! - +/-: Add/delete item
//! - E: Edit selected item
//! - Space: Toggle MCP on/off (when MCP focused)
//! - Escape: Back to chat

use objc2::rc::Retained;
use objc2::sel;
use objc2_app_kit::{NSMenu, NSMenuItem, NSEventModifierFlags};
use objc2_foundation::{MainThreadMarker, NSString};

/// Create the application menu with keyboard shortcuts
pub fn create_app_menu_with_shortcuts(mtm: MainThreadMarker) -> Retained<NSMenu> {
    let menu = NSMenu::initWithTitle(mtm.alloc(), &NSString::from_str("PersonalAgent"));
    
    unsafe {
        // Chat View shortcuts
        add_menu_item(&menu, mtm, "New Conversation", sel!(newConversationShortcut:), "n", true, true);
        add_menu_item(&menu, mtm, "Rename Conversation", sel!(renameConversationShortcut:), "r", true, true);
        add_menu_item(&menu, mtm, "Toggle Thinking", sel!(toggleThinkingShortcut:), "t", true, true);
        add_menu_item(&menu, mtm, "Show History", sel!(showHistoryShortcut:), "h", true, true);
        add_menu_item(&menu, mtm, "Show Settings", sel!(showSettingsShortcut:), "s", true, true);
        
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        
        // Settings View shortcuts
        add_menu_item(&menu, mtm, "Focus Profiles", sel!(focusProfilesShortcut:), "p", true, true);
        add_menu_item(&menu, mtm, "Focus MCPs", sel!(focusMcpsShortcut:), "m", true, true);
        add_menu_item(&menu, mtm, "Add Item", sel!(addItemShortcut:), "=", true, true); // Cmd+Shift+=
        add_menu_item(&menu, mtm, "Delete Item", sel!(deleteItemShortcut:), "-", true, true);
        add_menu_item(&menu, mtm, "Edit Item", sel!(editItemShortcut:), "e", true, true);
        add_menu_item(&menu, mtm, "Toggle MCP", sel!(toggleMcpShortcut:), " ", true, true); // Cmd+Shift+Space
        
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        
        // Navigation
        add_menu_item(&menu, mtm, "Back/Close", sel!(backShortcut:), "\u{1b}", true, false); // Escape
    }
    
    menu
}

unsafe fn add_menu_item(
    menu: &NSMenu,
    mtm: MainThreadMarker,
    title: &str,
    action: objc2::runtime::Sel,
    key: &str,
    cmd: bool,
    shift: bool,
) {
    let item = NSMenuItem::initWithTitle_action_keyEquivalent(
        mtm.alloc(),
        &NSString::from_str(title),
        Some(action),
        &NSString::from_str(key),
    );
    
    let mut modifiers = NSEventModifierFlags::empty();
    if cmd {
        modifiers |= NSEventModifierFlags::Command;
    }
    if shift {
        modifiers |= NSEventModifierFlags::Shift;
    }
    item.setKeyEquivalentModifierMask(modifiers);
    
    menu.addItem(&item);
}

/// Add shortcuts menu to the main menu bar
pub fn add_shortcuts_to_menu_bar(main_menu: &NSMenu, mtm: MainThreadMarker) {
    let shortcuts_menu = create_app_menu_with_shortcuts(mtm);
    
    let shortcuts_menu_item = NSMenuItem::new(mtm);
    shortcuts_menu_item.setSubmenu(Some(&shortcuts_menu));
    shortcuts_menu_item.setTitle(&NSString::from_str("Actions"));
    
    unsafe {
        main_menu.addItem(&shortcuts_menu_item);
    }
}
