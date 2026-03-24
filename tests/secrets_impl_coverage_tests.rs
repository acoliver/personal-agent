use personal_agent::services::{SecretsService, SecretsServiceImpl};
use tempfile::TempDir;

#[tokio::test]
async fn secrets_service_round_trip_lists_and_deletes_secrets_and_api_keys() {
    let temp_dir = TempDir::new().unwrap();
    let service = SecretsServiceImpl::new(temp_dir.path().to_path_buf()).expect("secrets service");

    service
        .store("token_one".to_string(), "value-1".to_string())
        .await
        .expect("store regular secret");
    service
        .store("token-two".to_string(), "value-2".to_string())
        .await
        .expect("store second secret");
    service
        .store_api_key("openai".to_string(), "sk-123".to_string())
        .await
        .expect("store api key");

    assert_eq!(
        service.get("token_one").await.expect("load secret"),
        Some("value-1".to_string())
    );
    assert_eq!(
        service.get_api_key("openai").await.expect("load api key"),
        Some("sk-123".to_string())
    );
    assert!(service.exists("token-two").await.expect("exists secret"));

    let keys = service.list_keys().await.expect("list keys");
    assert_eq!(keys, vec!["token-two".to_string(), "token_one".to_string()]);

    service
        .delete("token_one")
        .await
        .expect("delete regular secret");
    service
        .delete_api_key("openai")
        .await
        .expect("delete api key");

    assert_eq!(
        service.get("token_one").await.expect("secret missing"),
        None
    );
    assert_eq!(
        service
            .get_api_key("openai")
            .await
            .expect("api key missing"),
        None
    );
}

#[tokio::test]
async fn secrets_service_rejects_invalid_keys_and_reports_missing_entries() {
    let temp_dir = TempDir::new().unwrap();
    let service = SecretsServiceImpl::new(temp_dir.path().to_path_buf()).expect("secrets service");

    let invalid_store = service
        .store("../bad".to_string(), "nope".to_string())
        .await;
    assert!(matches!(
        invalid_store,
        Err(personal_agent::services::ServiceError::Validation(message))
            if message.contains("invalid characters")
    ));

    let missing_delete = service.delete("missing").await;
    assert!(matches!(
        missing_delete,
        Err(personal_agent::services::ServiceError::NotFound(message))
            if message.contains("Secret not found")
    ));

    let missing_api_delete = service.delete_api_key("missing_provider").await;
    assert!(matches!(
        missing_api_delete,
        Err(personal_agent::services::ServiceError::NotFound(message))
            if message.contains("API key not found")
    ));

    let invalid_exists = service.exists("bad/slash").await;
    assert!(matches!(
        invalid_exists,
        Err(personal_agent::services::ServiceError::Validation(message))
            if message.contains("invalid characters")
    ));
}
