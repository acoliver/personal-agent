use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

/// Global tokio runtime for agent operations.
///
/// This runtime is created once and persists for the entire application lifetime.
/// It solves the "runtime shutdown" problem where MCP clients would die when
/// temporary runtimes shut down. All agent and MCP operations should use this
/// runtime instead of creating temporary runtimes.
///
/// # Pattern
/// - Created lazily on first access using `once_cell::sync::Lazy`
/// - Multi-threaded runtime with all features enabled
/// - Thread prefix: "agent-runtime"
/// - Never dropped until application exits
static AGENT_RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("agent-runtime")
        .build()
        .expect("Failed to create agent runtime")
});

/// Get the global agent runtime.
///
/// This runtime persists for the entire application lifetime and should be used
/// for all agent and MCP operations to avoid runtime shutdown issues.
#[must_use]
pub fn agent_runtime() -> &'static Runtime {
    &AGENT_RUNTIME
}

/// Run a future in the agent runtime (blocking).
///
/// This function blocks the current thread until the future completes.
/// Use this when you need to call async code from a sync context.
///
/// # Example
/// ```ignore
/// let result = run_in_agent_runtime(async {
///     // Your async code here
///     42
/// });
/// ```
pub fn run_in_agent_runtime<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    AGENT_RUNTIME.block_on(future)
}

/// Spawn a task in the agent runtime (non-blocking).
///
/// This function returns immediately with a `JoinHandle`.
/// Use this when you want to fire-and-forget an async task or when you
/// can await the handle later.
///
/// # Example
/// ```ignore
/// let handle = spawn_in_agent_runtime(async {
///     // Your async code here
///     42
/// });
/// // Can await the handle later if needed
/// ```
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

    #[test]
    fn run_in_agent_runtime_returns_future_result() {
        let output = run_in_agent_runtime(async { 42 });
        assert_eq!(output, 42);
    }

    #[test]
    fn spawn_in_agent_runtime_executes_task() {
        let handle = spawn_in_agent_runtime(async { "done".to_string() });
        let output = run_in_agent_runtime(async move { handle.await.unwrap() });
        assert_eq!(output, "done");
    }
}
