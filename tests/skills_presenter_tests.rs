//! Behavioral tests for Skills-related presenter flows.
//!
//! These tests verify that user events produce the correct observable
//! `ViewCommand` outputs, using real services and persistence.

use std::sync::Arc;

use tokio::sync::broadcast;
use tokio::time::{sleep, timeout, Duration};
use uuid::Uuid;
#[cfg(test)]
use wiremock::matchers::{method, path};
#[cfg(test)]
use wiremock::{Mock, MockServer, ResponseTemplate};

use personal_agent::events::types::{AppEvent, UserEvent};
use personal_agent::presentation::{
    settings_presenter::SettingsPresenter,
    view_command::{ErrorSeverity, ViewCommand},
};
use personal_agent::services::AppSettingsService;

const PROCESSING_DELAY: Duration = Duration::from_millis(30);
const RECV_TIMEOUT: Duration = Duration::from_millis(500);

// ---------------------------------------------------------------------------
// Stub ProfileService
// ---------------------------------------------------------------------------

/// Minimal stub that returns empty results for all profile operations.
/// Skills tests don't need profile functionality.
#[derive(Clone)]
struct MinimalProfileService;

#[async_trait::async_trait]
impl personal_agent::services::ProfileService for MinimalProfileService {
    async fn list(
        &self,
    ) -> Result<Vec<personal_agent::models::ModelProfile>, personal_agent::services::ServiceError>
    {
        Ok(vec![])
    }
    async fn get(
        &self,
        _id: Uuid,
    ) -> Result<personal_agent::models::ModelProfile, personal_agent::services::ServiceError> {
        Err(personal_agent::services::ServiceError::NotFound(
            "no profiles".to_string(),
        ))
    }
    async fn create(
        &self,
        _name: String,
        _provider: String,
        _model: String,
        _base_url: Option<String>,
        _auth: personal_agent::models::AuthConfig,
        _parameters: personal_agent::models::ModelParameters,
        _system_prompt: Option<String>,
    ) -> Result<personal_agent::models::ModelProfile, personal_agent::services::ServiceError> {
        Err(personal_agent::services::ServiceError::NotFound(
            "stub".to_string(),
        ))
    }
    async fn update(
        &self,
        _id: Uuid,
        _name: Option<String>,
        _provider: Option<String>,
        _model: Option<String>,
        _base_url: Option<String>,
        _auth: Option<personal_agent::models::AuthConfig>,
        _parameters: Option<personal_agent::models::ModelParameters>,
        _system_prompt: Option<String>,
    ) -> Result<personal_agent::models::ModelProfile, personal_agent::services::ServiceError> {
        Err(personal_agent::services::ServiceError::NotFound(
            "stub".to_string(),
        ))
    }
    async fn delete(&self, _id: Uuid) -> Result<(), personal_agent::services::ServiceError> {
        Ok(())
    }
    async fn set_default(&self, _id: Uuid) -> Result<(), personal_agent::services::ServiceError> {
        Ok(())
    }
    async fn get_default(
        &self,
    ) -> Result<Option<personal_agent::models::ModelProfile>, personal_agent::services::ServiceError>
    {
        Ok(None)
    }
    async fn test_connection(
        &self,
        _id: Uuid,
    ) -> Result<(), personal_agent::services::ServiceError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup() -> (
    SettingsPresenter,
    broadcast::Sender<AppEvent>,
    broadcast::Receiver<ViewCommand>,
    tempfile::TempDir,
) {
    let temp_dir = tempfile::TempDir::new().expect("temp dir should exist");
    let (event_tx, _) = broadcast::channel(64);
    let (view_tx, view_rx) = broadcast::channel(128);

    let app_settings_service: Arc<dyn AppSettingsService> = Arc::new(
        personal_agent::services::AppSettingsServiceImpl::new(
            temp_dir.path().join("settings.json"),
        )
        .expect("app settings service should initialize"),
    );
    let skills_service = Arc::new(
        personal_agent::services::SkillsServiceImpl::new_for_tests(
            app_settings_service.clone(),
            temp_dir.path().join("bundled-skills"),
            temp_dir.path().join("user-skills"),
        )
        .expect("skills service should initialize"),
    );

    let presenter = SettingsPresenter::new(
        Arc::new(MinimalProfileService),
        app_settings_service,
        skills_service,
        &event_tx,
        view_tx,
    );

    (presenter, event_tx, view_rx, temp_dir)
}

async fn send_event(tx: &broadcast::Sender<AppEvent>, event: UserEvent) {
    let _ = tx.send(AppEvent::User(event));
    sleep(PROCESSING_DELAY).await;
}

/// Receive a specific `ViewCommand` variant, skipping others.
async fn recv_matching(
    rx: &mut broadcast::Receiver<ViewCommand>,
    predicate: fn(&ViewCommand) -> bool,
) -> ViewCommand {
    let deadline = tokio::time::Instant::now() + RECV_TIMEOUT * 3;
    loop {
        assert!(
            tokio::time::Instant::now() <= deadline,
            "timed out waiting for matching command"
        );
        match timeout(RECV_TIMEOUT, rx.recv()).await {
            Ok(Ok(cmd)) if predicate(&cmd) => return cmd,
            Ok(Ok(_) | Err(broadcast::error::RecvError::Lagged(_))) => {}
            Ok(Err(broadcast::error::RecvError::Closed)) => panic!("channel closed"),
            Err(tokio::time::error::Elapsed { .. }) => panic!("timed out"),
        }
    }
}

const fn is_skills_loaded(cmd: &ViewCommand) -> bool {
    matches!(cmd, ViewCommand::SkillsLoaded { .. })
}

const fn is_show_error(cmd: &ViewCommand) -> bool {
    matches!(cmd, ViewCommand::ShowError { .. })
}

const fn is_show_notification(cmd: &ViewCommand) -> bool {
    matches!(cmd, ViewCommand::ShowNotification { .. })
}

// ---------------------------------------------------------------------------
// BEHAVIORAL TESTS: Refresh and initial load
// ---------------------------------------------------------------------------

/// `RefreshSkills` event should emit a `SkillsLoaded` snapshot reflecting current state.
#[tokio::test]
async fn refresh_skills_emits_skills_loaded_snapshot() {
    let (mut presenter, event_tx, mut view_rx, _temp_dir) = setup();
    presenter.start().await.expect("start");

    // Drain startup snapshot
    let _ = recv_matching(&mut view_rx, is_skills_loaded).await;

    send_event(&event_tx, UserEvent::RefreshSkills).await;
    let cmd = recv_matching(&mut view_rx, is_skills_loaded).await;
    match cmd {
        ViewCommand::SkillsLoaded {
            skills,
            watched_directories,
            default_directory,
        } => {
            // Verify the snapshot shape is correct
            let _ = skills;
            let _ = watched_directories;
            assert!(
                !default_directory.is_empty(),
                "should have a non-empty default directory"
            );
        }
        other => panic!("expected SkillsLoaded after refresh, got {other:?}"),
    }
}

/// Initial startup should emit `SkillsLoaded` with the default directory populated.
#[tokio::test]
async fn skills_loaded_includes_default_directory() {
    let (mut presenter, _event_tx, mut view_rx, _temp_dir) = setup();
    presenter.start().await.expect("start");

    let cmd = recv_matching(&mut view_rx, is_skills_loaded).await;
    match cmd {
        ViewCommand::SkillsLoaded {
            default_directory, ..
        } => {
            assert!(
                !default_directory.is_empty(),
                "default_directory should not be empty"
            );
            assert!(
                default_directory.contains("skills") || default_directory.contains("Skills"),
                "default_directory should contain 'skills': {default_directory}"
            );
        }
        other => panic!("expected SkillsLoaded, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// BEHAVIORAL TESTS: Skill enable/disable
// ---------------------------------------------------------------------------

/// Attempting to disable a nonexistent skill should emit `ShowError`.
#[tokio::test]
async fn set_skill_enabled_on_unknown_skill_emits_error() {
    let (mut presenter, event_tx, mut view_rx, _temp_dir) = setup();
    presenter.start().await.expect("start");

    let _ = recv_matching(&mut view_rx, is_skills_loaded).await;

    send_event(
        &event_tx,
        UserEvent::SetSkillEnabled {
            name: "nonexistent-skill".to_string(),
            enabled: false,
        },
    )
    .await;

    let cmd = recv_matching(&mut view_rx, is_show_error).await;
    match cmd {
        ViewCommand::ShowError {
            title,
            message,
            severity,
        } => {
            assert_eq!(title, "Skills");
            assert!(
                message.contains("nonexistent-skill"),
                "error should mention the skill name: {message}"
            );
            assert_eq!(severity, ErrorSeverity::Warning);
        }
        other => panic!("expected ShowError, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// BEHAVIORAL TESTS: Watched directories
// ---------------------------------------------------------------------------

/// Adding a watched directory should emit notification and updated snapshot.
#[tokio::test]
async fn add_skills_directory_success_emits_notification_and_snapshot() {
    let (mut presenter, event_tx, mut view_rx, _temp_dir) = setup();
    presenter.start().await.expect("start");

    let _ = recv_matching(&mut view_rx, is_skills_loaded).await;

    let test_dir = std::env::temp_dir().join(format!("skills-add-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&test_dir).expect("create test dir");

    send_event(
        &event_tx,
        UserEvent::AddSkillsDirectory {
            path: test_dir.to_string_lossy().to_string(),
        },
    )
    .await;

    let notification = recv_matching(&mut view_rx, is_show_notification).await;
    match notification {
        ViewCommand::ShowNotification { message } => {
            assert!(
                message.contains("Added watched skills directory"),
                "got: {message}"
            );
        }
        other => panic!("expected ShowNotification, got {other:?}"),
    }

    let snapshot = recv_matching(&mut view_rx, is_skills_loaded).await;
    match snapshot {
        ViewCommand::SkillsLoaded {
            watched_directories,
            ..
        } => {
            assert!(
                watched_directories
                    .iter()
                    .any(|dir| dir.contains(&test_dir.to_string_lossy().to_string())),
                "watched_directories should contain the added dir: {watched_directories:?}"
            );
        }
        other => panic!("expected SkillsLoaded, got {other:?}"),
    }

    std::fs::remove_dir_all(&test_dir).ok();
}

/// Removing a watched directory should emit notification and updated snapshot.
#[tokio::test]
async fn remove_skills_directory_success_emits_notification_and_snapshot() {
    let (mut presenter, event_tx, mut view_rx, _temp_dir) = setup();
    presenter.start().await.expect("start");

    let _ = recv_matching(&mut view_rx, is_skills_loaded).await;

    // First add a directory
    let test_dir = std::env::temp_dir().join(format!("skills-rm-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&test_dir).expect("create test dir");
    send_event(
        &event_tx,
        UserEvent::AddSkillsDirectory {
            path: test_dir.to_string_lossy().to_string(),
        },
    )
    .await;
    let _ = recv_matching(&mut view_rx, is_show_notification).await;
    let _ = recv_matching(&mut view_rx, is_skills_loaded).await;

    // Now remove it
    send_event(
        &event_tx,
        UserEvent::RemoveSkillsDirectory {
            path: test_dir.to_string_lossy().to_string(),
        },
    )
    .await;

    let notification = recv_matching(&mut view_rx, is_show_notification).await;
    match notification {
        ViewCommand::ShowNotification { message } => {
            assert!(
                message.contains("Removed watched skills directory"),
                "got: {message}"
            );
        }
        other => panic!("expected ShowNotification for remove, got {other:?}"),
    }

    let snapshot = recv_matching(&mut view_rx, is_skills_loaded).await;
    match snapshot {
        ViewCommand::SkillsLoaded {
            watched_directories,
            ..
        } => {
            assert!(
                !watched_directories
                    .iter()
                    .any(|dir| dir.contains(&test_dir.to_string_lossy().to_string())),
                "watched_directories should NOT contain the removed dir: {watched_directories:?}"
            );
        }
        other => panic!("expected SkillsLoaded, got {other:?}"),
    }

    std::fs::remove_dir_all(&test_dir).ok();
}

/// Removing a directory that was never added should succeed idempotently.
#[tokio::test]
async fn remove_skills_directory_not_in_list_is_idempotent() {
    let (mut presenter, event_tx, mut view_rx, _temp_dir) = setup();
    presenter.start().await.expect("start");

    let _ = recv_matching(&mut view_rx, is_skills_loaded).await;

    send_event(
        &event_tx,
        UserEvent::RemoveSkillsDirectory {
            path: "/nonexistent/skills/dir".to_string(),
        },
    )
    .await;

    let notification = recv_matching(&mut view_rx, is_show_notification).await;
    match notification {
        ViewCommand::ShowNotification { message } => {
            assert!(
                message.contains("Removed watched skills directory"),
                "got: {message}"
            );
        }
        other => panic!("expected ShowNotification, got {other:?}"),
    }

    let snapshot = recv_matching(&mut view_rx, is_skills_loaded).await;
    match snapshot {
        ViewCommand::SkillsLoaded {
            watched_directories,
            ..
        } => {
            assert!(
                watched_directories.is_empty(),
                "no directories should be watched: {watched_directories:?}"
            );
        }
        other => panic!("expected SkillsLoaded, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// BEHAVIORAL TESTS: Skill installation
// ---------------------------------------------------------------------------

/// Installing from an invalid URL should emit `ShowError`.
#[tokio::test]
async fn install_skill_from_invalid_url_emits_error() {
    let (mut presenter, event_tx, mut view_rx, _temp_dir) = setup();
    presenter.start().await.expect("start");

    let _ = recv_matching(&mut view_rx, is_skills_loaded).await;

    send_event(
        &event_tx,
        UserEvent::InstallSkillFromUrl {
            url: "not-a-valid-url".to_string(),
        },
    )
    .await;

    let cmd = recv_matching(&mut view_rx, is_show_error).await;
    match cmd {
        ViewCommand::ShowError {
            title,
            message,
            severity,
        } => {
            assert_eq!(title, "Skills");
            assert!(
                message.contains("not-a-valid-url"),
                "error should mention the URL: {message}"
            );
            assert_eq!(severity, ErrorSeverity::Warning);
        }
        other => panic!("expected ShowError for bad URL, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// BEHAVIORAL TESTS: Skill installation from mock HTTP server
// ---------------------------------------------------------------------------

/// Installing from a valid URL with a valid skill file should create the skill
/// and emit a notification and updated snapshot with the new skill present.
#[tokio::test]
async fn install_skill_from_valid_url_creates_skill_and_emits_notification() {
    let mock_server = MockServer::start().await;

    // Mock a valid SKILL.md response
    let skill_body = "---\nname: installed-test-skill\ndescription: A skill installed from mock server\n---\nThis is the skill body.\n";
    Mock::given(method("GET"))
        .and(path("/skills/SKILL.md"))
        .respond_with(ResponseTemplate::new(200).set_body_string(skill_body))
        .mount(&mock_server)
        .await;

    let (mut presenter, event_tx, mut view_rx, _temp_dir) = setup();
    presenter.start().await.expect("start");

    let _ = recv_matching(&mut view_rx, is_skills_loaded).await;

    let url = format!("{}/skills/SKILL.md", mock_server.uri());
    send_event(
        &event_tx,
        UserEvent::InstallSkillFromUrl { url: url.clone() },
    )
    .await;

    // Should get a notification about success
    let notification = recv_matching(&mut view_rx, is_show_notification).await;
    match notification {
        ViewCommand::ShowNotification { message } => {
            assert!(
                message.contains("installed-test-skill"),
                "notification should mention skill name: {message}"
            );
        }
        other => panic!("expected ShowNotification, got {other:?}"),
    }

    // Should get an updated snapshot with the new skill
    let snapshot = recv_matching(&mut view_rx, is_skills_loaded).await;
    match snapshot {
        ViewCommand::SkillsLoaded { skills, .. } => {
            assert!(
                skills.iter().any(|s| s.name == "installed-test-skill"),
                "skills should contain the newly installed skill: {skills:?}"
            );
        }
        other => panic!("expected SkillsLoaded, got {other:?}"),
    }
}

/// Installing from a URL that returns HTTP 404 should emit `ShowError`.
#[tokio::test]
async fn install_skill_from_url_404_emits_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/missing/SKILL.md"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    let (mut presenter, event_tx, mut view_rx, _temp_dir) = setup();
    presenter.start().await.expect("start");

    let _ = recv_matching(&mut view_rx, is_skills_loaded).await;

    let url = format!("{}/missing/SKILL.md", mock_server.uri());
    send_event(
        &event_tx,
        UserEvent::InstallSkillFromUrl { url: url.clone() },
    )
    .await;

    let cmd = recv_matching(&mut view_rx, is_show_error).await;
    match cmd {
        ViewCommand::ShowError {
            title,
            message,
            severity,
        } => {
            assert_eq!(title, "Skills");
            assert!(
                message.contains("HTTP 404") || message.contains("Failed to download"),
                "error should mention HTTP failure: {message}"
            );
            assert_eq!(severity, ErrorSeverity::Warning);
        }
        other => panic!("expected ShowError for HTTP 404, got {other:?}"),
    }
}

/// Installing from a URL that returns invalid YAML frontmatter should emit `ShowError`.
#[tokio::test]
async fn install_skill_from_url_invalid_skill_content_emits_error() {
    let mock_server = MockServer::start().await;

    // Return content without proper frontmatter
    Mock::given(method("GET"))
        .and(path("/invalid/SKILL.md"))
        .respond_with(ResponseTemplate::new(200).set_body_string("This is not a valid skill file"))
        .mount(&mock_server)
        .await;

    let (mut presenter, event_tx, mut view_rx, _temp_dir) = setup();
    presenter.start().await.expect("start");

    let _ = recv_matching(&mut view_rx, is_skills_loaded).await;

    let url = format!("{}/invalid/SKILL.md", mock_server.uri());
    send_event(
        &event_tx,
        UserEvent::InstallSkillFromUrl { url: url.clone() },
    )
    .await;

    let cmd = recv_matching(&mut view_rx, is_show_error).await;
    match cmd {
        ViewCommand::ShowError {
            title,
            message,
            severity,
        } => {
            assert_eq!(title, "Skills");
            assert!(
                message.contains("frontmatter") || message.contains("parse"),
                "error should mention parsing issue: {message}"
            );
            assert_eq!(severity, ErrorSeverity::Warning);
        }
        other => panic!("expected ShowError for invalid skill content, got {other:?}"),
    }
}
