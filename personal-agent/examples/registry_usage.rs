//! Example demonstrating how to use the registry module

use personal_agent::RegistryManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a registry manager with default settings
    // This uses default cache location: ~/Library/Application Support/PersonalAgent/cache/models.json
    // Cache expires after 24 hours
    let manager = RegistryManager::new()?;

    println!("Fetching model registry from models.dev...\n");
    let registry = manager.get_registry().await?;

    // List all providers
    let provider_ids = registry.get_provider_ids();
    println!("Available providers ({} total):", provider_ids.len());
    for (i, provider_id) in provider_ids.iter().take(10).enumerate() {
        if let Some(provider) = registry.get_provider(provider_id) {
            println!(
                "  {}. {} ({}) - {} models",
                i + 1,
                provider.name,
                provider.id,
                provider.models.len()
            );
        }
    }
    if provider_ids.len() > 10 {
        println!("  ... and {} more", provider_ids.len() - 10);
    }

    // Example: Get models with tool calling capability
    println!("\n--- Models with Tool Calling ---");
    let tool_models = registry.get_tool_call_models();
    println!("Found {} models with tool calling capability", tool_models.len());
    for (provider_id, model) in tool_models.iter().take(5) {
        println!("  - {} / {} ({})", provider_id, model.name, model.id);
        if let Some(limit) = &model.limit {
            println!(
                "    Context: {}, Output: {}",
                limit.context, limit.output
            );
        }
    }

    // Example: Get models with reasoning capability
    println!("\n--- Models with Reasoning ---");
    let reasoning_models = registry.get_reasoning_models();
    println!("Found {} models with reasoning capability", reasoning_models.len());
    for (provider_id, model) in reasoning_models.iter().take(5) {
        println!("  - {} / {} ({})", provider_id, model.name, model.id);
    }

    // Example: Search for multimodal models (accept multiple input types)
    println!("\n--- Multimodal Models ---");
    let multimodal = registry.search_models(|model| {
        model
            .modalities
            .as_ref()
            .map(|m| m.input.len() > 1)
            .unwrap_or(false)
    });
    println!("Found {} multimodal models", multimodal.len());
    for (provider_id, model) in multimodal.iter().take(5) {
        if let Some(modalities) = &model.modalities {
            println!(
                "  - {} / {} ({})",
                provider_id, model.name, model.id
            );
            println!(
                "    Input: {}, Output: {}",
                modalities.input.join(", "),
                modalities.output.join(", ")
            );
        }
    }

    // Example: Get models from a specific provider
    if let Some(provider_id) = provider_ids.first() {
        println!("\n--- Models from {} ---", provider_id);
        if let Some(models) = registry.get_models_for_provider(provider_id) {
            println!("Found {} models", models.len());
            for model in models.iter().take(5) {
                println!("  - {} ({})", model.name, model.id);
                if let Some(cost) = &model.cost {
                    println!(
                        "    Cost: input ${}, output ${}",
                        cost.input, cost.output
                    );
                }
            }
        }
    }

    // Cache metadata
    println!("\n--- Cache Information ---");
    if let Some(metadata) = manager.cache_metadata()? {
        println!("Cache created: {}", metadata.cached_at);
        println!("Cache size: {} bytes", metadata.size_bytes);
        println!("Cache expired: {}", metadata.is_expired);
    } else {
        println!("No cache found");
    }

    // Force refresh (uncomment to test)
    // println!("\nForce refreshing registry...");
    // let fresh_registry = manager.refresh().await?;
    // println!("Refreshed! {} providers available", fresh_registry.get_provider_ids().len());

    // Clear cache (uncomment to test)
    // manager.clear_cache()?;
    // println!("\nCache cleared");

    Ok(())
}
