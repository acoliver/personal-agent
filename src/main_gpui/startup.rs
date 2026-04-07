//! Startup and bootstrap helpers for the GPUI binary.
//!
//! Resolves runtime paths, builds initial `StartupInputs`, and migrates
//! legacy data from `~/.llxprt` to platform-standard directories.

use std::sync::Arc;

use personal_agent::db::spawn_db_thread;
use personal_agent::services::{
    AppSettingsService, AppSettingsServiceImpl, ConversationService, ProfileService,
    ProfileServiceImpl, SqliteConversationService,
};
use personal_agent::ui_gpui::app_store::{
    StartupInputs, StartupMode, StartupSelectedConversation, StartupTranscriptResult,
};
use personal_agent::ui_gpui::theme::{
    is_valid_theme_slug, migrate_legacy_theme_slug, set_active_font_size,
    set_active_mono_font_family, set_active_mono_ligatures, set_active_theme_slug,
    set_active_ui_font_family, DEFAULT_FONT_SIZE, DEFAULT_MONO_FONT_FAMILY, MAX_FONT_SIZE,
    MIN_FONT_SIZE, SETTING_KEY_FONT_SIZE, SETTING_KEY_MONO_FONT_FAMILY, SETTING_KEY_MONO_LIGATURES,
    SETTING_KEY_UI_FONT_FAMILY,
};

// ============================================================================
// Runtime paths
// ============================================================================

#[derive(Clone, Debug)]
pub struct RuntimePaths {
    pub base_dir: std::path::PathBuf,
    pub profiles_dir: std::path::PathBuf,
    pub secrets_dir: std::path::PathBuf,
    pub conversations_dir: std::path::PathBuf,
    pub mcp_configs_dir: std::path::PathBuf,
    pub app_settings_path: std::path::PathBuf,
}

pub fn resolve_runtime_paths() -> Result<RuntimePaths, String> {
    let data_dir = dirs::data_local_dir()
        .ok_or_else(|| "Could not determine data_local_dir for runtime paths".to_string())?
        .join("PersonalAgent");

    let config_dir = dirs::config_dir()
        .ok_or_else(|| "Could not determine config_dir for runtime paths".to_string())?
        .join("PersonalAgent");

    let profiles_dir = config_dir.join("profiles");

    Ok(RuntimePaths {
        base_dir: data_dir.clone(),
        profiles_dir,
        secrets_dir: data_dir.join("secrets"),
        conversations_dir: data_dir.join("conversations"),
        mcp_configs_dir: data_dir.join("mcp_configs"),
        app_settings_path: data_dir.join("app_settings.json"),
    })
}

// ============================================================================
// build_startup_inputs
// ============================================================================

/// @plan PLAN-20260304-GPUIREMEDIATE.P06
/// @requirement REQ-ARCH-002.1
/// @requirement REQ-ARCH-002.2
/// @requirement REQ-ARCH-002.5
/// @requirement REQ-ARCH-006.3
/// @pseudocode analysis/pseudocode/03-main-panel-integration.md:001-013
/// @plan PLAN-20260304-GPUIREMEDIATE.P08
/// @requirement REQ-ARCH-005.1
/// @pseudocode analysis/pseudocode/03-main-panel-integration.md:014-127
pub fn build_startup_inputs(runtime_paths: &RuntimePaths) -> Result<StartupInputs, String> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to create startup bootstrap runtime: {e}"))?;

    rt.block_on(build_startup_inputs_async(runtime_paths))
}

async fn build_startup_inputs_async(runtime_paths: &RuntimePaths) -> Result<StartupInputs, String> {
    let app_settings = build_startup_app_settings(runtime_paths)?;
    let conversation_service = build_startup_conversation_service(runtime_paths).await?;
    let profile_service_impl = build_startup_profile_service(runtime_paths).await?;

    apply_startup_theme_settings(&app_settings).await;
    apply_startup_font_settings(&app_settings).await;

    let selected_profile_id =
        resolve_selected_profile_id(&app_settings, &profile_service_impl).await;
    let profiles = build_profile_summaries(&profile_service_impl, selected_profile_id).await?;
    let (conversation_summaries, selected_conversation) =
        build_conversation_data(&conversation_service).await?;

    Ok(StartupInputs {
        profiles,
        selected_profile_id,
        conversations: conversation_summaries,
        selected_conversation,
    })
}

fn build_startup_app_settings(
    runtime_paths: &RuntimePaths,
) -> Result<AppSettingsServiceImpl, String> {
    AppSettingsServiceImpl::new(runtime_paths.app_settings_path.clone())
        .map_err(|e| format!("Failed to create AppSettingsService for startup bootstrap: {e}"))
}

async fn build_startup_conversation_service(
    runtime_paths: &RuntimePaths,
) -> Result<SqliteConversationService, String> {
    let db_path = runtime_paths.base_dir.join("personalagent.db");
    let db = tokio::task::spawn_blocking(move || spawn_db_thread(&db_path))
        .await
        .map_err(|e| format!("Failed to join DB spawn task for startup bootstrap: {e}"))?
        .map_err(|e| format!("Failed to spawn DB thread for startup bootstrap: {e}"))?;
    Ok(SqliteConversationService::new(db))
}

async fn build_startup_profile_service(
    runtime_paths: &RuntimePaths,
) -> Result<ProfileServiceImpl, String> {
    let profile_service_impl = ProfileServiceImpl::new(runtime_paths.profiles_dir.clone())
        .map_err(|e| format!("Failed to create ProfileService for startup bootstrap: {e}"))?;
    profile_service_impl
        .initialize()
        .await
        .map_err(|e| format!("Failed to initialize ProfileService for startup bootstrap: {e}"))?;
    Ok(profile_service_impl)
}

async fn apply_startup_theme_settings(app_settings: &AppSettingsServiceImpl) {
    // Apply persisted theme before first render so the UI uses the correct
    // palette immediately. Legacy slug values written by older versions of the
    // app are mapped to their canonical equivalents before being applied:
    //   "dark"  → "green-screen"  (was the old dark-default behavior)
    //   "light" → "default-light" (was the default light theme)
    //   "auto"  → "mac-native"    (was the OS-appearance-following option)
    // Unknown or missing slugs (after migration) fall back to "green-screen"
    // inside the theme engine.
    //
    // `PA_FORCE_THEME` overrides the persisted slug — used by UI automation
    // tests (scn_004/scn_005) to capture screenshots of each theme without
    // modifying real user settings.
    let raw_theme = read_startup_theme_slug(app_settings).await;
    let migrated_theme = migrate_legacy_theme_slug(&raw_theme).to_string();
    let saved_theme = if is_valid_theme_slug(&migrated_theme) {
        migrated_theme
    } else {
        tracing::warn!(
            "Startup: persisted theme '{}' migrated to '{}' is invalid; falling back to 'green-screen'",
            raw_theme,
            migrated_theme
        );
        "green-screen".to_string()
    };

    set_active_theme_slug(&saved_theme);
    tracing::info!(
        "Startup: applied theme '{}' (persisted: '{}')",
        saved_theme,
        raw_theme
    );
}

async fn read_startup_theme_slug(app_settings: &AppSettingsServiceImpl) -> String {
    if let Ok(forced) = std::env::var("PA_FORCE_THEME") {
        if !forced.is_empty() {
            tracing::info!("Startup: PA_FORCE_THEME override active: '{}'", forced);
            return forced;
        }
    }

    app_settings
        .get_theme()
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "green-screen".to_string())
}

async fn apply_startup_font_settings(app_settings: &AppSettingsServiceImpl) {
    // Apply persisted font settings so the first render uses the correct
    // font size, families, and ligature preference.
    let font_size = app_settings
        .get_setting(SETTING_KEY_FONT_SIZE)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(DEFAULT_FONT_SIZE)
        .clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
    set_active_font_size(font_size);

    let ui_font_family = app_settings
        .get_setting(SETTING_KEY_UI_FONT_FAMILY)
        .await
        .ok()
        .flatten()
        .filter(|v| !v.is_empty());
    set_active_ui_font_family(ui_font_family);

    let mono_font_family = app_settings
        .get_setting(SETTING_KEY_MONO_FONT_FAMILY)
        .await
        .ok()
        .flatten()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_MONO_FONT_FAMILY.to_string());
    set_active_mono_font_family(&mono_font_family);

    let mono_ligatures = app_settings
        .get_setting(SETTING_KEY_MONO_LIGATURES)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(true);
    set_active_mono_ligatures(mono_ligatures);

    tracing::info!(
        "Startup: applied font settings — size={}, mono_family={}, ligatures={}",
        font_size,
        mono_font_family,
        mono_ligatures,
    );
}

async fn resolve_selected_profile_id(
    app_settings: &AppSettingsServiceImpl,
    profile_service_impl: &ProfileServiceImpl,
) -> Option<uuid::Uuid> {
    match app_settings.get_default_profile_id().await {
        Ok(Some(id)) => Some(id),
        _ => profile_service_impl
            .get_default()
            .await
            .ok()
            .flatten()
            .map(|profile| profile.id),
    }
}

async fn build_profile_summaries(
    profile_service: &ProfileServiceImpl,
    selected_profile_id: Option<uuid::Uuid>,
) -> Result<Vec<personal_agent::presentation::view_command::ProfileSummary>, String> {
    let profiles = profile_service
        .list()
        .await
        .map_err(|e| format!("Failed to list profiles for startup bootstrap: {e}"))?;

    Ok(profiles
        .into_iter()
        .map(
            |profile| personal_agent::presentation::view_command::ProfileSummary {
                id: profile.id,
                name: profile.name,
                provider_id: profile.provider_id,
                model_id: profile.model_id,
                is_default: Some(profile.id) == selected_profile_id,
            },
        )
        .collect())
}

async fn build_conversation_data(
    conversation_service: &SqliteConversationService,
) -> Result<
    (
        Vec<personal_agent::presentation::view_command::ConversationSummary>,
        Option<StartupSelectedConversation>,
    ),
    String,
> {
    let conversations = conversation_service
        .list_metadata(None, None)
        .await
        .map_err(|e| format!("Failed to list conversations for startup bootstrap: {e}"))?;

    let summaries = conversations
        .iter()
        .map(
            |metadata| personal_agent::presentation::view_command::ConversationSummary {
                id: metadata.id,
                title: metadata
                    .title
                    .clone()
                    .filter(|title| !title.trim().is_empty())
                    .unwrap_or_else(|| "Untitled Conversation".to_string()),
                updated_at: metadata.updated_at,
                message_count: metadata.message_count,
                preview: metadata.last_message_preview.clone(),
            },
        )
        .collect::<Vec<_>>();

    let selected = match conversations.first().map(|m| m.id) {
        Some(conversation_id) => {
            let transcript_result =
                load_startup_transcript(conversation_service, conversation_id).await;
            Some(StartupSelectedConversation {
                conversation_id,
                mode: StartupMode::ModeA { transcript_result },
            })
        }
        None => None,
    };

    Ok((summaries, selected))
}

async fn load_startup_transcript(
    conversation_service: &SqliteConversationService,
    conversation_id: uuid::Uuid,
) -> StartupTranscriptResult {
    conversation_service
        .get_messages(conversation_id)
        .await
        .map(|messages| {
            StartupTranscriptResult::Success(
                messages
                    .into_iter()
                    .filter_map(|message| {
                        let role = match message.role {
                            personal_agent::models::MessageRole::User => {
                                personal_agent::presentation::view_command::MessageRole::User
                            }
                            personal_agent::models::MessageRole::Assistant => {
                                personal_agent::presentation::view_command::MessageRole::Assistant
                            }
                            personal_agent::models::MessageRole::System => return None,
                        };

                        Some(
                            personal_agent::presentation::view_command::ConversationMessagePayload {
                                role,
                                content: message.content,
                                thinking_content: message.thinking_content,
                                timestamp: Some(message.timestamp.timestamp_millis() as u64),
                                model_id: message.model_id,
                            },
                        )
                    })
                    .collect(),
            )
        })
        .unwrap_or_else(|e| {
            StartupTranscriptResult::Failure(format!(
                "Failed to load startup conversation messages for bootstrap: {e}"
            ))
        })
}

// ============================================================================
// Legacy data migration
// ============================================================================

pub fn bootstrap_legacy_runtime_data(runtime_paths: &RuntimePaths) -> Result<(), String> {
    let home = dirs::home_dir()
        .ok_or_else(|| "Could not determine home directory for bootstrap".to_string())?;
    let legacy_base = home.join(".llxprt");

    if !legacy_base.exists() {
        return Ok(());
    }

    let legacy_profiles = legacy_base.join("profiles");
    let legacy_conversations = legacy_base.join("conversations");
    let legacy_mcp_configs = legacy_base.join("mcp_configs");

    copy_json_files_if_target_empty(&legacy_profiles, &runtime_paths.profiles_dir)?;
    copy_json_files_if_target_empty(&legacy_conversations, &runtime_paths.conversations_dir)?;
    copy_json_files_if_target_empty(&legacy_mcp_configs, &runtime_paths.mcp_configs_dir)?;

    let legacy_app_settings = legacy_base.join("app_settings.json");
    if legacy_app_settings.exists() && !runtime_paths.app_settings_path.exists() {
        if let Some(parent) = runtime_paths.app_settings_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::copy(&legacy_app_settings, &runtime_paths.app_settings_path).map_err(|e| {
            format!(
                "Failed copying app settings from {} to {}: {}",
                legacy_app_settings.display(),
                runtime_paths.app_settings_path.display(),
                e
            )
        })?;
        tracing::info!(
            source = %legacy_app_settings.display(),
            target = %runtime_paths.app_settings_path.display(),
            "Bootstrapped app settings from legacy data"
        );
    }

    Ok(())
}

fn copy_json_files_if_target_empty(
    source_dir: &std::path::Path,
    target_dir: &std::path::Path,
) -> Result<(), String> {
    if !source_dir.exists() {
        return Ok(());
    }

    let source_entries = std::fs::read_dir(source_dir).map_err(|e| {
        format!(
            "Failed reading source directory {}: {}",
            source_dir.display(),
            e
        )
    })?;

    let source_json_files = source_entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect::<Vec<_>>();

    if source_json_files.is_empty() {
        return Ok(());
    }

    std::fs::create_dir_all(target_dir).map_err(|e| {
        format!(
            "Failed creating target directory {}: {}",
            target_dir.display(),
            e
        )
    })?;

    let target_has_json = std::fs::read_dir(target_dir)
        .map_err(|e| {
            format!(
                "Failed reading target directory {}: {}",
                target_dir.display(),
                e
            )
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .any(|path| path.extension().and_then(|s| s.to_str()) == Some("json"));

    if target_has_json {
        tracing::info!(
            source_dir = %source_dir.display(),
            target_dir = %target_dir.display(),
            "Skipping legacy bootstrap copy; target already has json files"
        );
        return Ok(());
    }

    for source_path in source_json_files {
        if let Some(file_name) = source_path.file_name() {
            let target_path = target_dir.join(file_name);
            if !target_path.exists() {
                std::fs::copy(&source_path, &target_path).map_err(|e| {
                    format!(
                        "Failed copying {} to {}: {}",
                        source_path.display(),
                        target_path.display(),
                        e
                    )
                })?;
            }
        }
    }

    tracing::info!(
        source_dir = %source_dir.display(),
        target_dir = %target_dir.display(),
        "Bootstrapped runtime directory from legacy data"
    );

    Ok(())
}
