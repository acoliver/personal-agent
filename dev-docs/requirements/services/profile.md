# Profile Service Requirements

The Profile Service manages model profiles - configurations that specify which LLM to use, authentication, and model parameters. It coordinates with SecretsService for API key storage.

**Note:** ProfileService does NOT manage which profile is the "default". That is managed by AppSettingsService. ProfileService is pure CRUD for profile data.

---

## Responsibilities

- CRUD operations for model profiles
- Profile validation
- API key coordination with SecretsService
- Keyfile path resolution

---

## Service Interface

```rust
pub trait ProfileService: Send + Sync {
    /// Create a new profile
    fn create(&self, profile: &NewProfile) -> Result<ModelProfile>;
    
    /// Get profile by ID
    fn get(&self, id: Uuid) -> Result<Option<ModelProfile>>;
    
    /// List all profiles
    fn list(&self) -> Result<Vec<ModelProfile>>;
    
    /// Update an existing profile
    fn update(&self, id: Uuid, updates: &ProfileUpdate) -> Result<ModelProfile>;
    
    /// Delete a profile
    fn delete(&self, id: Uuid) -> Result<()>;
    
    /// Validate profile configuration
    fn validate(&self, profile: &NewProfile) -> Result<Vec<ValidationError>>;
    
    /// Test profile connection by sending a minimal test message
    async fn test_connection(&self, id: Uuid) -> Result<ConnectionTestResult>;
    
    /// Get profile with resolved API key for ChatService use
    /// This is the only way ChatService should get API keys - not via SecretsService directly
    fn get_model_config(&self, id: Uuid) -> Result<ResolvedModelConfig>;
}

/// Profile config with resolved API key, ready for use by ChatService
pub struct ResolvedModelConfig {
    pub profile: ModelProfile,
    /// None for AuthMethod::None (local models)
    pub api_key: Option<String>,
}
```

**Note:** `show_thinking` is stored in the profile's `ModelParameters` but is only used as an initialization value. The Chat View's runtime toggle does not persist changes - it's session-only state.

---

## Data Model

### Model Profile

```rust
pub struct ModelProfile {
    /// Unique identifier
    pub id: Uuid,
    
    /// User-friendly display name
    pub name: String,
    
    /// API type determines SDK/protocol
    pub api_type: ApiType,
    
    /// API base URL (e.g., "https://api.anthropic.com")
    pub base_url: String,
    
    /// Model identifier (e.g., "claude-sonnet-4-20250514")
    pub model_id: String,
    
    /// System prompt template
    pub system_prompt: String,
    
    /// Model parameters
    pub parameters: ModelParameters,
    
    /// Authentication method
    pub auth_method: AuthMethod,
    
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    
    /// Last modification timestamp
    pub updated_at: DateTime<Utc>,
}

pub enum ApiType {
    /// Anthropic API
    Anthropic,
    /// OpenAI-compatible API (includes local models like Ollama, LM Studio)
    OpenAI,
}

impl ApiType {
    pub fn default_base_url(&self) -> &str {
        match self {
            ApiType::Anthropic => "https://api.anthropic.com",
            ApiType::OpenAI => "https://api.openai.com/v1",
        }
    }
    
    /// Parse from string (case-insensitive)
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "anthropic" => ApiType::Anthropic,
            _ => ApiType::OpenAI, // Default to OpenAI-compatible
        }
    }
    
    /// Serialize to lowercase string
    pub fn as_str(&self) -> &'static str {
        match self {
            ApiType::Anthropic => "anthropic",
            ApiType::OpenAI => "openai",
        }
    }
}
```

### Model Parameters

```rust
pub struct ModelParameters {
    /// Temperature (0.0 - 2.0)
    pub temperature: Option<f32>,
    
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    
    /// Enable extended thinking (for models that support it)
    pub enable_thinking: bool,
    
    /// Budget tokens for thinking (when enabled)
    pub thinking_budget: Option<u32>,
    
    /// Reasoning effort for OpenAI o-series models ("low", "medium", "high")
    pub reasoning_effort: Option<String>,
    
    /// Context window size (required - models don't report this at runtime)
    /// Pre-filled from models.dev during profile creation
    pub context_limit: u32,
    
    /// Whether to show thinking content in UI (user preference, not model setting)
    pub show_thinking: bool,
}

impl Default for ModelParameters {
    fn default() -> Self {
        Self {
            temperature: Some(0.7),
            max_tokens: Some(4096),
            enable_thinking: false,
            thinking_budget: None,
            reasoning_effort: None,
            context_limit: 128_000, // Safe default, should be overridden from models.dev
            show_thinking: true,
        }
    }
}
```

### Authentication Method

```rust
pub enum AuthMethod {
    /// No authentication required (local models, some self-hosted)
    None,
    
    /// API key stored in SecretsService
    ApiKey,
    
    /// Read API key from file at runtime
    Keyfile { path: PathBuf },
}
```

### Profile Creation Input

```rust
pub struct NewProfile {
    pub name: String,
    pub api_type: ApiType,
    pub base_url: Option<String>, // None = use default for api_type
    pub model_id: String,
    pub system_prompt: String,
    pub parameters: ModelParameters,
    pub auth_method: AuthMethod,
    pub api_key: Option<String>, // If AuthMethod::ApiKey, initial key to store
}
```

### Profile Update Input

```rust
pub struct ProfileUpdate {
    pub name: Option<String>,
    pub api_type: Option<ApiType>,
    pub base_url: Option<String>,
    pub model_id: Option<String>,
    pub system_prompt: Option<String>,
    pub parameters: Option<ModelParameters>,
    pub auth_method: Option<AuthMethod>,
    pub api_key: Option<String>, // New key to store
}
```

### Validation Error

```rust
pub struct ValidationError {
    pub field: String,
    pub message: String,
}
```

### Connection Test Result

```rust
pub struct ConnectionTestResult {
    pub success: bool,
    pub latency_ms: Option<u64>,
    pub model_info: Option<String>,
    pub error_message: Option<String>,
}
```

---

## Storage Format

### File Location

```
~/Library/Application Support/PersonalAgent/profiles/
├── a1b2c3d4-e5f6-....json
├── b2c3d4e5-f6a7-....json
└── ...
```

### Profile JSON Format

```json
{
  "id": "a1b2c3d4-e5f6-...",
  "name": "Claude Sonnet",
  "api_type": "anthropic",
  "base_url": "https://api.anthropic.com",
  "model_id": "claude-sonnet-4-20250514",
  "system_prompt": "You are a helpful assistant...",
  "parameters": {
    "temperature": 0.7,
    "max_tokens": 4096,
    "enable_thinking": false,
    "thinking_budget": null,
    "context_limit": 200000
  },
  "auth_method": {
    "type": "api_key"
  },
  "created_at": "2025-01-21T19:30:00Z",
  "updated_at": "2025-01-21T20:45:00Z"
}
```

### Auth Method Serialization

```json
// No authentication (local models)
{ "type": "none" }

// API Key (stored in SecretsService)
{ "type": "api_key" }

// Keyfile
{ "type": "keyfile", "path": "~/.anthropic/key" }
```

---

## Operations

### Create Profile

| Step | Action |
|------|--------|
| 1 | Validate input |
| 2 | Generate UUID |
| 3 | Set base_url to default if not provided |
| 4 | If AuthMethod::ApiKey and api_key provided, store in SecretsService |
| 5 | Write profile JSON |
| 6 | Return created profile |

```rust
fn create(&self, input: &NewProfile) -> Result<ModelProfile> {
    // Validate
    let errors = self.validate(input)?;
    if !errors.is_empty() {
        return Err(Error::Validation(errors));
    }
    
    let id = Uuid::new_v4();
    let now = Utc::now();
    
    // Store API key if provided
    if matches!(input.auth_method, AuthMethod::ApiKey) {
        if let Some(api_key) = &input.api_key {
            self.secrets_service.store(
                &SecretKey::profile_api_key(id),
                api_key,
            )?;
        }
    }
    
    let profile = ModelProfile {
        id,
        name: input.name.clone(),
        api_type: input.api_type.clone(),
        base_url: input.base_url.clone()
            .unwrap_or_else(|| input.api_type.default_base_url().to_string()),
        model_id: input.model_id.clone(),
        system_prompt: input.system_prompt.clone(),
        parameters: input.parameters.clone(),
        auth_method: input.auth_method.clone(),
        created_at: now,
        updated_at: now,
    };
    
    self.save_profile(&profile)?;
    
    Ok(profile)
}
```

### Get Profile

```rust
fn get(&self, id: Uuid) -> Result<Option<ModelProfile>> {
    let path = self.profile_path(&id);
    
    if !path.exists() {
        return Ok(None);
    }
    
    let content = std::fs::read_to_string(&path)?;
    let profile: ModelProfile = serde_json::from_str(&content)?;
    
    Ok(Some(profile))
}
```

### List Profiles

```rust
fn list(&self) -> Result<Vec<ModelProfile>> {
    let dir = self.profiles_dir();
    let mut profiles = Vec::new();
    
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(profile) = serde_json::from_str::<ModelProfile>(&content) {
                    profiles.push(profile);
                }
            }
        }
    }
    
    // Sort by name
    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    
    Ok(profiles)
}
```

### Update Profile

| Step | Action |
|------|--------|
| 1 | Load existing profile |
| 2 | Apply updates |
| 3 | If api_key in updates, store in SecretsService |
| 4 | If auth_method changed away from ApiKey, delete old key |
| 5 | Update updated_at |
| 6 | Save profile |

```rust
fn update(&self, id: Uuid, updates: &ProfileUpdate) -> Result<ModelProfile> {
    let mut profile = self.get(id)?
        .ok_or(Error::NotFound)?;
    
    // Track if auth method changed
    let old_auth_method = profile.auth_method.clone();
    
    // Apply updates
    if let Some(name) = &updates.name {
        profile.name = name.clone();
    }
    if let Some(api_type) = &updates.api_type {
        profile.api_type = api_type.clone();
    }
    if let Some(base_url) = &updates.base_url {
        profile.base_url = base_url.clone();
    }
    if let Some(model_id) = &updates.model_id {
        profile.model_id = model_id.clone();
    }
    if let Some(system_prompt) = &updates.system_prompt {
        profile.system_prompt = system_prompt.clone();
    }
    if let Some(parameters) = &updates.parameters {
        profile.parameters = parameters.clone();
    }
    if let Some(auth_method) = &updates.auth_method {
        profile.auth_method = auth_method.clone();
    }
    
    // Handle API key updates
    if let Some(api_key) = &updates.api_key {
        if matches!(profile.auth_method, AuthMethod::ApiKey) {
            self.secrets_service.store(
                &SecretKey::profile_api_key(id),
                api_key,
            )?;
        }
    }
    
    // If changed away from ApiKey, delete old secret
    if matches!(old_auth_method, AuthMethod::ApiKey) 
        && !matches!(profile.auth_method, AuthMethod::ApiKey) {
        self.secrets_service.delete(&SecretKey::profile_api_key(id))?;
    }
    
    profile.updated_at = Utc::now();
    self.save_profile(&profile)?;
    
    Ok(profile)
}
```

### Delete Profile

| Step | Action |
|------|--------|
| 1 | Delete API key from SecretsService if AuthMethod::ApiKey |
| 2 | Delete profile JSON file |

**Note:** Caller (usually the code coordinating with AppSettingsService) should check if this was the default profile and clear it from AppSettingsService.

```rust
fn delete(&self, id: Uuid) -> Result<()> {
    let profile = self.get(id)?
        .ok_or(Error::NotFound)?;
    
    // Delete secrets
    self.secrets_service.delete_all_for_resource(
        ResourceType::Profile,
        id,
    )?;
    
    // Delete profile file
    let path = self.profile_path(&id);
    std::fs::remove_file(&path)?;
    
    Ok(())
}
```

### Get Model Config (for ChatService)

```rust
fn get_model_config(&self, id: Uuid) -> Result<ResolvedModelConfig> {
    let profile = self.get(id)?
        .ok_or(Error::NotFound)?;
    
    let api_key = self.resolve_api_key(&profile)?;
    
    Ok(ResolvedModelConfig { profile, api_key })
}
```

This is the **only way ChatService should get API keys**. ChatService never calls SecretsService directly.

### Validate Profile

```rust
fn validate(&self, input: &NewProfile) -> Result<Vec<ValidationError>> {
    let mut errors = Vec::new();
    
    // Name required
    if input.name.trim().is_empty() {
        errors.push(ValidationError {
            field: "name".to_string(),
            message: "Name is required".to_string(),
        });
    }
    
    // Model ID required
    if input.model_id.trim().is_empty() {
        errors.push(ValidationError {
            field: "model_id".to_string(),
            message: "Model ID is required".to_string(),
        });
    }
    
    // Temperature range
    if let Some(temp) = input.parameters.temperature {
        if temp < 0.0 || temp > 2.0 {
            errors.push(ValidationError {
                field: "parameters.temperature".to_string(),
                message: "Temperature must be between 0.0 and 2.0".to_string(),
            });
        }
    }
    
    // Keyfile existence (if applicable)
    if let AuthMethod::Keyfile { path } = &input.auth_method {
        let expanded = expand_tilde(path);
        if !expanded.exists() {
            errors.push(ValidationError {
                field: "auth_method.path".to_string(),
                message: format!("Keyfile not found: {}", expanded.display()),
            });
        }
    }
    
    // API key required for ApiKey auth method
    if matches!(input.auth_method, AuthMethod::ApiKey) && input.api_key.is_none() {
        // Check if we already have a key stored (for updates)
        // For new profiles, this is an error
        errors.push(ValidationError {
            field: "api_key".to_string(),
            message: "API key is required".to_string(),
        });
    }
    
    Ok(errors)
}
```

### Test Connection

Sends a minimal test message to verify the profile's API key and configuration work.

```rust
async fn test_connection(&self, id: Uuid) -> Result<ConnectionTestResult> {
    let config = self.get_model_config(id)?;
    
    let start = std::time::Instant::now();
    
    // Build model spec: "provider:model_id"
    let spec = format!("{}:{}", config.profile.api_type.as_str(), config.profile.model_id);
    
    // Build a minimal SerdesAI agent and send a test message
    let mut model_config = ModelConfig::new(&spec)
        .with_base_url(&config.profile.base_url);
    
    // API key is optional (None for AuthMethod::None)
    if let Some(ref api_key) = config.api_key {
        model_config = model_config.with_api_key(api_key);
    }
    
    let result = async {
        let agent = AgentBuilder::from_config(&model_config)?
            .build();
        
        // Send minimal prompt, don't save to any conversation
        let response = agent.run("Say 'ok'", ()).await?;
        Ok::<_, Error>(response.output)
    }.await;
    
    let latency = start.elapsed().as_millis() as u64;
    
    match result {
        Ok(_) => Ok(ConnectionTestResult {
            success: true,
            latency_ms: Some(latency),
            error_message: None,
        }),
        Err(e) => Ok(ConnectionTestResult {
            success: false,
            latency_ms: Some(latency),
            error_message: Some(e.to_string()),
        }),
    }
}
```

**Note:** This uses a small amount of tokens but provides a real end-to-end verification that the API key, base URL, and model ID are all correct.

---

## API Key Resolution

```rust
fn resolve_api_key(&self, profile: &ModelProfile) -> Result<Option<String>> {
    match &profile.auth_method {
        AuthMethod::None => Ok(None),
        AuthMethod::ApiKey => {
            let key = self.secrets_service
                .get(&SecretKey::profile_api_key(profile.id))?
                .ok_or(Error::NoApiKey)?;
            Ok(Some(key))
        }
        AuthMethod::Keyfile { path } => {
            let key = self.secrets_service.read_keyfile(path)?;
            Ok(Some(key))
        }
    }
}
```

**Note:** Returns `Option<String>` since `AuthMethod::None` is valid for local models.

---

## UI Integration

### Profile Editor

| Action | Service Call |
|--------|--------------|
| Load profile | `get(id)` |
| Save new | `create(input)` |
| Save existing | `update(id, updates)` |
| Delete | `delete(id)` |
| Validate | `validate(input)` |
| Test connection | `test_connection(id)` |

### Model Selector

| Action | Service Call |
|--------|--------------|
| Show profiles | `list()` |
| Get selected | `get(id)` |

### Settings View

| Action | Service Call |
|--------|--------------|
| Show profiles | `list()` |
| Set default | `AppSettingsService.set_default_profile_id(id)` |
| Delete profile | `delete(id)` then `AppSettingsService.clear_default_profile()` if needed |

### Chat View

| Action | Service Call |
|--------|--------------|
| Get current profile | `AppSettingsService.get_default_profile_id()` then `get(id)` |
| Initialize show_thinking toggle | Read `profile.parameters.show_thinking` (on app start or profile change) |

**Note:** The Chat View's [T] toggle does NOT call ProfileService to persist changes. It only updates local view state. The toggle is reset to the profile's default on profile change or app restart.

---

## Test Requirements

| ID | Test |
|----|------|
| PR-T1 | create() generates valid UUID |
| PR-T2 | create() stores API key in SecretsService |
| PR-T3 | create() uses default base_url when not provided |
| PR-T4 | get() returns None for non-existent ID |
| PR-T5 | list() returns profiles sorted by name |
| PR-T6 | update() merges partial updates |
| PR-T7 | update() stores new API key |
| PR-T8 | update() deletes old key when auth method changes |
| PR-T9 | delete() removes secrets |
| PR-T10 | delete() removes profile file |

| PR-T13 | validate() catches empty name |
| PR-T14 | validate() catches invalid temperature |
| PR-T15 | validate() catches missing keyfile |
| PR-T16 | test_connection() returns latency |
| PR-T17 | resolve_api_key() handles all auth methods |
| PR-T18 | get_model_config() returns profile with resolved API key |
| PR-T19 | get_model_config() fails if API key missing |
| PR-T20 | test_connection() sends minimal message and returns success/failure |
