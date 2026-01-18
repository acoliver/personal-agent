#![allow(clippy::unwrap_used)]

use objc2_foundation::NSString;
use personal_agent::registry::{Cost, Limit, ModelInfo};
use personal_agent::RegistryManager;

#[test]
fn model_selector_format_helpers() {
    let limit = Limit {
        context: 128_000,
        output: 4096,
    };
    let cost = Cost {
        input: 0.000001,
        output: 0.000002,
        cache_read: None,
    };

    let model = ModelInfo {
        id: "test-model".to_string(),
        name: "Test Model".to_string(),
        family: None,
        attachment: false,
        reasoning: true,
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
        cost: Some(cost),
        limit: Some(limit),
    };

    assert!(model.id.contains("test"));
    assert!(model.reasoning);
    assert!(model.tool_call);
}

#[test]
fn registry_manager_default() {
    let manager = RegistryManager::new();
    assert!(manager.is_ok());
}

#[test]
fn nsstring_round_trip() {
    let text = NSString::from_str("hello");
    assert_eq!(text.to_string(), "hello");
}
