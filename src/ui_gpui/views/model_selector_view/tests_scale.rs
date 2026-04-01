//! Phase 3 scale tests for `ModelSelectorView`.
//!
//! These are pure state-level tests (no GPUI context) that validate
//! performance and correctness at 4K+ model scale.

use super::*;

/// Generate scale-test data: 4,108 models across 105 providers.
fn scale_test_data() -> (Vec<ProviderInfo>, Vec<ModelInfo>) {
    let num_providers = 105;
    let num_models = 4108;
    let mut providers = Vec::with_capacity(num_providers);
    let mut models = Vec::with_capacity(num_models);

    for p in 0..num_providers {
        let pid = format!("provider-{p:03}");
        providers.push(ProviderInfo::new(pid.clone(), pid.clone()));
    }

    for m in 0..num_models {
        let pid = format!("provider-{:03}", m % num_providers);
        let mid = format!("model-{m:04}");
        models.push(
            ModelInfo::new(mid, pid)
                .with_context(128_000)
                .with_capabilities(m % 3 == 0, m % 5 == 0),
        );
    }

    (providers, models)
}

// --- Test 37: scale test with 4K models ---
#[test]
fn test_scale_test_4k_models_rebuild_display_rows() {
    let (providers, models) = scale_test_data();
    let mut state = ModelSelectorState::new();
    state.load_models(providers, models);

    assert_eq!(state.cached_providers.len(), 105);
    // 105 provider headers + 4108 model rows = 4213 total
    assert_eq!(state.cached_display_rows.len(), 4213);
    assert_eq!(state.cached_model_count, 4108);
    assert_eq!(state.cached_provider_count, 105);

    // Search filter: "model-0001" should match exactly 1 model
    state.search_query = "model-0001".to_string();
    state.rebuild_display_rows();
    // 1 model + 1 provider header = 2 rows
    assert_eq!(state.cached_model_count, 1);
    assert!(state.cached_display_rows.len() <= 2);
}

// --- Test 38: scale performance benchmark ---
#[test]
#[ignore = "Performance benchmark — run explicitly: cargo test scale_test_4k -- --ignored"]
fn test_scale_test_4k_models_filter_performance() {
    let (providers, models) = scale_test_data();
    let mut state = ModelSelectorState::new();
    state.load_models(providers, models);

    let start = std::time::Instant::now();
    state.rebuild_display_rows();
    let elapsed = start.elapsed();

    eprintln!("rebuild_display_rows with 4108 models: {elapsed:?}");
    assert!(
        elapsed.as_millis() < 20,
        "rebuild_display_rows took {elapsed:?}, exceeds 20ms threshold"
    );

    // Also measure filtered rebuild
    state.search_query = "model-00".to_string();
    let start = std::time::Instant::now();
    state.rebuild_display_rows();
    let filtered_elapsed = start.elapsed();

    eprintln!("rebuild_display_rows with search filter: {filtered_elapsed:?}");
    assert!(
        filtered_elapsed.as_millis() < 20,
        "filtered rebuild took {filtered_elapsed:?}, exceeds 20ms threshold"
    );
}

// --- Test 39: scale memory — no unbounded growth ---
#[test]
fn test_scale_test_4k_models_memory_no_growth() {
    let (providers, models) = scale_test_data();
    let mut state = ModelSelectorState::new();
    state.load_models(providers, models);

    let initial_rows = state.cached_display_rows.len();
    assert_eq!(initial_rows, 4213);

    // Apply 10 different filter combinations and verify rows track correctly
    let filters: Vec<(&str, Option<&str>, bool, bool)> = vec![
        ("", None, false, false),                         // all
        ("model-00", None, false, false),                 // search
        ("", Some("provider-001"), false, false),         // provider filter
        ("", None, true, false),                          // reasoning only
        ("", None, false, true),                          // vision only
        ("model-01", Some("provider-001"), false, false), // search + provider
        ("", None, true, true),                           // both capabilities
        ("model-0", None, false, false),                  // broad search
        ("nonexistent_zzz", None, false, false),          // no matches
        ("", None, false, false),                         // back to all
    ];

    for (query, provider, reasoning, vision) in filters {
        state.search_query = query.to_string();
        state.selected_provider = provider.map(String::from);
        state.filter_reasoning = reasoning;
        state.filter_vision = vision;
        state.rebuild_display_rows();

        // Verify consistency: model_count + provider_count = display_rows
        let header_count = state
            .cached_display_rows
            .iter()
            .filter(|r| matches!(r, DisplayRow::ProviderHeader(_)))
            .count();
        let model_count = state
            .cached_display_rows
            .iter()
            .filter(|r| matches!(r, DisplayRow::Model(_)))
            .count();
        assert_eq!(header_count + model_count, state.cached_display_rows.len());
        assert_eq!(model_count, state.cached_model_count);
    }

    // Final: back to all, should match initial
    assert_eq!(state.cached_display_rows.len(), initial_rows);
}
