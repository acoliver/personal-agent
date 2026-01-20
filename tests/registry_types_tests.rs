use personal_agent::registry::{ModelInfo, ModelRegistry, Provider};
use std::collections::HashMap;

#[test]
fn registry_search_helpers_filter_models() {
    let mut models = HashMap::new();
    models.insert(
        "model-a".to_string(),
        ModelInfo {
            id: "model-a".to_string(),
            name: "Model A".to_string(),
            family: None,
            attachment: false,
            reasoning: true,
            tool_call: false,
            structured_output: false,
            temperature: true,
            interleaved: false,
            provider: None,
            status: None,
            knowledge: None,
            release_date: None,
            last_updated: None,
            modalities: None,
            open_weights: false,
            cost: None,
            limit: None,
        },
    );
    models.insert(
        "model-b".to_string(),
        ModelInfo {
            id: "model-b".to_string(),
            name: "Model B".to_string(),
            family: None,
            attachment: false,
            reasoning: false,
            tool_call: true,
            structured_output: false,
            temperature: true,
            interleaved: false,
            provider: None,
            status: None,
            knowledge: None,
            release_date: None,
            last_updated: None,
            modalities: None,
            open_weights: false,
            cost: None,
            limit: None,
        },
    );

    let provider = Provider {
        id: "provider".to_string(),
        name: "Provider".to_string(),
        env: vec!["API_KEY".to_string()],
        npm: None,
        api: None,
        doc: None,
        models,
    };

    let mut providers = HashMap::new();
    providers.insert("provider".to_string(), provider);

    let registry = ModelRegistry { providers };

    let tool_models = registry.get_tool_call_models();
    assert_eq!(tool_models.len(), 1);
    assert_eq!(tool_models[0].1.id, "model-b");

    let reasoning_models = registry.get_reasoning_models();
    assert_eq!(reasoning_models.len(), 1);
    assert_eq!(reasoning_models[0].1.id, "model-a");
}
