use personal_agent::{AuthConfig, LlmClient, ModelProfile};
use tempfile::TempDir;

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
