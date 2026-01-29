# MCP Registry Service Requirements

The MCP Registry Service discovers and fetches MCP server information from external registries (Official MCP Registry and Smithery). This is **separate from McpService** which manages running MCP instances.

---

## Responsibilities

- Search Official MCP registry (registry.modelcontextprotocol.io)
- Search Smithery registry (registry.smithery.ai)
- Fetch MCP package details and metadata
- Parse authentication requirements from registry data
- Cache registry responses

---

## Service Interface

```rust
pub trait McpRegistryService: Send + Sync {
    /// Search for MCPs across registries
    async fn search(
        &self,
        query: &str,
        source: RegistrySource,
    ) -> Result<Vec<McpSearchResult>>;
    
    /// Get detailed information about a specific MCP
    async fn get_details(
        &self,
        source: &McpRegistrySource,
    ) -> Result<McpDetails>;
    
    /// Check if cache is stale
    fn is_cache_stale(&self) -> bool;
    
    /// Clear cached data
    fn clear_cache(&self);
}

/// Which registries to search
pub enum RegistrySource {
    Official,
    Smithery,
    Both,
}
```

---

## Data Model

### Search Result

```rust
pub struct McpSearchResult {
    /// Display name for the MCP
    pub name: String,
    
    /// Unique identifier within the registry
    pub id: String,
    
    /// Short description
    pub description: String,
    
    /// Which registry this came from
    pub source: McpRegistrySource,
    
    /// How to run this MCP
    pub package: McpPackageInfo,
}

pub enum McpRegistrySource {
    /// Official MCP registry
    Official {
        /// Package name (e.g., "server-github")
        name: String,
        /// Version if specified
        version: Option<String>,
    },
    /// Smithery registry
    Smithery {
        /// Qualified name (e.g., "@anthropic/github-mcp")
        qualified_name: String,
    },
}

pub struct McpPackageInfo {
    /// Package type determines how to run
    pub package_type: McpPackageType,
    
    /// The identifier (npm package, docker image, URL)
    pub identifier: String,
    
    /// Runtime hint (e.g., "node", "python")
    pub runtime_hint: Option<String>,
}

pub enum McpPackageType {
    /// npx @scope/package
    Npm,
    /// docker run image
    Docker,
    /// Direct executable
    Binary,
    /// HTTP endpoint (SSE)
    Http,
}
```

### Detailed MCP Info

```rust
pub struct McpDetails {
    /// Basic search result info
    pub base: McpSearchResult,
    
    /// Required environment variables
    pub env_vars: Vec<EnvVarSpec>,
    
    /// Optional configuration schema
    pub config_schema: Option<serde_json::Value>,
    
    /// List of tools provided (if available)
    pub tools: Option<Vec<ToolPreview>>,
    
    /// README or documentation
    pub readme: Option<String>,
    
    /// Repository URL
    pub repository: Option<String>,
    
    /// License
    pub license: Option<String>,
}

pub struct EnvVarSpec {
    /// Environment variable name (e.g., "GITHUB_TOKEN")
    pub name: String,
    
    /// Whether this is required
    pub required: bool,
    
    /// Whether this should be treated as a secret
    pub is_secret: bool,
    
    /// Human-readable description
    pub description: Option<String>,
    
    /// Default value if any
    pub default: Option<String>,
}

pub struct ToolPreview {
    pub name: String,
    pub description: Option<String>,
}
```

---

## Official MCP Registry

### API Endpoint

```
GET https://registry.modelcontextprotocol.io/api/v1/servers
GET https://registry.modelcontextprotocol.io/api/v1/servers/{name}
```

### Search Response Format

```json
{
  "servers": [
    {
      "name": "server-github",
      "description": "GitHub API integration for repositories, issues, and PRs",
      "package": "@modelcontextprotocol/server-github",
      "runtime": "node",
      "env": [
        {
          "name": "GITHUB_TOKEN",
          "required": true,
          "secret": true,
          "description": "GitHub Personal Access Token"
        }
      ]
    }
  ]
}
```

### Parsing Logic

```rust
fn parse_official_result(server: &JsonValue) -> McpSearchResult {
    McpSearchResult {
        name: server["name"].as_str().unwrap_or_default().to_string(),
        id: server["name"].as_str().unwrap_or_default().to_string(),
        description: server["description"].as_str().unwrap_or_default().to_string(),
        source: McpRegistrySource::Official {
            name: server["name"].as_str().unwrap_or_default().to_string(),
            version: None,
        },
        package: McpPackageInfo {
            package_type: McpPackageType::Npm,
            identifier: server["package"].as_str().unwrap_or_default().to_string(),
            runtime_hint: server["runtime"].as_str().map(String::from),
        },
    }
}
```

---

## Smithery Registry

### API Endpoint

```
GET https://registry.smithery.ai/api/v1/search?q={query}
GET https://registry.smithery.ai/api/v1/servers/{qualified_name}
```

### Search Response Format

```json
{
  "results": [
    {
      "qualifiedName": "@anthropic/github-mcp",
      "displayName": "GitHub MCP",
      "description": "Alternative GitHub integration",
      "deploymentType": "npm",
      "npmPackage": "@anthropic/github-mcp",
      "envVars": [
        {
          "name": "GITHUB_PAT",
          "required": true,
          "secret": true
        }
      ]
    }
  ]
}
```

### Parsing Logic

```rust
fn parse_smithery_result(result: &JsonValue) -> McpSearchResult {
    let deployment_type = result["deploymentType"].as_str().unwrap_or("npm");
    
    let (package_type, identifier) = match deployment_type {
        "npm" => (McpPackageType::Npm, result["npmPackage"].as_str()),
        "docker" => (McpPackageType::Docker, result["dockerImage"].as_str()),
        "http" => (McpPackageType::Http, result["httpUrl"].as_str()),
        _ => (McpPackageType::Npm, result["npmPackage"].as_str()),
    };
    
    McpSearchResult {
        name: result["displayName"].as_str().unwrap_or_default().to_string(),
        id: result["qualifiedName"].as_str().unwrap_or_default().to_string(),
        description: result["description"].as_str().unwrap_or_default().to_string(),
        source: McpRegistrySource::Smithery {
            qualified_name: result["qualifiedName"].as_str().unwrap_or_default().to_string(),
        },
        package: McpPackageInfo {
            package_type,
            identifier: identifier.unwrap_or_default().to_string(),
            runtime_hint: None,
        },
    }
}
```

---

## Search Algorithm

### Combined Search (Both)

```rust
async fn search(&self, query: &str, source: RegistrySource) -> Result<Vec<McpSearchResult>> {
    match source {
        RegistrySource::Official => self.search_official(query).await,
        RegistrySource::Smithery => self.search_smithery(query).await,
        RegistrySource::Both => {
            // Search both in parallel
            let (official, smithery) = tokio::join!(
                self.search_official(query),
                self.search_smithery(query),
            );
            
            let mut results = Vec::new();
            
            // Add official results first
            if let Ok(mut r) = official {
                results.append(&mut r);
            }
            
            // Add smithery results
            if let Ok(mut r) = smithery {
                results.append(&mut r);
            }
            
            Ok(results)
        }
    }
}
```

### Result Ordering

| Priority | Criteria |
|----------|----------|
| 1 | Exact name match |
| 2 | Name starts with query |
| 3 | Name contains query |
| 4 | Description contains query |
| 5 | Alphabetical within each group |

---

## Caching

### Cache Location

```
~/Library/Application Support/PersonalAgent/cache/
├── mcp-registry-official.json
├── mcp-registry-smithery.json
└── mcp-details-{id}.json
```

### Cache Entry Format

```json
{
  "fetched_at": "2025-01-21T19:30:00Z",
  "expires_at": "2025-01-22T19:30:00Z",
  "data": { ... }
}
```

### Cache Policy

| Data Type | TTL | Notes |
|-----------|-----|-------|
| Search results | 1 hour | Frequent queries |
| MCP details | 24 hours | Less frequent |
| On error | Use stale | Graceful degradation |

---

## Environment Variable Detection

### Secret Detection Heuristics

If registry doesn't provide `secret` flag, detect from name:

```rust
fn is_likely_secret(name: &str) -> bool {
    let secret_patterns = [
        "_TOKEN", "_PAT", "_KEY", "_SECRET",
        "_PASSWORD", "_CREDENTIAL", "_AUTH",
        "API_KEY", "ACCESS_TOKEN", "PRIVATE_KEY",
    ];
    
    let name_upper = name.to_uppercase();
    secret_patterns.iter().any(|p| name_upper.contains(p))
}
```

### Auth Method Suggestion

```rust
fn suggest_auth_method(env_vars: &[EnvVarSpec]) -> AuthMethodSuggestion {
    let has_client_id = env_vars.iter().any(|e| e.name.contains("CLIENT_ID"));
    let has_client_secret = env_vars.iter().any(|e| e.name.contains("CLIENT_SECRET"));
    
    if has_client_id && has_client_secret {
        return AuthMethodSuggestion::OAuth;
    }
    
    let has_secret = env_vars.iter().any(|e| e.is_secret);
    if has_secret {
        return AuthMethodSuggestion::ApiKey;
    }
    
    AuthMethodSuggestion::None
}
```

---

## Error Handling

| Error | Handling |
|-------|----------|
| Network error | Return cached results if available, else error |
| Parse error | Log warning, skip malformed entries |
| Rate limit | Respect retry-after header, use cache |
| Registry down | Use cache, show stale warning |

---

## UI Integration

### MCP Add View Dependencies

| UI Action | Service Method |
|-----------|----------------|
| Search MCPs | `search(query, source)` |
| Select from results | Data from `McpSearchResult` |
| Proceed to configure | `get_details(source)` for full env vars |

### Data Flow

```
MCP Add View
     │
     ├─ User types search query
     │
     ▼
McpRegistryService.search(query, source)
     │
     ├─ Returns Vec<McpSearchResult>
     │
     ▼
User selects result
     │
     ├─ UI extracts McpSearchResult
     │
     ▼
McpRegistryService.get_details(source)
     │
     ├─ Returns McpDetails with env_vars
     │
     ▼
Navigate to MCP Configure View
     │
     └─ Pass McpDetails for form population
```

---

## Event Emissions

McpRegistryService does **not** emit events directly. It is a read-only query service.

Search and fetch operations are synchronous queries that return data directly to the caller.

**Rationale:** The registry service provides data on-demand; there's no background state changes to notify about.

---

## Test Requirements

| ID | Test |
|----|------|
| MR-T1 | Search official registry returns results |
| MR-T2 | Search smithery registry returns results |
| MR-T3 | Combined search merges results |
| MR-T4 | Results ordered by relevance |
| MR-T5 | Cache stores search results |
| MR-T6 | Stale cache used on network error |
| MR-T7 | Env vars parsed from registry data |
| MR-T8 | Secret detection heuristics work |
| MR-T9 | Auth method suggestion accurate |
| MR-T10 | Package type correctly identified |
