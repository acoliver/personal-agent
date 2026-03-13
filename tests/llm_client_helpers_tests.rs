use personal_agent::{AuthConfig, LlmClient, ModelProfile};
use tempfile::TempDir;

fn profile(provider_id: &str, model_id: &str, base_url: &str) -> ModelProfile {
    ModelProfile::new(
        "Test".to_string(),
        provider_id.to_string(),
        model_id.to_string(),
        base_url.to_string(),
        AuthConfig::Key {
            value: "secret".to_string(),
        },
    )
}

#[test]
fn llm_client_rejects_empty_api_key() {
    let profile = ModelProfile::new(
        "Test".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        String::new(),
        AuthConfig::Key {
            value: String::new(),
        },
    );

    let result = LlmClient::from_profile(&profile);
    assert!(result.is_err());
}

#[test]
fn llm_client_rejects_whitespace_only_api_key() {
    let profile = ModelProfile::new(
        "Test".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        String::new(),
        AuthConfig::Key {
            value: "   
	  "
            .to_string(),
        },
    );

    let result = LlmClient::from_profile(&profile);
    assert!(result.is_err());
}

#[test]
fn llm_client_reads_keyfile() {
    let temp_dir = TempDir::new().unwrap();
    let key_path = temp_dir.path().join("key.txt");
    std::fs::write(&key_path, "secret").unwrap();

    let profile = ModelProfile::new(
        "Test".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        String::new(),
        AuthConfig::Keyfile {
            path: key_path.to_string_lossy().to_string(),
        },
    );

    let result = LlmClient::from_profile(&profile);
    assert!(result.is_ok());
}

#[test]
fn llm_client_errors_on_missing_keyfile() {
    let temp_dir = TempDir::new().unwrap();
    let missing_path = temp_dir.path().join("missing.txt");

    let profile = ModelProfile::new(
        "Test".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        String::new(),
        AuthConfig::Keyfile {
            path: missing_path.to_string_lossy().to_string(),
        },
    );

    let result = LlmClient::from_profile(&profile);
    assert!(result.is_err());
}

#[test]
fn llm_client_reads_keyfile_with_surrounding_whitespace_in_path() {
    let temp_dir = TempDir::new().unwrap();
    let key_path = temp_dir.path().join("trimmed-key.txt");
    std::fs::write(&key_path, "secret").unwrap();

    let profile = ModelProfile::new(
        "Test".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        String::new(),
        AuthConfig::Keyfile {
            path: format!(
                "  {}
",
                key_path.to_string_lossy()
            ),
        },
    );

    let result = LlmClient::from_profile(&profile);
    assert!(result.is_ok());
}

#[test]
fn llm_client_rejects_whitespace_only_keyfile_path() {
    let profile = ModelProfile::new(
        "Test".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        String::new(),
        AuthConfig::Keyfile {
            path: "  	
 "
            .to_string(),
        },
    );

    let result = LlmClient::from_profile(&profile);
    assert!(result.is_err());
}

#[test]
fn llm_client_model_spec_uses_openai_transport_for_kimi_quirk() {
    let profile = profile("kimi-for-coding", "kimi-for-coding", "");

    let client = LlmClient::from_profile(&profile).expect("client");

    assert_eq!(client.model_spec(), "openai:kimi-for-coding");
}
