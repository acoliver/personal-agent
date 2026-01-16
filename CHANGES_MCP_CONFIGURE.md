# MCP Configure Flow - Changes Summary

## Overview
Fixed the MCP configuration flow to properly handle authentication and environment variables from registry search results. Previously, registry MCPs were saved without configuration, and the configure view didn't show env_vars from the registry.

## Changes Made

### 1. `src/ui/mcp_add_view.rs`

#### Added Import
- Added `McpAuthType` to imports

#### Modified `result_clicked` Handler (lines ~432-488)
**Before:** Always saved MCP directly when clicking a registry search result, skipping configuration entirely.

**After:** Now checks if configuration is needed before saving:
```rust
// Check if configuration is needed
let needs_config = !mcp_config.env_vars.is_empty() 
    || mcp_config.auth_type != McpAuthType::None;

if needs_config {
    // Store config and navigate to configure view
    SELECTED_MCP_CONFIG.with(|cell| {
        *cell.borrow_mut() = Some(mcp_config);
    });
    // Navigate to PersonalAgentShowConfigureMcp
} else {
    // No config needed - save directly
}
```

### 2. `src/ui/mcp_configure_view.rs`

#### Added Imports
- Added `EnvVarConfig` to imports

#### Extended `McpConfigureViewIvars` Struct
Added new fields:
- `selected_config: RefCell<Option<McpConfig>>` - Stores the registry config
- `form_stack: RefCell<Option<Retained<super::FlippedStackView>>>` - Reference to form for dynamic fields
- `env_var_inputs: RefCell<Vec<(String, Retained<NSTextField>)>>` - Dynamic env var input fields
- `auth_section: RefCell<Option<Retained<NSView>>>` - Reference to auth section (for hiding when using env vars)

#### Modified `loadView` Method
**Before:** Would immediately save registry configs without showing any UI, or only showed manual URL entry form.

**After:** 
- Takes `selected_config` from thread-local
- Stores it in ivars for use throughout the view
- Lets the form build continue with the config data
- Removed the automatic save-and-exit logic

#### Modified `build_form_scroll` Method
Enhanced to handle both registry configs and manual entry:
- Pre-populates name from `selected_config` if available
- Checks if config has `env_vars`
- If env_vars present, calls `build_env_var_fields()` instead of showing auth section
- Otherwise shows standard auth section (for manual entry or configs without env_vars)

#### Added `build_env_var_fields` Method (New)
Creates dynamic input fields for each environment variable:
- Displays label with "(required)" or "(optional)" indicator
- Creates text field for each env var
- Detects likely secrets (names containing "key", "secret", "token", "password", "pat")
- Stores field references in `env_var_inputs` for later retrieval
- Adds logging for debugging

#### Completely Rewrote `save_clicked` Method
Now handles two paths:
1. **Registry config path** (when `selected_config` is present):
   - Collects values from dynamic env var fields
   - Validates required fields
   - Detects and stores secrets using `SecretsManager::store_api_key_named()`
   - Stores non-secret values in `config.config` JSON
   - Uses the pre-populated package/source info from registry

2. **Manual entry path** (when only `parsed_mcp` is present):
   - Uses auth type popup
   - Stores API key via `SecretsManager::store_api_key()` (single key)
   - Builds McpConfig from parsed data

#### Updated `new()` Constructor
Added initialization for all new ivars:
```rust
selected_config: RefCell::new(None),
form_stack: RefCell::new(None),
env_var_inputs: RefCell::new(Vec::new()),
auth_section: RefCell::new(None),
```

## How It Works Now

### Registry MCP with Env Vars (e.g., GitHub)
1. User searches registry, clicks result
2. `result_clicked` detects `!env_vars.is_empty()` → needs_config = true
3. Stores config in `SELECTED_MCP_CONFIG` thread-local
4. Navigates to configure view
5. `loadView` pulls from thread-local, stores in `selected_config`
6. `build_form_scroll` sees env_vars, calls `build_env_var_fields()`
7. User fills in fields (e.g., GITHUB_TOKEN)
8. `save_clicked` collects values, detects secrets, stores via SecretsManager
9. Saves McpConfig with env_values in `config.config`

### Registry MCP without Env Vars
1. User searches registry, clicks result
2. `result_clicked` detects `env_vars.is_empty() && auth_type == None` → needs_config = false
3. Saves directly, goes to settings (existing fast path)

### Manual URL Entry
1. User enters URL, clicks Next
2. Goes to configure view (existing path)
3. `selected_config` is None, uses `parsed_mcp`
4. Shows standard auth section
5. Saves with manual-entry logic (existing behavior)

## Secret Storage
Secrets are now stored using the existing `SecretsManager`:
- Named storage: `mcp_{uuid}_{VAR_NAME}.key`
- File permissions: 0o600 (owner read/write only)
- Detection: Variable name contains "key", "secret", "token", "password", or "pat" (case-insensitive)

## Files Modified
1. `/Users/acoliver/projects/personalAgent/src/ui/mcp_add_view.rs`
2. `/Users/acoliver/projects/personalAgent/src/ui/mcp_configure_view.rs`

## Build Status
[OK] Successfully compiled with `cargo build --bin personal_agent_menubar`

## Testing Checklist
- [ ] Test registry MCP with required env vars (e.g., GitHub with GITHUB_TOKEN)
- [ ] Test registry MCP with multiple env vars
- [ ] Test registry MCP with optional env vars
- [ ] Test registry MCP with no env vars (should save directly)
- [ ] Test manual URL entry (existing flow)
- [ ] Verify secrets are stored with correct file names and permissions
- [ ] Verify required field validation works
- [ ] Verify non-secret fields are stored in config.config JSON
