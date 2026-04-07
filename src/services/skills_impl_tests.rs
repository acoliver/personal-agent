use super::{SkillsServiceImpl, DISABLED_SKILLS_SETTING_KEY};
use crate::services::{
    app_settings_impl::AppSettingsServiceImpl, AppSettingsService, ServiceError, SkillsService,
};
use async_trait::async_trait;
use tempfile::TempDir;
use uuid::Uuid;

/// Stub settings service that fails all persistence operations.
/// Used to test rollback behavior when settings cannot be saved.
struct FailingSetSettingAppSettingsService;

#[async_trait]
impl AppSettingsService for FailingSetSettingAppSettingsService {
    async fn get_default_profile_id(&self) -> crate::services::ServiceResult<Option<Uuid>> {
        Ok(None)
    }

    async fn set_default_profile_id(&self, _id: Uuid) -> crate::services::ServiceResult<()> {
        Ok(())
    }

    async fn clear_default_profile_id(&self) -> crate::services::ServiceResult<()> {
        Ok(())
    }

    async fn get_current_conversation_id(&self) -> crate::services::ServiceResult<Option<Uuid>> {
        Ok(None)
    }

    async fn set_current_conversation_id(&self, _id: Uuid) -> crate::services::ServiceResult<()> {
        Ok(())
    }

    async fn get_hotkey(&self) -> crate::services::ServiceResult<Option<String>> {
        Ok(None)
    }

    async fn set_hotkey(&self, _hotkey: String) -> crate::services::ServiceResult<()> {
        Ok(())
    }

    async fn get_theme(&self) -> crate::services::ServiceResult<Option<String>> {
        Ok(None)
    }

    async fn set_theme(&self, _theme: String) -> crate::services::ServiceResult<()> {
        Ok(())
    }

    async fn get_setting(&self, _key: &str) -> crate::services::ServiceResult<Option<String>> {
        Ok(None)
    }

    async fn set_setting(&self, _key: &str, _value: String) -> crate::services::ServiceResult<()> {
        Err(ServiceError::Io(
            "simulated persistence failure".to_string(),
        ))
    }

    async fn reset_to_defaults(&self) -> crate::services::ServiceResult<()> {
        Ok(())
    }
}

/// Write a skill file to disk with YAML frontmatter and body.
/// This simulates what bundled/user skill directories look like.
fn write_skill(root: &std::path::Path, dir_name: &str, name: &str, description: &str, body: &str) {
    let skill_dir = root.join(dir_name);
    std::fs::create_dir_all(&skill_dir).expect("skill dir should exist");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n{body}"),
    )
    .expect("skill file should write");
}

/// Create a skills service with real persistence to a temp directory.
fn create_service(temp_dir: &TempDir) -> SkillsServiceImpl {
    let settings = std::sync::Arc::new(
        AppSettingsServiceImpl::new(temp_dir.path().join("settings.json"))
            .expect("settings should initialize"),
    );
    SkillsServiceImpl::new_for_tests(
        settings,
        temp_dir.path().join("bundled"),
        temp_dir.path().join("user"),
    )
    .expect("skills service should initialize")
}

// ---------------------------------------------------------------------------
// BEHAVIORAL TESTS: Discovery and precedence
// ---------------------------------------------------------------------------

/// When both bundled and user directories contain a skill with the same name,
/// the user version takes precedence. This allows users to override bundled skills.
#[tokio::test]
async fn discover_skills_prefers_user_skill_on_name_collision() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    write_skill(
        &temp_dir.path().join("bundled"),
        "shared",
        "shared-skill",
        "Bundled version",
        "Bundled body\n",
    );
    write_skill(
        &temp_dir.path().join("user"),
        "shared",
        "shared-skill",
        "User version",
        "User body\n",
    );

    let service = create_service(&temp_dir);
    service
        .discover_skills()
        .await
        .expect("discovery should succeed");

    let skill = service
        .get_skill("shared-skill")
        .await
        .expect("lookup should succeed")
        .expect("skill should exist");
    assert_eq!(skill.source, crate::models::SkillSource::User);
    assert_eq!(skill.description, "User version");
}

/// When a skill is disabled, it should not appear in the enabled skills list.
/// The disabled state is persisted so it survives app restart.
#[tokio::test]
async fn set_skill_enabled_persists_disabled_list() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    write_skill(
        &temp_dir.path().join("bundled"),
        "writer",
        "docs-writer",
        "Write docs",
        "Body\n",
    );
    let service = create_service(&temp_dir);
    service
        .discover_skills()
        .await
        .expect("discovery should succeed");

    service
        .set_skill_enabled("docs-writer", false)
        .await
        .expect("disable should succeed");

    let disabled = service
        .app_settings_service
        .get_setting(DISABLED_SKILLS_SETTING_KEY)
        .await
        .expect("settings read should succeed")
        .expect("disabled list should exist");
    assert!(disabled.contains("docs-writer"));
}

/// When persistence fails, the in-memory skill state should be rolled back.
/// This ensures the UI shows the correct state even when saving fails.
#[tokio::test]
async fn set_skill_enabled_rolls_back_in_memory_state_on_persistence_error() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    write_skill(
        &temp_dir.path().join("bundled"),
        "writer",
        "docs-writer",
        "Write docs",
        "Body\n",
    );
    let service = SkillsServiceImpl::new_for_tests(
        std::sync::Arc::new(FailingSetSettingAppSettingsService),
        temp_dir.path().join("bundled"),
        temp_dir.path().join("user"),
    )
    .expect("skills service should initialize");
    service
        .discover_skills()
        .await
        .expect("discovery should succeed");

    let error = service
        .set_skill_enabled("docs-writer", false)
        .await
        .expect_err("disable should surface persistence failure");
    assert!(error.to_string().contains("simulated persistence failure"));

    let skill = service
        .get_skill("docs-writer")
        .await
        .expect("lookup should succeed")
        .expect("skill should exist");
    assert!(
        skill.enabled,
        "failed persistence should restore in-memory state"
    );
}

/// Symlink loops in skill directories should be skipped gracefully.
/// This prevents infinite recursion during discovery.
#[cfg(unix)]
#[tokio::test]
async fn discover_skills_skips_symlinked_directories() {
    use std::os::unix::fs::symlink;

    let temp_dir = TempDir::new().expect("temp dir should exist");
    let bundled_dir = temp_dir.path().join("bundled");
    write_skill(
        &bundled_dir,
        "writer",
        "docs-writer",
        "Write docs",
        "Body\n",
    );
    symlink(&bundled_dir, bundled_dir.join("loop")).expect("symlinked directory should be created");

    let service = create_service(&temp_dir);
    service
        .discover_skills()
        .await
        .expect("discovery should skip symlink loops");

    let skills = service
        .list_skills()
        .await
        .expect("listing skills should succeed");
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "docs-writer");
}

/// The skill body should be readable from disk after discovery.
/// This is what gets injected into the agent prompt.
#[tokio::test]
async fn get_skill_body_returns_markdown_body_for_discovered_skill() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    write_skill(
        &temp_dir.path().join("bundled"),
        "writer",
        "docs-writer",
        "Write docs",
        "Body line one\nBody line two\n",
    );
    let service = create_service(&temp_dir);
    service
        .discover_skills()
        .await
        .expect("discovery should succeed");

    let body = service
        .get_skill_body("docs-writer")
        .await
        .expect("body lookup should succeed")
        .expect("skill body should exist");
    assert_eq!(body, "Body line one\nBody line two\n");
}

/// Corrupt persisted settings should produce a meaningful error.
/// This prevents silent data corruption from causing undefined behavior.
#[tokio::test]
async fn discover_skills_rejects_invalid_disabled_skills_setting() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);
    service
        .app_settings_service
        .set_setting(DISABLED_SKILLS_SETTING_KEY, "not-json".to_string())
        .await
        .expect("settings write should succeed");

    let error = service
        .discover_skills()
        .await
        .expect_err("invalid disabled skills setting should fail discovery");
    assert!(error
        .to_string()
        .contains("Failed to parse disabled skills setting"));
}

// ---------------------------------------------------------------------------
// BEHAVIORAL TESTS: Watched directories
// ---------------------------------------------------------------------------

/// Adding a watched directory should persist it and make skills inside discoverable.
/// Removing it should remove from persistence and from the discovered list.
#[tokio::test]
async fn add_and_remove_watched_directory_round_trip() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);

    let watched = service
        .watched_directories()
        .await
        .expect("initial watched should succeed");
    assert!(watched.is_empty(), "initially no watched directories");

    let extra_dir = temp_dir.path().join("extra_skills");
    service
        .add_watched_directory(extra_dir.clone())
        .await
        .expect("add should succeed");

    let watched = service
        .watched_directories()
        .await
        .expect("watched after add should succeed");
    assert_eq!(watched.len(), 1);

    service
        .remove_watched_directory(&extra_dir)
        .await
        .expect("remove should succeed");

    let watched = service
        .watched_directories()
        .await
        .expect("watched after remove should succeed");
    assert!(watched.is_empty(), "directory should be removed");
}

/// When a watched directory contains skills, they should become discoverable.
#[tokio::test]
async fn add_watched_directory_with_skills_discovers_them() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);

    let extra_dir = temp_dir.path().join("extra_skills");
    write_skill(
        &extra_dir,
        "external",
        "ext-skill",
        "External skill",
        "Body\n",
    );

    service
        .add_watched_directory(extra_dir.clone())
        .await
        .expect("add should succeed");

    let skills = service.list_skills().await.expect("list should succeed");
    assert!(
        skills.iter().any(|s| s.name == "ext-skill"),
        "external skill should be discovered: {skills:?}"
    );
}

// ---------------------------------------------------------------------------
// BEHAVIORAL TESTS: Enabled/disabled filtering
// ---------------------------------------------------------------------------

/// The enabled skills list should only contain skills that are not disabled.
#[tokio::test]
async fn get_enabled_skills_filters_disabled() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    write_skill(
        &temp_dir.path().join("bundled"),
        "alpha",
        "alpha-skill",
        "Alpha",
        "A\n",
    );
    write_skill(
        &temp_dir.path().join("bundled"),
        "beta",
        "beta-skill",
        "Beta",
        "B\n",
    );
    let service = create_service(&temp_dir);
    service
        .discover_skills()
        .await
        .expect("discovery should succeed");

    service
        .set_skill_enabled("alpha-skill", false)
        .await
        .expect("disable should succeed");

    let enabled = service
        .get_enabled_skills()
        .await
        .expect("get_enabled should succeed");
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].name, "beta-skill");
}

/// Calling refresh should re-scan directories and pick up new skills.
#[tokio::test]
async fn refresh_rediscovers_skills() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);

    let skills = service.list_skills().await.expect("list should succeed");
    assert!(skills.is_empty());

    write_skill(
        &temp_dir.path().join("bundled"),
        "writer",
        "docs-writer",
        "Write docs",
        "Body\n",
    );

    service.refresh().await.expect("refresh should succeed");

    let skills = service.list_skills().await.expect("list should succeed");
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "docs-writer");
}

/// Toggling a skill from disabled back to enabled should update persistence.
#[tokio::test]
async fn set_skill_enabled_toggle_on_after_off_removes_from_disabled_list() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    write_skill(
        &temp_dir.path().join("bundled"),
        "writer",
        "docs-writer",
        "Write docs",
        "Body\n",
    );
    let service = create_service(&temp_dir);
    service.discover_skills().await.expect("discovery ok");

    service
        .set_skill_enabled("docs-writer", false)
        .await
        .expect("disable ok");
    let skill = service.get_skill("docs-writer").await.expect("ok").unwrap();
    assert!(!skill.enabled);

    service
        .set_skill_enabled("docs-writer", true)
        .await
        .expect("re-enable ok");
    let skill = service.get_skill("docs-writer").await.expect("ok").unwrap();
    assert!(skill.enabled, "skill should be enabled again");

    let disabled_setting = service
        .app_settings_service
        .get_setting(DISABLED_SKILLS_SETTING_KEY)
        .await
        .expect("settings read ok");
    match disabled_setting {
        None => {}
        Some(raw) => {
            assert!(
                !raw.contains("docs-writer"),
                "disabled list should not contain re-enabled skill: {raw}"
            );
        }
    }
}

/// Corrupt watched directories setting should produce a meaningful error.
#[tokio::test]
async fn load_watched_directories_rejects_invalid_json() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);
    service
        .app_settings_service
        .set_setting(
            super::WATCHED_SKILLS_DIRECTORIES_SETTING_KEY,
            "not-json-array".to_string(),
        )
        .await
        .expect("write ok");

    let error = service
        .watched_directories()
        .await
        .expect_err("invalid JSON should fail");
    assert!(
        error.to_string().contains("Failed to parse"),
        "unexpected error: {error}"
    );
}

/// Adding an empty skills directory path should fail validation.
#[tokio::test]
async fn add_watched_directory_empty_path_fails_validation() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);

    let error = service
        .add_watched_directory(std::path::PathBuf::new())
        .await
        .expect_err("empty path should fail");

    assert!(error.to_string().contains("cannot be empty"));
}

/// Adding a relative path should resolve against current directory.
#[tokio::test]
async fn add_watched_directory_relative_path_resolves() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);

    // Use a relative path
    let relative_path = std::path::PathBuf::from("relative/skills");
    service
        .add_watched_directory(relative_path.clone())
        .await
        .expect("relative path should succeed");

    let watched = service.watched_directories().await.expect("should list");
    assert_eq!(watched.len(), 1);

    // The path should have been resolved to absolute
    let cwd = std::env::current_dir().expect("should get cwd");
    let expected = cwd.join(relative_path);
    assert_eq!(watched[0], expected);
}

/// Adding a path with ~ should expand to home directory.
#[tokio::test]
async fn add_watched_directory_expands_tilde() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);

    let home_path = std::path::PathBuf::from("~/skills");
    service
        .add_watched_directory(home_path)
        .await
        .expect("tilde path should succeed");

    let watched = service.watched_directories().await.expect("should list");
    assert_eq!(watched.len(), 1);

    // The path should have been expanded to home directory
    let home = dirs::home_dir().expect("should have home dir");
    assert!(watched[0].starts_with(home));
}

/// Whitespace-only path should fail validation.
#[tokio::test]
async fn add_watched_directory_whitespace_path_fails() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);

    let whitespace_path = std::path::PathBuf::from("   ");
    let error = service
        .add_watched_directory(whitespace_path)
        .await
        .expect_err("whitespace path should fail");

    assert!(error.to_string().contains("cannot be empty"));
}
/// Discovering skills from a non-readable directory should return an error.
#[tokio::test]
#[cfg(unix)]
async fn discover_skills_fails_on_unreadable_directory() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TempDir::new().expect("temp dir should exist");
    let bundled_dir = temp_dir.path().join("bundled");
    std::fs::create_dir_all(&bundled_dir).expect("bundled dir should exist");

    // Create a subdirectory with no read permissions
    let unreadable_dir = bundled_dir.join("unreadable");
    std::fs::create_dir_all(&unreadable_dir).expect("unreadable dir should exist");
    std::fs::set_permissions(&unreadable_dir, std::fs::Permissions::from_mode(0o000))
        .expect("should set permissions");

    let service = create_service(&temp_dir);
    let result = service.discover_skills().await;

    // Clean up before assertions
    std::fs::set_permissions(&unreadable_dir, std::fs::Permissions::from_mode(0o755)).ok();

    assert!(result.is_err(), "should fail on unreadable directory");
}

/// `get_skill_body` returns `None` for a skill whose file has been deleted.
#[tokio::test]
async fn get_skill_body_returns_none_when_file_deleted() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    write_skill(
        &temp_dir.path().join("bundled"),
        "vanishing",
        "vanishing-skill",
        "Disappears",
        "Body\n",
    );

    let service = create_service(&temp_dir);
    service
        .discover_skills()
        .await
        .expect("discovery should succeed");

    // Delete the skill file after discovery
    let skill = service
        .get_skill("vanishing-skill")
        .await
        .expect("lookup should succeed")
        .expect("skill should exist");
    std::fs::remove_file(&skill.path).expect("should delete skill file");

    let result = service.get_skill_body("vanishing-skill").await;
    assert!(result.is_err(), "should error when skill file is missing");
}

/// Adding a watched directory that cannot be created should fail.
#[tokio::test]
async fn add_watched_directory_fails_on_io_error() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);

    // Try to add a path with invalid characters (on Unix, null byte is invalid)
    let invalid_path = std::path::PathBuf::from("/nonexistent\0/skills");
    let result = service.add_watched_directory(invalid_path).await;

    assert!(result.is_err(), "should fail on invalid path");
}

/// Verifies toggle-on removes from disabled list.
#[tokio::test]
async fn set_skill_enabled_toggle_removal_from_disabled_list() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    write_skill(
        &temp_dir.path().join("bundled"),
        "toggle",
        "toggle-skill",
        "Toggle test",
        "Body\n",
    );

    let service = create_service(&temp_dir);
    service
        .discover_skills()
        .await
        .expect("discovery should succeed");

    // Disable the skill
    service
        .set_skill_enabled("toggle-skill", false)
        .await
        .expect("disable should succeed");

    let disabled = service
        .app_settings_service
        .get_setting(DISABLED_SKILLS_SETTING_KEY)
        .await
        .expect("read should succeed")
        .expect("setting should exist");
    assert!(disabled.contains("toggle-skill"));

    // Re-enable the skill
    service
        .set_skill_enabled("toggle-skill", true)
        .await
        .expect("enable should succeed");

    let disabled = service
        .app_settings_service
        .get_setting(DISABLED_SKILLS_SETTING_KEY)
        .await
        .expect("read should succeed");

    if let Some(disabled_str) = disabled {
        assert!(
            !disabled_str.contains("toggle-skill"),
            "skill should be removed from disabled list"
        );
    }
}
