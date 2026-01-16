# OAuth UI Implementation for MCP Configure View

## Summary

Successfully implemented the UI portion of the Smithery OAuth flow in `src/ui/mcp_configure_view.rs`.

## Changes Made

### 1. Updated Ivars
Added two new instance variables to track OAuth UI elements:
- `oauth_button: RefCell<Option<Retained<NSButton>>>` - The "Connect with Smithery" button
- `oauth_status_label: RefCell<Option<Retained<NSTextField>>>` - Status label showing connection state

### 2. OAuth Detection in loadView
Modified the form building logic to detect `McpAuthType::OAuth` and show the OAuth section instead of standard auth fields:

```rust
if config.auth_type == McpAuthType::OAuth {
    // Show OAuth section
    let oauth_section = self.build_oauth_section(mtm);
    form_stack.addArrangedSubview(&oauth_section);
}
```

### 3. OAuth UI Section (build_oauth_section)
Created a new method that builds:
- Section label ("Authentication")
- Status label (shows "Not connected" or "Connected" based on presence of `oauth_token`)
- "Connect with Smithery" button
- Informational text

The button is disabled if already connected.

### 4. OAuth Flow Handler (connect_smithery_clicked)
Implemented the OAuth flow:
1. Extracts qualified name from config (e.g., "@owner/server-name")
2. Updates UI to show "Connecting..." and disables button
3. Spawns background thread with tokio runtime
4. Starts local callback server on random port
5. Generates Smithery OAuth URL
6. Opens browser with `open` command (macOS)
7. Waits for callback with 5-minute timeout
8. Saves token to config on success
9. Posts notification to update UI

### 5. Notification Observer
Added notification observer in `loadView` to listen for "PersonalAgentOAuthSuccess" notifications.

### 6. Success Handler (oauth_success_notification)
Updates UI when OAuth succeeds:
- Changes status label to "Connected"
- Disables button and changes text to "Already Connected"

### 7. Helper Function
Added `save_oauth_token()` function to save the token to the MCP config file.

### 8. Module Exports
Updated `src/mcp/mod.rs` to export OAuth-related types and functions:
- `OAuthCallbackResult`
- `SmitheryOAuthConfig`
- `start_oauth_callback_server`
- `generate_smithery_oauth_url`

## User Flow

1. User adds a Smithery MCP (e.g., Filesystem) from the registry
2. Configure view shows "Connect with Smithery" button instead of standard auth fields
3. User clicks button → browser opens to Smithery authorization page
4. User authorizes → redirected back to localhost
5. Token is saved and UI updates to show "Connected"
6. MCP can now be used with OAuth token

## Files Modified

- `src/ui/mcp_configure_view.rs` - Main implementation
- `src/mcp/mod.rs` - Export OAuth functions
- `src/ui/theme.rs` - Already had necessary color functions

## Testing

Build successful with:
```bash
cargo build --bin personal_agent_menubar
```

## Next Steps

To fully test the OAuth flow:
1. Run the menubar app
2. Add a Smithery MCP that requires OAuth
3. Click "Connect with Smithery"
4. Verify browser opens
5. Complete authorization
6. Verify UI updates and token is saved

## Notes

- OAuth callback server uses a random available port
- 5-minute timeout for user to complete authorization
- Token is stored in the MCP config JSON file
- UI is updated via NSNotificationCenter to handle cross-thread communication
