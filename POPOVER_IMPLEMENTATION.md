# Native NSPopover Implementation Status

## Summary
Converted the personal-agent Rust application from manual window positioning to using native macOS NSPopover, following the BarTranslate implementation pattern.

## Changes Made

### 1. Dependencies Added (`Cargo.toml`)
- Updated `tray-icon` from `0.19` to `0.21` (needed for `ns_status_item()` method)
- Added `objc2 = "0.6"` for Objective-C runtime
- Added `objc2-app-kit` with features for `NSPopover`, `NSView`, `NSWindow`, `NSStatusBarButton`, `NSStatusItem`
- Added `objc2-foundation` with `NSGeometry` feature
- Added `raw-window-handle = "0.6"` for window handle access
- Changed `unsafe_code = "forbid"` to `unsafe_code = "deny"` (needed for NSPopover FFI calls)

### 2. New Module: `src/popover.rs`
Created a new macOS-only module that provides:

- **Thread-local state management**: Uses `thread_local!` storage for NSPopover state (required because NSPopover is not `Send`)
- **`initialize_popover_state()`**: Initializes the thread-local popover state
- **`setup_native_popover()`**: 
  - Gets the NSWindow from eframe's window handle
  - Creates an NSPopover with proper behavior (Transient)
  - Sets content size to 400x500
  - Hides the original window (it will be shown through the popover)
  - Stores the popover and window references
- **`show_popover_at_statusbar()`**: Shows the popover using `showRelativeToRect_ofView_preferredEdge` (native NSPopover method)
- **`hide_popover()`**: Hides the popover using `performClose`
- **`is_popover_shown()`**: Checks if popover is currently visible

### 3. Main Application Changes (`src/main.rs`)

#### PersonalAgentApp struct:
- Added `popover_initialized: bool` field to track initialization state

#### `new()` method:
- Calls `popover::initialize_popover_state()` on macOS

#### `update()` method:
- On first frame, calls `popover::setup_native_popover(frame)` to initialize the NSPopover
- Routes to platform-specific tray event handlers

#### `handle_tray_events_popover()` (new macOS-only method):
- Gets the NSStatusItem from the tray icon using the new `tray-icon` 0.21 API
- Gets the NSStatusBarButton from the NSStatusItem
- Uses `button.bounds()` to get the button rectangle
- Calls `popover::show_popover_at_statusbar()` with the button bounds and button view
- Toggles between show/hide on clicks

## Current Implementation Details

### Working [OK]
1. **Compilation**: Code compiles without errors
2. **NSPopover creation**: Native NSPopover is created with correct settings
3. **Tray icon integration**: Access to NSStatusBarButton through tray-icon crate
4. **Popover anchoring API**: Using `showRelativeToRect_ofView_preferredEdge` like BarTranslate

### Known Issues / Incomplete WARNING:

1. **Content View Integration**: 
   - Current code gets the window's content view and tries to add it to the popover
   - This approach may not work correctly - egui's window content view cannot be easily reparented
   - **Problem**: The egui window is created by eframe, and we're trying to steal its content view
   
2. **Window Hiding**:
   - We hide the original window with `setAlphaValue(0.0)` and `orderOut(None)`
   - But the egui content is still tied to that window
   
3. **Proper Approach Needed**:
   - We may need to create a separate NSView that the popover owns
   - Then tell eframe to render into that view instead of its own window
   - Or we need to use a different eframe initialization approach that doesn't create a window

## Correct Implementation Path (TODO)

The proper way to do this (based on BarTranslate's approach) is:

1. **Create NSView for popover content**:
   ```swift
   // BarTranslate does:
   let popover = NSPopover()
   popover.contentViewController = NSHostingController(rootView: contentView)
   ```

2. **In Rust/egui, we need to**:
   - Create a custom NSView
   - Tell eframe to render into that NSView instead of creating a window
   - OR use eframe in a mode where it doesn't manage the window
   - OR use raw egui (not eframe) and create our own rendering context

3. **Possible Solutions**:
   
   **Option A**: Use eframe's viewport builder to render into a custom NSView
   - Create the NSView for the popover
   - Pass it to eframe somehow (may not be directly supported)
   
   **Option B**: Use raw egui + winit + wgpu
   - More control over window/view creation
   - Can create a view for the popover and render egui into it
   - More complex integration
   
   **Option C**: Keep the window hidden, show popover as overlay
   - Hide the eframe window completely
   - Use the popover to show a separate SwiftUI or AppKit view
   - Use inter-process communication to sync state
   - Not ideal

## Testing Status

- **Build**: [OK] Compiles successfully
- **Runtime**: WARNING: Not yet tested - likely the content view reparenting will not work correctly
- **Visual**: Need to verify popover appears with arrow anchored to menu bar icon

## Next Steps

1. Test run the application and observe behavior
2. If content view doesn't display properly (likely):
   - Investigate eframe's window creation options
   - Research if eframe supports rendering into an existing NSView
   - Consider using raw egui rendering instead of eframe
3. Ensure the popover shows with proper arrow alignment to menu bar icon
4. Test popover dismissal (clicking outside, escape key)
5. Add proper error handling and logging

## Reference Implementation
- Swift code: `/Users/acoliver/projects/personalAgent/research/BarTranslate/BarTranslate/BarTranslateApp.swift`
- Lines 114-152, specifically the `togglePopover` method at lines 138-152

## Files Modified
- `personal-agent/Cargo.toml` - Dependencies and unsafe_code lint
- `personal-agent/src/main.rs` - Added popover integration
- `personal-agent/src/popover.rs` - New module for NSPopover bridging

## Testing the Implementation

To test:
```bash
cd personal-agent
cargo run
```

Expected behavior:
- App should start with menu bar icon
- Clicking icon should attempt to show NSPopover
- Check logs for any errors about popover initialization or display

Likely issues to debug:
- Content view may not display inside popover (needs different approach)
- Popover may show but be empty
- Popover arrow alignment may need adjustment
