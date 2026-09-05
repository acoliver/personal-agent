//! Shared provider-backed E2E configuration.
//!
//! Every `PA_E2E_*` variable is required; none has a default, so an
//! unconfigured run fails immediately instead of silently pointing at a
//! stale provider.
//!
//! `PA_E2E_PROVIDER_ID` doubles as the wire-protocol selector: `anthropic`
//! (or `claude`) selects the Anthropic wire protocol, and any other value
//! selects an OpenAI-compatible protocol. Point `PA_E2E_BASE_URL` at the
//! endpoint on the vendor that matches that choice.

use personal_agent::{AuthConfig, ModelProfile};

pub const PROVIDER_ENV: &str = "PA_E2E_PROVIDER_ID";
pub const MODEL_ENV: &str = "PA_E2E_MODEL_ID";
pub const BASE_URL_ENV: &str = "PA_E2E_BASE_URL";
pub const KEY_LABEL_ENV: &str = "PA_E2E_KEY_LABEL";

/// Reads a required environment variable, trimming surrounding whitespace.
///
/// Returns the variable name as the error when it is unset or empty.
fn required_env(name: &str) -> Result<String, String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| name.to_string())
}

/// Builds the [`ModelProfile`] used by the provider-backed E2E tests.
///
/// Panics when any required variable is unset or empty, naming every
/// missing variable at once.
#[must_use]
pub fn load_e2e_profile() -> ModelProfile {
    let mut missing = Vec::new();
    let mut values = Vec::new();
    for name in [PROVIDER_ENV, MODEL_ENV, BASE_URL_ENV, KEY_LABEL_ENV] {
        match required_env(name) {
            Ok(value) => values.push(value),
            Err(_) => missing.push(name),
        }
    }

    assert!(
        missing.is_empty(),
        "provider-backed E2E tests are not configured; missing environment variables: {} \
         (the API key itself must be in the secure store under the PA_E2E_KEY_LABEL label)",
        missing.join(", ")
    );

    let [provider_id, model_id, base_url, key_label]: [String; 4] = values
        .try_into()
        .expect("exactly four variables were collected above");

    ModelProfile::new(
        "E2E Configured Profile".to_string(),
        provider_id,
        model_id,
        base_url,
        AuthConfig::Keychain { label: key_label },
    )
}
