# Secrets Service Requirements

The Secrets Service manages secure storage and retrieval of sensitive credentials (API keys, OAuth tokens, keyfile contents). It provides a unified interface for all credential operations across the application.

---

## Responsibilities

- Store API keys securely
- Store and refresh OAuth tokens
- Read keyfile contents at runtime
- Mask/unmask secrets for display
- Delete credentials on resource deletion
- Future: macOS Keychain integration

---

## Service Interface

```rust
pub trait SecretsService: Send + Sync {
    /// Store a secret value
    fn store(&self, key: &SecretKey, value: &str) -> Result<()>;
    
    /// Retrieve a secret value
    fn get(&self, key: &SecretKey) -> Result<Option<String>>;
    
    /// Delete a secret
    fn delete(&self, key: &SecretKey) -> Result<()>;
    
    /// Check if a secret exists
    fn exists(&self, key: &SecretKey) -> bool;
    
    /// Store OAuth tokens
    fn store_oauth_tokens(&self, key: &SecretKey, tokens: &OAuthTokens) -> Result<()>;
    
    /// Get OAuth tokens (refreshes if expired)
    async fn get_oauth_tokens(&self, key: &SecretKey) -> Result<Option<OAuthTokens>>;
    
    /// Read content from a keyfile path
    fn read_keyfile(&self, path: &Path) -> Result<String>;
    
    /// Mask a secret for display (show last N chars)
    fn mask(&self, value: &str, visible_chars: usize) -> String;
    
    /// Delete all secrets for a resource (e.g., when MCP deleted)
    fn delete_all_for_resource(&self, resource_type: ResourceType, resource_id: Uuid) -> Result<()>;
}
```

---

## Data Model

### Secret Key

```rust
/// Identifies a specific secret
pub struct SecretKey {
    /// Type of resource this secret belongs to
    pub resource_type: ResourceType,
    
    /// ID of the resource
    pub resource_id: Uuid,
    
    /// Name of the secret within the resource
    pub secret_name: String,
}

impl SecretKey {
    pub fn profile_api_key(profile_id: Uuid) -> Self {
        Self {
            resource_type: ResourceType::Profile,
            resource_id: profile_id,
            secret_name: "api_key".to_string(),
        }
    }
    
    pub fn mcp_env_var(mcp_id: Uuid, env_var_name: &str) -> Self {
        Self {
            resource_type: ResourceType::Mcp,
            resource_id: mcp_id,
            secret_name: env_var_name.to_string(),
        }
    }
    
    pub fn mcp_oauth(mcp_id: Uuid) -> Self {
        Self {
            resource_type: ResourceType::Mcp,
            resource_id: mcp_id,
            secret_name: "oauth_tokens".to_string(),
        }
    }
}

pub enum ResourceType {
    Profile,
    Mcp,
    Global,
}
```

### OAuth Tokens

```rust
pub struct OAuthTokens {
    /// The access token for API calls
    pub access_token: String,
    
    /// Refresh token for getting new access tokens
    pub refresh_token: Option<String>,
    
    /// When the access token expires
    pub expires_at: Option<DateTime<Utc>>,
    
    /// Token type (usually "Bearer")
    pub token_type: String,
    
    /// Scopes granted
    pub scopes: Vec<String>,
    
    /// Associated username/identity (for display)
    pub username: Option<String>,
}

impl OAuthTokens {
    /// Check if access token is expired (with 5 min buffer)
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(exp) => Utc::now() + Duration::minutes(5) >= exp,
            None => false, // No expiry = doesn't expire
        }
    }
    
    /// Check if we can refresh
    pub fn can_refresh(&self) -> bool {
        self.refresh_token.is_some()
    }
}
```

---

## Storage Backend

### Current: File-Based Storage

```
~/Library/Application Support/PersonalAgent/secrets/
├── profile_{uuid}_api_key.enc
├── mcp_{uuid}_GITHUB_TOKEN.enc
├── mcp_{uuid}_oauth_tokens.json.enc
└── ...
```

### File Format

Each secret file is encrypted at rest:

```rust
struct EncryptedSecret {
    /// Version for migration
    version: u32,
    
    /// Encryption algorithm used
    algorithm: String,
    
    /// Nonce/IV for decryption
    nonce: Vec<u8>,
    
    /// Encrypted payload
    ciphertext: Vec<u8>,
}
```

### Encryption

| Aspect | Specification |
|--------|---------------|
| Algorithm | AES-256-GCM |
| Key derivation | PBKDF2 from machine-specific seed |
| Machine seed | Combination of hardware IDs |

```rust
fn derive_encryption_key() -> [u8; 32] {
    // Get machine-specific identifiers
    let machine_id = get_machine_id(); // Hardware UUID
    let user_id = get_user_id();       // Unix UID
    
    // Derive key using PBKDF2
    let salt = format!("personalagent-{}-{}", machine_id, user_id);
    pbkdf2_hmac_sha256(
        machine_id.as_bytes(),
        salt.as_bytes(),
        100_000, // iterations
    )
}
```

### Future: macOS Keychain

```rust
pub trait KeychainBackend {
    fn store(&self, service: &str, account: &str, secret: &[u8]) -> Result<()>;
    fn retrieve(&self, service: &str, account: &str) -> Result<Option<Vec<u8>>>;
    fn delete(&self, service: &str, account: &str) -> Result<()>;
}

// Service name format: "PersonalAgent-{resource_type}"
// Account name format: "{resource_id}-{secret_name}"
```

---

## Operations

### Store Secret

| Step | Action |
|------|--------|
| 1 | Validate secret is non-empty |
| 2 | Derive encryption key |
| 3 | Generate random nonce |
| 4 | Encrypt secret with AES-256-GCM |
| 5 | Write encrypted file atomically |

```rust
fn store(&self, key: &SecretKey, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(Error::EmptySecret);
    }
    
    let encryption_key = self.derive_key();
    let nonce = generate_nonce();
    let ciphertext = aes_gcm_encrypt(&encryption_key, &nonce, value.as_bytes())?;
    
    let encrypted = EncryptedSecret {
        version: 1,
        algorithm: "AES-256-GCM".to_string(),
        nonce: nonce.to_vec(),
        ciphertext,
    };
    
    let path = self.secret_path(key);
    atomic_write(&path, &encrypted)?;
    
    Ok(())
}
```

### Retrieve Secret

| Step | Action |
|------|--------|
| 1 | Check file exists |
| 2 | Read encrypted file |
| 3 | Derive encryption key |
| 4 | Decrypt with AES-256-GCM |
| 5 | Return plaintext |

```rust
fn get(&self, key: &SecretKey) -> Result<Option<String>> {
    let path = self.secret_path(key);
    
    if !path.exists() {
        return Ok(None);
    }
    
    let encrypted: EncryptedSecret = read_json(&path)?;
    let encryption_key = self.derive_key();
    
    let plaintext = aes_gcm_decrypt(
        &encryption_key,
        &encrypted.nonce,
        &encrypted.ciphertext,
    )?;
    
    Ok(Some(String::from_utf8(plaintext)?))
}
```

### OAuth Token Refresh

| Step | Action |
|------|--------|
| 1 | Get stored tokens |
| 2 | Check if access token expired |
| 3 | If expired and has refresh token, refresh |
| 4 | Store new tokens |
| 5 | Return tokens |

```rust
async fn get_oauth_tokens(&self, key: &SecretKey) -> Result<Option<OAuthTokens>> {
    let tokens_json = self.get(key)?;
    
    let Some(json) = tokens_json else {
        return Ok(None);
    };
    
    let mut tokens: OAuthTokens = serde_json::from_str(&json)?;
    
    if tokens.is_expired() && tokens.can_refresh() {
        tokens = self.refresh_oauth_tokens(key, &tokens).await?;
        self.store_oauth_tokens(key, &tokens)?;
    }
    
    Ok(Some(tokens))
}

async fn refresh_oauth_tokens(&self, key: &SecretKey, tokens: &OAuthTokens) -> Result<OAuthTokens> {
    let provider = self.get_oauth_provider(key)?;
    
    let response = reqwest::Client::new()
        .post(&provider.token_url)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", tokens.refresh_token.as_ref().unwrap()),
            ("client_id", &provider.client_id),
            ("client_secret", &provider.client_secret),
        ])
        .send()
        .await?;
    
    let new_tokens: TokenResponse = response.json().await?;
    
    Ok(OAuthTokens {
        access_token: new_tokens.access_token,
        refresh_token: new_tokens.refresh_token.or(tokens.refresh_token.clone()),
        expires_at: new_tokens.expires_in.map(|s| Utc::now() + Duration::seconds(s)),
        token_type: new_tokens.token_type,
        scopes: tokens.scopes.clone(),
        username: tokens.username.clone(),
    })
}
```

### Read Keyfile

| Step | Action |
|------|--------|
| 1 | Expand path (~ to home) |
| 2 | Check file exists |
| 3 | Check file permissions (warn if too permissive) |
| 4 | Read file content |
| 5 | Trim whitespace |
| 6 | Return content |

```rust
fn read_keyfile(&self, path: &Path) -> Result<String> {
    let expanded = expand_tilde(path);
    
    if !expanded.exists() {
        return Err(Error::KeyfileNotFound(expanded));
    }
    
    // Check permissions (Unix only)
    #[cfg(unix)]
    {
        let mode = expanded.metadata()?.permissions().mode();
        if mode & 0o077 != 0 {
            log::warn!("Keyfile {} has permissive permissions {:o}", expanded.display(), mode);
        }
    }
    
    let content = std::fs::read_to_string(&expanded)?;
    Ok(content.trim().to_string())
}
```

### Mask Secret

```rust
fn mask(&self, value: &str, visible_chars: usize) -> String {
    if value.len() <= visible_chars {
        return "•".repeat(value.len());
    }
    
    let visible = &value[value.len() - visible_chars..];
    let masked_len = value.len() - visible_chars;
    
    format!("{}{}", "•".repeat(masked_len.min(20)), visible)
}

// Examples:
// mask("sk-abc123xyz", 4) -> "•••••••xyz"
// mask("short", 4)        -> "•hort"
// mask("ab", 4)           -> "••"
```

### Delete All for Resource

| Step | Action |
|------|--------|
| 1 | List all secrets for resource |
| 2 | Delete each secret file |
| 3 | Log deletions |

```rust
fn delete_all_for_resource(&self, resource_type: ResourceType, resource_id: Uuid) -> Result<()> {
    let prefix = format!("{}_{}", resource_type.prefix(), resource_id);
    let secrets_dir = self.secrets_dir();
    
    for entry in std::fs::read_dir(&secrets_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        
        if name.starts_with(&prefix) {
            std::fs::remove_file(entry.path())?;
            log::info!("Deleted secret: {}", name);
        }
    }
    
    Ok(())
}
```

---

## Security Considerations

### Secret Handling Rules

| Rule | Implementation |
|------|----------------|
| No logging | Never log secret values |
| Memory clearing | Zero memory after use (when possible) |
| Secure comparison | Use constant-time comparison |
| File permissions | 0600 for secret files |
| Atomic writes | Use temp file + rename |

### Logging Safety

```rust
impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never log the actual secret name for security
        write!(f, "SecretKey({:?}, {}, [redacted])", 
            self.resource_type, self.resource_id)
    }
}
```

### Error Messages

```rust
pub enum SecretsError {
    // Safe to display
    NotFound,
    KeyfileNotFound(PathBuf),
    InvalidPath,
    PermissionDenied,
    
    // Sanitize before display
    DecryptionFailed, // Don't expose why
    StorageFailed,    // Don't expose path
}
```

---

## OAuth Flow Support

### OAuth Provider Configuration

```rust
pub struct OAuthProvider {
    pub provider_id: String,
    pub authorization_url: String,
    pub token_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
    pub callback_scheme: String, // "personalagent"
}

// Well-known providers
impl OAuthProvider {
    pub fn github() -> Self {
        Self {
            provider_id: "github".to_string(),
            authorization_url: "https://github.com/login/oauth/authorize".to_string(),
            token_url: "https://github.com/login/oauth/access_token".to_string(),
            client_id: env!("GITHUB_CLIENT_ID").to_string(),
            client_secret: env!("GITHUB_CLIENT_SECRET").to_string(),
            scopes: vec!["repo".to_string(), "read:user".to_string()],
            callback_scheme: "personalagent".to_string(),
        }
    }
}
```

### OAuth Exchange

```rust
impl SecretsService {
    /// Exchange authorization code for tokens
    pub async fn exchange_oauth_code(
        &self,
        provider: &OAuthProvider,
        code: &str,
        state: &str,
        expected_state: &str,
    ) -> Result<OAuthTokens> {
        // Verify state to prevent CSRF
        if state != expected_state {
            return Err(Error::OAuthStateMismatch);
        }
        
        let response = reqwest::Client::new()
            .post(&provider.token_url)
            .header("Accept", "application/json")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", &provider.client_id),
                ("client_secret", &provider.client_secret),
            ])
            .send()
            .await?;
        
        let token_response: TokenResponse = response.json().await?;
        
        // Fetch user info if available
        let username = self.fetch_oauth_username(provider, &token_response.access_token).await.ok();
        
        Ok(OAuthTokens {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token,
            expires_at: token_response.expires_in.map(|s| Utc::now() + Duration::seconds(s)),
            token_type: token_response.token_type,
            scopes: provider.scopes.clone(),
            username,
        })
    }
}
```

---

## UI Integration

### Profile Editor

| Action | Service Call |
|--------|--------------|
| Save API key | `store(SecretKey::profile_api_key(id), key)` |
| Load API key | `get(SecretKey::profile_api_key(id))` |
| Display masked | `mask(key, 4)` → "•••••xyz" |
| Delete profile | `delete_all_for_resource(Profile, id)` |

### MCP Configure

| Action | Service Call |
|--------|--------------|
| Save env var | `store(SecretKey::mcp_env_var(id, name), value)` |
| Load env var | `get(SecretKey::mcp_env_var(id, name))` |
| OAuth login | `exchange_oauth_code(provider, code, state, expected)` |
| Store OAuth | `store_oauth_tokens(SecretKey::mcp_oauth(id), tokens)` |
| Delete MCP | `delete_all_for_resource(Mcp, id)` |

---

## Event Emissions

SecretsService does **not** emit events directly. It is a low-level infrastructure service.

Events related to secrets are emitted by higher-level services:
- `ProfileEvent::Created/Updated` when profiles with API keys are saved (emitted by ProfileService)
- `McpEvent::ConfigSaved` when MCP env vars are stored (emitted by McpService)

**Rationale:** Secret operations are implementation details of higher-level operations. The UI cares about "profile saved" not "secret stored".

---

## Test Requirements

| ID | Test |
|----|------|
| SE-T1 | Store and retrieve secret |
| SE-T2 | Delete secret removes file |
| SE-T3 | Non-existent secret returns None |
| SE-T4 | Mask shows correct visible chars |
| SE-T5 | Keyfile read trims whitespace |
| SE-T6 | Keyfile not found returns error |
| SE-T7 | OAuth tokens serialization |
| SE-T8 | OAuth expiry detection |
| SE-T9 | OAuth refresh flow |
| SE-T10 | Delete all for resource cleans up |
| SE-T11 | Encryption/decryption roundtrip |
| SE-T12 | Secret files have 0600 permissions |
