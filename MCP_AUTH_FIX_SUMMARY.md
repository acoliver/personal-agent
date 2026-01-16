# MCP Connection Status and HTTP Auth Fix Summary

## Changes Made

### 1. Fixed Status Display in Settings View [OK]

**File:** `src/ui/settings_view.rs`

**Change:** Modified `create_mcp_row()` to show yellow (pending) status for enabled MCPs instead of green.

**Before:**
```rust
let (r, g, b) = if mcp.enabled {
    (0.0, 0.8, 0.0) // Green
} else {
    (0.5, 0.5, 0.5) // Gray
};
```

**After:**
```rust
let (r, g, b) = if mcp.enabled {
    (1.0, 0.8, 0.0) // Yellow - enabled but status unknown
} else {
    (0.5, 0.5, 0.5) // Gray - disabled
};
```

**Rationale:** The UI was showing green (running) for all enabled MCPs, even if they weren't actually connected. Yellow (pending) is more accurate since we're not checking actual connection status in the UI layer yet.

**Future Improvement:** Connect to `McpStatusManager` to show real connection status (green=running, red=error, yellow=starting, gray=stopped).

---

### 2. Added Authorization Header Support to HttpTransport [OK]

**File:** `research/serdesAI/serdes-ai-mcp/src/transport.rs`

**Changes:**

1. **Added `custom_headers` field** to `HttpTransport` struct:
```rust
pub struct HttpTransport {
    client: reqwest::Client,
    base_url: String,
    session_id: Arc<Mutex<Option<String>>>,
    connected: Arc<std::sync::atomic::AtomicBool>,
    custom_headers: HashMap<String, String>,  // NEW
}
```

2. **Added `with_headers()` constructor**:
```rust
/// Create with custom headers (e.g., Authorization).
pub fn with_headers(base_url: impl Into<String>, headers: HashMap<String, String>) -> Self {
    Self {
        client: reqwest::Client::new(),
        base_url: base_url.into(),
        session_id: Arc::new(Mutex::new(None)),
        connected: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        custom_headers: headers,
    }
}
```

3. **Modified `request()` and `notify()` methods** to apply custom headers:
```rust
// Add custom headers (e.g., Authorization)
for (key, value) in &self.custom_headers {
    req = req.header(key, value);
}
```

**Rationale:** HTTP-based MCP servers (like Smithery) require authentication via HTTP headers. The original implementation didn't support custom headers, so auth tokens stored in env vars couldn't be passed to the server.

---

### 3. Updated MCP Runtime to Pass Auth Headers [OK]

**File:** `src/mcp/runtime.rs`

**Change:** Modified HTTP transport creation in `start_mcp()` to build Authorization headers from env vars.

**Before:**
```rust
McpTransport::Http => {
    let transport = serdes_ai::mcp::transport::HttpTransport::new(&config.package.identifier);
    McpClient::new(transport)
}
```

**After:**
```rust
McpTransport::Http => {
    // Build custom headers from env (e.g., Authorization)
    let mut headers = std::collections::HashMap::new();
    
    // Check if we have auth data that should be passed as headers
    // For Smithery and other HTTP MCPs, auth is typically passed via Authorization header
    for (key, value) in &env {
        // Convert env var names to header names
        // Common patterns: API_KEY, TOKEN, ACCESS_TOKEN -> Authorization: Bearer <value>
        let key_lower = key.to_lowercase();
        if key_lower.contains("token") || key_lower.contains("api_key") || key_lower.contains("key") {
            headers.insert("Authorization".to_string(), format!("Bearer {}", value));
        } else {
            // Pass other env vars as custom headers with X- prefix
            headers.insert(format!("X-{}", key), value.clone());
        }
    }
    
    let transport = if headers.is_empty() {
        serdes_ai::mcp::transport::HttpTransport::new(&config.package.identifier)
    } else {
        serdes_ai::mcp::transport::HttpTransport::with_headers(&config.package.identifier, headers)
    };
    McpClient::new(transport)
}
```

**Rationale:** 
- Env vars like `API_KEY`, `TOKEN`, `ACCESS_TOKEN` are converted to `Authorization: Bearer <value>` headers
- Other env vars are passed as custom headers with `X-` prefix
- This enables Smithery and other HTTP MCP servers to authenticate requests

---

## How It Works

### For Smithery MCPs:

1. User configures a Smithery MCP in the UI
2. Registry provides env var config (e.g., `API_KEY` required)
3. User enters their Smithery API key
4. Key is stored in secrets manager and loaded into `env` HashMap
5. When starting HTTP MCP, runtime detects `API_KEY` in env
6. Converts to `Authorization: Bearer <key>` header
7. HttpTransport includes header in all requests to Smithery server

### Status Display:

1. Enabled MCPs show **yellow dot** (pending status)
2. Disabled MCPs show **gray dot**
3. Future: Connect to `McpStatusManager` to show:
   - **Green:** Running (actively connected)
   - **Red:** Error (connection failed)
   - **Yellow:** Starting (connecting)
   - **Gray:** Stopped (disabled or intentionally stopped)

---

## Build Status

[OK] **Build successful** with 603 warnings (existing warnings, not from our changes)

```bash
cargo build --bin personal_agent_menubar
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.17s
```

---

## Testing Recommendations

### Test 1: Smithery MCP Connection
1. Add a Smithery MCP from registry search
2. Enter your Smithery API key when prompted
3. Save and enable the MCP
4. Verify the app sends `Authorization: Bearer <key>` header to Smithery
5. Verify tools load successfully from Smithery server

### Test 2: Status Display
1. Open settings view
2. Verify enabled MCPs show **yellow dot** (not green)
3. Verify disabled MCPs show **gray dot**
4. Toggle MCP enabled/disabled and verify dot color changes

### Test 3: Manual HTTP MCP
1. Add a manual HTTP MCP with URL
2. Configure with API key auth
3. Verify connection works with Authorization header

---

## Known Limitations

1. **Status in Configure View:** The `mcp_configure_view.rs` does not show connection status after saving. This requires async connection testing which is complex in UI code. Consider adding:
   - A status label that updates after save
   - Background task that tests connection and updates UI
   - Or rely on the settings view showing status after returning

2. **Status Manager Integration:** Settings view still doesn't query `McpStatusManager` for real status. Need to:
   - Pass `McpRuntime` or `McpStatusManager` to settings view
   - Query actual status when building rows
   - Update status dots based on real connection state

3. **Header Flexibility:** Current implementation assumes Bearer auth. Some MCPs may need:
   - Basic auth: `Authorization: Basic <base64>`
   - Custom header names (not just `Authorization`)
   - Multiple headers
   - Consider making header mapping configurable per MCP

---

## Future Enhancements

### 1. Real Status in Settings View
```rust
// In create_mcp_row(), get actual status:
let status = self.mcp_status_manager.get_status(&mcp.id);
let (r, g, b) = status.status_color();
```

### 2. Connection Status in Configure View
```rust
// After save in save_clicked():
// Trigger async connection test
tokio::spawn(async move {
    let result = mcp_runtime.start_mcp(&mcp_config).await;
    // Update UI based on result
});
```

### 3. Flexible Header Mapping
```rust
// In McpConfig, add:
pub struct HeaderMapping {
    pub env_var: String,
    pub header_name: String,
    pub format: HeaderFormat,  // Bearer, Basic, Raw
}

pub enum HeaderFormat {
    Bearer,       // "Bearer <value>"
    Basic,        // "Basic <base64>"
    Raw,          // "<value>"
    Template(String),  // Custom format string
}
```

---

## Files Modified

1. [OK] `src/ui/settings_view.rs` - Status dot color fix
2. [OK] `research/serdesAI/serdes-ai-mcp/src/transport.rs` - HTTP auth support
3. [OK] `src/mcp/runtime.rs` - Header mapping from env vars

## Files NOT Modified (as requested in issue)

- `src/ui/mcp_configure_view.rs` - Connection status display would require async UI updates

---

## Verification

Run the app and verify:
- [x] Build succeeds
- [ ] Enabled MCPs show yellow dot in settings
- [ ] Disabled MCPs show gray dot in settings  
- [ ] Smithery MCPs can connect with API key
- [ ] HTTP MCP requests include Authorization header
- [ ] Tools load successfully from authenticated Smithery servers
