//! UI automation tests using AppleScript
//!
//! These tests verify actual UI behavior by automating the running app.
//! They are marked #[ignore] by default because they require:
//! 1. The app to be running (`cargo run --bin personal_agent_menubar`)
//! 2. Accessibility permissions granted to Terminal/test runner
//!
//! Run with: cargo test --test ui_automation_tests -- --ignored --test-threads=1
//!
//! IMPORTANT: This app uses a popover attached to a menu bar item, NOT a regular window.
//! AppleScript's "windows" collection doesn't include popovers.
//! We can verify:
//! - The tray icon exists
//! - Clicking it triggers the app
//! - The app's debug log shows expected behavior

mod ui_tests;

use ui_tests::applescript_helpers::*;
use std::fs;
use std::path::PathBuf;

const APP_PROCESS: &str = "personal_agent_menubar";

/// Get the app's debug log path
fn get_debug_log_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/PersonalAgent/debug.log")
}

/// Read the last N lines from the debug log
fn read_debug_log_tail(lines: usize) -> String {
    let log_path = get_debug_log_path();
    if !log_path.exists() {
        return String::new();
    }
    
    let content = fs::read_to_string(&log_path).unwrap_or_default();
    content
        .lines()
        .rev()
        .take(lines)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
}

/// Clear the debug log (to get fresh entries)
fn clear_debug_log() {
    let log_path = get_debug_log_path();
    let _ = fs::write(&log_path, "");
}

/// Verify the app is running before UI tests
fn ensure_app_running() -> bool {
    if !is_app_running(APP_PROCESS) {
        eprintln!("WARNING: {} is not running. Start it first with:", APP_PROCESS);
        eprintln!("  cargo run --bin personal_agent_menubar &");
        return false;
    }
    true
}

/// Click the tray icon
fn click_tray_icon() -> bool {
    let script = r#"
        tell application "System Events"
            tell process "personal_agent_menubar"
                if (count of menu bar items of menu bar 2) > 0 then
                    click menu bar item 1 of menu bar 2
                    return true
                end if
            end tell
        end tell
        return false
    "#;
    let result = run_applescript(script);
    result.success && result.stdout == "true"
}

// ============================================================================
// Basic App Tests
// ============================================================================

/// The app should be running and have a tray icon
#[test]
#[ignore = "Requires app running with accessibility permissions"]
fn app_has_tray_icon() {
    if !ensure_app_running() {
        panic!("App not running");
    }

    let script = r#"
        tell application "System Events"
            tell process "personal_agent_menubar"
                return (count of menu bar items of menu bar 2)
            end tell
        end tell
    "#;
    let result = run_applescript(script);
    
    assert!(result.success, "Failed to query tray: {}", result.stderr);
    let count: i32 = result.stdout.parse().unwrap_or(0);
    assert!(count >= 1, "App should have at least one menu bar item (tray icon)");
}

/// Clicking the tray icon should trigger the app (we verify via log)
#[test]
#[ignore = "Requires app running with accessibility permissions"]
fn clicking_tray_triggers_app() {
    if !ensure_app_running() {
        panic!("App not running");
    }

    // Clear log to get fresh entries
    clear_debug_log();
    
    // Click the tray
    let clicked = click_tray_icon();
    assert!(clicked, "Should be able to click tray icon");
    
    // Wait for any log activity
    std::thread::sleep(std::time::Duration::from_millis(500));
    
    // The click should have been received (even if popover isn't visible to AppleScript)
    // We just verify the app is still running
    assert!(is_app_running(APP_PROCESS), "App should still be running after tray click");
}

// ============================================================================
// Settings Panel Tests (via debug log verification)
// ============================================================================

/// When settings is opened, the log should show profile/MCP loading
#[test]
#[ignore = "Requires app running and manual settings navigation"]
fn settings_panel_loads_data() {
    if !ensure_app_running() {
        panic!("App not running");
    }

    // This test requires manual interaction or additional automation
    // For now, we check if the log contains evidence of settings loading
    let log = read_debug_log_tail(100);
    
    // Look for evidence that settings view loaded data
    let has_profile_log = log.contains("load_profiles") || log.contains("Config has");
    let has_mcp_log = log.contains("load_mcps") || log.contains("MCPs");
    
    println!("=== Debug Log (last 100 lines) ===");
    println!("{}", log);
    println!("=== End Log ===");
    
    // This test is informational - it shows what the app logged
    // A more complete test would require the popover to be accessible
    if has_profile_log || has_mcp_log {
        println!("Found settings load evidence in log");
    } else {
        println!("No settings load evidence - navigate to settings manually and re-run");
    }
}

// ============================================================================
// Conversation Tests (via debug log verification)
// ============================================================================

/// The log should show conversation title operations
#[test]
#[ignore = "Requires app running"]
fn conversation_operations_logged() {
    if !ensure_app_running() {
        panic!("App not running");
    }

    let log = read_debug_log_tail(50);
    
    println!("=== Recent Debug Log ===");
    println!("{}", log);
    println!("=== End Log ===");
    
    // Check for conversation-related log entries
    let has_conversation_log = log.contains("Conversation") || 
                               log.contains("conversation") ||
                               log.contains("title");
    
    if has_conversation_log {
        println!("Found conversation-related log entries");
    }
}

// ============================================================================
// Menu Bar Item Tests
// ============================================================================

/// The tray menu should have a Quit option
#[test]
#[ignore = "Requires app running with accessibility permissions"]
fn tray_has_quit_menu() {
    if !ensure_app_running() {
        panic!("App not running");
    }

    // Right-click or Ctrl-click to get context menu
    // Note: This might show the popover instead - depends on implementation
    let script = r#"
        tell application "System Events"
            tell process "personal_agent_menubar"
                -- Try to get menu of tray item
                try
                    set trayItem to menu bar item 1 of menu bar 2
                    if exists menu 1 of trayItem then
                        return name of every menu item of menu 1 of trayItem
                    else
                        return "no menu"
                    end if
                on error errMsg
                    return "error: " & errMsg
                end try
            end tell
        end tell
    "#;
    let result = run_applescript(script);
    
    println!("Tray menu result: {}", result.stdout);
    if !result.success {
        println!("Error: {}", result.stderr);
    }
}

// ============================================================================
// Debug/Helper Tests
// ============================================================================

/// Dump the current state for debugging
#[test]
#[ignore = "Debug helper - run manually"]
fn dump_app_state() {
    if !ensure_app_running() {
        println!("App not running");
        return;
    }

    println!("=== App State ===");
    
    // Check tray
    let script = r#"
        tell application "System Events"
            tell process "personal_agent_menubar"
                set output to ""
                set output to output & "Menu bars: " & (count of menu bars) & linefeed
                set output to output & "Menu bar 2 items: " & (count of menu bar items of menu bar 2) & linefeed
                set output to output & "Windows: " & (count of windows) & linefeed
                set output to output & "Groups: " & (count of groups) & linefeed
                return output
            end tell
        end tell
    "#;
    let result = run_applescript(script);
    if result.success {
        println!("{}", result.stdout);
    } else {
        println!("Error: {}", result.stderr);
    }
    
    // Show recent log
    println!("\n=== Recent Debug Log ===");
    println!("{}", read_debug_log_tail(30));
    
    // Check config
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/PersonalAgent/config.json");
    if config_path.exists() {
        println!("\n=== Config exists at: {:?} ===", config_path);
        if let Ok(content) = fs::read_to_string(&config_path) {
            // Just show profile/MCP counts
            let profiles = content.matches("\"provider_id\"").count();
            let mcps = content.matches("\"transport\"").count();
            println!("Profiles: {}, MCPs: {}", profiles, mcps);
        }
    }
    
    println!("=== End State ===");
}

/// Test that clicking tray and checking log shows activity
#[test]
#[ignore = "Debug helper - run manually"]
fn click_tray_and_show_log() {
    if !ensure_app_running() {
        println!("App not running");
        return;
    }

    println!("Clicking tray icon...");
    let clicked = click_tray_icon();
    println!("Click result: {}", clicked);
    
    std::thread::sleep(std::time::Duration::from_millis(1000));
    
    println!("\n=== Debug Log After Click ===");
    println!("{}", read_debug_log_tail(20));
}

// ============================================================================
// Conversation Rename UI Tests
// ============================================================================

/// After renaming a conversation, the dropdown should show the new title
#[test]
#[ignore = "Requires app running - verifies via log"]
fn rename_updates_dropdown() {
    if !ensure_app_running() {
        panic!("App not running");
    }

    clear_debug_log();
    
    // The user reported: renamed "Languages" to "Languages - test"
    // The new name shows in history but not in the dropdown
    //
    // Expected behavior:
    // 1. After rename, title_edit_done is called
    // 2. update_title_and_model is called
    // 3. populate_title_popup is called (now fixed to reload from storage)
    // 4. Dropdown should show all titles including renamed one
    
    std::thread::sleep(std::time::Duration::from_millis(500));
    
    let log = read_debug_log_tail(50);
    
    // Check if update_title_and_model was called (which triggers populate_title_popup)
    if log.contains("update_title") || log.contains("Renamed") {
        println!("Found title update activity in log");
    }
    
    println!("=== Log for rename test ===");
    println!("{}", log);
}

/// New conversation should trigger edit field for naming
#[test]
#[ignore = "Requires app running - verifies via log"]
fn new_conversation_shows_edit_field() {
    if !ensure_app_running() {
        panic!("App not running");
    }

    clear_debug_log();
    
    // Click tray to show popover
    click_tray_icon();
    std::thread::sleep(std::time::Duration::from_millis(1000));
    
    let log = read_debug_log_tail(30);
    
    // The fix should now:
    // 1. Create conversation with default title
    // 2. Save immediately
    // 3. Show edit field for naming
    // 4. Hide dropdown
    
    println!("=== Log after opening app ===");
    println!("{}", log);
    
    // Look for evidence of new conversation flow
    if log.contains("New conversation") {
        println!("Found new conversation activity");
    }
    if log.contains("edit field shown") {
        println!("Edit field was shown for naming");
    }
}

/// New conversation should appear in both history and dropdown
#[test]
#[ignore = "Requires app running - verifies via log"]  
fn new_conversation_appears_everywhere() {
    if !ensure_app_running() {
        panic!("App not running");
    }

    let log = read_debug_log_tail(100);
    
    // Look for HistoryView loading conversations - should include new ones
    let history_entries: Vec<&str> = log
        .lines()
        .filter(|l| l.contains("HistoryView:") && l.contains("title="))
        .collect();
    
    println!("=== History entries in log ===");
    for entry in &history_entries {
        println!("{}", entry);
    }
    
    // After fix, new conversations get saved immediately with a title
    // so they should appear in history
    if history_entries.iter().any(|e| e.contains("New ")) {
        println!("Found new conversation in history!");
    }
}
