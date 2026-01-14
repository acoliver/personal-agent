# Tray Icon Visibility Debugging Guide

## Problem

The personal-agent app runs successfully and logs show "Tray icon created successfully" and "Popover initialized successfully", but:
- No tray icon appears in the macOS menu bar
- Nothing happens when clicking where the icon should be
- The process runs but is essentially invisible

## Root Causes to Investigate

1. **Template Mode Too Subtle**: The icon uses `.with_icon_as_template(true)`, which renders as monochrome and may blend into the menu bar background
2. **Icon Creation Silent Failure**: The icon might "succeed" in creation but not actually appear in the menu bar
3. **NSStatusItem Not Properly Added**: The tray-icon crate might not be properly registering with the macOS status bar
4. **Event Loop Issues**: eframe's event loop might not be processing tray icon events properly
5. **macOS Permissions**: The app might lack proper entitlements or permissions

## Debug Tools

Three scripts have been created to diagnose and fix the issue:

### 1. Quick Visibility Check (Fastest)

```bash
./check_tray_visibility.sh
```

**What it does:**
- Checks if icon file exists and is valid PNG
- Verifies image properties (dimensions, alpha channel)
- Checks if template mode is enabled
- Verifies process is running
- Analyzes recent logs for errors

**When to use:** First step - quick diagnosis before deeper testing

### 2. Debug Build Runner (Recommended)

```bash
./run_debug_build.sh
```

**What it does:**
- Temporarily replaces main.rs with enhanced debug version
- Disables template mode (full color icon, more visible)
- Adds extensive logging at every step
- Verifies NSStatusItem is actually created
- Logs all tray icon click events
- Runs in foreground with live log output
- Automatically restores original main.rs when closed

**When to use:** When quick check shows potential issues, or icon is not visible

**Key features of debug version:**
- Non-template mode: `.with_icon_as_template(false)` - icon appears in full color
- Verification logging: Confirms NSStatusItem and NSStatusBarButton creation
- Click event logging: Loud messages when icon is clicked
- Frame-by-frame updates: Shows what's happening during initialization

### 3. Comprehensive Test Script (Most Thorough)

```bash
./debug_tray_icon.sh
```

**What it does:**
- Kills existing processes
- Builds both normal and non-template versions
- Runs with RUST_LOG=trace
- Interactive visibility test (asks if you can see the icon)
- Interactive click test (asks if clicks work)
- Monitors logs in real-time
- Provides recommendations based on results
- Checks system logs for permission issues

**When to use:** When debug build runner shows the icon but issues persist, or for comprehensive analysis

## Recommended Debugging Workflow

### Step 1: Quick Check
```bash
./check_tray_visibility.sh
```

Look for:
- [OK] Icon file exists and is valid
-  Template mode is ENABLED (this is likely the issue)
- [OK] Process is running

### Step 2: Run Debug Build
```bash
./run_debug_build.sh
```

Watch for these log messages:
```
[OK] Tray icon created successfully!
[OK] NSStatusItem obtained from tray icon
[OK] NSStatusBarButton obtained
[OK] Popover initialized successfully
```

**Check your menu bar** (top right corner) - the icon should now be **much more visible** in full color.

**Try clicking the icon** - you should see:
```
>>> TRAY ICON CLICKED! <<<
    Click rect: ...
```

### Step 3: If Still No Icon
```bash
./debug_tray_icon.sh
```

Follow the interactive prompts. The script will:
- Ask if you can see the icon
- Ask if clicks work
- Provide specific recommendations

## Common Issues and Fixes

### Issue 1: Icon Too Subtle (Template Mode)

**Symptoms:**
- Logs show success but no visible icon
- Other menu bar apps work fine

**Fix:**
The debug build automatically disables template mode. If this makes the icon visible, you can permanently fix it by editing main.rs:

```rust
// Change this:
.with_icon_as_template(true)

// To this:
.with_icon_as_template(false)
```

**Trade-off:** Non-template icons don't automatically adapt to light/dark mode, but they're much more visible for debugging.

### Issue 2: NSStatusItem Creation Failure

**Symptoms:**
- Debug logs show: " Failed to get NSStatusItem"
- Icon never appears

**Possible causes:**
- tray-icon crate compatibility issue
- macOS version incompatibility
- Sandboxing restrictions

**Fix:** Check the tray-icon crate version and macOS compatibility in Cargo.toml

### Issue 3: Icon Visible But Clicks Don't Work

**Symptoms:**
- Icon appears in menu bar
- Clicking does nothing
- No "TRAY ICON CLICKED" logs

**Possible causes:**
- Event receiver not working
- NSStatusBarButton not properly configured
- Event loop not processing events

**Debug steps:**
1. Check if menu items work (right-click → Quit)
2. Look for event receiver errors in logs
3. Verify MainThreadMarker is available

### Issue 4: macOS Permissions

**Symptoms:**
- Icon briefly appears then disappears
- System logs show permission denials
- App crashes on startup

**Fix:**
1. Check System Settings > Privacy & Security
2. Look for any permission requests
3. Check system logs: `log show --predicate 'process == "personal_agent"' --last 5m`

## Understanding the Logs

### Successful Initialization
```
=== PersonalAgentApp::new called ===
Loading icon...
Icon data size: 2847 bytes
Icon dimensions: 32x32
[OK] Icon loaded successfully
[OK] Menu created
Building tray icon with settings:
   - Template mode: FALSE (full color, more visible)
[OK] TrayIcon built successfully
[OK] Tray icon created successfully!
[OK] NSStatusItem obtained from tray icon
[OK] NSStatusBarButton obtained
```

### Failed Initialization
```
Loading icon...
 Failed to create tray icon: [error message]
   Icon will NOT be visible in menu bar
```

### Click Event (Working)
```
>>> TRAY ICON CLICKED! <<<
    Click rect: PhysicalPosition { x: 1650, y: 10 }
Popover is hidden, showing it...
[OK] Popover shown
```

### Click Event (Not Working)
```
(no logs when clicking)
```

## Files Created

- `check_tray_visibility.sh` - Quick diagnostic tool
- `run_debug_build.sh` - Debug build runner (recommended)
- `debug_tray_icon.sh` - Comprehensive test script
- `personal-agent/src/main_debug.rs` - Enhanced debug version of main.rs
- `DEBUG_TRAY_ICON.md` - This file

## Temporary Files

The scripts create these temporary files:
- `/tmp/personal_agent_debug.log` - Runtime logs
- `/tmp/personal_agent_debug.log.build` - Build logs
- `personal-agent/src/main.rs.original` - Backup of original main.rs (auto-restored)

## Next Steps After Diagnosis

### If Debug Build Shows Icon Successfully
The issue is template mode. Update main.rs:
```rust
.with_icon_as_template(false) // Changed from true
```

### If Debug Build Still Doesn't Show Icon
Check for:
1. tray-icon crate version compatibility
2. macOS version requirements
3. Alternative tray icon libraries (try `tray_item` instead)

### If Icon Shows But Clicks Don't Work
Focus on event handling:
1. Review `handle_tray_events_popover()` logic
2. Check TrayIconEvent::receiver() implementation
3. Verify NSStatusBarButton event routing

## Clean Up

After debugging, to remove temporary files:
```bash
rm -f /tmp/personal_agent_debug.log*
rm -f personal-agent/src/main.rs.original
```

To remove debug scripts (once issue is fixed):
```bash
rm check_tray_visibility.sh debug_tray_icon.sh run_debug_build.sh
rm personal-agent/src/main_debug.rs
rm DEBUG_TRAY_ICON.md
```

## Additional Resources

- tray-icon crate docs: https://docs.rs/tray-icon
- macOS NSStatusBar docs: https://developer.apple.com/documentation/appkit/nsstatusbar
- eframe window handling: https://docs.rs/eframe

## Questions to Answer During Debugging

1. **Can you see ANY icon in the menu bar after running debug build?**
   - Yes → Template mode was the issue
   - No → NSStatusItem creation problem

2. **Does the Quit menu item work when right-clicking?**
   - Yes → Event system works, focus on click events
   - No → Event receiver broken

3. **Do other menu bar apps work normally?**
   - Yes → Problem is with personal-agent
   - No → System-wide menu bar issue

4. **What macOS version are you running?**
   - Check compatibility with tray-icon crate

5. **Are there any permission prompts or dialogs?**
   - Check System Settings > Privacy & Security
