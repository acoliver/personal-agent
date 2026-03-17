use personal_agent::mcp::secrets::SecretsError;
use personal_agent::mcp::SecretsManager;
use uuid::Uuid;

#[test]
fn secrets_manager_round_trip_and_delete() {
    personal_agent::services::secure_store::use_mock_backend();
    let manager = SecretsManager::new();

    let mcp_id = Uuid::new_v4();
    manager.store_api_key(mcp_id, "secret").unwrap();
    let loaded = manager.load_api_key(mcp_id).unwrap();
    assert_eq!(loaded, "secret");

    manager.delete_api_key(mcp_id).unwrap();
    let result = manager.load_api_key(mcp_id);
    assert!(matches!(result, Err(SecretsError::SecretNotFound(_))));
}
