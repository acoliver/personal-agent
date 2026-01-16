//! Quick test to verify cache loading works
use personal_agent::registry::RegistryCache;

fn main() {
    let cache_path = RegistryCache::default_path().expect("get path");
    println!("Cache path: {:?}", cache_path);

    let cache = RegistryCache::new(cache_path, 24);
    match cache.load() {
        Ok(Some(registry)) => {
            println!("Registry loaded! Providers: {}", registry.providers.len());
            if let Some(provider) = registry.providers.get("synthetic") {
                println!("Synthetic provider api: {:?}", provider.api);
            }
            if let Some(provider) = registry.providers.get("zai-coding-plan") {
                println!("zai-coding-plan provider api: {:?}", provider.api);
            }
        }
        Ok(None) => println!("No cache found or expired"),
        Err(e) => println!("Error loading cache: {:?}", e),
    }
}
