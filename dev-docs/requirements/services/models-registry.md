# Models Registry Service Requirements

The Models Registry Service fetches and caches model information from models.dev.

---

## Canonical Terminology

- Provider and ModelInfo are defined in `dev-docs/requirements/data-models.md`.
- ModelsRegistryService owns normalized Provider/ModelInfo and raw cache.


## Responsibilities

- Fetch model data from models.dev API
- Cache data locally
- Parse provider and model information
- Provide search/filter capabilities

---

## Service Interface

```rust
pub trait ModelsRegistryService: Send + Sync {
    /// Get all providers
    fn providers(&self) -> Vec<Provider>;
    
    /// Get provider by ID
    fn provider(&self, id: &str) -> Option<Provider>;
    
    /// Get all models (across all providers)
    fn all_models(&self) -> Vec<ModelInfo>;
    
    /// Get models for a specific provider
    fn models_for_provider(&self, provider_id: &str) -> Vec<ModelInfo>;
    
    /// Search models by name or ID
    fn search(&self, query: &str) -> Vec<ModelInfo>;
    
    /// Refresh data from API
    async fn refresh(&self) -> Result<()>;
    
    /// Check if cache is stale
    fn is_stale(&self) -> bool;
    
    /// Get base URL for a provider
    fn base_url(&self, provider_id: &str) -> Option<String>;
}
```

---

## Data Model

### Provider

```rust
pub struct Provider {
    pub id: String,
    pub name: String,
    pub api_url: String,
    pub models: Vec<ModelInfo>,
}
```

### ModelInfo

```rust
pub struct ModelInfo {
    pub provider_id: String,
    pub model_id: String,
    pub display_name: String,
    pub description: Option<String>,
    pub capabilities: ModelCapabilities,
    pub limits: ModelLimits,
    pub cost: Option<ModelCost>,
}

pub struct ModelCapabilities {
    pub tool_call: bool,
    pub reasoning: bool,
    pub vision: bool,
    pub structured_output: bool,
    pub streaming: bool,
}

pub struct ModelLimits {
    pub context: u64,           // Context window size
    pub max_output: Option<u64>, // Max output tokens
}

pub struct ModelCost {
    pub input_per_million: f64,
    pub output_per_million: f64,

## Normalized Schema Mapping

Raw API → Internal ModelInfo mapping:

| Raw Field | Internal Field | Required | Default |
|-----------|----------------|----------|---------|
| provider.id | Provider.id | Yes | none |
| provider.name | Provider.name | Yes | provider.id |
| provider.api | Provider.api_url | No | "" |
| model.id | ModelInfo.model_id | Yes | none |
| model.name | ModelInfo.display_name | No | model.id |
| model.description | ModelInfo.description | No | null |
| model.tool_call | capabilities.tool_call | No | false |
| model.reasoning | capabilities.reasoning | No | false |
| model.vision | capabilities.vision | No | false |
| model.structured_output | capabilities.structured_output | No | false |
| model.streaming | capabilities.streaming | No | true |
| model.limits.context | limits.context | Yes | 0 |
| model.limits.output | limits.max_output | No | null |
| model.cost.input | cost.input_per_million | No | null |
| model.cost.output | cost.output_per_million | No | null |
| model.cost.currency | cost.currency | No | "USD" |

Required fields must be present to include a model in results. Missing required fields yield a parse error and the model is excluded.

    pub currency: String,       // "USD"
}
```

---

## API: models.dev

### Endpoint

```
GET https://models.dev/api.json
```

### Response Format

```json
{
  "anthropic": {
    "id": "anthropic",
    "name": "Anthropic",
    "api": "https://api.anthropic.com/v1",
    "models": {
      "claude-3-5-sonnet-20241022": {
        "id": "claude-3-5-sonnet-20241022",
        "name": "Claude 3.5 Sonnet",
        "tool_call": true,
        "reasoning": true,
        "vision": true,
        "limits": {
          "context": 200000,
          "output": 8192
        },
        "cost": {
          "input": 3,
          "output": 15
        }
      }
    }
  },
  "openai": {
    ...
  }
}
```

### Parsing

```rust
fn parse_models_dev(json: &serde_json::Value) -> Vec<Provider> {
    let mut providers = Vec::new();
    
    if let Some(obj) = json.as_object() {
        for (provider_id, provider_data) in obj {
            let provider = Provider {
                id: provider_id.clone(),
                name: provider_data["name"].as_str().unwrap_or(provider_id).to_string(),
                api_url: provider_data["api"].as_str().unwrap_or("").to_string(),
                models: parse_models(provider_id, &provider_data["models"]),
            };
            providers.push(provider);
        }
    }
    
    providers
}
```

---

## Caching

### Cache Location

```
~/Library/Application Support/PersonalAgent/cache/models.json
```

### Cache Format

```json
{
  "fetched_at": "2025-01-21T19:30:00Z",
  "data": { ... }  // Raw models.dev response
}
```

### Staleness

| Condition | Action |
|-----------|--------|
| No cache file | Fetch immediately |
| Cache < 24 hours | Use cache |
| Cache > 24 hours | Fetch in background, use cache |
| Fetch fails | Use cache regardless of age |

### Refresh Behavior

```rust
async fn refresh(&self) -> Result<()> {
    let response = reqwest::get("https://models.dev/api.json").await?;
    let data: serde_json::Value = response.json().await?;
    
    let cache = CacheEntry {
        fetched_at: Utc::now(),
        data,
    };
    let path = cache_path();
    fs::write(&path, serde_json::to_string(&cache)?)?;
    
    // Update in-memory data
    self.providers = parse_models_dev(&cache.data);
    
    Ok(())
}
```

## Validation Rules

| Field | Rule | Error Code | Error Message |
|-------|------|------------|---------------|
| query | <= 200 chars | VALIDATION_ERROR | Query too long |
| provider_id | Non-empty | VALIDATION_ERROR | Provider required |

## Negative Test Cases

| ID | Scenario | Expected Result |
|----|----------|----------------|
| MR-NT1 | search with 500-char query | VALIDATION_ERROR, no results returned |
| MR-NT2 | refresh with network failure and no cache | NETWORK_ERROR, providers empty |
| MR-NT3 | parse model missing id | SERVICE_UNAVAILABLE, model excluded |

## End-to-End Flow (Refresh + Cache)

1. refresh() fetches raw API response.
2. If success, write cache then parse providers/models.
3. If write fails, return SERVICE_UNAVAILABLE and keep in-memory data.
4. If fetch fails and cache exists, keep cache and set stale=true.

---

## Search

### Algorithm

```rust
fn search(&self, query: &str) -> Vec<ModelInfo> {
    let query_lower = query.to_lowercase();
    
    self.all_models()
        .into_iter()
        .filter(|m| {
            m.model_id.to_lowercase().contains(&query_lower) ||
            m.display_name.to_lowercase().contains(&query_lower) ||
            m.provider_id.to_lowercase().contains(&query_lower)
        })
        .collect()
}
```

### Sort Order

1. Exact ID match first
2. ID starts with query
3. Name contains query
4. Alphabetical by display_name within each group

### Registry Sorts (UI default)

When returning `all_models()` or `models_for_provider()`, default sort order is:

1. Provider name (A-Z)
2. Model display_name (A-Z)

### Filter Rules

Filters are applied in this order:

1. Provider filter (exact provider_id match)
2. Capability filters (tool_call/reasoning/vision/structured_output)
3. Search text filter (case-insensitive match on model_id or display_name)
4. Cost filter (optional): max input/output price per million tokens

---

## Error Handling

| Error | Handling |
|-------|----------|
| Network error | Use cache, log warning |
| Parse error | Use cache, log error |
| No cache, no network | Return empty, show error |

---

## Test Requirements

| ID | Test |
|----|------|
| MR-T1 | Parse models.dev response correctly |
| MR-T2 | Cache written after fetch |
| MR-T3 | Stale cache triggers refresh |
| MR-T4 | Network error falls back to cache |
| MR-T5 | Search finds by model ID |
| MR-T6 | Search finds by display name |
| MR-T7 | Provider base_url resolved |
| MR-T8 | Capabilities parsed correctly |
