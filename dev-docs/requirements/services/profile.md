# Profile Service Requirements

The Profile Service manages model profiles (configuration for connecting to LLM providers).

---

## Responsibilities

- CRUD operations for profiles
- Default profile selection
- API key management
- Profile validation

---

## Service Interface

```rust
pub trait ProfileService: Send + Sync {
    /// Get all profiles
    fn list(&self) -> Vec<ModelProfile>;
    
    /// Get profile by ID
    fn get(&self, id: Uuid) -> Option<ModelProfile>;
    
    /// Get the default/active profile
    fn get_default(&self) -> Option<ModelProfile>;
    
    /// Create a new profile
    fn create(&self, profile: ModelProfile) -> Result<()>;
    
    /// Update an existing profile
    fn update(&self, profile: ModelProfile) -> Result<()>;
    
    /// Delete a profile
    fn delete(&self, id: Uuid) -> Result<()>;
    
    /// Set the default profile
    fn set_default(&self, id: Uuid) -> Result<()>;
    
    /// Validate profile configuration
    fn validate(&self, profile: &ModelProfile) -> ValidationResult;
}
```

---

## Data Model

### ModelProfile

```rust
pub struct ModelProfile {
    pub id: Uuid,
    pub name: String,
    pub provider_id: String,
    pub model_id: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub keyfile_path: Option<PathBuf>,
    pub system_prompt: String,
    pub parameters: ModelParameters,
}

pub struct ModelParameters {
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f64>,
    pub thinking_budget: Option<u32>,
    pub enable_thinking: bool,
    pub show_thinking: bool,
}

impl Default for ModelParameters {
    fn default() -> Self {
        Self {
            temperature: Some(1.0),
            max_tokens: Some(4096),
            top_p: None,
            thinking_budget: Some(10000),
            enable_thinking: false,
            show_thinking: true,
        }
    }
}
```

### Default System Prompt

```
You are a helpful assistant.
```

---

## Storage

Profiles stored in `config.json`:

```json
{
  "profiles": [
    {
      "id": "uuid-1",
      "name": "Claude Sonnet",
      "provider_id": "anthropic",
      "model_id": "claude-3-5-sonnet-20241022",
      "base_url": "https://api.anthropic.com/v1",
      "api_key": "sk-...",
      "system_prompt": "You are a helpful assistant.",
      "parameters": {
        "temperature": 1.0,
        "max_tokens": 4096,
        "enable_thinking": false,
        "show_thinking": true
      }
    }
  ],
  "default_profile": "uuid-1"
}
```

---

## Operations

### Create Profile

| Step | Action |
|------|--------|
| 1 | Validate profile data |
| 2 | Generate UUID if not provided |
| 3 | Add to config.profiles |
| 4 | If first profile, set as default |
| 5 | Save config |

### Update Profile

| Step | Action |
|------|--------|
| 1 | Validate profile data |
| 2 | Find existing by ID |
| 3 | Replace in config.profiles |
| 4 | Save config |

### Delete Profile

| Step | Action |
|------|--------|
| 1 | Check not the only profile |
| 2 | Remove from config.profiles |
| 3 | If was default, select another |
| 4 | Save config |

### Set Default

| Step | Action |
|------|--------|
| 1 | Verify profile exists |
| 2 | Update config.default_profile |
| 3 | Save config |

---

## Validation

### Required Fields

| Field | Validation |
|-------|------------|
| name | Non-empty string |
| provider_id | Non-empty string |
| model_id | Non-empty string |
| api_key OR keyfile_path | At least one auth method |

### Optional Validation

| Field | Validation |
|-------|------------|
| temperature | 0.0 - 2.0 |
| max_tokens | > 0 |
| base_url | Valid URL format |
| keyfile_path | File exists |

### Validation Result

```rust
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
}

pub struct ValidationError {
    pub field: String,
    pub message: String,
}
```

---

## API Key Security

### Storage

- API keys stored in config.json (current)
- Future: macOS Keychain integration

### Display

- Mask API keys in UI (show last 4 chars)
- Never log full API keys

### Keyfile Support

- Read key from file at runtime
- Don't store key in config
- Validate file exists and readable

---

## Test Requirements

| ID | Test |
|----|------|
| PR-T1 | Create adds profile to config |
| PR-T2 | Update modifies existing profile |
| PR-T3 | Delete removes profile |
| PR-T4 | Delete last profile fails |
| PR-T5 | Set default updates config |
| PR-T6 | Validation catches missing name |
| PR-T7 | Validation catches missing auth |
| PR-T8 | Get default returns correct profile |
| PR-T9 | Keyfile read works |
