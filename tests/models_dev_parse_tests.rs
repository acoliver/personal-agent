use personal_agent::registry::ModelRegistry;

#[test]
fn model_registry_parses_bool_or_object_fields() {
    let raw = r#"{
        "openai": {
            "id": "openai",
            "name": "OpenAI",
            "env": ["OPENAI_API_KEY"],
            "models": {
                "gpt-test": {
                    "id": "gpt-test",
                    "name": "GPT Test",
                    "interleaved": {"field": "reasoning_content"},
                    "tool_call": true
                }
            }
        }
    }"#;

    let registry: ModelRegistry = serde_json::from_str(raw).expect("valid registry json");
    let model = registry
        .get_model("openai", "gpt-test")
        .expect("model exists");

    assert!(model.interleaved);
    assert!(model.tool_call);
}
