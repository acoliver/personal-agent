use personal_agent::{AuthConfig, LlmClient, ModelProfile};

fn mock_backend() {
    personal_agent::services::secure_store::use_mock_backend();
}

fn store_test_key(label: &str, value: &str) {
    mock_backend();
    personal_agent::services::secure_store::api_keys::store(label, value)
        .expect("store test key in keychain");
}

fn delete_test_key(label: &str) {
    let _ = personal_agent::services::secure_store::api_keys::delete(label);
}

fn profile_with_label(
    provider_id: &str,
    model_id: &str,
    base_url: &str,
    label: &str,
) -> ModelProfile {
    ModelProfile::new(
        "Test".to_string(),
        provider_id.to_string(),
        model_id.to_string(),
        base_url.to_string(),
        AuthConfig::Keychain {
            label: label.to_string(),
        },
    )
}

#[test]
fn llm_client_rejects_empty_api_key() {
    let profile = profile_with_label("openai", "gpt-4o", "", "");
    let result = LlmClient::from_profile(&profile);
    assert!(result.is_err());
}

#[test]
fn llm_client_rejects_whitespace_only_api_key() {
    let profile = profile_with_label("openai", "gpt-4o", "", "   \n\t  ");
    let result = LlmClient::from_profile(&profile);
    assert!(result.is_err());
}

#[test]
fn llm_client_resolves_keychain_label() {
    store_test_key("_test_helpers_resolve", "sk-test-secret");

    let profile = profile_with_label("openai", "gpt-4o", "", "_test_helpers_resolve");
    let result = LlmClient::from_profile(&profile);
    assert!(result.is_ok());

    delete_test_key("_test_helpers_resolve");
}

#[test]
fn llm_client_errors_on_missing_keychain_label() {
    // Use a label that definitely doesn't exist in the keychain
    let profile = profile_with_label("openai", "gpt-4o", "", "_nonexistent_test_label_xyz");
    let result = LlmClient::from_profile(&profile);
    assert!(result.is_err());
}

#[test]
fn llm_client_rejects_empty_keychain_value() {
    store_test_key("_test_helpers_empty_val", "   ");

    let profile = profile_with_label("openai", "gpt-4o", "", "_test_helpers_empty_val");
    let result = LlmClient::from_profile(&profile);
    assert!(result.is_err());

    delete_test_key("_test_helpers_empty_val");
}

#[test]
fn llm_client_trims_label_whitespace() {
    store_test_key("_test_helpers_trim", "sk-trimmed-key");

    let profile = profile_with_label("openai", "gpt-4o", "", "  _test_helpers_trim\n");
    let result = LlmClient::from_profile(&profile);
    assert!(result.is_ok());

    delete_test_key("_test_helpers_trim");
}

#[test]
fn llm_client_model_spec_uses_openai_transport_for_kimi_quirk() {
    store_test_key("_test_helpers_kimi", "sk-kimi-test");

    let profile = profile_with_label("kimi-for-coding", "kimi-for-coding", "", "_test_helpers_kimi");
    let client = LlmClient::from_profile(&profile).expect("client");
    assert_eq!(client.model_spec(), "openai:kimi-for-coding");

    delete_test_key("_test_helpers_kimi");
}
