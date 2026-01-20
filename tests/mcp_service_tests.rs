use personal_agent::mcp::McpService;

#[test]
fn mcp_service_global_initializes_empty() {
    let service = McpService::global();
    let guard = service.blocking_lock();

    assert_eq!(guard.active_count(), 0);
    assert!(!guard.has_active_mcps());
    assert!(guard.get_tools().is_empty());
    assert!(guard.get_llm_tools().is_empty());
}
