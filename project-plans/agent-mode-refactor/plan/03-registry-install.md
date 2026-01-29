# Phase 03: MCP Registry Install

**Plan:** PLAN-20250128-AGENT
**Phase:** P03
**Prerequisites:** P02 evidence with PASS
**Subagent:** rustexpert

---

## Objective

`McpRegistryService.install()` is a stub. Implement it so tests can install MCPs via service layer.

---

## Current Code (STUB)

**File:** `src/services/mcp_registry_impl.rs` lines 265-277

```rust
async fn install(&self, name: &str, _config_name: Option<String>) -> ServiceResult<()> {
    let details = self.get_details(name).await?
        .ok_or_else(|| ServiceError::NotFound(...))?;

    // TODO: Add the server configuration to the MCP service
    // For now, this is a stub
    let _ = details;

    Ok(())  // DOES NOTHING
}
```

---

## Implementation

Replace with:

```rust
async fn install(&self, name: &str, config_name: Option<String>) -> ServiceResult<()> {
    // 1. Get MCP details from registry
    let details = self.get_details(name).await?
        .ok_or_else(|| ServiceError::NotFound(format!("MCP '{}' not found in registry", name)))?;

    // 2. Find npm package (most common type)
    let npm_package = details.packages.iter()
        .find(|p| p.registry_type == "npm")
        .ok_or_else(|| ServiceError::NotFound(
            format!("No npm package found for MCP '{}'", name)
        ))?;

    // 3. Build command: npx -y @package/name
    let command = "npx".to_string();
    let args = vec!["-y".to_string(), npm_package.name.clone()];

    // 4. Create config for McpService
    let display_name = config_name.unwrap_or_else(|| details.name.clone());
    
    let config = crate::mcp::McpConfig {
        id: uuid::Uuid::new_v4(),
        name: display_name.clone(),
        command,
        args,
        env: std::collections::HashMap::new(),
        enabled: true,
    };

    // 5. Add to McpService (this persists to disk)
    let mcp_service = crate::mcp::McpService::global();
    let mut mcp = mcp_service.lock().await;
    
    mcp.add_config(config).await
        .map_err(|e| ServiceError::Internal(format!("Failed to add MCP config: {}", e)))?;

    // 6. Optionally start immediately
    mcp.start(&display_name).await
        .map_err(|e| ServiceError::Internal(format!("Failed to start MCP: {}", e)))?;

    Ok(())
}
```

---

## If McpService.add_config() Doesn't Exist

Check if it exists:
```bash
grep -n "add_config\|add_mcp" src/mcp/service.rs
```

If not, add it to `src/mcp/service.rs`:

```rust
impl McpService {
    /// Add a new MCP configuration and persist to disk
    pub async fn add_config(&mut self, config: McpConfig) -> Result<(), String> {
        // Load current app config
        let config_path = crate::config::Config::default_path()
            .map_err(|e| e.to_string())?;
        let mut app_config = crate::config::Config::load(&config_path)
            .map_err(|e| e.to_string())?;
        
        // Add new MCP
        app_config.mcps.push(config.clone());
        
        // Save to disk
        app_config.save(&config_path)
            .map_err(|e| e.to_string())?;
        
        // Add to runtime list
        self.configs.push(config);
        
        Ok(())
    }
    
    /// Start a specific MCP by name
    pub async fn start(&mut self, name: &str) -> Result<(), String> {
        let config = self.configs.iter()
            .find(|c| c.name == name)
            .ok_or_else(|| format!("MCP '{}' not found", name))?
            .clone();
        
        self.runtime.start_mcp(&config).await
    }
}
```

---

## Verification Commands (BLOCKING)

### Check 1: No TODO/stub in install
```bash
grep -A20 "async fn install" src/services/mcp_registry_impl.rs | grep -E "TODO|stub|let _ ="
```
**Expected:** NO OUTPUT (stub code removed)

### Check 2: install calls add_config
```bash
grep -A20 "async fn install" src/services/mcp_registry_impl.rs | grep "add_config"
```
**Expected:** At least one match

### Check 3: install extracts npm package
```bash
grep -A20 "async fn install" src/services/mcp_registry_impl.rs | grep "npm"
```
**Expected:** At least one match

### Check 4: Build passes
```bash
cargo build --all-targets 2>&1 | tail -5
```

### Check 5: Existing tests pass
```bash
cargo test --lib 2>&1 | grep -E "^test result"
```

---

## Deliverables

1. `McpRegistryServiceImpl.install()` actually installs MCPs
2. `McpService.add_config()` exists (add if needed)
3. No stubs or TODOs in install path
4. Evidence file at `plan/.completed/P03.md`
