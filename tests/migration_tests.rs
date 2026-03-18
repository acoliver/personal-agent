use std::fs;
use std::path::Path;

use personal_agent::config::Config;
use personal_agent::mcp::{
    McpAuthType, McpConfig, McpPackage, McpPackageType, McpSource, McpTransport,
};
use personal_agent::migration::{
    convert_conversation_format, detect_config_version, MigrationReport, MigrationRunner,
};
use personal_agent::models::{AuthConfig, ModelParameters, ModelProfile};
use personal_agent::services::secure_store;
use tempfile::TempDir;
use uuid::Uuid;

fn write_json(path: &Path, contents: &str) {
    fs::write(path, contents).expect("failed to write json fixture");
}

fn sample_profile(name: &str) -> ModelProfile {
    ModelProfile {
        id: Uuid::new_v4(),
        name: name.to_string(),
        provider_id: "openai".to_string(),
        model_id: "gpt-4.1".to_string(),
        base_url: "https://api.openai.com/v1".to_string(),
        auth: AuthConfig::Keychain {
            label: format!("label-{name}"),
        },
        parameters: ModelParameters::default(),
        system_prompt: "System prompt".to_string(),
    }
}

fn sample_mcp(name: &str) -> McpConfig {
    McpConfig {
        id: Uuid::new_v4(),
        name: name.to_string(),
        enabled: true,
        source: McpSource::Manual {
            url: "https://example.com/mcp".to_string(),
        },
        package: McpPackage {
            package_type: McpPackageType::Http,
            identifier: format!("pkg-{name}"),
            runtime_hint: None,
        },
        transport: McpTransport::Http,
        auth_type: McpAuthType::None,
        env_vars: vec![],
        package_args: vec![],
        keyfile_path: None,
        config: serde_json::json!({}),
        oauth_token: None,
    }
}

#[test]
fn detect_config_version_returns_default_for_missing_file() {
    let temp_dir = TempDir::new().expect("tempdir should be created");
    let config_path = temp_dir.path().join("missing-config.json");

    let version =
        detect_config_version(&config_path).expect("missing config should return default");

    assert_eq!(version, "1.0");
}

#[test]
fn detect_config_version_reads_saved_config_version() {
    let temp_dir = TempDir::new().expect("tempdir should be created");
    let config_path = temp_dir.path().join("config.json");
    let config = Config {
        version: "2.4".to_string(),
        profiles: vec![sample_profile("work")],
        mcps: vec![sample_mcp("search")],
        ..Config::default()
    };

    config.save(&config_path).expect("config should save");

    let version = detect_config_version(&config_path).expect("config version should parse");

    assert_eq!(version, "2.4");
}

#[test]
fn detect_config_version_errors_for_invalid_json() {
    let temp_dir = TempDir::new().expect("tempdir should be created");
    let config_path = temp_dir.path().join("config.json");
    write_json(&config_path, "{ not valid json }");

    let error = detect_config_version(&config_path).expect_err("invalid json should fail");

    assert!(
        error.to_string().contains("Failed to parse config file"),
        "unexpected error: {error:#}"
    );
}

#[test]
fn convert_conversation_format_loads_current_schema() {
    let temp_dir = TempDir::new().expect("tempdir should be created");
    let conversation_path = temp_dir.path().join("conversation.json");
    let conversation_id = Uuid::new_v4();
    let profile_id = Uuid::new_v4();
    let message_id = Uuid::new_v4();

    write_json(
        &conversation_path,
        &format!(
            r#"{{
  "id": "{conversation_id}",
  "title": "Migrated Conversation",
  "profile_id": "{profile_id}",
  "messages": [
    {{
      "id": "{message_id}",
      "role": "user",
      "content": "hello",
      "timestamp": "2025-01-27T12:00:00Z"
    }}
  ],
  "created_at": "2025-01-27T12:00:00Z",
  "updated_at": "2025-01-27T12:00:00Z"
}}"#
        ),
    );

    let conversation =
        convert_conversation_format(&conversation_path).expect("conversation should parse");

    assert_eq!(conversation.id, conversation_id);
    assert_eq!(conversation.title.as_deref(), Some("Migrated Conversation"));
    assert_eq!(conversation.messages.len(), 1);
    assert_eq!(conversation.messages[0].content, "hello");
}

#[test]
fn convert_conversation_format_errors_for_invalid_json() {
    let temp_dir = TempDir::new().expect("tempdir should be created");
    let conversation_path = temp_dir.path().join("conversation.json");
    write_json(&conversation_path, "this is not json");

    let error = convert_conversation_format(&conversation_path)
        .expect_err("invalid conversation should fail");

    assert!(
        error
            .to_string()
            .contains("Failed to parse conversation file"),
        "unexpected error: {error:#}"
    );
}

#[test]
fn convert_conversation_format_errors_for_missing_file() {
    let temp_dir = TempDir::new().expect("tempdir should be created");
    let conversation_path = temp_dir.path().join("missing.json");

    let error =
        convert_conversation_format(&conversation_path).expect_err("missing file should fail");

    assert!(
        error
            .to_string()
            .contains("Failed to read conversation file"),
        "unexpected error: {error:#}"
    );
}

#[tokio::test]
async fn run_migrations_creates_backup_and_counts_profiles_and_mcps() {
    let temp_dir = TempDir::new().expect("tempdir should be created");
    let data_dir = temp_dir.path();
    let config_path = data_dir.join("config.json");
    let conversations_dir = data_dir.join("conversations");
    fs::create_dir_all(&conversations_dir).expect("conversations dir should exist");

    let profile_one = sample_profile("alpha");
    let profile_two = sample_profile("beta");
    let mcp = sample_mcp("docs");
    let config = Config {
        profiles: vec![profile_one, profile_two],
        mcps: vec![mcp],
        ..Config::default()
    };
    config.save(&config_path).expect("config should save");

    let conversation_file = conversations_dir.join("conversation.json");
    write_json(
        &conversation_file,
        &format!(
            r#"{{
  "id": "{}",
  "title": "Conversation",
  "profile_id": "{}",
  "messages": [],
  "created_at": "2025-01-27T12:00:00Z",
  "updated_at": "2025-01-27T12:00:00Z"
}}"#,
            Uuid::new_v4(),
            Uuid::new_v4()
        ),
    );

    let runner = MigrationRunner::new(data_dir.to_path_buf());

    let report = runner
        .run_migrations()
        .await
        .expect("migration should succeed with local config");

    assert_eq!(report.profiles_verified, 2);
    assert_eq!(report.mcp_configs_verified, 1);

    let backup_dir = data_dir.join("backup_before_migration");
    assert!(backup_dir.exists(), "backup dir should be created");
    assert_eq!(
        fs::read_to_string(backup_dir.join("config.json.bak")).expect("backup config should exist"),
        fs::read_to_string(&config_path).expect("original config should exist")
    );
    assert_eq!(
        fs::read_to_string(backup_dir.join("conversations").join("conversation.json"))
            .expect("backup conversation should exist"),
        fs::read_to_string(&conversation_file).expect("original conversation should exist")
    );
}

#[tokio::test]
async fn run_migrations_handles_missing_config_and_conversations() {
    let temp_dir = TempDir::new().expect("tempdir should be created");
    let runner = MigrationRunner::new(temp_dir.path().to_path_buf());

    let report = runner
        .run_migrations()
        .await
        .expect("migration should succeed without data files");

    assert_eq!(report.profiles_verified, 0);
    assert_eq!(report.mcp_configs_verified, 0);
    assert!(
        temp_dir.path().join("backup_before_migration").exists(),
        "backup dir should still be created"
    );
}

#[tokio::test]
async fn run_migrations_keeps_report_zero_when_config_is_invalid() {
    let temp_dir = TempDir::new().expect("tempdir should be created");
    let config_path = temp_dir.path().join("config.json");
    write_json(&config_path, "{ invalid json }");

    let runner = MigrationRunner::new(temp_dir.path().to_path_buf());

    let report = runner
        .run_migrations()
        .await
        .expect("migration should continue despite unreadable config");

    assert_eq!(report.profiles_verified, 0);
    assert_eq!(report.mcp_configs_verified, 0);
}

#[tokio::test]
async fn rollback_restores_backed_up_config_and_conversations() {
    let temp_dir = TempDir::new().expect("tempdir should be created");
    let data_dir = temp_dir.path();
    let config_path = data_dir.join("config.json");
    let conversations_dir = data_dir.join("conversations");
    fs::create_dir_all(&conversations_dir).expect("conversations dir should exist");

    let original_config = Config {
        version: "1.7".to_string(),
        profiles: vec![sample_profile("stable")],
        ..Config::default()
    };
    original_config
        .save(&config_path)
        .expect("original config should save");
    let conversation_path = conversations_dir.join("conv.json");
    write_json(&conversation_path, "{\"conversation\":\"original\"}");

    let runner = MigrationRunner::new(data_dir.to_path_buf());
    runner
        .run_migrations()
        .await
        .expect("initial migration should succeed");

    write_json(&config_path, "{\"version\":\"mutated\"}");
    write_json(&conversation_path, "{\"conversation\":\"mutated\"}");

    runner.rollback().await.expect("rollback should succeed");

    let restored_config =
        fs::read_to_string(&config_path).expect("restored config should be readable");
    let restored_conversation =
        fs::read_to_string(&conversation_path).expect("restored conversation should be readable");

    assert!(restored_config.contains("\"version\": \"1.7\""));
    assert_eq!(restored_conversation, "{\"conversation\":\"original\"}");
}

#[tokio::test]
async fn rollback_without_backup_is_a_noop() {
    let temp_dir = TempDir::new().expect("tempdir should be created");
    let config_path = temp_dir.path().join("config.json");
    write_json(&config_path, "{\"version\":\"current\"}");

    let runner = MigrationRunner::new(temp_dir.path().to_path_buf());
    runner
        .rollback()
        .await
        .expect("rollback without backup should succeed");

    assert_eq!(
        fs::read_to_string(&config_path).expect("config should remain"),
        "{\"version\":\"current\"}"
    );
}

#[test]
fn migration_report_fields_are_accessible() {
    let report = MigrationReport {
        conversations_migrated: 4,
        profiles_verified: 3,
        mcp_configs_verified: 2,
    };

    assert_eq!(report.conversations_migrated, 4);
    assert_eq!(report.profiles_verified, 3);
    assert_eq!(report.mcp_configs_verified, 2);
}

#[test]
fn secure_store_mock_backend_supports_process_local_test_setup() {
    secure_store::use_mock_backend();
    let label = format!("migration-tests-{}", Uuid::new_v4());
    secure_store::api_keys::store(&label, "secret-value").expect("mock store should accept writes");

    let exists = secure_store::api_keys::exists(&label).expect("exists check should succeed");
    let value = secure_store::api_keys::get(&label).expect("lookup should succeed");

    assert!(exists);
    assert_eq!(value.as_deref(), Some("secret-value"));

    secure_store::api_keys::delete(&label).expect("delete should succeed");
}
