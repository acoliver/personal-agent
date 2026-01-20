use personal_agent::{AuthConfig, ModelParameters, ModelProfile};

#[test]
fn model_profile_apply_parameters_and_auth() {
    let parameters = ModelParameters {
        temperature: 0.2,
        top_p: 0.3,
        max_tokens: 512,
        thinking_budget: Some(256),
        enable_thinking: true,
        show_thinking: true,
    };

    let mut profile = ModelProfile::new(
        "Test".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        "https://api.example.com".to_string(),
        AuthConfig::Key {
            value: "secret".to_string(),
        },
    )
    .with_parameters(parameters.clone());

    profile.set_name("Updated".to_string());
    profile.set_auth(AuthConfig::Keyfile {
        path: "/tmp/key".to_string(),
    });

    assert_eq!(profile.name, "Updated");
    assert_eq!(profile.parameters, parameters);
    assert!(matches!(profile.auth, AuthConfig::Keyfile { .. }));
}
