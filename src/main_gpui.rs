//! PersonalAgent GPUI
//!
//! A macOS menu bar app with chat interface using GPUI.
//!
//! Uses NSEvent local monitor to capture button clicks within the app's run loop.
//!
//! @plan PLAN-20260219-NEXTGPUIREMEDIATE.P03
//! @requirement REQ-WIRE-001
//! @pseudocode component-001-event-pipeline.md lines 090-136

#![allow(unexpected_cfgs)]
#![allow(clippy::all)]
#![allow(clippy::pedantic)]
#![allow(unused_imports)]
#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{fs, path::PathBuf};

use gpui::*;
use tokio::sync::watch;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

// Use the library crate
use personal_agent::db::spawn_db_thread;
use personal_agent::events::types::UserEvent;
use personal_agent::events::EventBus;
use personal_agent::llm::client_agent::ApprovalGate;
use personal_agent::presentation::{
    ApiKeyManagerPresenter, ChatPresenter, ErrorPresenter, HistoryPresenter, McpAddPresenter,
    McpConfigurePresenter, ModelSelectorPresenter, ProfileEditorPresenter, SettingsPresenter,
    ViewCommand,
};
use personal_agent::services::{
    AppSettingsService, AppSettingsServiceImpl, BackupService, BackupServiceImpl, ChatService,
    ChatServiceImpl, ConversationService, McpRegistryService, McpRegistryServiceImpl, McpService,
    McpServiceImpl, ModelsRegistryService, ModelsRegistryServiceImpl, ProfileService,
    ProfileServiceImpl, SecretsService, SecretsServiceImpl, SkillsService, SkillsServiceImpl,
    SqliteConversationService,
};
use personal_agent::ui_gpui::app_store::{
    BeginSelectionMode, BeginSelectionResult, StartupInputs, StartupMode,
    StartupSelectedConversation, StartupTranscriptResult,
};
use personal_agent::ui_gpui::bridge::{spawn_user_event_forwarder, GpuiBridge};
use personal_agent::ui_gpui::selection_intent_channel;
use personal_agent::ui_gpui::theme::Theme;
use personal_agent::ui_gpui::views::main_panel::MainPanel;
use personal_agent::ui_gpui::views::main_panel::MainPanelAppState;
use personal_agent::ui_gpui::GpuiAppStore;

#[path = "main_gpui/startup.rs"]
mod startup;
#[path = "main_gpui/system_tray.rs"]
mod system_tray;

use startup::{build_startup_inputs, resolve_runtime_paths, RuntimePaths};
use system_tray::SystemTray;

#[cfg(target_os = "macos")]
use objc2::MainThreadMarker;

// ============================================================================
// Global application state
// ============================================================================

/// Global application state (full version with all fields).
///
/// @plan PLAN-20260304-GPUIREMEDIATE.P04
/// @requirement REQ-ARCH-001.1
/// @requirement REQ-ARCH-001.3
/// @pseudocode analysis/pseudocode/01-app-store.md:001-098

#[derive(Clone)]
pub struct AppState {
    /// Event bus for the application
    event_bus: Arc<EventBus>,
    /// GPUI bridge for UI events
    gpui_bridge: Arc<GpuiBridge>,
    /// View command sender (to send commands to UI)
    view_cmd_tx: flume::Sender<personal_agent::presentation::ViewCommand>,
    /// Process-lifetime authoritative GPUI store skeleton.
    app_store: Arc<GpuiAppStore>,
}

impl Global for AppState {}

/// Re-emit MCP config snapshot directly into the flume channel.
///
/// Called from `open_popup` so that a newly-created `MainPanel` (and its
/// fresh `SettingsView`) receives the current MCP list.  The one-shot
/// broadcast emission at startup was already consumed by the previous
/// (now-closed) window.
fn emit_mcp_snapshot_to_flume(tx: &flume::Sender<personal_agent::presentation::ViewCommand>) {
    use personal_agent::presentation::view_command::{McpStatus, ViewCommand};

    let config_path = match personal_agent::config::Config::default_path() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Cannot resolve config path for MCP snapshot (open_popup): {e}");
            return;
        }
    };
    let config = match personal_agent::config::Config::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Cannot load config for MCP snapshot (open_popup): {e}");
            return;
        }
    };

    let global_mcp = personal_agent::mcp::McpService::global();

    for mcp in &config.mcps {
        let runtime_status = global_mcp
            .try_lock()
            .ok()
            .and_then(|svc| svc.get_status(&mcp.id));

        let status = match runtime_status {
            Some(personal_agent::mcp::McpStatus::Running) => McpStatus::Running,
            Some(
                personal_agent::mcp::McpStatus::Starting
                | personal_agent::mcp::McpStatus::Restarting,
            ) => McpStatus::Starting,
            Some(personal_agent::mcp::McpStatus::Error(_)) => McpStatus::Failed,
            Some(
                personal_agent::mcp::McpStatus::Stopped | personal_agent::mcp::McpStatus::Disabled,
            ) => McpStatus::Stopped,
            None if mcp.enabled => McpStatus::Starting,
            None => McpStatus::Stopped,
        };

        let _ = tx.send(ViewCommand::McpServerStarted {
            id: mcp.id,
            name: Some(mcp.name.clone()),
            tool_count: 0,
            enabled: Some(mcp.enabled),
        });
        let _ = tx.send(ViewCommand::McpStatusChanged { id: mcp.id, status });
    }

    tracing::info!(
        "emit_mcp_snapshot_to_flume: sent {} MCP entries on popup reopen",
        config.mcps.len()
    );
}

/// Re-emit backup settings snapshot directly into the flume channel.
///
/// Called from `open_popup` so that a newly-created `MainPanel` (and its
/// fresh `SettingsView`) receives the current backup settings.  The one-shot
/// broadcast emission at startup was already consumed by the previous
/// (now-closed) window.
fn emit_backup_snapshot_to_flume(tx: &flume::Sender<personal_agent::presentation::ViewCommand>) {
    use personal_agent::backup::DatabaseBackupSettings;
    use personal_agent::presentation::view_command::ViewCommand;

    // Get backup settings from app_settings.json
    let settings_path = dirs::data_local_dir()
        .map(|d| d.join("PersonalAgent").join("app_settings.json"))
        .unwrap_or_else(|| PathBuf::from("app_settings.json"));

    let settings = if settings_path.exists() {
        fs::read_to_string(&settings_path)
            .ok()
            .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
            .and_then(|storage| storage.get("extra_settings")?.as_object().cloned())
            .map_or_else(DatabaseBackupSettings::default, |extra| {
                extra
                    .get("database_backup_settings")
                    .and_then(|v| serde_json::from_value::<DatabaseBackupSettings>(v.clone()).ok())
                    .unwrap_or_default()
            })
    } else {
        DatabaseBackupSettings::default()
    };

    // List backups from directory
    let backup_dir = settings.effective_backup_directory();
    let backups = backup_dir.map_or_else(Vec::new, |dir| {
        personal_agent::services::BackupServiceImpl::list_backups_in_dir(&dir).unwrap_or_default()
    });

    let _ = tx.send(ViewCommand::BackupSettingsLoaded {
        settings,
        backups,
        last_backup_time: None, // We don't have easy access to this here
    });

    tracing::info!("emit_backup_snapshot_to_flume: sent backup settings on popup reopen");
}

fn spawn_mpsc_to_flume_view_command_bridge(
    mut rx: tokio::sync::mpsc::Receiver<personal_agent::presentation::ViewCommand>,
    tx: flume::Sender<personal_agent::presentation::ViewCommand>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Some(cmd) => {
                    if tx.send(cmd).is_err() {
                        tracing::warn!("Main view-command bridge: flume receiver dropped");
                        break;
                    }
                }
                None => {
                    tracing::info!("Main view-command bridge: mpsc sender closed");
                    break;
                }
            }
        }
    })
}

fn spawn_broadcast_to_mpsc_view_command_bridge(
    mut rx: tokio::sync::broadcast::Receiver<personal_agent::presentation::ViewCommand>,
    tx: tokio::sync::mpsc::Sender<personal_agent::presentation::ViewCommand>,
    presenter_name: &'static str,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!(
            "{} bridge: task started, waiting for commands",
            presenter_name
        );
        loop {
            match rx.recv().await {
                Ok(cmd) => {
                    tracing::info!(
                        "{} bridge: forwarding command {:?}",
                        presenter_name,
                        std::mem::discriminant(&cmd)
                    );
                    if tx.send(cmd).await.is_err() {
                        tracing::warn!("{} bridge: view command receiver dropped", presenter_name);
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("{} bridge lagged: {} commands dropped", presenter_name, n);
                }

                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    tracing::info!("{} bridge closed", presenter_name);
                    break;
                }
            }
        }
    })
}

// ============================================================================
// Runtime bridge pump + helpers
// ============================================================================

/// @plan PLAN-20260304-GPUIREMEDIATE.P05
/// @requirement REQ-ARCH-003.6
/// @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:004-014
fn handle_select_conversation_intent(app_state: &AppState, conversation_id: uuid::Uuid) {
    match app_state
        .app_store
        .begin_selection(conversation_id, BeginSelectionMode::PublishImmediately)
    {
        BeginSelectionResult::NoOpSameSelection => {}
        BeginSelectionResult::BeganSelection { generation } => {
            let sent = app_state.gpui_bridge.emit(UserEvent::SelectConversation {
                id: conversation_id,
                selection_generation: generation,
            });
            if !sent {
                app_state
                    .app_store
                    .reduce_batch(vec![ViewCommand::ConversationLoadFailed {
                        conversation_id,
                        selection_generation: generation,
                        message: "SelectConversation transport enqueue failed".to_string(),
                    }]);
            }
        }
    }
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P05
/// @requirement REQ-ARCH-003.6
/// @pseudocode analysis/pseudocode/03-main-panel-integration.md:037-055
fn drain_selection_intents(app_state: &AppState) {
    while let Some(conversation_id) = selection_intent_channel().take_pending() {
        handle_select_conversation_intent(app_state, conversation_id);
    }
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P05
/// @requirement REQ-ARCH-003.4
/// @requirement REQ-ARCH-004.1
/// @pseudocode analysis/pseudocode/03-main-panel-integration.md:014-036
fn forward_runtime_commands_to_main_panel(commands: Vec<ViewCommand>, cx: &mut AsyncApp) {
    if commands.is_empty() {
        return;
    }

    let popup_window = cx
        .try_read_global::<MainPanelAppState, _>(|state, _| state.popup_window)
        .flatten();

    if let Some(window_handle) = popup_window {
        let _ = window_handle.update(cx, |main_panel: &mut MainPanel, _, cx| {
            for command in commands {
                main_panel.handle_command(command, cx);
            }
            cx.notify();
        });
    }
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P05
/// @requirement REQ-ARCH-003.4
/// @requirement REQ-ARCH-004.1
/// @pseudocode analysis/pseudocode/03-main-panel-integration.md:014-036
#[allow(clippy::needless_pass_by_ref_mut)]
fn spawn_runtime_bridge_pump(app_state: AppState, cx: &mut App) {
    cx.spawn(async move |cx| loop {
        cx.background_executor()
            .timer(Duration::from_millis(100))
            .await;

        drain_selection_intents(&app_state);

        let commands = app_state.gpui_bridge.drain_commands();
        let mut non_store_commands: Vec<ViewCommand> = Vec::new();
        let mut toggle_window_mode_count = 0usize;
        for cmd in commands.iter() {
            if personal_agent::ui_gpui::is_store_managed(cmd) {
                continue;
            }
            if matches!(cmd, ViewCommand::ToggleWindowMode) {
                toggle_window_mode_count += 1;
                continue;
            }
            non_store_commands.push(cmd.clone());
        }
        // Fixes Issue #178: when the reducer auto-selects a successor
        // conversation (e.g. after a delete) it records the pending selection
        // in the `BatchReduceResult`. Emit the corresponding
        // `UserEvent::SelectConversation` so the presenter loads the
        // replacement transcript — without this, the sidebar shows the
        // new selection but the chat view stays empty.
        let reduce_result = app_state.app_store.reduce_batch_with_result(commands);
        if let Some((id, selection_generation)) = reduce_result.pending_selection {
            let sent = app_state.gpui_bridge.emit(UserEvent::SelectConversation {
                id,
                selection_generation,
            });
            if !sent {
                app_state
                    .app_store
                    .reduce_batch(vec![ViewCommand::ConversationLoadFailed {
                        conversation_id: id,
                        selection_generation,
                        message: "SelectConversation transport enqueue failed".to_string(),
                    }]);
            }
        }
        forward_runtime_commands_to_main_panel(non_store_commands, cx);
        if toggle_window_mode_count % 2 == 1 {
            let _ = cx.update_global::<SystemTray, _>(|tray, cx| {
                tray.toggle_window_mode(cx);
            });
        }
    })
    .detach();
}

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber).ok();

    info!("PersonalAgent GPUI starting...");
    Application::new()
        .with_assets(personal_agent::ui_gpui::app_assets::AppAssets)
        .run(|cx: &mut App| run_gpui_app(cx));
}

#[cfg(target_os = "linux")]
fn create_linux_system_tray() -> Option<SystemTray> {
    match SystemTray::new() {
        Ok(tray) => Some(tray),
        Err(error) => {
            tracing::error!(?error, "Failed to initialize Linux system tray");
            None
        }
    }
}

fn start_tray_and_apply_popup_flags(tray: &mut SystemTray, cx: &mut App) {
    tray.start_click_listener(cx);

    if std::env::var("PA_AUTO_OPEN_POPUP").ok().as_deref() == Some("1") {
        tray.toggle_popup(cx);
        info!("GPUI initialized in tray mode; popup auto-opened for automation");
    } else {
        info!("GPUI initialized in tray mode; click the status icon to open popup");
    }

    if std::env::var("PA_TEST_POPUP_ONSCREEN").ok().as_deref() == Some("1") {
        info!("PA_TEST_POPUP_ONSCREEN=1 active (automation popup positioning override)");
    }
}

#[allow(clippy::cognitive_complexity)]
fn run_gpui_app(cx: &mut App) {
    cx.set_quit_mode(QuitMode::Explicit);

    #[cfg(target_os = "macos")]
    let Some(mtm) = MainThreadMarker::new() else {
        tracing::error!("Not on main thread!");
        return;
    };

    let event_bus = Arc::new(personal_agent::events::global::get_event_bus_clone());
    let (user_tx, user_rx) = flume::bounded(256);
    let (view_cmd_tx, view_cmd_rx) = flume::bounded(1024);
    let gpui_bridge = Arc::new(GpuiBridge::new(user_tx, view_cmd_rx));

    let runtime_paths = resolve_runtime_paths()
        .expect("Could not resolve runtime paths from platform config/data directories");
    let startup_inputs = match build_startup_inputs(&runtime_paths) {
        Ok(inputs) => inputs,
        Err(e) => {
            tracing::warn!("Failed to build startup bootstrap inputs: {}", e);
            StartupInputs {
                profiles: Vec::new(),
                selected_profile_id: None,
                conversations: Vec::new(),
                selected_conversation: None,
            }
        }
    };
    let app_store = Arc::new(GpuiAppStore::from_startup_inputs(startup_inputs));

    let app_state = AppState {
        event_bus: Arc::clone(&event_bus),
        gpui_bridge: Arc::clone(&gpui_bridge),
        view_cmd_tx,
        app_store: Arc::clone(&app_store),
    };
    cx.set_global(app_state.clone());

    cx.set_global(MainPanelAppState {
        gpui_bridge,
        popup_window: None,
        app_store,
        app_mode: personal_agent::presentation::view_command::AppMode::Popup,
    });

    use personal_agent::ui_gpui::views::main_panel::{
        NavigateBack, NavigateToHistory, NavigateToSettings, NewConversation, ToggleSidebar,
        ToggleWindowMode, ZoomIn, ZoomOut, ZoomReset,
    };
    cx.bind_keys([
        KeyBinding::new("ctrl-h", NavigateToHistory, None),
        KeyBinding::new("ctrl-s", NavigateToSettings, None),
        KeyBinding::new("ctrl-n", NewConversation, None),
        KeyBinding::new("cmd-w", NavigateBack, None),
        KeyBinding::new("cmd-=", ZoomIn, None),
        KeyBinding::new("cmd-+", ZoomIn, None),
        KeyBinding::new("cmd--", ZoomOut, None),
        KeyBinding::new("cmd-0", ZoomReset, None),
        KeyBinding::new("cmd-shift-p", ToggleWindowMode, None),
        KeyBinding::new("cmd-b", ToggleSidebar, None),
    ]);

    spawn_runtime_bridge_pump(app_state, cx);

    #[cfg(target_os = "macos")]
    let mut tray = SystemTray::new(mtm);
    #[cfg(target_os = "linux")]
    let mut tray = match create_linux_system_tray() {
        Some(tray) => tray,
        None => return,
    };
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let mut tray = SystemTray::new();
    start_tray_and_apply_popup_flags(&mut tray, cx);

    cx.set_global(tray);

    let event_bus_for_tokio = Arc::clone(&event_bus);
    let view_cmd_tx_for_tokio = cx.global::<AppState>().view_cmd_tx.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(run_tokio_runtime(
            event_bus_for_tokio,
            view_cmd_tx_for_tokio,
            user_rx,
        ));
    });

    info!("PersonalAgent GPUI initialized - click tray icon to open");
}

/// Boot all services and presenters inside the background tokio runtime,
/// then keep the runtime alive indefinitely.
fn log_runtime_paths(runtime_paths: &RuntimePaths) {
    tracing::info!(
        data_dir = %runtime_paths.base_dir.display(),
        config_dir = %runtime_paths.profiles_dir.parent().unwrap_or(&runtime_paths.profiles_dir).display(),
        profiles_dir = %runtime_paths.profiles_dir.display(),
        conversations_dir = %runtime_paths.conversations_dir.display(),
        mcp_configs_dir = %runtime_paths.mcp_configs_dir.display(),
        "Using platform-standard runtime directories"
    );
}

fn ensure_runtime_directories(runtime_paths: &RuntimePaths) {
    let _ = std::fs::create_dir_all(&runtime_paths.profiles_dir);
    let _ = std::fs::create_dir_all(&runtime_paths.secrets_dir);
    let _ = std::fs::create_dir_all(&runtime_paths.conversations_dir);
    let _ = std::fs::create_dir_all(&runtime_paths.mcp_configs_dir);
}

async fn initialize_global_mcp_runtime() {
    info!("Initializing global MCP runtime...");
    let global = personal_agent::mcp::McpService::global();
    let mut svc = global.lock().await;
    if let Err(e) = svc.initialize().await {
        tracing::error!("Global MCP initialization failed: {e}");
    } else {
        info!("Global MCP runtime initialized");
    }
}

fn start_backup_scheduler(
    backup_service: Arc<dyn personal_agent::services::BackupService>,
) -> (tokio::task::JoinHandle<()>, watch::Sender<bool>) {
    info!("Starting backup scheduler...");
    personal_agent::backup::spawn_backup_scheduler(backup_service)
}

async fn runtime_keepalive_loop() {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_hours(1)).await;
    }
}

async fn run_tokio_runtime(
    event_bus: Arc<EventBus>,
    view_cmd_tx: flume::Sender<personal_agent::presentation::ViewCommand>,
    user_rx: flume::Receiver<UserEvent>,
) {
    // Spawn user event forwarder (bridges GPUI events to EventBus)
    let _ = spawn_user_event_forwarder(Arc::clone(&event_bus), user_rx);

    let runtime_paths =
        resolve_runtime_paths().expect("Could not resolve runtime paths from platform directories");

    log_runtime_paths(&runtime_paths);
    ensure_runtime_directories(&runtime_paths);

    // Create mpsc channel for ViewCommands (presenter -> view_cmd_tx -> flume)
    let (view_tx, view_rx) = tokio::sync::mpsc::channel(256);
    let _main_view_cmd_bridge =
        spawn_mpsc_to_flume_view_command_bridge(view_rx, view_cmd_tx.clone());

    let services = create_services(&runtime_paths, view_tx.clone()).await;

    let (presenter_bridges, settings_view_tx_for_snapshot) =
        create_presenter_channels_and_bridges(&event_bus, &services, view_tx).await;

    // Emit MCP snapshot immediately so settings view is populated
    // before the slow (~90s) global MCP init.
    personal_agent::presentation::SettingsPresenter::emit_mcp_snapshot(
        &settings_view_tx_for_snapshot,
    );

    initialize_global_mcp_runtime().await;
    let (backup_scheduler_handle, _backup_shutdown_tx) =
        start_backup_scheduler(services.backup.clone());

    // Prevent handles in `presenter_bridges` from being dropped (which would close the channels)
    let _keep_alive = presenter_bridges;

    // Keep the backup scheduler handle alive
    let _backup_handle = backup_scheduler_handle;

    runtime_keepalive_loop().await;
}

struct Services {
    _secrets: Arc<dyn SecretsService>,
    app_settings: Arc<dyn AppSettingsService>,
    conversation: Arc<dyn ConversationService>,
    profile: Arc<dyn ProfileService>,
    skills: Arc<dyn SkillsService>,
    mcp: Arc<dyn McpService>,
    models_registry: Arc<dyn ModelsRegistryService>,
    mcp_registry: Arc<dyn McpRegistryService>,
    chat: Arc<dyn ChatService>,
    backup: Arc<dyn personal_agent::services::BackupService>,
}

async fn create_services(
    runtime_paths: &RuntimePaths,
    view_tx: tokio::sync::mpsc::Sender<personal_agent::presentation::ViewCommand>,
) -> Services {
    let secrets: Arc<dyn SecretsService> = Arc::new(
        SecretsServiceImpl::new(runtime_paths.secrets_dir.clone())
            .expect("Failed to create SecretsService"),
    );
    let app_settings: Arc<dyn AppSettingsService> = Arc::new(
        AppSettingsServiceImpl::new(runtime_paths.app_settings_path.clone())
            .expect("Failed to create AppSettingsService"),
    );
    let db_path = runtime_paths.base_dir.join("personalagent.db");
    let db_path_for_backup = db_path.clone();
    let db = tokio::task::spawn_blocking(move || spawn_db_thread(&db_path))
        .await
        .expect("spawn_blocking join failed")
        .expect("Failed to spawn DB thread");

    // Clone the DbHandle for the backup service (DbHandle is cheap to clone)
    let db_for_backup = db.clone();

    let conversation: Arc<dyn ConversationService> = Arc::new(SqliteConversationService::new(db));

    // Create backup service
    let backup: Arc<dyn personal_agent::services::BackupService> =
        Arc::new(personal_agent::services::BackupServiceImpl::new(
            db_for_backup,
            app_settings.clone(),
            db_path_for_backup,
        ));

    let profile_impl = ProfileServiceImpl::new(runtime_paths.profiles_dir.clone())
        .expect("Failed to create ProfileService");
    profile_impl
        .initialize()
        .await
        .expect("Failed to initialize ProfileService");
    let profile: Arc<dyn ProfileService> = Arc::new(profile_impl);
    let skills_impl =
        SkillsServiceImpl::new(app_settings.clone()).expect("Failed to create SkillsService");
    skills_impl
        .discover_skills()
        .await
        .expect("Failed to discover startup skills");
    let skills: Arc<dyn SkillsService> = Arc::new(skills_impl);
    let mcp: Arc<dyn McpService> = Arc::new(
        McpServiceImpl::new(runtime_paths.mcp_configs_dir.clone())
            .expect("Failed to create McpService"),
    );
    let models_registry: Arc<dyn ModelsRegistryService> =
        Arc::new(ModelsRegistryServiceImpl::new().expect("Failed to create ModelsRegistryService"));
    let mcp_registry: Arc<dyn McpRegistryService> =
        Arc::new(McpRegistryServiceImpl::new().expect("Failed to create McpRegistryService"));

    let approval_gate = Arc::new(ApprovalGate::new());

    let chat: Arc<dyn ChatService> = Arc::new(
        ChatServiceImpl::new_with_settings(
            conversation.clone(),
            profile.clone(),
            app_settings.clone(),
            skills.clone(),
            view_tx,
            approval_gate,
        )
        .await,
    );

    Services {
        _secrets: secrets,
        app_settings,
        conversation,
        profile,
        skills,
        mcp,
        models_registry,
        mcp_registry,
        chat,
        backup,
    }
}

/// Wires up broadcast channels for each presenter, bridges them into the
/// shared mpsc -> flume path, creates + starts all 8 presenters.
/// Returns the bridge join handles (must be kept alive) and the
/// settings broadcast sender for the MCP snapshot emission.
#[allow(clippy::type_complexity)]
async fn create_presenter_channels_and_bridges(
    event_bus: &Arc<EventBus>,
    services: &Services,
    view_tx: tokio::sync::mpsc::Sender<personal_agent::presentation::ViewCommand>,
) -> (
    Vec<tokio::task::JoinHandle<()>>,
    tokio::sync::broadcast::Sender<personal_agent::presentation::ViewCommand>,
) {
    let (settings_view_tx, _) =
        tokio::sync::broadcast::channel::<personal_agent::presentation::ViewCommand>(100);
    let (model_selector_view_tx, _) =
        tokio::sync::broadcast::channel::<personal_agent::presentation::ViewCommand>(100);
    let (profile_editor_view_tx, _) =
        tokio::sync::broadcast::channel::<personal_agent::presentation::ViewCommand>(100);
    let (mcp_add_view_tx, _) =
        tokio::sync::broadcast::channel::<personal_agent::presentation::ViewCommand>(100);
    let (mcp_configure_view_tx, _) =
        tokio::sync::broadcast::channel::<personal_agent::presentation::ViewCommand>(100);
    let (api_key_manager_view_tx, _) =
        tokio::sync::broadcast::channel::<personal_agent::presentation::ViewCommand>(100);

    let bridges = vec![
        spawn_broadcast_to_mpsc_view_command_bridge(
            settings_view_tx.subscribe(),
            view_tx.clone(),
            "SettingsPresenter",
        ),
        spawn_broadcast_to_mpsc_view_command_bridge(
            model_selector_view_tx.subscribe(),
            view_tx.clone(),
            "ModelSelectorPresenter",
        ),
        spawn_broadcast_to_mpsc_view_command_bridge(
            profile_editor_view_tx.subscribe(),
            view_tx.clone(),
            "ProfileEditorPresenter",
        ),
        spawn_broadcast_to_mpsc_view_command_bridge(
            mcp_add_view_tx.subscribe(),
            view_tx.clone(),
            "McpAddPresenter",
        ),
        spawn_broadcast_to_mpsc_view_command_bridge(
            mcp_configure_view_tx.subscribe(),
            view_tx.clone(),
            "McpConfigurePresenter",
        ),
        spawn_broadcast_to_mpsc_view_command_bridge(
            api_key_manager_view_tx.subscribe(),
            view_tx.clone(),
            "ApiKeyManagerPresenter",
        ),
    ];

    let settings_view_tx_for_snapshot = settings_view_tx.clone();

    // Create and start all 8 presenters
    start_all_presenters(
        event_bus,
        services,
        view_tx,
        settings_view_tx,
        model_selector_view_tx,
        profile_editor_view_tx,
        mcp_add_view_tx,
        mcp_configure_view_tx,
        api_key_manager_view_tx,
    )
    .await;

    (bridges, settings_view_tx_for_snapshot)
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::cognitive_complexity)]
async fn start_all_presenters(
    event_bus: &Arc<EventBus>,
    services: &Services,
    view_tx: tokio::sync::mpsc::Sender<personal_agent::presentation::ViewCommand>,
    settings_view_tx: tokio::sync::broadcast::Sender<personal_agent::presentation::ViewCommand>,
    model_selector_view_tx: tokio::sync::broadcast::Sender<
        personal_agent::presentation::ViewCommand,
    >,
    profile_editor_view_tx: tokio::sync::broadcast::Sender<
        personal_agent::presentation::ViewCommand,
    >,
    mcp_add_view_tx: tokio::sync::broadcast::Sender<personal_agent::presentation::ViewCommand>,
    mcp_configure_view_tx: tokio::sync::broadcast::Sender<
        personal_agent::presentation::ViewCommand,
    >,
    api_key_manager_view_tx: tokio::sync::broadcast::Sender<
        personal_agent::presentation::ViewCommand,
    >,
) {
    macro_rules! start_presenter {
        ($name:expr, $presenter:expr) => {
            info!(concat!("Starting ", $name, "..."));
            if let Err(e) = $presenter.start().await {
                tracing::error!(concat!("Failed to start ", $name, ": {:?}"), e);
            }
            info!(concat!("Started ", $name));
        };
    }

    let mut chat = ChatPresenter::new(
        Arc::clone(event_bus),
        services.conversation.clone(),
        services.chat.clone(),
        services.profile.clone(),
        services.app_settings.clone(),
        view_tx.clone(),
    );
    let mut history = HistoryPresenter::new(
        Arc::clone(event_bus),
        services.conversation.clone(),
        view_tx.clone(),
    );
    let mut settings = SettingsPresenter::new_with_event_bus(
        services.profile.clone(),
        services.app_settings.clone(),
        services.backup.clone(),
        services.skills.clone(),
        event_bus,
        settings_view_tx,
    );
    let mut model_selector = ModelSelectorPresenter::new_with_event_bus(
        services.models_registry.clone(),
        event_bus,
        model_selector_view_tx,
    );
    let mut profile_editor = ProfileEditorPresenter::new_with_event_bus(
        services.profile.clone(),
        event_bus,
        profile_editor_view_tx,
    );
    let mut mcp_add = McpAddPresenter::new_with_event_bus(
        services.mcp_registry.clone(),
        event_bus,
        mcp_add_view_tx,
    );
    let mut mcp_configure = McpConfigurePresenter::new_with_event_bus(
        services.mcp.clone(),
        event_bus,
        mcp_configure_view_tx,
    );
    let mut api_key_manager = ApiKeyManagerPresenter::new_with_event_bus(
        services.profile.clone(),
        event_bus,
        api_key_manager_view_tx,
    );

    let mut error = ErrorPresenter::new_with_event_bus(event_bus, view_tx);

    start_presenter!("ChatPresenter", chat);
    start_presenter!("HistoryPresenter", history);
    start_presenter!("SettingsPresenter", settings);
    start_presenter!("ModelSelectorPresenter", model_selector);
    start_presenter!("ProfileEditorPresenter", profile_editor);
    start_presenter!("McpAddPresenter", mcp_add);
    start_presenter!("McpConfigurePresenter", mcp_configure);
    start_presenter!("ApiKeyManagerPresenter", api_key_manager);
    start_presenter!("ErrorPresenter", error);
    info!("All 9 presenters started");
}
