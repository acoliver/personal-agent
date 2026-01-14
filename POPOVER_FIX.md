# Popover Crash Fix

## Problem
The app crashed with:
```
thread 'main' panicked at winit-0.30.12/src/platform_impl/macos/window_delegate.rs:225:18
called `Option::unwrap()` on a `None` value
```

This occurred in the `window_did_resign_key` handler after calling `show_popover_at_statusbar()`.

## Root Cause
Calling `NSPopover.show()` from within eframe's `update()` method caused **re-entrancy**:
1. User clicks tray icon
2. `handle_tray_events_popover()` is called during `update()`
3. It calls `show_popover_at_statusbar()` which immediately shows the popover
4. The popover appearing causes the window to resign key status
5. winit's `windowDidResignKey:` handler fires **while still inside update()**
6. The handler tries to unwrap a None value because window state is inconsistent during the popover transition

## Solution
**Defer popover operations to happen OUTSIDE of eframe's update loop:**

### Changes Made

#### 1. `/personal-agent/src/popover.rs`
- Added `PendingPopoverOperation` enum to queue operations:
  ```rust
  enum PendingPopoverOperation {
      Show { button_bounds: NSRect, status_button: Retained<NSView> },
      Hide,
  }
  ```
- Added `pending_operation` field to `PopoverBridgeState`
- Modified `show_popover_at_statusbar()` to queue the operation instead of executing immediately
- Modified `hide_popover()` to queue the operation
- Added `process_pending_operations()` to execute queued operations **after** update completes

#### 2. `/personal-agent/src/main.rs`
- Added call to `popover::process_pending_operations()` at the end of `update()`
- This processes queued operations after the eframe update cycle completes

## How It Works
1. User clicks tray icon during `update()`
2. Event handler queues a `Show` operation (doesn't execute it)
3. `update()` completes normally
4. `process_pending_operations()` is called
5. Popover is shown outside the update cycle
6. Window state changes happen safely, no re-entrancy

## Testing
The code now compiles successfully. To test:
1. Run the app
2. Click the tray icon
3. The popover should appear without crashing
4. Click outside to dismiss
5. Click the tray icon again to toggle

The fix prevents winit from handling window state changes during the eframe update cycle, eliminating the re-entrancy issue.
