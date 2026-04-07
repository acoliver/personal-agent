//! Startup and bootstrap helpers for the GPUI binary.
//!
//! Resolves runtime paths, builds initial `StartupInputs`, and migrates
//! legacy data from `~/.llxprt` to platform-standard directories.
//!
//! Recovery support: Provides `RecoveryResult` type and `scan_backup_directory()`
//! for detecting database failures and offering automatic backup restoration.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use personal_agent::backup::BackupInfo;
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
// Database startup recovery support
// ============================================================================

/// Result of database startup check
///
/// Used to determine if the application should show the recovery view
/// or proceed with normal startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryResult {
    /// Database loaded successfully, no recovery needed
    Success,
    /// Database failed to load, recovery is required
    Required {
        /// Error message explaining why the database could not be loaded
        error: String,
        /// Available backups for recovery
        available_backups: Vec<BackupInfo>,
    },
}

/// Check database health at startup and scan for available backups
///
/// This function attempts to validate the database file and returns
/// a `RecoveryResult` indicating whether recovery is needed.
///
/// # Arguments
/// * `runtime_paths` - Runtime paths containing the database location
///
/// # Returns
/// * `RecoveryResult::Success` - Database is healthy, proceed with normal startup
/// * `RecoveryResult::Required` - Database is corrupted/missing, show recovery view
///
/// # Example
/// ```rust
/// use personal_agent::main_gpui::startup::{resolve_runtime_paths, check_database_health};
///
/// let paths = resolve_runtime_paths().expect("resolve paths");
/// match check_database_health(&paths) {
///     RecoveryResult::Success => println!("Database healthy, starting normally"),
///     RecoveryResult::Required { error, available_backups } => {
///         println!("Database error: {}", error);
///         println!("Available backups: {}", available_backups.len());
///     }
/// }
/// ```
pub fn check_database_health(runtime_paths: &RuntimePaths) -> RecoveryResult {
    let db_path = runtime_paths.base_dir.join("personalagent.db");

    // First, try to open the database to check if it's valid
    match validate_database(&db_path) {
        Ok(()) => {
            tracing::info!("Database validated successfully at: {}", db_path.display());
            RecoveryResult::Success
        }
        Err(error) => {
            tracing::error!(
                "Database validation failed at {}: {}",
                db_path.display(),
                error
            );

            // Scan for available backups
            let available_backups = scan_backup_directory(runtime_paths);

            RecoveryResult::Required {
                error,
                available_backups,
            }
        }
    }
}

/// Validate a SQLite database file
///
/// Attempts to open the database and run a quick validation check.
/// Returns Ok if the database is valid, Err with description if not.
fn validate_database(db_path: &Path) -> Result<(), String> {
    // Check if the file exists
    if !db_path.exists() {
        return Err(format!("Database file not found at: {}", db_path.display()));
    }

    // Check if it's a file (not a directory)
    if !db_path.is_file() {
        return Err(format!(
            "Database path is not a file: {}",
            db_path.display()
        ));
    }

    // Try to open and validate using SQLite
    match rusqlite::Connection::open(db_path) {
        Ok(conn) => {
            // Run PRAGMA quick_check to validate the database integrity
            let result: Result<String, rusqlite::Error> =
                conn.query_row("PRAGMA quick_check", [], |row| row.get(0));

            match result {
                Ok(check_result) => {
                    if check_result == "ok" {
                        Ok(())
                    } else {
                        Err(format!("Database integrity check failed: {}", check_result))
                    }
                }
                Err(e) => Err(format!("Failed to run integrity check: {}", e)),
            }
        }
        Err(e) => Err(format!("Failed to open database: {}", e)),
    }
}

/// Scan the backup directory for available backups
///
/// Searches for `personalagent-*.db.gz` files in the backup directory,
/// parses timestamps from filenames, and returns a sorted list of
/// `BackupInfo` structs (newest first).
///
/// # Arguments
/// * `runtime_paths` - Runtime paths for determining backup directory location
///
/// # Returns
/// A vector of `BackupInfo` structs sorted by timestamp (newest first)
pub fn scan_backup_directory(runtime_paths: &RuntimePaths) -> Vec<BackupInfo> {
    let backup_dir = get_backup_directory(runtime_paths);

    tracing::info!("Scanning for backups in: {}", backup_dir.display());

    let mut backups = Vec::new();

    if !backup_dir.exists() {
        tracing::warn!("Backup directory does not exist: {}", backup_dir.display());
        return backups;
    }

    let entries = match std::fs::read_dir(&backup_dir) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::error!("Failed to read backup directory: {}", e);
            return backups;
        }
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();

        // Check if it's a file with the expected pattern
        if !path.is_file() {
            continue;
        }

        if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
            if let Some(backup_info) = parse_backup_filename(&path, filename) {
                backups.push(backup_info);
            }
        }
    }

    // Sort by timestamp, newest first
    backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    tracing::info!(
        "Found {} backup(s) in {}",
        backups.len(),
        backup_dir.display()
    );

    backups
}

/// Get the backup directory path
///
/// Returns the default backup directory location based on the runtime paths.
fn get_backup_directory(runtime_paths: &RuntimePaths) -> PathBuf {
    runtime_paths.base_dir.join("backups")
}

/// Parse a backup filename and extract metadata
///
/// Expected format: `personalagent-YYYY-MM-DDTHH-MM-SSZ.db.gz`
///
/// # Arguments
/// * `path` - Full path to the backup file
/// * `filename` - The filename component
///
/// # Returns
/// `Some(BackupInfo)` if the filename matches the expected pattern, `None` otherwise
fn parse_backup_filename(path: &Path, filename: &str) -> Option<BackupInfo> {
    // Check if it matches the expected pattern: personalagent-*.db.gz
    if !filename.starts_with("personalagent-") || !filename.ends_with(".db.gz") {
        return None;
    }

    // Extract the timestamp portion: personalagent-YYYY-MM-DDTHH-MM-SSZ.db.gz
    let timestamp_part = filename
        .strip_prefix("personalagent-")?
        .strip_suffix(".db.gz")?;

    // Parse the timestamp: YYYY-MM-DDTHH-MM-SSZ
    let timestamp = parse_backup_timestamp(timestamp_part)?;

    // Get file size
    let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    Some(BackupInfo::new(path.to_path_buf(), timestamp, size_bytes))
}

/// Parse a backup timestamp string
///
/// Expected format: `YYYY-MM-DDTHH-MM-SSZ`
fn parse_backup_timestamp(s: &str) -> Option<DateTime<Utc>> {
    // Try to parse the timestamp
    // Format: YYYY-MM-DDTHH-MM-SSZ (with dashes instead of colons for time)
    // We replace dashes with colons in the time portion for parsing

    if s.len() != 20 {
        return None;
    }

    // Format: YYYY-MM-DDTHH-MM-SSZ
    //         01234567890123456789
    //                   ^T at 10
    //                          ^Z at 19

    if &s[10..11] != "T" || &s[19..20] != "Z" {
        return None;
    }

    let date_part = &s[0..10];
    let time_part = &s[11..19];

    // Parse date: YYYY-MM-DD
    let year: i32 = date_part[0..4].parse().ok()?;
    let month: u32 = date_part[5..7].parse().ok()?;
    let day: u32 = date_part[8..10].parse().ok()?;

    // Parse time: HH-MM-SS (with dashes instead of colons)
    let hour: u32 = time_part[0..2].parse().ok()?;
    let minute: u32 = time_part[3..5].parse().ok()?;
    let second: u32 = time_part[6..8].parse().ok()?;

    chrono::DateTime::from_timestamp(
        chrono::NaiveDate::from_ymd_opt(year, month, day)?
            .and_hms_opt(hour, minute, second)?
            .and_utc()
            .timestamp(),
        0,
    )
}

/// Decompress and restore a backup to the database location
///
/// This function decompresses a gzip-compressed backup file and writes
/// it to the database path.
///
/// # Arguments
/// * `backup_path` - Path to the `.db.gz` backup file
/// * `db_path` - Destination path for the restored database
///
/// # Returns
/// * `Ok(())` - Restore completed successfully
/// * `Err(String)` - Restore failed with error message
///
/// # Example
/// ```rust
/// use std::path::Path;
/// use personal_agent::main_gpui::startup::restore_backup_to_db;
///
/// # async fn example() -> Result<(), String> {
/// let backup_path = Path::new("/backups/personalagent-2026-04-05.db.gz");
/// let db_path = Path::new("/data/personalagent.db");
/// restore_backup_to_db(backup_path, db_path).await?;
/// # Ok(())
/// # }
/// ```
pub async fn restore_backup_to_db(backup_path: &Path, db_path: &Path) -> Result<(), String> {
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;

    tracing::info!(
        "Starting restore from {} to {}",
        backup_path.display(),
        db_path.display()
    );

    // Verify the backup file exists
    if !backup_path.exists() {
        return Err(format!("Backup file not found: {}", backup_path.display()));
    }

    // Create parent directory if needed
    if let Some(parent) = db_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create database directory: {}", e))?;
    }

    // Decompress the backup file
    let decompressed_data = decompress_backup_file(backup_path).await?;

    // Write to the database location
    let mut file = File::create(db_path)
        .await
        .map_err(|e| format!("Failed to create database file: {}", e))?;

    file.write_all(&decompressed_data)
        .await
        .map_err(|e| format!("Failed to write database file: {}", e))?;

    file.flush()
        .await
        .map_err(|e| format!("Failed to flush database file: {}", e))?;

    // Drop the file to ensure it's closed
    drop(file);

    // Verify the restored database is valid
    validate_database(db_path)?;

    tracing::info!(
        "Restore completed successfully: {} bytes written to {}",
        decompressed_data.len(),
        db_path.display()
    );

    Ok(())
}

/// Decompress a gzip backup file
///
/// # Arguments
/// * `backup_path` - Path to the `.db.gz` file
///
/// # Returns
/// * `Ok(Vec<u8>)` - Decompressed data
/// * `Err(String)` - Decompression failed
async fn decompress_backup_file(backup_path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;

    let file = std::fs::File::open(backup_path)
        .map_err(|e| format!("Failed to open backup file: {}", e))?;

    let mut decoder = GzDecoder::new(file);
    let mut decompressed = Vec::new();

    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| format!("Failed to decompress backup file: {}", e))?;

    Ok(decompressed)
}

/// Async wrapper for database health check
///
/// This allows the health check to be run in an async context.
pub async fn check_database_health_async(runtime_paths: &RuntimePaths) -> RecoveryResult {
    let runtime_paths = runtime_paths.clone();
    tokio::task::spawn_blocking(move || check_database_health(&runtime_paths))
        .await
        .unwrap_or_else(|e| RecoveryResult::Required {
            error: format!("Database check panicked: {}", e),
            available_backups: Vec::new(),
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
