//! Tests for local model engine settings persistence (REQ-LM-001).
//!
//! @plan:PLAN-20260903-LOCALMODEL.P01
//! @requirement:REQ-LM-001

use std::path::PathBuf;
use std::sync::Mutex;

use personal_agent::services::app_settings::AppSettingsService;
use personal_agent::services::app_settings_impl::AppSettingsServiceImpl;
use personal_agent::services::local_model_settings::{
    default_model_path, try_load_from_disk, LocalModelSettings, ENV_MODEL_PATH,
    LOCAL_MODEL_SETTINGS_KEY,
};

/// `std::env` mutation is process-global, so every test that touches
/// `PA_LOCAL_GGUF` serializes through this lock.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn test_service(dir: &tempfile::TempDir) -> AppSettingsServiceImpl {
    AppSettingsServiceImpl::new(dir.path().join("app_settings.json")).expect("settings service")
}

#[tokio::test]
async fn load_without_persisted_settings_returns_defaults() {
    let dir = tempfile::TempDir::new().unwrap();
    let service = test_service(&dir);

    let loaded = LocalModelSettings::load(&service).await.unwrap();

    assert_eq!(loaded, LocalModelSettings::default());
}

#[tokio::test]
async fn save_then_load_round_trips() {
    let dir = tempfile::TempDir::new().unwrap();
    let service = test_service(&dir);

    let settings = LocalModelSettings {
        n_ctx: 4096,
        gpu_layers: 41,
        idle_unload: false,
        idle_timeout_minutes: 2,
        model_path: PathBuf::from("/tmp/models/tiny.gguf"),
    };

    settings.save(&service).await.unwrap();
    let loaded = LocalModelSettings::load(&service).await.unwrap();

    assert_eq!(loaded, settings);
}

#[tokio::test]
async fn persisted_blob_lives_under_the_local_model_key() {
    let dir = tempfile::TempDir::new().unwrap();
    let service = test_service(&dir);

    LocalModelSettings::default().save(&service).await.unwrap();

    let stored = service
        .get_setting(LOCAL_MODEL_SETTINGS_KEY)
        .await
        .unwrap()
        .expect("blob persisted");
    let parsed: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(parsed["n_ctx"], 8192);
    assert_eq!(parsed["gpu_layers"], 999);
}

#[test]
fn env_override_changes_the_default_model_path() {
    let _guard = ENV_LOCK.lock().unwrap();
    // SAFETY: single-threaded w.r.t. env mutation via ENV_LOCK; tests in this
    // binary that read the var hold the same lock.
    std::env::set_var(ENV_MODEL_PATH, "/tmp/env-chosen.gguf");
    assert_eq!(default_model_path(), PathBuf::from("/tmp/env-chosen.gguf"));
    std::env::remove_var(ENV_MODEL_PATH);
}

#[test]
fn env_override_wins_over_data_dir_but_not_persisted_settings() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::set_var(ENV_MODEL_PATH, "/tmp/env-chosen.gguf");
    let defaults = LocalModelSettings::default();
    assert_eq!(defaults.model_path, PathBuf::from("/tmp/env-chosen.gguf"));

    // A persisted path must survive a load even with the env var set.
    let dir = tempfile::TempDir::new().unwrap();
    let app_settings = dir.path().join("app_settings.json");
    std::fs::write(
        &app_settings,
        format!(
            r#"{{"{LOCAL_MODEL_SETTINGS_KEY}": {{"model_path": "/data/persisted.gguf", "n_ctx": 2048}}}}"#
        ),
    )
    .unwrap();
    let loaded = try_load_from_disk(&app_settings).unwrap().unwrap();
    assert_eq!(loaded.model_path, PathBuf::from("/data/persisted.gguf"));
    assert_eq!(loaded.n_ctx, 2048);
    // Fields absent from the blob fall back to their defaults.
    assert_eq!(loaded.gpu_layers, 999);
    std::env::remove_var(ENV_MODEL_PATH);
}

#[test]
fn disk_loader_reports_missing_key_and_broken_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let app_settings = dir.path().join("app_settings.json");

    // Absent file: first install.
    assert!(try_load_from_disk(&app_settings).is_err());

    // File without the key: normal install that never configured a local model.
    std::fs::write(&app_settings, r#"{"theme":"dark"}"#).unwrap();
    assert_eq!(try_load_from_disk(&app_settings).unwrap(), None);

    // Present but malformed: an error, not silent defaults.
    std::fs::write(
        &app_settings,
        format!(r#"{{"{LOCAL_MODEL_SETTINGS_KEY}": {{"n_ctx": "not-a-number"}}}}"#),
    )
    .unwrap();
    assert!(try_load_from_disk(&app_settings).is_err());
}
