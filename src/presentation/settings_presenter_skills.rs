//! Skills handlers for `SettingsPresenter`.

use std::sync::Arc;

use tokio::sync::broadcast;

use super::settings_presenter::SettingsPresenter;
use super::view_command::{SkillSummary, ViewCommand};
use crate::services::SkillsService;

impl SettingsPresenter {
    pub(super) async fn emit_skills_snapshot(
        skills_service: &Arc<dyn SkillsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
    ) {
        let skills = match skills_service.list_skills().await {
            Ok(skills) => skills,
            Err(error) => {
                tracing::warn!("Failed to list skills for settings snapshot: {error}");
                return;
            }
        };
        let watched_directories = match skills_service.watched_directories().await {
            Ok(directories) => directories,
            Err(error) => {
                tracing::warn!("Failed to load watched skills directories: {error}");
                Vec::new()
            }
        };
        let default_directory = skills_service.default_user_skills_dir();

        let summaries = skills
            .into_iter()
            .map(|skill| SkillSummary {
                name: skill.name,
                description: skill.description,
                source: skill.source,
                enabled: skill.enabled,
                path: skill.path.to_string_lossy().to_string(),
            })
            .collect::<Vec<_>>();

        let _ = view_tx.send(ViewCommand::SkillsLoaded {
            skills: summaries,
            watched_directories: watched_directories
                .into_iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect(),
            default_directory: default_directory.to_string_lossy().to_string(),
        });
    }

    pub(super) async fn on_set_skill_enabled(
        skills_service: &Arc<dyn SkillsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        name: String,
        enabled: bool,
    ) {
        if let Err(error) = skills_service.set_skill_enabled(&name, enabled).await {
            tracing::warn!(
                "Failed to update skill enabled state for {}: {}",
                name,
                error
            );
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Skills".to_string(),
                message: format!("Failed to update skill '{name}': {error}"),
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        }

        Self::emit_skills_snapshot(skills_service, view_tx).await;
    }

    pub(super) async fn on_add_skills_directory(
        skills_service: &Arc<dyn SkillsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        path: String,
    ) {
        if let Err(error) = skills_service
            .add_watched_directory(std::path::PathBuf::from(path.clone()))
            .await
        {
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Skills".to_string(),
                message: format!("Failed to add skills directory '{path}': {error}"),
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        }

        let _ = view_tx.send(ViewCommand::ShowNotification {
            message: format!("Added watched skills directory: {path}"),
        });
        Self::emit_skills_snapshot(skills_service, view_tx).await;
    }

    pub(super) async fn on_remove_skills_directory(
        skills_service: &Arc<dyn SkillsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        path: String,
    ) {
        if let Err(error) = skills_service
            .remove_watched_directory(std::path::Path::new(&path))
            .await
        {
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Skills".to_string(),
                message: format!("Failed to remove skills directory '{path}': {error}"),
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        }

        let _ = view_tx.send(ViewCommand::ShowNotification {
            message: format!("Removed watched skills directory: {path}"),
        });
        Self::emit_skills_snapshot(skills_service, view_tx).await;
    }

    pub(super) async fn on_install_skill_from_url(
        skills_service: &Arc<dyn SkillsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        url: String,
    ) {
        match skills_service.install_skill_from_url(&url).await {
            Ok(skill) => {
                let _ = view_tx.send(ViewCommand::ShowNotification {
                    message: format!("Installed skill '{}' from URL", skill.name),
                });
                Self::emit_skills_snapshot(skills_service, view_tx).await;
            }
            Err(error) => {
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Skills".to_string(),
                    message: format!("Failed to install skill from '{url}': {error}"),
                    severity: super::view_command::ErrorSeverity::Warning,
                });
            }
        }
    }
}
