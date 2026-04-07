use std::path::{Path, PathBuf};
use std::sync::Arc;

use personal_agent::services::{
    app_settings_impl::AppSettingsServiceImpl, skill_parser::parse_skill_file, SkillsService,
    SkillsServiceImpl,
};
use personal_agent::SkillSource;
use tempfile::TempDir;

fn fixture_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("skills")
        .join(relative)
}

fn make_skills_service(temp_dir: &TempDir) -> SkillsServiceImpl {
    let app_settings = Arc::new(
        AppSettingsServiceImpl::new(temp_dir.path().join("settings.json"))
            .expect("app settings should initialize"),
    );
    SkillsServiceImpl::new_for_tests(app_settings, fixture_path("bundled"), fixture_path("user"))
        .expect("skills service should initialize")
}

#[test]
fn parse_real_world_fixture_skills() {
    let fixtures = [
        (
            "bundled/claude-code/doc-coauthoring/SKILL.md",
            "doc-coauthoring",
            "Use when asked to draft, revise, or collaboratively refine documents.",
        ),
        (
            "bundled/gemini-cli/docs-writer/SKILL.md",
            "docs-writer",
            "Use when writing or revising documentation with consistent structure and tone.",
        ),
        (
            "user/openclaw/meeting-notes-writer/SKILL.md",
            "meeting-notes-writer",
            "Use when converting rough notes into polished meeting summaries and action items.",
        ),
    ];

    for (relative, expected_name, expected_description) in fixtures {
        let (metadata, body) =
            parse_skill_file(&fixture_path(relative)).expect("fixture skill should parse");
        assert_eq!(metadata.name, expected_name);
        assert_eq!(metadata.description, expected_description);
        assert!(
            !body.trim().is_empty(),
            "body should not be empty for {relative}"
        );
        assert!(
            !body.contains("name:"),
            "body should not leak frontmatter for {relative}"
        );
    }
}

#[test]
fn parse_extended_metadata_fixture() {
    let (metadata, body) = parse_skill_file(&fixture_path(
        "bundled/edge-cases/extended-metadata/SKILL.md",
    ))
    .expect("extended metadata skill should parse");

    assert_eq!(metadata.name, "extended-metadata-skill");
    assert_eq!(
        metadata.metadata.get("ecosystem"),
        Some(&"test".to_string())
    );
    assert_eq!(
        metadata.metadata.get("complexity"),
        Some(&"moderate".to_string())
    );
    assert!(body.contains("Verify optional metadata parsing"));
}

#[tokio::test]
async fn discovery_lists_cross_ecosystem_skills_and_user_precedence() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = make_skills_service(&temp_dir);

    service
        .discover_skills()
        .await
        .expect("discovery should succeed");
    let skills = service.list_skills().await.expect("list should succeed");

    let names = skills
        .iter()
        .map(|skill| skill.name.as_str())
        .collect::<Vec<_>>();
    assert!(names.contains(&"doc-coauthoring"));
    assert!(names.contains(&"docs-writer"));
    assert!(names.contains(&"meeting-notes-writer"));
    assert!(names.contains(&"shared-writing-skill"));

    let shared = skills
        .iter()
        .find(|skill| skill.name == "shared-writing-skill")
        .expect("shared-writing-skill should exist");
    assert_eq!(shared.source, SkillSource::User);
    assert_eq!(
        shared.description,
        "User override version of the shared writing skill."
    );
}

#[tokio::test]
async fn activation_returns_full_body_for_hyphenated_skill_names() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = make_skills_service(&temp_dir);

    service
        .discover_skills()
        .await
        .expect("discovery should succeed");

    let body = service
        .get_skill_body("my-writing-helper")
        .await
        .expect("body read should succeed")
        .expect("skill body should exist");

    assert!(body.contains("Create a concise outline"));
    assert!(!body.contains("description:"));
}

#[tokio::test]
async fn disabling_skill_filters_enabled_skills() {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    let service = make_skills_service(&temp_dir);

    service
        .discover_skills()
        .await
        .expect("discovery should succeed");
    service
        .set_skill_enabled("docs-writer", false)
        .await
        .expect("disable should succeed");

    let enabled = service
        .get_enabled_skills()
        .await
        .expect("enabled list should succeed");
    assert!(enabled.iter().all(|skill| skill.name != "docs-writer"));
}

#[test]
fn parser_rejects_missing_required_fields_and_malformed_yaml() {
    let temp = TempDir::new().expect("temp dir should exist");

    let missing_name = temp.path().join("missing-name.md");
    std::fs::write(&missing_name, "---\ndescription: Missing name\n---\nBody\n")
        .expect("fixture should write");
    let error = parse_skill_file(&missing_name).expect_err("missing name should fail");
    assert!(
        error.to_string().contains("missing field `name`")
            || error.to_string().contains("non-empty name")
    );

    let missing_description = temp.path().join("missing-description.md");
    std::fs::write(
        &missing_description,
        "---\nname: missing-description\n---\nBody\n",
    )
    .expect("fixture should write");
    let error =
        parse_skill_file(&missing_description).expect_err("missing description should fail");
    assert!(
        error.to_string().contains("missing field `description`")
            || error.to_string().contains("non-empty description")
    );

    let malformed_yaml = temp.path().join("malformed.md");
    std::fs::write(
        &malformed_yaml,
        "---\nname: bad\ndescription: [unterminated\n---\nBody\n",
    )
    .expect("fixture should write");
    let error = parse_skill_file(&malformed_yaml).expect_err("malformed yaml should fail");
    assert!(error.to_string().contains("Invalid skill frontmatter YAML"));
}
