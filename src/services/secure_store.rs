//! Secure credential storage backed by the OS keychain.
//!
//! Uses the `keyring` crate which maps to:
//!   - macOS: Keychain Services
//!   - Windows: Credential Manager
//!   - Linux: Secret Service (GNOME Keyring / KDE Wallet)
//!
//! All keys are stored under the service name "personal-agent".

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use thiserror::Error;

const SERVICE_NAME: &str = "personal-agent";

fn api_key_index_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join("PersonalAgent").join("api_key_index.json"))
}

// ── In-memory test backend ──────────────────────────────────────────────

static MOCK_ACTIVE: OnceLock<bool> = OnceLock::new();
static MOCK_STORE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn mock_store() -> &'static Mutex<HashMap<String, String>> {
    MOCK_STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn is_mock() -> bool {
    MOCK_ACTIVE.get().copied().unwrap_or(false)
}

/// Switch to an in-memory store for the rest of this process.
/// Call before any keychain access in tests to avoid OS permission prompts.
/// Safe to call multiple times — the switch happens exactly once.
pub fn use_mock_backend() {
    let _ = MOCK_ACTIVE.set(true);
}

#[derive(Debug, Error)]
pub enum SecureStoreError {
    #[error("Keychain error: {0}")]
    Keychain(String),
    #[error("Key not found: {0}")]
    NotFound(String),
}

#[cfg(target_os = "macos")]
fn macos_security_error(stderr: &[u8]) -> SecureStoreError {
    let message = String::from_utf8_lossy(stderr).trim().to_string();
    let message = if message.is_empty() {
        "security CLI command failed".to_string()
    } else {
        message
    };
    SecureStoreError::Keychain(message)
}

#[cfg(target_os = "macos")]
fn macos_security_not_found(stderr: &[u8]) -> bool {
    let stderr = String::from_utf8_lossy(stderr);
    stderr.contains("could not be found")
        || stderr.contains("The specified item could not be found")
}

#[cfg(target_os = "macos")]
fn macos_security_set_secret(key: &str, value: &str) -> Result<(), SecureStoreError> {
    let output = Command::new("security")
        .arg("add-generic-password")
        .arg("-U")
        .arg("-a")
        .arg(key)
        .arg("-s")
        .arg(SERVICE_NAME)
        .arg("-w")
        .arg(value)
        .output()
        .map_err(|e| SecureStoreError::Keychain(format!("Failed to run security CLI: {e}")))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(macos_security_error(&output.stderr))
    }
}

#[cfg(target_os = "macos")]
fn macos_security_get_secret(key: &str) -> Result<Option<String>, SecureStoreError> {
    let output = Command::new("security")
        .arg("find-generic-password")
        .arg("-a")
        .arg(key)
        .arg("-s")
        .arg(SERVICE_NAME)
        .arg("-w")
        .output()
        .map_err(|e| SecureStoreError::Keychain(format!("Failed to run security CLI: {e}")))?;

    if output.status.success() {
        let value = String::from_utf8(output.stdout).map_err(|e| {
            SecureStoreError::Keychain(format!("Invalid UTF-8 from security CLI: {e}"))
        })?;
        Ok(Some(value.trim_end_matches(['\n', '\r']).to_string()))
    } else if macos_security_not_found(&output.stderr) {
        Ok(None)
    } else {
        Err(macos_security_error(&output.stderr))
    }
}

#[cfg(target_os = "macos")]
fn macos_security_delete_secret(key: &str) -> Result<(), SecureStoreError> {
    let output = Command::new("security")
        .arg("delete-generic-password")
        .arg("-a")
        .arg(key)
        .arg("-s")
        .arg(SERVICE_NAME)
        .output()
        .map_err(|e| SecureStoreError::Keychain(format!("Failed to run security CLI: {e}")))?;

    if output.status.success() || macos_security_not_found(&output.stderr) {
        Ok(())
    } else {
        Err(macos_security_error(&output.stderr))
    }
}

/// Store a secret in the OS keychain (or mock store in tests).
///
/// # Errors
///
/// Returns `SecureStoreError` if the keychain entry cannot be created or written.
///
/// # Panics
///
/// Panics in mock mode if the in-memory mock store mutex is poisoned.
pub fn set_secret(key: &str, value: &str) -> Result<(), SecureStoreError> {
    if is_mock() {
        mock_store()
            .lock()
            .expect("mock store poisoned")
            .insert(key.to_string(), value.to_string());
        return Ok(());
    }
    let entry = keyring::Entry::new(SERVICE_NAME, key)
        .map_err(|e| SecureStoreError::Keychain(e.to_string()))?;
    match entry.set_password(value) {
        Ok(()) => Ok(()),
        Err(e) => {
            #[cfg(target_os = "macos")]
            {
                tracing::warn!(key = %key, error = %e, "Keyring set failed; falling back to security CLI");
                macos_security_set_secret(key, value)
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err(SecureStoreError::Keychain(e.to_string()))
            }
        }
    }
}

/// Retrieve a secret from the OS keychain (or mock store in tests).
/// Returns `None` if not found.
///
/// # Errors
///
/// Returns `SecureStoreError` if the keychain entry cannot be created or read.
///
/// # Panics
///
/// Panics in mock mode if the in-memory mock store mutex is poisoned.
pub fn get_secret(key: &str) -> Result<Option<String>, SecureStoreError> {
    if is_mock() {
        return Ok(mock_store()
            .lock()
            .expect("mock store poisoned")
            .get(key)
            .cloned());
    }
    let entry = keyring::Entry::new(SERVICE_NAME, key)
        .map_err(|e| SecureStoreError::Keychain(e.to_string()))?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => {
            #[cfg(target_os = "macos")]
            {
                tracing::warn!(key = %key, error = %e, "Keyring get failed; falling back to security CLI");
                macos_security_get_secret(key)
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err(SecureStoreError::Keychain(e.to_string()))
            }
        }
    }
}

/// Delete a secret from the OS keychain (or mock store in tests).
///
/// # Errors
///
/// Returns `SecureStoreError` if the keychain entry cannot be created or deleted.
///
/// # Panics
///
/// Panics in mock mode if the in-memory mock store mutex is poisoned.
pub fn delete_secret(key: &str) -> Result<(), SecureStoreError> {
    if is_mock() {
        mock_store()
            .lock()
            .expect("mock store poisoned")
            .remove(key);
        return Ok(());
    }
    let entry = keyring::Entry::new(SERVICE_NAME, key)
        .map_err(|e| SecureStoreError::Keychain(e.to_string()))?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => {
            #[cfg(target_os = "macos")]
            {
                tracing::warn!(key = %key, error = %e, "Keyring delete failed; falling back to security CLI");
                macos_security_delete_secret(key)
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err(SecureStoreError::Keychain(e.to_string()))
            }
        }
    }
}

/// Check if a secret exists in the OS keychain.
///
/// # Errors
///
/// Returns `SecureStoreError` if the underlying secret lookup fails.
pub fn has_secret(key: &str) -> Result<bool, SecureStoreError> {
    Ok(get_secret(key)?.is_some())
}

/// API key entry in the secure store.
/// The keychain key is `apikey:{label}`.
///
/// Because the keyring crate cannot enumerate entries, a JSON label index is
/// stored under the special keychain key `apikey:__index__`.
pub mod api_keys {
    use super::{
        api_key_index_path, delete_secret, fs, get_secret, has_secret, set_secret, SecureStoreError,
    };

    const PREFIX: &str = "apikey:";

    // ── label index helpers ──────────────────────────────────────────

    fn load_index() -> Vec<String> {
        api_key_index_path().map_or_else(Vec::new, |path| {
            fs::read_to_string(path).map_or_else(
                |_| Vec::new(),
                |json| serde_json::from_str(&json).unwrap_or_default(),
            )
        })
    }

    fn save_index(labels: &[String]) -> Result<(), SecureStoreError> {
        let json =
            serde_json::to_string(labels).map_err(|e| SecureStoreError::Keychain(e.to_string()))?;
        let Some(path) = api_key_index_path() else {
            return Err(SecureStoreError::Keychain(
                "Unable to resolve runtime path for api_key_index.json".to_string(),
            ));
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                SecureStoreError::Keychain(format!("Failed to create api key index dir: {e}"))
            })?;
        }
        fs::write(path, json)
            .map_err(|e| SecureStoreError::Keychain(format!("Failed to write api key index: {e}")))
    }

    fn add_to_index(label: &str) -> Result<(), SecureStoreError> {
        let mut idx = load_index();
        if !idx.iter().any(|l| l == label) {
            idx.push(label.to_string());
            idx.sort();
            save_index(&idx)?;
        }
        Ok(())
    }

    fn remove_from_index(label: &str) -> Result<(), SecureStoreError> {
        let mut idx = load_index();
        idx.retain(|l| l != label);
        save_index(&idx)
    }

    // ── public API ───────────────────────────────────────────────────

    /// Store an API key in the keychain under the given label and update the index.
    ///
    /// # Errors
    ///
    /// Returns `SecureStoreError` if the key cannot be stored or the label index cannot be updated.
    pub fn store(label: &str, value: &str) -> Result<(), SecureStoreError> {
        let key = format!("{PREFIX}{label}");
        set_secret(&key, value)?;
        add_to_index(label)
    }

    /// Retrieve an API key by label.
    ///
    /// # Errors
    ///
    /// Returns `SecureStoreError` if the underlying keychain lookup fails.
    pub fn get(label: &str) -> Result<Option<String>, SecureStoreError> {
        let key = format!("{PREFIX}{label}");
        get_secret(&key)
    }

    /// Delete an API key by label and remove it from the index.
    ///
    /// # Errors
    ///
    /// Returns `SecureStoreError` if the key cannot be deleted or the label index cannot be updated.
    pub fn delete(label: &str) -> Result<(), SecureStoreError> {
        let key = format!("{PREFIX}{label}");
        delete_secret(&key)?;
        remove_from_index(label)
    }

    /// Check if an API key exists for the given label.
    ///
    /// # Errors
    ///
    /// Returns `SecureStoreError` if the underlying keychain lookup fails.
    pub fn exists(label: &str) -> Result<bool, SecureStoreError> {
        let key = format!("{PREFIX}{label}");
        has_secret(&key)
    }

    /// Return all stored API key labels (sorted alphabetically).
    #[must_use]
    pub fn list() -> Vec<String> {
        load_index()
    }

    /// Get the masked display form of a key value (first 4 + last 4 chars visible).
    #[must_use]
    pub fn masked_display(value: &str) -> String {
        if value.len() <= 8 {
            "••••••••".to_string()
        } else {
            let prefix = &value[..4];
            let suffix = &value[value.len() - 4..];
            format!("{prefix}••••••••{suffix}")
        }
    }
}

/// MCP secret entry in the secure store.
/// The keychain key is `mcp:{config_id}` or `mcp:{config_id}:{var_name}`.
pub mod mcp_keys {
    use super::{delete_secret, get_secret, set_secret, SecureStoreError};
    use uuid::Uuid;

    /// Store an MCP API key.
    ///
    /// # Errors
    ///
    /// Returns `SecureStoreError` if the key cannot be stored.
    pub fn store(mcp_id: Uuid, value: &str) -> Result<(), SecureStoreError> {
        store_named(mcp_id, "default", value)
    }

    /// Store a named MCP API key (for MCPs with multiple env vars).
    ///
    /// # Errors
    ///
    /// Returns `SecureStoreError` if the key cannot be stored.
    pub fn store_named(mcp_id: Uuid, var_name: &str, value: &str) -> Result<(), SecureStoreError> {
        let key = if var_name == "default" {
            format!("mcp:{mcp_id}")
        } else {
            format!("mcp:{mcp_id}:{var_name}")
        };
        set_secret(&key, value)
    }

    /// Load an MCP API key.
    ///
    /// # Errors
    ///
    /// Returns `SecureStoreError` if the underlying keychain lookup fails.
    pub fn get(mcp_id: Uuid) -> Result<Option<String>, SecureStoreError> {
        get_named(mcp_id, "default")
    }

    /// Load a named MCP API key.
    ///
    /// # Errors
    ///
    /// Returns `SecureStoreError` if the underlying keychain lookup fails.
    pub fn get_named(mcp_id: Uuid, var_name: &str) -> Result<Option<String>, SecureStoreError> {
        let key = if var_name == "default" {
            format!("mcp:{mcp_id}")
        } else {
            format!("mcp:{mcp_id}:{var_name}")
        };
        get_secret(&key)
    }

    /// Delete all MCP keys for a config id.
    ///
    /// # Errors
    ///
    /// Returns `SecureStoreError` if the key cannot be deleted.
    pub fn delete(mcp_id: Uuid) -> Result<(), SecureStoreError> {
        // Delete default key
        let key = format!("mcp:{mcp_id}");
        delete_secret(&key)
    }

    /// Delete a specific named MCP key.
    ///
    /// # Errors
    ///
    /// Returns `SecureStoreError` if the key cannot be deleted.
    pub fn delete_named(mcp_id: Uuid, var_name: &str) -> Result<(), SecureStoreError> {
        let key = if var_name == "default" {
            format!("mcp:{mcp_id}")
        } else {
            format!("mcp:{mcp_id}:{var_name}")
        };
        delete_secret(&key)
    }
}
