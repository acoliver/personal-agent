use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

static AGENT_RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("agent-runtime")
        .build()
        .expect("Failed to create agent runtime")
});

/// Get the global agent runtime
pub fn agent_runtime() -> &'static Runtime {
    &AGENT_RUNTIME
}

/// Run a future in the agent runtime (blocking)
pub fn run_in_agent_runtime<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    AGENT_RUNTIME.block_on(future)
}

/// Spawn a task in the agent runtime (non-blocking)
pub fn spawn_in_agent_runtime<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    AGENT_RUNTIME.spawn(future)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_global_runtime_exists() {
        let runtime = agent_runtime();
        // Runtime exists and can spawn tasks
        let handle = runtime.spawn(async { 42 });
        let result = runtime.block_on(handle).unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn test_runtime_survives_multiple_calls() {
        // First spawn
        let result1 = run_in_agent_runtime(async { 1 });
        assert_eq!(result1, 1);

        // Second spawn - same runtime, still works
        let result2 = run_in_agent_runtime(async { 2 });
        assert_eq!(result2, 2);
    }

    #[test]
    fn test_spawn_in_global_runtime() {
        let result = run_in_agent_runtime(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            42
        });
        assert_eq!(result, 42);
    }

    #[test]
    fn test_concurrent_operations() {
        let handles: Vec<_> = (0..10)
            .map(|i| {
                std::thread::spawn(move || run_in_agent_runtime(async move { i * 2 }))
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(results.len(), 10);
    }
}
