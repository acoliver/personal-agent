use std::fs;

use personal_agent::services::{secure_store, SecretsService, SecretsServiceImpl};
use tempfile::TempDir;

#[tokio::test]
async fn secrets_service_uses_secure_store_as_primary_backend() {
    secure_store::use_mock_backend();

    let temp_dir = TempDir::new().unwrap();
    let service = SecretsServiceImpl::new(temp_dir.path().to_path_buf()).expect("secrets service");

    service
        .store("token_one".to_string(), "value-1".to_string())
        .await
        .expect("store regular secret");
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
    assert!(service.exists("token_one").await.expect("exists secret"));

    let keys = service.list_keys().await.expect("list keys");
    assert_eq!(keys, vec!["token_one".to_string()]);

    let dir_entries = fs::read_dir(temp_dir.path())
        .expect("read temp dir")
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    assert!(
        dir_entries.is_empty(),
        "secure-store primary path should not create fallback files when keyring succeeds"
    );

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
async fn secrets_service_file_fallback_encrypts_values_without_plaintext_or_legacy_paths() {
    let temp_dir = TempDir::new().unwrap();
    let service = SecretsServiceImpl::new_file_fallback_only(temp_dir.path().to_path_buf())
        .expect("secrets service");

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

    let mut file_names = Vec::new();
    for entry in fs::read_dir(temp_dir.path()).expect("read fallback dir") {
        let entry = entry.expect("fallback entry");
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("utf8 file name")
            .to_string();
        file_names.push(file_name.clone());

        assert!(
            path.extension().and_then(|ext| ext.to_str()) == Some("enc"),
            "fallback files should use encrypted .enc extension"
        );

        let contents = fs::read_to_string(&path).expect("read encrypted fallback file");
        assert!(
            !contents.contains("value-1")
                && !contents.contains("value-2")
                && !contents.contains("sk-123")
                && !contents.contains(".llxprt")
                && !contents.contains(".keys"),
            "fallback files must not contain plaintext secrets or legacy path bindings"
        );
    }

    assert!(file_names.iter().all(|name| {
        !std::path::Path::new(name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"))
    }));
    assert!(
        file_names.contains(&"token_one.enc".to_string())
            && file_names.contains(&"token-two.enc".to_string())
            && file_names.contains(&"api_key_openai.enc".to_string())
    );

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
    let service = SecretsServiceImpl::new_file_fallback_only(temp_dir.path().to_path_buf())
        .expect("secrets service");

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
