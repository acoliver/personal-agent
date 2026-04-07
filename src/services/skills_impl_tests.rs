use super::{SkillsServiceImpl, DISABLED_SKILLS_SETTING_KEY};
use crate::services::{
    app_settings_impl::AppSettingsServiceImpl, AppSettingsService, ServiceError, SkillsService,
};
use async_trait::async_trait;
use tempfile::TempDir;
use uuid::Uuid;

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

fn write_skill(root: &std::path::Path, dir_name: &str, name: &str, description: &str, body: &str) {
    let skill_dir = root.join(dir_name);
    std::fs::create_dir_all(&skill_dir).expect("skill dir should exist");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n{body}"),
    )
    .expect("skill file should write");
}

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

#[test]
fn normalize_github_url_converts_blob_to_raw() {
    let input = "https://github.com/anthropics/skills/blob/main/skills/docx/SKILL.md";
    let expected = "https://raw.githubusercontent.com/anthropics/skills/main/skills/docx/SKILL.md";
    assert_eq!(SkillsServiceImpl::normalize_github_url(input), expected);
}

#[test]
fn normalize_github_url_passes_through_raw_url() {
    let input = "https://raw.githubusercontent.com/anthropics/skills/main/skills/docx/SKILL.md";
    assert_eq!(SkillsServiceImpl::normalize_github_url(input), input);
}

#[test]
fn normalize_github_url_passes_through_non_github() {
    let input = "https://example.com/skills/SKILL.md";
    assert_eq!(SkillsServiceImpl::normalize_github_url(input), input);
}

#[test]
fn normalize_github_url_trims_whitespace() {
    let input = "  https://github.com/anthropics/skills/blob/main/skills/docx/SKILL.md  ";
    let expected = "https://raw.githubusercontent.com/anthropics/skills/main/skills/docx/SKILL.md";
    assert_eq!(SkillsServiceImpl::normalize_github_url(input), expected);
}

#[test]
fn sanitize_skill_slug_lowercases_and_replaces_non_alnum() {
    assert_eq!(
        SkillsServiceImpl::sanitize_skill_slug("My Cool Skill!"),
        "my-cool-skill"
    );
}

#[test]
fn sanitize_skill_slug_collapses_consecutive_separators() {
    assert_eq!(SkillsServiceImpl::sanitize_skill_slug("a---b___c"), "a-b-c");
}

#[test]
fn sanitize_skill_slug_trims_leading_trailing_dashes() {
    assert_eq!(SkillsServiceImpl::sanitize_skill_slug("--hello--"), "hello");
}

#[test]
fn sanitize_skill_slug_empty_input_returns_empty() {
    assert_eq!(SkillsServiceImpl::sanitize_skill_slug(""), "");
}

#[test]
fn install_dir_name_prefers_metadata_name() {
    let url = "https://example.com/skills/SKILL.md";
    assert_eq!(
        SkillsServiceImpl::install_dir_name_for_url(url, "My Skill"),
        "my-skill"
    );
}

#[test]
fn install_dir_name_falls_back_to_url_path_segment() {
    let url = "https://example.com/skills/docx/SKILL.md";
    assert_eq!(
        SkillsServiceImpl::install_dir_name_for_url(url, ""),
        "skill-md"
    );
}

#[test]
fn install_dir_name_uses_fallback_for_unparseable_url() {
    assert_eq!(
        SkillsServiceImpl::install_dir_name_for_url("not a url", ""),
        "imported-skill"
    );
}

#[test]
fn serialize_disabled_skill_names_produces_json_array() {
    let names = vec!["alpha".to_string(), "beta".to_string()];
    let json = SkillsServiceImpl::serialize_disabled_skill_names(&names).unwrap();
    assert_eq!(json, r#"["alpha","beta"]"#);
}

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

#[tokio::test]
async fn add_watched_directory_deduplicates() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);

    let extra_dir = temp_dir.path().join("extra_skills");
    std::fs::create_dir_all(&extra_dir).expect("create dir");

    service
        .add_watched_directory(extra_dir.clone())
        .await
        .expect("first add should succeed");
    service
        .add_watched_directory(extra_dir.clone())
        .await
        .expect("second add should succeed");

    let watched = service
        .watched_directories()
        .await
        .expect("watched should succeed");
    assert_eq!(watched.len(), 1, "duplicate should not be added");
}

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

#[tokio::test]
async fn default_user_skills_dir_returns_configured_path() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);
    let expected = temp_dir.path().join("user");
    assert_eq!(service.default_user_skills_dir(), expected);
}

#[tokio::test]
async fn remove_watched_directory_nonexistent_is_harmless() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);

    service
        .remove_watched_directory(std::path::Path::new("/nonexistent/dir"))
        .await
        .expect("removing nonexistent dir should not error");
}

#[test]
fn normalize_github_url_handles_tree_urls() {
    let input = "https://github.com/user/repo/tree/main/skills/docx";
    assert_eq!(
        SkillsServiceImpl::normalize_github_url(input),
        input,
        "tree URLs are not blob URLs and should pass through"
    );
}

#[test]
fn normalize_github_url_handles_empty_string() {
    assert_eq!(SkillsServiceImpl::normalize_github_url(""), "");
}

#[test]
fn install_dir_name_for_url_uses_metadata_slug_when_sanitized() {
    assert_eq!(
        SkillsServiceImpl::install_dir_name_for_url(
            "https://example.com/SKILL.md",
            "My Cool Skill!"
        ),
        "my-cool-skill"
    );
}

#[test]
fn normalize_directory_rejects_empty_path() {
    let error = SkillsServiceImpl::normalize_directory(std::path::Path::new(""))
        .expect_err("empty path should fail");
    assert!(error.to_string().contains("cannot be empty"));
}

#[test]
fn normalize_directory_expands_tilde_prefix() {
    let result =
        SkillsServiceImpl::normalize_directory(std::path::Path::new("~/my-skills")).unwrap();
    assert!(
        result.is_absolute(),
        "tilde expansion should produce absolute path: {result:?}"
    );
    assert!(
        result.to_string_lossy().ends_with("my-skills"),
        "should end with user-supplied segment: {result:?}"
    );
}

#[test]
fn normalize_directory_absolute_path_unchanged() {
    let result =
        SkillsServiceImpl::normalize_directory(std::path::Path::new("/usr/local/skills")).unwrap();
    assert_eq!(
        result,
        std::path::PathBuf::from("/usr/local/skills"),
        "absolute path should pass through unchanged"
    );
}

#[test]
fn normalize_directory_relative_path_resolved_against_cwd() {
    let result =
        SkillsServiceImpl::normalize_directory(std::path::Path::new("relative/skills")).unwrap();
    assert!(
        result.is_absolute(),
        "relative path should be resolved to absolute: {result:?}"
    );
    assert!(
        result.to_string_lossy().ends_with("relative/skills"),
        "should end with user-supplied relative segment: {result:?}"
    );
}

#[tokio::test]
async fn add_watched_directory_rejects_empty_path() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);

    let error = service
        .add_watched_directory(std::path::PathBuf::from(""))
        .await
        .expect_err("empty path should be rejected");
    assert!(
        error.to_string().contains("cannot be empty"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn get_skill_returns_none_for_missing_name() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);
    service.discover_skills().await.expect("discovery ok");

    let result = service
        .get_skill("no-such-skill")
        .await
        .expect("get_skill should succeed");
    assert!(result.is_none(), "missing skill should return None");
}

#[tokio::test]
async fn get_skill_body_returns_none_for_missing_name() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);
    service.discover_skills().await.expect("discovery ok");

    let result = service
        .get_skill_body("no-such-skill")
        .await
        .expect("get_skill_body should succeed");
    assert!(result.is_none(), "missing skill body should return None");
}

#[tokio::test]
async fn set_skill_enabled_unknown_name_returns_error() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);
    service.discover_skills().await.expect("discovery ok");

    let error = service
        .set_skill_enabled("no-such-skill", false)
        .await
        .expect_err("should fail for unknown skill");
    assert!(
        error.to_string().contains("no-such-skill"),
        "error should mention the skill name: {error}"
    );
}

#[tokio::test]
async fn list_skills_returns_sorted_by_name() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    write_skill(
        &temp_dir.path().join("bundled"),
        "zebra",
        "zebra-skill",
        "Z",
        "Z\n",
    );
    write_skill(
        &temp_dir.path().join("bundled"),
        "alpha",
        "alpha-skill",
        "A",
        "A\n",
    );
    let service = create_service(&temp_dir);
    service.discover_skills().await.expect("discovery ok");

    let skills = service.list_skills().await.expect("list ok");
    assert_eq!(skills.len(), 2);
    assert_eq!(skills[0].name, "alpha-skill");
    assert_eq!(skills[1].name, "zebra-skill");
}

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
        None => {} // fine — empty list was removed
        Some(raw) => {
            assert!(
                !raw.contains("docs-writer"),
                "disabled list should not contain re-enabled skill: {raw}"
            );
        }
    }
}

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

#[tokio::test]
async fn persist_and_load_watched_directories_deduplicates() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = create_service(&temp_dir);

    let dir_a = temp_dir.path().join("dir_a");
    let dir_b = temp_dir.path().join("dir_b");
    std::fs::create_dir_all(&dir_a).expect("create dir_a");
    std::fs::create_dir_all(&dir_b).expect("create dir_b");

    service
        .add_watched_directory(dir_a.clone())
        .await
        .expect("add a");
    service
        .add_watched_directory(dir_b.clone())
        .await
        .expect("add b");
    service
        .add_watched_directory(dir_a.clone())
        .await
        .expect("add a duplicate");

    let watched = service.watched_directories().await.expect("load ok");
    assert_eq!(watched.len(), 2, "should have deduplicated: {watched:?}");
}
