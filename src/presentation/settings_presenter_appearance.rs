//! Profile, theme, and font handlers for `SettingsPresenter`.

use std::sync::Arc;

use tokio::sync::broadcast;

use super::settings_presenter::SettingsPresenter;
use super::view_command::ViewCommand;
use super::view_command::{ProfileSummary, ThemeSummary};
use crate::services::{AppSettingsService, ProfileService};

use crate::ui_gpui::theme::{
    active_mono_font_family, active_mono_ligatures, available_theme_options, is_valid_theme_slug,
    set_active_font_size, set_active_mono_font_family, set_active_mono_ligatures,
    set_active_theme_slug, set_active_ui_font_family, MAX_FONT_SIZE, MIN_FONT_SIZE,
    SETTING_KEY_FONT_SIZE, SETTING_KEY_MONO_FONT_FAMILY, SETTING_KEY_MONO_LIGATURES,
    SETTING_KEY_UI_FONT_FAMILY,
};

impl SettingsPresenter {
    pub(super) async fn emit_profiles_snapshot(
        profile_service: &Arc<dyn ProfileService>,
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
    ) {
        let selected_profile_id = match profile_service.get_default().await {
            Ok(Some(profile)) => Some(profile.id),
            _ => app_settings_service
                .get_default_profile_id()
                .await
                .ok()
                .flatten(),
        };

        Self::emit_profiles_snapshot_with_default(profile_service, selected_profile_id, view_tx)
            .await;
    }

    #[allow(clippy::cognitive_complexity)]
    pub(super) async fn emit_profiles_snapshot_with_default(
        profile_service: &Arc<dyn ProfileService>,
        selected_profile_id: Option<uuid::Uuid>,
        view_tx: &broadcast::Sender<ViewCommand>,
    ) {
        let profiles = match profile_service.list().await {
            Ok(profiles) => profiles,
            Err(e) => {
                tracing::warn!("Failed to list profiles for settings snapshot: {}", e);
                return;
            }
        };

        let selected_profile_id = selected_profile_id
            .filter(|selected_id| profiles.iter().any(|profile| profile.id == *selected_id));

        let summaries = profiles
            .into_iter()
            .map(|profile| ProfileSummary {
                id: profile.id,
                name: profile.name,
                provider_id: profile.provider_id,
                model_id: profile.model_id,
                is_default: Some(profile.id) == selected_profile_id,
            })
            .collect::<Vec<_>>();

        tracing::info!(
            "SettingsPresenter::emit_profiles_snapshot: sending {} profiles, default={:?}",
            summaries.len(),
            selected_profile_id
        );
        match view_tx.send(ViewCommand::ShowSettings {
            profiles: summaries.clone(),
            selected_profile_id,
        }) {
            Ok(n) => tracing::info!("SettingsPresenter: ShowSettings sent to {} receivers", n),
            Err(e) => tracing::error!("SettingsPresenter: ShowSettings send failed: {}", e),
        }
        match view_tx.send(ViewCommand::ChatProfilesUpdated {
            profiles: summaries,
            selected_profile_id,
        }) {
            Ok(n) => tracing::info!(
                "SettingsPresenter: ChatProfilesUpdated sent to {} receivers",
                n
            ),
            Err(e) => tracing::error!("SettingsPresenter: ChatProfilesUpdated send failed: {}", e),
        }
    }

    /// Emit the list of available themes and the currently-active slug to the
    /// settings view.  Called on startup and after a successful theme switch.
    pub(super) async fn emit_theme_snapshot(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        selected_override: Option<String>,
    ) {
        let options: Vec<ThemeSummary> = available_theme_options()
            .into_iter()
            .map(|opt| ThemeSummary {
                name: opt.name,
                slug: opt.slug,
            })
            .collect();

        let persisted_slug = app_settings_service
            .get_theme()
            .await
            .ok()
            .flatten()
            .filter(|slug| is_valid_theme_slug(slug));

        let selected_slug = selected_override
            .filter(|slug| is_valid_theme_slug(slug))
            .or(persisted_slug)
            .unwrap_or_else(|| "green-screen".to_string());

        let _ = view_tx.send(ViewCommand::ShowSettingsTheme {
            options,
            selected_slug,
        });
    }

    /// Persist the selected theme slug and apply it to the runtime.
    pub(super) async fn on_select_theme(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        slug: String,
    ) {
        if !is_valid_theme_slug(&slug) {
            tracing::warn!(
                "Rejected invalid theme selection '{}'; emitting persisted snapshot",
                slug
            );
            Self::emit_theme_snapshot(app_settings_service, view_tx, None).await;
            return;
        }

        if let Err(e) = app_settings_service.set_theme(slug.clone()).await {
            tracing::warn!("Failed to persist theme selection '{}': {}", slug, e);
            Self::emit_theme_snapshot(app_settings_service, view_tx, None).await;
            return;
        }

        let applied_slug = app_settings_service
            .get_theme()
            .await
            .ok()
            .flatten()
            .filter(|persisted| is_valid_theme_slug(persisted))
            .unwrap_or_else(|| "green-screen".to_string());

        set_active_theme_slug(&applied_slug);

        Self::emit_theme_snapshot(app_settings_service, view_tx, Some(applied_slug)).await;
    }

    /// Persist and apply a new font size, then emit a font settings snapshot.
    pub(super) async fn on_set_font_size(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        size: f32,
    ) {
        let clamped_size = size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        if let Err(e) = app_settings_service
            .set_setting(SETTING_KEY_FONT_SIZE, clamped_size.to_string())
            .await
        {
            tracing::warn!("Failed to persist font_size: {}", e);
            Self::emit_font_settings_snapshot(app_settings_service, view_tx).await;
            return;
        }
        set_active_font_size(clamped_size);
        Self::emit_font_settings_snapshot(app_settings_service, view_tx).await;
    }

    /// Persist and apply a UI font family override, then emit a font settings snapshot.
    pub(super) async fn on_set_ui_font_family(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        name: Option<String>,
    ) {
        let value = name.clone().unwrap_or_default();
        if let Err(e) = app_settings_service
            .set_setting(SETTING_KEY_UI_FONT_FAMILY, value)
            .await
        {
            tracing::warn!("Failed to persist ui_font_family: {}", e);
            Self::emit_font_settings_snapshot(app_settings_service, view_tx).await;
            return;
        }
        set_active_ui_font_family(name);
        Self::emit_font_settings_snapshot(app_settings_service, view_tx).await;
    }

    /// Persist and apply a monospace font family, then emit a font settings snapshot.
    pub(super) async fn on_set_mono_font_family(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        name: String,
    ) {
        if let Err(e) = app_settings_service
            .set_setting(SETTING_KEY_MONO_FONT_FAMILY, name.clone())
            .await
        {
            tracing::warn!("Failed to persist mono_font_family: {}", e);
            Self::emit_font_settings_snapshot(app_settings_service, view_tx).await;
            return;
        }
        set_active_mono_font_family(&name);
        Self::emit_font_settings_snapshot(app_settings_service, view_tx).await;
    }

    /// Persist and apply the mono-ligatures toggle, then emit a font settings snapshot.
    pub(super) async fn on_set_mono_ligatures(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        enabled: bool,
    ) {
        if let Err(e) = app_settings_service
            .set_setting(SETTING_KEY_MONO_LIGATURES, enabled.to_string())
            .await
        {
            tracing::warn!("Failed to persist mono_ligatures: {}", e);
            Self::emit_font_settings_snapshot(app_settings_service, view_tx).await;
            return;
        }
        set_active_mono_ligatures(enabled);
        Self::emit_font_settings_snapshot(app_settings_service, view_tx).await;
    }

    /// Read all four font settings from persistence and emit `ViewCommand::ShowFontSettings`.
    pub(super) async fn emit_font_settings_snapshot(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
    ) {
        use crate::ui_gpui::theme::DEFAULT_FONT_SIZE;

        let size = app_settings_service
            .get_setting(SETTING_KEY_FONT_SIZE)
            .await
            .ok()
            .flatten()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(DEFAULT_FONT_SIZE)
            .clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);

        let ui_family = app_settings_service
            .get_setting(SETTING_KEY_UI_FONT_FAMILY)
            .await
            .ok()
            .flatten()
            .filter(|v| !v.is_empty());

        let mono_family = app_settings_service
            .get_setting(SETTING_KEY_MONO_FONT_FAMILY)
            .await
            .ok()
            .flatten()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(active_mono_font_family);

        let ligatures = app_settings_service
            .get_setting(SETTING_KEY_MONO_LIGATURES)
            .await
            .ok()
            .flatten()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or_else(active_mono_ligatures);

        let _ = view_tx.send(ViewCommand::ShowFontSettings {
            size,
            ui_family,
            mono_family,
            ligatures,
        });
    }
}
