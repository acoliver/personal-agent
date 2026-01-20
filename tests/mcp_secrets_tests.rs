use personal_agent::mcp::secrets::SecretsError;
use personal_agent::mcp::SecretsManager;
use tempfile::TempDir;
use uuid::Uuid;

#[test]
fn secrets_manager_round_trip_and_delete() {
    let temp_dir = TempDir::new().unwrap();
    let manager = SecretsManager::new(temp_dir.path().to_path_buf());

    let mcp_id = Uuid::new_v4();
    manager.store_api_key(mcp_id, "secret").unwrap();
    let loaded = manager.load_api_key(mcp_id).unwrap();
    assert_eq!(loaded, "secret");

    manager.delete_api_key(mcp_id).unwrap();
    let result = manager.load_api_key(mcp_id);
    assert!(matches!(result, Err(SecretsError::SecretNotFound(_))));
}
