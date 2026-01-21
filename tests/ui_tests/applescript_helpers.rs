//! AppleScript helpers for UI automation testing
//!
//! This module provides utilities to run AppleScript commands from Rust tests
//! to automate and verify macOS UI behavior.

use std::process::Command;

/// Result of running an AppleScript
#[derive(Debug)]
pub struct AppleScriptResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Run an AppleScript and return the result
pub fn run_applescript(script: &str) -> AppleScriptResult {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .expect("Failed to execute osascript");

    AppleScriptResult {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    }
}

/// Run a multi-line AppleScript (each line as separate -e argument)
pub fn run_applescript_lines(lines: &[&str]) -> AppleScriptResult {
    let mut cmd = Command::new("osascript");
    for line in lines {
        cmd.arg("-e").arg(*line);
    }

    let output = cmd.output().expect("Failed to execute osascript");

    AppleScriptResult {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    }
}

/// Check if an application is running
pub fn is_app_running(app_name: &str) -> bool {
    let script = format!(
        r#"tell application "System Events" to (name of processes) contains "{}""#,
        app_name
    );
    let result = run_applescript(&script);
    result.success && result.stdout == "true"
}

/// Get the frontmost window title of an application
pub fn get_window_title(app_name: &str) -> Option<String> {
    let script = format!(
        r#"tell application "System Events"
            tell process "{}"
                if (count of windows) > 0 then
                    return name of front window
                else
                    return ""
                end if
            end tell
        end tell"#,
        app_name
    );
    let result = run_applescript(&script);
    if result.success && !result.stdout.is_empty() {
        Some(result.stdout)
    } else {
        None
    }
}

/// Click a UI element by description (button, menu item, etc.)
pub fn click_element(app_name: &str, element_path: &str) -> bool {
    let script = format!(
        r#"tell application "System Events"
            tell process "{}"
                click {}
            end tell
        end tell"#,
        app_name, element_path
    );
    let result = run_applescript(&script);
    result.success
}

/// Get the value of a text field
pub fn get_text_field_value(app_name: &str, field_path: &str) -> Option<String> {
    let script = format!(
        r#"tell application "System Events"
            tell process "{}"
                return value of {}
            end tell
        end tell"#,
        app_name, field_path
    );
    let result = run_applescript(&script);
    if result.success {
        Some(result.stdout)
    } else {
        None
    }
}

/// Set the value of a text field
pub fn set_text_field_value(app_name: &str, field_path: &str, value: &str) -> bool {
    let script = format!(
        r#"tell application "System Events"
            tell process "{}"
                set value of {} to "{}"
            end tell
        end tell"#,
        app_name, field_path, value
    );
    let result = run_applescript(&script);
    result.success
}

/// Get count of UI elements matching a description
pub fn count_elements(app_name: &str, element_type: &str, container_path: &str) -> usize {
    let script = format!(
        r#"tell application "System Events"
            tell process "{}"
                return count of {} of {}
            end tell
        end tell"#,
        app_name, element_type, container_path
    );
    let result = run_applescript(&script);
    if result.success {
        result.stdout.parse().unwrap_or(0)
    } else {
        0
    }
}

/// Get the title/value of a popup button (combo box)
pub fn get_popup_value(app_name: &str, popup_path: &str) -> Option<String> {
    let script = format!(
        r#"tell application "System Events"
            tell process "{}"
                return value of {}
            end tell
        end tell"#,
        app_name, popup_path
    );
    let result = run_applescript(&script);
    if result.success {
        Some(result.stdout)
    } else {
        None
    }
}

/// Get all items in a popup menu
pub fn get_popup_items(app_name: &str, popup_path: &str) -> Vec<String> {
    // Click to open the popup, get menu items, then close
    let script = format!(
        r#"tell application "System Events"
            tell process "{}"
                click {}
                delay 0.2
                set menuItems to name of every menu item of menu 1 of {}
                key code 53 -- Escape to close
                return menuItems
            end tell
        end tell"#,
        app_name, popup_path, popup_path
    );
    let result = run_applescript(&script);
    if result.success && !result.stdout.is_empty() {
        // AppleScript returns items as comma-separated list
        result
            .stdout
            .split(", ")
            .map(|s| s.to_string())
            .collect()
    } else {
        Vec::new()
    }
}

/// Check if a specific UI element exists
pub fn element_exists(app_name: &str, element_path: &str) -> bool {
    let script = format!(
        r#"tell application "System Events"
            tell process "{}"
                return exists {}
            end tell
        end tell"#,
        app_name, element_path
    );
    let result = run_applescript(&script);
    result.success && result.stdout == "true"
}

/// Get the entire UI element hierarchy (useful for debugging)
pub fn get_ui_hierarchy(app_name: &str) -> String {
    let script = format!(
        r#"tell application "System Events"
            tell process "{}"
                return entire contents of front window
            end tell
        end tell"#,
        app_name
    );
    let result = run_applescript(&script);
    if result.success {
        result.stdout
    } else {
        format!("Error: {}", result.stderr)
    }
}

/// Wait for an element to appear (with timeout)
pub fn wait_for_element(app_name: &str, element_path: &str, timeout_secs: u32) -> bool {
    let script = format!(
        r#"tell application "System Events"
            tell process "{}"
                set maxTime to {} 
                set startTime to current date
                repeat while ((current date) - startTime) < maxTime
                    if exists {} then
                        return true
                    end if
                    delay 0.1
                end repeat
                return false
            end tell
        end tell"#,
        app_name, timeout_secs, element_path
    );
    let result = run_applescript(&script);
    result.success && result.stdout == "true"
}
