# Models Registry Service Requirements

The Models Registry Service provides information about known LLM models - their capabilities, limits, costs, and provider details. It fetches data from models.dev and caches it locally.

---

## Responsibilities

- Fetch and cache model data from models.dev API
- Provide model information for profile creation
- Search/filter models by provider, capability, name
- Resolve provider API base URLs
- Provide model icons for UI display

---

## Service Interface

```rust
pub trait ModelsRegistryService: Send + Sync {
    // --- Provider Access ---
    
    /// Get all providers
    fn providers(&self) -> Vec<Provider>;
    
    /// Get provider by ID
    fn provider(&self, id: &str) -> Option<Provider>;
    
    /// Get base URL for a provider
    fn base_url(&self, provider_id: &str) -> Option<String>;
    
    // --- Model Access ---
    
    /// Get all models (across all providers)
    fn all_models(&self) -> Vec<ModelInfo>;
    
    /// Get models for a specific provider
    fn models_for_provider(&self, provider_id: &str) -> Vec<ModelInfo>;
    
    /// Get a specific model by provider and model ID
    fn model(&self, provider_id: &str, model_id: &str) -> Option<ModelInfo>;
    
    // --- Search ---
    
    /// Search models by name, ID, or provider
    fn search(&self, query: &str) -> Vec<ModelInfo>;
    
    /// Filter models by capabilities
    fn filter_by_capability(&self, capability: ModelCapability) -> Vec<ModelInfo>;
    
    // --- Cache Management ---
    
    /// Refresh data from API
    async fn refresh(&self) -> Result<()>;
    
    /// Check if cache is stale
    fn is_stale(&self) -> bool;
    
    /// Get last refresh timestamp
    fn last_refreshed(&self) -> Option<DateTime<Utc>>;
}
```

---

## Data Model

### Provider

```rust
pub struct Provider {
    /// Provider ID (e.g., "anthropic", "openai")
    pub id: String,
    
    /// Display name (e.g., "Anthropic", "OpenAI")
    pub name: String,
    
    /// Default API base URL
    pub api_url: String,
    
    /// API type for SDK selection
    pub api_type: ApiType,
    
    /// Provider icon (base64 or asset name)
    pub icon: Option<String>,
}

pub enum ApiType {
    /// Anthropic Messages API
    Anthropic,
    /// OpenAI Chat Completions API (and compatible)
    OpenAI,
}
```

### Model Info

```rust
pub struct ModelInfo {
    /// Provider ID
    pub provider_id: String,
    
    /// Model ID to use in API calls
    pub model_id: String,
    
    /// Human-readable display name
    pub display_name: String,
    
    /// Model description
    pub description: Option<String>,
    
    /// Model capabilities
    pub capabilities: ModelCapabilities,
    
    /// Token limits
    pub limits: ModelLimits,
    
    /// Pricing information
    pub cost: Option<ModelCost>,
    
    /// Release/knowledge cutoff date
    pub knowledge_cutoff: Option<String>,
    
    /// Whether this is a recommended/featured model
    pub featured: bool,
}

pub struct ModelCapabilities {
    /// Supports tool/function calling
    pub tool_call: bool,
    
    /// Supports extended thinking/reasoning
    pub reasoning: bool,
    
    /// Supports image inputs
    pub vision: bool,
    
    /// Supports structured output (JSON mode)
    pub structured_output: bool,
    
    /// Supports streaming responses
    pub streaming: bool,
    
    /// Supports caching
    pub caching: bool,
}

pub enum ModelCapability {
    ToolCall,
    Reasoning,
    Vision,
    StructuredOutput,
    Streaming,
    Caching,
}

pub struct ModelLimits {
    /// Context window size (input + output tokens)
    pub context: u32,
    
    /// Maximum output tokens
    pub max_output: Option<u32>,
    
    /// Maximum thinking budget (for reasoning models)
    pub max_thinking: Option<u32>,
}

pub struct ModelCost {
    /// Cost per million input tokens
    pub input_per_million: f64,
    
    /// Cost per million output tokens
    pub output_per_million: f64,
    
    /// Currency code
    pub currency: String,
    
    /// Cache read cost per million tokens (if applicable)
    pub cache_read_per_million: Option<f64>,
    
    /// Cache write cost per million tokens (if applicable)
    pub cache_write_per_million: Option<f64>,
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
      "claude-sonnet-4-20250514": {
        "id": "claude-sonnet-4-20250514",
        "name": "Claude Sonnet 4",
        "description": "Most intelligent model, best for complex tasks",
        "tool_call": true,
        "reasoning": true,
        "vision": true,
        "structured_output": true,
        "streaming": true,
        "limits": {
          "context": 200000,
          "output": 16384
        },
        "cost": {
          "input": 3,
          "output": 15
        }
      }
    }
  },
  "openai": {
    "id": "openai",
    "name": "OpenAI",
    "api": "https://api.openai.com/v1",
    "models": {
      "gpt-4o": {
        "id": "gpt-4o",
        "name": "GPT-4o",
        "tool_call": true,
        "vision": true,
        "limits": {
          "context": 128000,
          "output": 16384
        },
        "cost": {
          "input": 2.5,
          "output": 10
        }
      }
    }
  }
}
```

### Parsing

```rust
fn parse_models_dev(json: &serde_json::Value) -> (Vec<Provider>, Vec<ModelInfo>) {
    let mut providers = Vec::new();
    let mut models = Vec::new();
    
    if let Some(obj) = json.as_object() {
        for (provider_id, provider_data) in obj {
            // Parse provider
            let provider = Provider {
                id: provider_id.clone(),
                name: provider_data["name"].as_str()
                    .unwrap_or(provider_id).to_string(),
                api_url: provider_data["api"].as_str()
                    .unwrap_or("").to_string(),
                api_type: infer_api_type(provider_id),
                icon: None, // Loaded from bundled assets
            };
            providers.push(provider);
            
            // Parse models
            if let Some(models_obj) = provider_data["models"].as_object() {
                for (model_id, model_data) in models_obj {
                    let model = parse_model(provider_id, model_id, model_data);
                    models.push(model);
                }
            }
        }
    }
    
    (providers, models)
}

fn parse_model(provider_id: &str, model_id: &str, data: &serde_json::Value) -> ModelInfo {
    ModelInfo {
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        display_name: data["name"].as_str()
            .unwrap_or(model_id).to_string(),
        description: data["description"].as_str().map(String::from),
        capabilities: ModelCapabilities {
            tool_call: data["tool_call"].as_bool().unwrap_or(false),
            reasoning: data["reasoning"].as_bool().unwrap_or(false),
            vision: data["vision"].as_bool().unwrap_or(false),
            structured_output: data["structured_output"].as_bool().unwrap_or(false),
            streaming: data["streaming"].as_bool().unwrap_or(true),
            caching: data["caching"].as_bool().unwrap_or(false),
        },
        limits: ModelLimits {
            context: data["limits"]["context"].as_u64().unwrap_or(100000) as u32,
            max_output: data["limits"]["output"].as_u64().map(|v| v as u32),
            max_thinking: data["limits"]["thinking"].as_u64().map(|v| v as u32),
        },
        cost: parse_cost(&data["cost"]),
        knowledge_cutoff: data["knowledge_cutoff"].as_str().map(String::from),
        featured: data["featured"].as_bool().unwrap_or(false),
    }
}

fn parse_cost(data: &serde_json::Value) -> Option<ModelCost> {
    if data.is_null() {
        return None;
    }
    
    Some(ModelCost {
        input_per_million: data["input"].as_f64().unwrap_or(0.0),
        output_per_million: data["output"].as_f64().unwrap_or(0.0),
        currency: "USD".to_string(),
        cache_read_per_million: data["cache_read"].as_f64(),
        cache_write_per_million: data["cache_write"].as_f64(),
    })
}

fn infer_api_type(provider_id: &str) -> ApiType {
    match provider_id {
        "anthropic" => ApiType::Anthropic,
        _ => ApiType::OpenAI, // Most providers are OpenAI-compatible
    }
}
```

---

## Caching

### Cache Location

```
~/Library/Application Support/PersonalAgent/cache/models-registry.json
```

### Cache Format

```json
{
  "fetched_at": "2025-01-21T19:30:00Z",
  "expires_at": "2025-01-22T19:30:00Z",
  "data": { ... }
}
```

### Cache Policy

| Condition | Action |
|-----------|--------|
| No cache file | Fetch on first access |
| Cache < 24 hours | Use cache |
| Cache > 24 hours | Use cache, refresh in background |
| Fetch fails | Use cache regardless of age |
| No cache + fetch fails | Use bundled fallback data |

### Staleness Check

```rust
fn is_stale(&self) -> bool {
    match self.last_refreshed() {
        Some(time) => Utc::now() - time > Duration::hours(24),
        None => true,
    }
}
```

### Refresh Operation

```rust
async fn refresh(&self) -> Result<()> {
    let response = reqwest::Client::new()
        .get("https://models.dev/api.json")
        .timeout(Duration::from_secs(10))
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err(Error::FetchFailed(response.status().to_string()));
    }
    
    let data: serde_json::Value = response.json().await?;
    
    // Validate response structure
    if !data.is_object() || data.as_object().unwrap().is_empty() {
        return Err(Error::InvalidResponse);
    }
    
    // Write to cache
    let cache = CacheEntry {
        fetched_at: Utc::now(),
        expires_at: Utc::now() + Duration::hours(24),
        data: data.clone(),
    };
    
    let path = self.cache_path();
    let json = serde_json::to_string_pretty(&cache)?;
    atomic_write(&path, &json)?;
    
    // Update in-memory data
    let (providers, models) = parse_models_dev(&data);
    *self.providers.write() = providers;
    *self.models.write() = models;
    
    Ok(())
}
```

### Bundled Fallback Data

For offline scenarios, include a snapshot of models.dev data in the app bundle:

```rust
const BUNDLED_MODELS: &str = include_str!("../resources/models-fallback.json");

fn load_fallback(&self) {
    let data: serde_json::Value = serde_json::from_str(BUNDLED_MODELS)
        .expect("Bundled models data should be valid");
    
    let (providers, models) = parse_models_dev(&data);
    *self.providers.write() = providers;
    *self.models.write() = models;
}
```

---

## Search and Filter

### Search Algorithm

```rust
fn search(&self, query: &str) -> Vec<ModelInfo> {
    let query_lower = query.to_lowercase();
    let query_terms: Vec<&str> = query_lower.split_whitespace().collect();
    
    let mut results: Vec<(ModelInfo, u32)> = self.all_models()
        .into_iter()
        .filter_map(|m| {
            let score = self.score_match(&m, &query_terms);
            if score > 0 {
                Some((m, score))
            } else {
                None
            }
        })
        .collect();
    
    // Sort by score descending, then by name
    results.sort_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| a.0.display_name.cmp(&b.0.display_name))
    });
    
    results.into_iter().map(|(m, _)| m).collect()
}

fn score_match(&self, model: &ModelInfo, terms: &[&str]) -> u32 {
    let mut score = 0;
    let model_id_lower = model.model_id.to_lowercase();
    let name_lower = model.display_name.to_lowercase();
    let provider_lower = model.provider_id.to_lowercase();
    
    for term in terms {
        // Exact ID match
        if model_id_lower == *term {
            score += 100;
        }
        // ID starts with term
        else if model_id_lower.starts_with(term) {
            score += 50;
        }
        // ID contains term
        else if model_id_lower.contains(term) {
            score += 20;
        }
        // Name contains term
        else if name_lower.contains(term) {
            score += 15;
        }
        // Provider contains term
        else if provider_lower.contains(term) {
            score += 10;
        }
        // Description contains term
        else if model.description.as_ref()
            .map(|d| d.to_lowercase().contains(term))
            .unwrap_or(false) 
        {
            score += 5;
        }
    }
    
    // Boost featured models
    if model.featured && score > 0 {
        score += 25;
    }
    
    score
}
```

### Filter by Capability

```rust
fn filter_by_capability(&self, capability: ModelCapability) -> Vec<ModelInfo> {
    self.all_models()
        .into_iter()
        .filter(|m| match capability {
            ModelCapability::ToolCall => m.capabilities.tool_call,
            ModelCapability::Reasoning => m.capabilities.reasoning,
            ModelCapability::Vision => m.capabilities.vision,
            ModelCapability::StructuredOutput => m.capabilities.structured_output,
            ModelCapability::Streaming => m.capabilities.streaming,
            ModelCapability::Caching => m.capabilities.caching,
        })
        .collect()
}
```

---

## Model Icons

### Icon Resolution

```rust
fn icon_for_provider(&self, provider_id: &str) -> Option<&[u8]> {
    match provider_id {
        "anthropic" => Some(include_bytes!("../assets/anthropic.png")),
        "openai" => Some(include_bytes!("../assets/openai.png")),
        "google" => Some(include_bytes!("../assets/google.png")),
        "mistral" => Some(include_bytes!("../assets/mistral.png")),
        "meta" => Some(include_bytes!("../assets/meta.png")),
        _ => None, // Generic icon
    }
}
```

---

## UI Integration

### Model Selector View

| Action | Service Call |
|--------|--------------|
| Show providers | `providers()` |
| Show models for provider | `models_for_provider(id)` |
| Search models | `search(query)` |
| Get model details | `model(provider_id, model_id)` |

### Profile Editor

| Action | Service Call |
|--------|--------------|
| Get model info | `model(provider_id, model_id)` |
| Get provider base URL | `base_url(provider_id)` |
| Get context limit | `model(...).limits.context` |

### Data Flow: Creating Profile from Model

```
Model Selector View
     │
     ├─ User browses/searches models
     │
     ▼
ModelsRegistryService.search("claude")
     │
     ├─ Returns Vec<ModelInfo>
     │
     ▼
User selects "Claude Sonnet 4"
     │
     ├─ ModelInfo includes:
     │   - model_id: "claude-sonnet-4-20250514"
     │   - provider_id: "anthropic"
     │   - limits.context: 200000
     │   - capabilities.reasoning: true
     │
     ▼
Navigate to Profile Editor
     │
     ├─ Pre-fill:
     │   - api_type: Anthropic (from provider)
     │   - base_url: "https://api.anthropic.com/v1"
     │   - model_id: "claude-sonnet-4-20250514"
     │   - context_limit: 200000
     │   - enable_thinking: (suggest if reasoning=true)
     │
     ▼
User completes profile (name, API key)
```

---

## Error Handling

| Error | Handling |
|-------|----------|
| Network error | Use cache, log warning, UI shows "offline" indicator |
| Parse error | Use cache, log error |
| No cache + no network | Use bundled fallback |
| Timeout | Use cache, retry later |

---

## Test Requirements

| ID | Test |
|----|------|
| MO-T1 | Parse models.dev response correctly |
| MO-T2 | Cache written after successful fetch |
| MO-T3 | Stale cache triggers background refresh |
| MO-T4 | Network error falls back to cache |
| MO-T5 | No cache falls back to bundled data |
| MO-T6 | Search finds by model ID |
| MO-T7 | Search finds by display name |
| MO-T8 | Search results ordered by relevance |
| MO-T9 | Filter by capability works |
| MO-T10 | Provider base_url resolved correctly |
| MO-T11 | Capabilities parsed correctly |
| MO-T12 | Costs parsed correctly |
| MO-T13 | Featured models boosted in search |
| MO-T14 | models_for_provider returns correct subset |
