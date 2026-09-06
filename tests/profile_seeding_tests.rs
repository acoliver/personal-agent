//! First-install local profile seeding (REQ-LM-002).
//!
//! @plan:PLAN-20260903-LOCALMODEL.P01
//! @requirement:REQ-LM-002

use personal_agent::models::{AuthConfig, ModelProfile};
use personal_agent::services::local_model_settings::try_load_from_disk;
use personal_agent::services::profile::ProfileService;
use personal_agent::services::profile_impl::{
    ProfileServiceImpl, SEED_MODEL_ID, SEED_PROFILE_NAME, SEED_PROVIDER_ID,
};

fn service(dir: &tempfile::TempDir) -> ProfileServiceImpl {
    ProfileServiceImpl::new(dir.path().to_path_buf()).expect("service")
}

/// A `default.json` holding this id counts as "install already configured"
/// even when the profiles dir has no profile files.
fn write_default_id(dir: &tempfile::TempDir, id: uuid::Uuid) {
    std::fs::write(dir.path().join("default.json"), format!("\"{id}\"")).unwrap();
}

#[tokio::test]
async fn empty_install_seeds_the_local_profile_as_default() {
    let dir = tempfile::TempDir::new().unwrap();
    let svc = service(&dir);

    svc.initialize().await.unwrap();

    let profiles = svc.list().await.unwrap();
    assert_eq!(profiles.len(), 1, "exactly the seeded profile");
    let seeded = &profiles[0];
    assert_eq!(seeded.name, SEED_PROFILE_NAME);
    assert_eq!(seeded.provider_id, SEED_PROVIDER_ID);
    assert_eq!(seeded.model_id, SEED_MODEL_ID);
    assert_eq!(seeded.auth, AuthConfig::None);
    assert_eq!(seeded.base_url, "", "local profile persists no base_url");

    let default_profile = svc.get_default().await.unwrap().expect("default set");
    assert_eq!(default_profile.id, seeded.id);

    // The seed is durable: it survives a reload like any created profile.
    let reloaded = service(&dir);
    reloaded.initialize().await.unwrap();
    let profiles = reloaded.list().await.unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].name, SEED_PROFILE_NAME);
}

#[tokio::test]
async fn existing_profiles_are_never_seeded() {
    let dir = tempfile::TempDir::new().unwrap();
    let svc = service(&dir);
    svc.create(
        "My OpenAI".to_string(),
        "openai".to_string(),
        "gpt-4.1".to_string(),
        None,
        AuthConfig::Keychain {
            label: "key".to_string(),
        },
        personal_agent::models::ModelParameters::default(),
        None,
    )
    .await
    .unwrap();

    svc.initialize().await.unwrap();

    let profiles = svc.list().await.unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].provider_id, "openai");
    assert_ne!(profiles[0].name, SEED_PROFILE_NAME);
}

#[tokio::test]
async fn default_json_present_suppresses_seeding() {
    let dir = tempfile::TempDir::new().unwrap();
    write_default_id(&dir, uuid::Uuid::new_v4());
    let svc = service(&dir);

    svc.initialize().await.unwrap();

    assert!(
        svc.list().await.unwrap().is_empty(),
        "a persisted default means the install was configured, seed nothing"
    );
}

#[tokio::test]
async fn repeated_initialize_is_idempotent_across_service_instances() {
    let dir = tempfile::TempDir::new().unwrap();

    // Both boot stacks construct their own service over the same directory.
    let first = service(&dir);
    first.initialize().await.unwrap();
    first.initialize().await.unwrap();

    let second = service(&dir);
    second.initialize().await.unwrap();

    let profiles = second.list().await.unwrap();
    assert_eq!(profiles.len(), 1, "seeding must not duplicate");
    assert_eq!(profiles[0].name, SEED_PROFILE_NAME);
}

#[tokio::test]
async fn seeded_profile_json_is_clean_for_a_local_provider() {
    let dir = tempfile::TempDir::new().unwrap();
    let svc = service(&dir);

    svc.initialize().await.unwrap();

    // The persisted file carries no endpoint and no key reference.
    let files: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|e| e.to_str()) == Some("json")
                && p.file_name().and_then(|n| n.to_str()) != Some("default.json")
        })
        .collect();
    assert_eq!(files.len(), 1);
    let content = std::fs::read_to_string(&files[0]).unwrap();
    assert!(!content.contains("api.openai.com"), "raw JSON: {content}");
    let parsed: ModelProfile = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed.base_url, "");
    assert_eq!(parsed.auth, AuthConfig::None);
}

#[tokio::test]
async fn legacy_local_profile_with_openai_base_url_is_normalized_on_load() {
    let dir = tempfile::TempDir::new().unwrap();
    let legacy_id = uuid::Uuid::new_v4();
    let profile_json = format!(
        r#"{{"id":"{legacy_id}","name":"old local","provider_id":"local","model_id":"granite-4.2-3b","base_url":"https://api.openai.com/v1","auth":{{"type":"none"}}}}"#
    );
    std::fs::write(dir.path().join(format!("{legacy_id}.json")), profile_json).unwrap();

    let svc = service(&dir);
    svc.initialize().await.unwrap();

    let profiles = svc.list().await.unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(
        profiles[0].base_url, "",
        "legacy baked endpoint must not survive a load"
    );

    // The cleaned form is what gets written back after an update.
    svc.update(
        profiles[0].id,
        Some("renamed".to_string()),
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    let stored = std::fs::read_to_string(dir.path().join(format!("{legacy_id}.json"))).unwrap();
    assert!(!stored.contains("api.openai.com"), "raw JSON: {stored}");
}

#[tokio::test]
async fn update_cannot_reintroduce_a_base_url_on_a_local_profile() {
    let dir = tempfile::TempDir::new().unwrap();
    let svc = service(&dir);
    svc.initialize().await.unwrap();
    let seeded = &svc.list().await.unwrap()[0];

    svc.update(
        seeded.id,
        None,
        None,
        None,
        Some("https://api.openai.com/v1".to_string()),
        None,
        None,
        None,
    )
    .await
    .unwrap();

    let after = svc.get(seeded.id).await.unwrap();
    assert_eq!(after.base_url, "");
}

#[test]
fn settings_disk_loader_skips_profile_directories() {
    // Sanity guard on an import used above: the loader only reads app
    // settings files, never profile JSON.
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("unrelated.json"), r#"{"other":1}"#).unwrap();
    assert_eq!(
        try_load_from_disk(&dir.path().join("unrelated.json")).unwrap(),
        None
    );
}
