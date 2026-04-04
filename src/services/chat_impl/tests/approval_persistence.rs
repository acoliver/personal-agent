use super::*;

#[tokio::test]
async fn resolve_tool_approval_proceed_always_is_atomic_when_persistence_fails() {
    let app_settings = Arc::new(FailingAppSettingsService) as Arc<dyn AppSettingsService>;
    let (service, _view_rx, approval_gate) = make_approval_test_chat_service(app_settings);
    let request_id = Uuid::new_v4().to_string();
    let waiter = approval_gate.wait_for_approvals(
        request_id.clone(),
        vec!["git status".to_string(), "pwd".to_string()],
    );

    let error = service
        .resolve_tool_approval(request_id, ToolApprovalResponseAction::ProceedAlways)
        .await
        .expect_err("ProceedAlways should fail when persistence fails");

    assert!(
        matches!(error, ServiceError::Storage(_)),
        "persistence failure should bubble up as storage error"
    );

    let policy_after = service.policy.lock().await.clone();
    assert!(
        policy_after.persistent_allowlist.is_empty(),
        "persistent allowlist should remain unchanged when batch persist fails"
    );

    drop(waiter);
}

#[tokio::test]
async fn resolve_tool_approval_returns_error_when_persistence_fails() {
    let app_settings = Arc::new(FailingAppSettingsService) as Arc<dyn AppSettingsService>;
    let (service, _view_rx, approval_gate) = make_approval_test_chat_service(app_settings);
    let request_id = Uuid::new_v4().to_string();
    let waiter = approval_gate.wait_for_approval(request_id.clone(), "WriteFile".to_string());

    let error = service
        .resolve_tool_approval(request_id, ToolApprovalResponseAction::ProceedAlways)
        .await
        .expect_err("ProceedAlways should fail when persistence fails");

    assert!(
        matches!(error, ServiceError::Storage(_)),
        "persistence failure should bubble up as storage error"
    );

    drop(waiter);
}
