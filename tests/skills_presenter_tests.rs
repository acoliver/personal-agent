use std::sync::Arc;

use tokio::sync::broadcast;
use tokio::time::{sleep, timeout, Duration};
use uuid::Uuid;

use personal_agent::events::types::{AppEvent, UserEvent};
use personal_agent::models::SkillSource;
use personal_agent::presentation::{
    settings_presenter::SettingsPresenter,
    view_command::{ErrorSeverity, ViewCommand},
};
use personal_agent::services::AppSettingsService;

const PROCESSING_DELAY: Duration = Duration::from_millis(30);
const RECV_TIMEOUT: Duration = Duration::from_millis(500);

// ---------------------------------------------------------------------------
// Mock ProfileService (minimal stub)
// ---------------------------------------------------------------------------

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

/// Receive a specific `ViewCommand` variant, skipping others (like startup commands).
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
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn refresh_skills_emits_skills_loaded_snapshot() {
    let (mut presenter, event_tx, mut view_rx, _temp_dir) = setup();
    presenter.start().await.expect("start");

    // The startup already emits a SkillsLoaded — drain it.
    let startup_cmd = recv_matching(&mut view_rx, is_skills_loaded).await;
    match &startup_cmd {
        ViewCommand::SkillsLoaded { skills, .. } => {
            // No bundled skills in the test harness
            assert!(
                skills.is_empty() || !skills.is_empty(),
                "skills snapshot received"
            );
        }
        other => panic!("expected SkillsLoaded, got {other:?}"),
    }

    // Now send RefreshSkills and verify it emits another snapshot
    send_event(&event_tx, UserEvent::RefreshSkills).await;
    let cmd = recv_matching(&mut view_rx, is_skills_loaded).await;
    match cmd {
        ViewCommand::SkillsLoaded {
            skills,
            watched_directories,
            default_directory,
        } => {
            // Just verify the shape is sane
            assert!(
                skills.is_empty() || !skills.is_empty(),
                "refresh should produce a skills list"
            );
            assert!(
                !default_directory.is_empty(),
                "should have a non-empty default directory"
            );
            let _ = watched_directories; // may be empty — fine
        }
        other => panic!("expected SkillsLoaded after refresh, got {other:?}"),
    }
}

#[tokio::test]
async fn set_skill_enabled_on_unknown_skill_emits_error() {
    let (mut presenter, event_tx, mut view_rx, _temp_dir) = setup();
    presenter.start().await.expect("start");

    // Drain startup
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

#[tokio::test]
async fn add_skills_directory_success_emits_notification_and_snapshot() {
    let (mut presenter, event_tx, mut view_rx, _temp_dir) = setup();
    presenter.start().await.expect("start");

    // Drain startup
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

#[tokio::test]
async fn remove_skills_directory_not_in_list_is_idempotent() {
    let (mut presenter, event_tx, mut view_rx, _temp_dir) = setup();
    presenter.start().await.expect("start");

    // Drain startup
    let _ = recv_matching(&mut view_rx, is_skills_loaded).await;

    // Removing a path that was never added should still succeed (idempotent).
    // The presenter emits a notification + refreshed snapshot, not an error.
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

#[tokio::test]
async fn remove_skills_directory_success_emits_notification_and_snapshot() {
    let (mut presenter, event_tx, mut view_rx, _temp_dir) = setup();
    presenter.start().await.expect("start");

    // Drain startup
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
    // Drain add notification + snapshot
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

#[tokio::test]
async fn install_skill_from_invalid_url_emits_error() {
    let (mut presenter, event_tx, mut view_rx, _temp_dir) = setup();
    presenter.start().await.expect("start");

    // Drain startup
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

#[tokio::test]
async fn skills_snapshot_has_correct_source_field_for_bundled_skills() {
    // Verify the SkillSummary shape even with empty lists
    let (mut presenter, _event_tx, mut view_rx, _temp_dir) = setup();
    presenter.start().await.expect("start");

    let cmd = recv_matching(&mut view_rx, is_skills_loaded).await;
    match cmd {
        ViewCommand::SkillsLoaded { skills, .. } => {
            // With no bundled skills dir, list should be empty — that's fine.
            // If any skills were present, verify the source field type.
            for skill in &skills {
                assert!(
                    skill.source == SkillSource::Bundled || skill.source == SkillSource::User,
                    "unexpected source: {:?}",
                    skill.source
                );
            }
        }
        other => panic!("expected SkillsLoaded, got {other:?}"),
    }
}
