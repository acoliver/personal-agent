use personal_agent::{AuthConfig, ModelProfile};

pub const PROVIDER_ENV: &str = "PA_E2E_PROVIDER_ID";
pub const MODEL_ENV: &str = "PA_E2E_MODEL_ID";
pub const BASE_URL_ENV: &str = "PA_E2E_BASE_URL";
pub const KEY_LABEL_ENV: &str = "PA_E2E_KEY_LABEL";

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

#[must_use]
pub fn load_e2e_profile() -> ModelProfile {
    let provider_id = env_or(PROVIDER_ENV, "ollama");
    let model_id = env_or(MODEL_ENV, "minimax-m2.7:cloud");
    let base_url = env_or(BASE_URL_ENV, "https://ollama.com/v1");
    let key_label = env_or(KEY_LABEL_ENV, "pa-e2e-ollama-cloud");

    ModelProfile::new(
        "E2E Configured Profile".to_string(),
        provider_id,
        model_id,
        base_url,
        AuthConfig::Keychain { label: key_label },
    )
}
