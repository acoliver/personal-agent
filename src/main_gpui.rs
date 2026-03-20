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

use gpui::*;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

// Use the library crate
use personal_agent::events::types::UserEvent;
use personal_agent::events::EventBus;
use personal_agent::presentation::{
    ApiKeyManagerPresenter, ChatPresenter, HistoryPresenter, McpAddPresenter,
    McpConfigurePresenter, ModelSelectorPresenter, ProfileEditorPresenter, SettingsPresenter,
    ViewCommand,
};
use personal_agent::services::{
    AppSettingsService, AppSettingsServiceImpl, ChatService, ChatServiceImpl, ConversationService,
    ConversationServiceImpl, McpRegistryService, McpRegistryServiceImpl, McpService,
    McpServiceImpl, ModelsRegistryService, ModelsRegistryServiceImpl, ProfileService,
    ProfileServiceImpl, SecretsService, SecretsServiceImpl,
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

// ============================================================================
// System Tray using objc2 with NSEvent local monitor
// ============================================================================

#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::MainThreadMarker;
#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSEvent, NSImage, NSScreen, NSStatusBar,
    NSStatusItem, NSVariableStatusItemLength,
};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSData, NSRect, NSSize, NSString};

// ============================================================================
// Thread-local storage for status item
// ============================================================================

#[cfg(target_os = "macos")]
thread_local! {
    static STATUS_ITEM: std::cell::Cell<Option<Retained<NSStatusItem>>> = const { std::cell::Cell::new(None) };
}

// Global flag for click detection
static TRAY_CLICKED: AtomicBool = AtomicBool::new(false);

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

#[derive(Clone, Debug)]
struct RuntimePaths {
    base_dir: std::path::PathBuf,
    profiles_dir: std::path::PathBuf,
    secrets_dir: std::path::PathBuf,
    conversations_dir: std::path::PathBuf,
    mcp_configs_dir: std::path::PathBuf,
    app_settings_path: std::path::PathBuf,
}

fn resolve_runtime_paths() -> Result<RuntimePaths, String> {
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

/// @plan PLAN-20260304-GPUIREMEDIATE.P06
/// @requirement REQ-ARCH-002.1
/// @requirement REQ-ARCH-002.2
/// @requirement REQ-ARCH-002.5
/// @requirement REQ-ARCH-006.3
/// @pseudocode analysis/pseudocode/03-main-panel-integration.md:001-013
/// @plan PLAN-20260304-GPUIREMEDIATE.P08
/// @requirement REQ-ARCH-005.1
/// @pseudocode analysis/pseudocode/03-main-panel-integration.md:014-127
fn build_startup_inputs(runtime_paths: &RuntimePaths) -> Result<StartupInputs, String> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to create startup bootstrap runtime: {e}"))?;

    rt.block_on(async {
        let app_settings = AppSettingsServiceImpl::new(runtime_paths.app_settings_path.clone())
            .map_err(|e| format!("Failed to create AppSettingsService for startup bootstrap: {e}"))?;
        let conversation_service = ConversationServiceImpl::new(runtime_paths.conversations_dir.clone())
            .map_err(|e| format!("Failed to create ConversationService for startup bootstrap: {e}"))?;
        let profile_service_impl = ProfileServiceImpl::new(runtime_paths.profiles_dir.clone())
            .map_err(|e| format!("Failed to create ProfileService for startup bootstrap: {e}"))?;
        profile_service_impl
            .initialize()
            .await
            .map_err(|e| format!("Failed to initialize ProfileService for startup bootstrap: {e}"))?;

        let selected_profile_id = match app_settings.get_default_profile_id().await {
            Ok(Some(id)) => Some(id),
            _ => profile_service_impl.get_default().await.ok().flatten().map(|profile| profile.id),
        };

        let profiles = profile_service_impl
            .list()
            .await
            .map_err(|e| format!("Failed to list profiles for startup bootstrap: {e}"))?
            .into_iter()
            .map(|profile| personal_agent::presentation::view_command::ProfileSummary {
                id: profile.id,
                name: profile.name,
                provider_id: profile.provider_id,
                model_id: profile.model_id,
                is_default: Some(profile.id) == selected_profile_id,
            })
            .collect::<Vec<_>>();

        let conversations = conversation_service
            .list(None, None)
            .await
            .map_err(|e| format!("Failed to list conversations for startup bootstrap: {e}"))?;

        let conversation_summaries = conversations
            .iter()
            .map(|conversation| personal_agent::presentation::view_command::ConversationSummary {
                id: conversation.id,
                title: conversation
                    .title
                    .clone()
                    .filter(|title| !title.trim().is_empty())
                    .unwrap_or_else(|| "Untitled Conversation".to_string()),
                updated_at: conversation.updated_at,
                message_count: conversation.messages.len(),
            })
            .collect::<Vec<_>>();

        let selected_conversation = if let Some(conversation_id) = conversations.first().map(|conversation| conversation.id) {
            let transcript_result = conversation_service
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

                                Some(personal_agent::presentation::view_command::ConversationMessagePayload {
                                    role,
                                    content: message.content,
                                    thinking_content: message.thinking_content,
                                    timestamp: Some(message.timestamp.timestamp_millis() as u64),
                                })
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .unwrap_or_else(|e| {
                    StartupTranscriptResult::Failure(format!(
                        "Failed to load startup conversation messages for bootstrap: {e}"
                    ))
                });

            Some(StartupSelectedConversation {
                conversation_id,
                mode: StartupMode::ModeA { transcript_result },
            })
        } else {
            None
        };

        Ok(StartupInputs {
            profiles,
            selected_profile_id,
            conversations: conversation_summaries,
            selected_conversation,
        })
    })
}

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
fn publish_store_snapshot_to_main_panel(app_store: &GpuiAppStore, cx: &mut AsyncApp) {
    let snapshot = app_store.current_snapshot();

    let popup_window = cx
        .try_read_global::<MainPanelAppState, _>(|state, _| state.popup_window)
        .flatten();

    if let Some(window_handle) = popup_window {
        let _ = window_handle.update(cx, |main_panel: &mut MainPanel, _, cx| {
            main_panel.apply_store_snapshot(snapshot.clone(), cx);
            cx.notify();
        });
    }
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P05
/// @requirement REQ-ARCH-003.4
/// @requirement REQ-ARCH-004.1
/// @pseudocode analysis/pseudocode/03-main-panel-integration.md:014-036
fn forward_runtime_commands_to_main_panel(commands: Vec<ViewCommand>, cx: &mut AsyncApp) {
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
        let forwarded_commands = commands.clone();
        let changed = app_state.app_store.reduce_batch(commands);
        if changed {
            publish_store_snapshot_to_main_panel(&app_state.app_store, cx);
        }
        forward_runtime_commands_to_main_panel(forwarded_commands, cx);
    })
    .detach();
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

fn bootstrap_legacy_runtime_data(runtime_paths: &RuntimePaths) -> Result<(), String> {
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

// ============================================================================
// System Tray Manager
// ============================================================================

/// System tray manager - holds tray state
pub struct SystemTray {
    /// Current popup window handle
    popup_window: Option<AnyWindowHandle>,
}

impl Global for SystemTray {}

impl Default for SystemTray {
    fn default() -> Self {
        Self { popup_window: None }
    }
}

#[cfg(target_os = "macos")]
impl SystemTray {
    /// Create a new system tray with menu bar icon
    pub fn new(mtm: MainThreadMarker) -> Self {
        // Set activation policy to Regular (normal app with dock icon)
        // Accessory mode prevents proper event handling in some cases
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        info!("Set activation policy to Regular");

        // Activate the application to ensure it receives events
        app.activate();
        info!("Application activated");

        // Create status item
        let status_bar = NSStatusBar::systemStatusBar();
        let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);

        // Set up icon
        if let Some(button) = status_item.button(mtm) {
            let icon_data = include_bytes!("../assets/MenuBarIcon.imageset/icon-32.png");
            let data = NSData::with_bytes(icon_data);
            use objc2::AllocAnyThread;
            if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
                image.setSize(NSSize::new(18.0, 18.0));
                button.setImage(Some(&image));
            } else {
                button.setTitle(&NSString::from_str("PA"));
            }
        }

        // Store status item
        STATUS_ITEM.set(Some(status_item));
        info!("Status item created");

        // Set up local event monitor for left mouse up
        // Local monitors catch events that are already targeted at our app
        Self::setup_local_event_monitor();

        Self { popup_window: None }
    }

    /// Set up local event monitor - not used currently, relying on polling
    fn setup_local_event_monitor() {
        // Local monitors only work for events targeted at the app
        // For menu bar apps with Accessory policy, the status item button
        // doesn't route through the normal event loop
        // We rely on polling instead
        info!("Event monitoring via polling (local monitor not applicable for Accessory apps)");
    }

    /// Start polling for clicks on status item
    #[allow(clippy::option_if_let_else)]
    pub fn start_click_listener(&self, cx: &mut App) {
        cx.spawn(async move |cx| {
            let mut last_buttons: usize = 0;

            loop {
                smol::Timer::after(std::time::Duration::from_millis(50)).await;

                // Check mouse button state
                let current_buttons = NSEvent::pressedMouseButtons();
                let was_down = (last_buttons & 1) != 0;
                let is_down = (current_buttons & 1) != 0;
                last_buttons = current_buttons;

                // Detect mouse up (was pressed, now released)
                if was_down && !is_down {
                    // Check if mouse is over our status item
                    let mouse_loc = NSEvent::mouseLocation();

                    let status_item = STATUS_ITEM.take();
                    let is_our_click = if let Some(ref item) = status_item {
                        if let Some(mtm) = MainThreadMarker::new() {
                            if let Some(button) = item.button(mtm) {
                                if let Some(window) = button.window() {
                                    let button_bounds = button.bounds();
                                    let button_in_window =
                                        button.convertRect_toView(button_bounds, None);
                                    let button_on_screen =
                                        window.convertRectToScreen(button_in_window);

                                    let in_x = mouse_loc.x >= button_on_screen.origin.x
                                        && mouse_loc.x
                                            <= button_on_screen.origin.x
                                                + button_on_screen.size.width;
                                    let in_y = mouse_loc.y >= button_on_screen.origin.y
                                        && mouse_loc.y
                                            <= button_on_screen.origin.y
                                                + button_on_screen.size.height;
                                    in_x && in_y
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    STATUS_ITEM.set(status_item);

                    if is_our_click {
                        info!(
                            mouse_x = mouse_loc.x,
                            mouse_y = mouse_loc.y,
                            "Tray click detected on status item"
                        );
                        let _ = cx.update_global::<Self, _>(|tray, cx| {
                            tray.toggle_popup(cx);
                        });
                    }
                }
            }
        })
        .detach();

        info!("Click polling started");
    }

    /// Toggle the popup window
    pub fn toggle_popup(&mut self, cx: &mut App) {
        if self.popup_window.is_some() {
            info!("Closing popup...");
            self.close_popup(cx);
        } else {
            info!("Opening popup...");
            self.open_popup(cx);
        }
    }

    /// Open the popup window
    #[allow(clippy::option_if_let_else)]
    fn open_popup(&mut self, cx: &mut App) {
        self.close_popup(cx);

        let menu_width = 780.0_f32;
        let menu_height = 600.0_f32;

        let (origin_x, origin_y) = self.get_popup_position(menu_width, menu_height);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point {
                    x: px(origin_x),
                    y: px(origin_y),
                },
                size: Size {
                    width: px(menu_width),
                    height: px(menu_height),
                },
            })),
            kind: WindowKind::Normal, // Use Normal instead of PopUp to allow interaction
            focus: true,
            show: true,
            display_id: None,
            titlebar: None,
            window_background: WindowBackgroundAppearance::Opaque,
            app_id: Some("com.personalagent.gpui".to_string()),
            window_min_size: None,
            window_decorations: Some(WindowDecorations::Client),
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            tabbing_identifier: None,
        };

        match cx.open_window(window_options, |_window, cx| {
            cx.new(|cx| MainPanel::new(cx))
        }) {
            Ok(handle) => {
                let any_handle: AnyWindowHandle = handle.into();
                self.popup_window = Some(any_handle);
                if let Some(state) = cx.try_global::<MainPanelAppState>().cloned() {
                    cx.set_global(MainPanelAppState {
                        gpui_bridge: state.gpui_bridge,
                        popup_window: Some(handle),
                        app_store: state.app_store,
                    });
                }
                let _ = handle.update(cx, |main_panel, _window, cx| {
                    if !main_panel.is_runtime_started() {
                        tracing::info!("MainPanel: starting runtime from open_popup");
                        main_panel.start_runtime(cx);
                    }
                });
                info!(x = origin_x, y = origin_y, "Popup opened");
            }
            Err(e) => {
                tracing::warn!(error = ?e, "Failed to open popup");
            }
        }
    }

    /// Close the popup window
    fn close_popup(&mut self, cx: &mut App) {
        if let Some(handle) = self.popup_window.take() {
            let _ = handle.update(cx, |_, window, _cx| {
                window.remove_window();
            });
        }
    }

    /// Get position for popup window (below status item)
    #[allow(clippy::option_if_let_else)]
    fn get_popup_position(&self, menu_width: f32, menu_height: f32) -> (f32, f32) {
        if std::env::var("PA_TEST_POPUP_ONSCREEN").ok().as_deref() == Some("1") {
            // Keep automation popup visible near the top-right on the main screen.
            // This avoids tray-coordinate edge cases during test startup.
            if let Some(mtm) = MainThreadMarker::new() {
                if let Some(main_screen) = NSScreen::mainScreen(mtm) {
                    let frame = main_screen.frame();
                    let x = (frame.size.width as f32 - menu_width - 24.0).max(0.0);
                    return (x, 36.0);
                }
            }

            return (100.0, 30.0);
        }

        let status_item = STATUS_ITEM.take();
        let result = if let Some(ref item) = status_item {
            if let Some(mtm) = MainThreadMarker::new() {
                if let Some(button) = item.button(mtm) {
                    if let Some(window) = button.window() {
                        let button_bounds = button.bounds();
                        let button_in_window = button.convertRect_toView(button_bounds, None);
                        let button_on_screen = window.convertRectToScreen(button_in_window);

                        let icon_center_x =
                            button_on_screen.origin.x + (button_on_screen.size.width / 2.0);
                        let icon_bottom_y = button_on_screen.origin.y;

                        // GPUI expects window origins in display-relative top-left coordinates.
                        // AppKit screen coordinates are bottom-left based, so convert accordingly.
                        if let Some(screen) = window.screen() {
                            let screen_frame = screen.frame();

                            let popup_left = icon_center_x - (menu_width as f64 / 2.0);
                            let popup_bottom = icon_bottom_y - menu_height as f64 - 6.0;

                            let x = (popup_left - screen_frame.origin.x) as f32;
                            let y = (screen_frame.origin.y + screen_frame.size.height
                                - (popup_bottom + menu_height as f64))
                                as f32;

                            let max_x = (screen_frame.size.width as f32 - menu_width).max(0.0);
                            let max_y = (screen_frame.size.height as f32 - menu_height).max(0.0);
                            let clamped_x = x.clamp(0.0, max_x);
                            let clamped_y = y.clamp(0.0, max_y);

                            info!(
                                screen_x = screen_frame.origin.x,
                                screen_y = screen_frame.origin.y,
                                screen_w = screen_frame.size.width,
                                screen_h = screen_frame.size.height,
                                icon_x = button_on_screen.origin.x,
                                icon_y = button_on_screen.origin.y,
                                icon_w = button_on_screen.size.width,
                                icon_h = button_on_screen.size.height,
                                raw_x = x,
                                raw_y = y,
                                clamped_x,
                                clamped_y,
                                "Computed popup position from tray icon"
                            );

                            (clamped_x, clamped_y)
                        } else {
                            info!("No screen on status item window; using fallback popup position");
                            let x = icon_center_x as f32 - (menu_width / 2.0);
                            let y = icon_bottom_y as f32 - menu_height - 6.0;
                            (x, y)
                        }
                    } else {
                        info!("No window on status item button; using fallback popup position");
                        (100.0, 30.0)
                    }
                } else {
                    info!("No status item button; using fallback popup position");
                    (100.0, 30.0)
                }
            } else {
                info!("No main thread marker; using fallback popup position");
                (100.0, 30.0)
            }
        } else {
            info!("No status item available; using fallback popup position");
            (100.0, 30.0)
        };
        STATUS_ITEM.set(status_item);
        result
    }
}

#[cfg(not(target_os = "macos"))]
impl SystemTray {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_click_listener(&self, _cx: &mut App) {}
    pub fn toggle_popup(&mut self, _cx: &mut App) {}
}

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber).ok();

    info!("PersonalAgent GPUI starting...");

    // Run the GPUI application
    Application::new().run(|cx: &mut App| {
        // Tray apps must not quit when popup closes
        cx.set_quit_mode(QuitMode::Explicit);

        // Get main thread marker (required for AppKit operations)
        let Some(mtm) = MainThreadMarker::new() else {
            tracing::error!("Not on main thread!");
            return;
        };

        // Create event bus and bridge channels
        // Use the global event bus so services and presenters share the same bus
        // Services use events::emit() which publishes to the global bus
        // Presenters subscribe to the same global bus
        let event_bus = Arc::new(personal_agent::events::global::get_event_bus_clone());
        let (user_tx, user_rx) = flume::bounded(256);
        let (view_cmd_tx, view_cmd_rx) = flume::bounded(1024);

        // Create GPUI bridge
        let gpui_bridge = Arc::new(GpuiBridge::new(user_tx, view_cmd_rx));

        let runtime_paths = resolve_runtime_paths()
            .expect("Could not resolve runtime paths from platform config/data directories");
        if let Err(e) = bootstrap_legacy_runtime_data(&runtime_paths) {
            tracing::warn!("Legacy bootstrap copy failed: {}", e);
        }
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

        // Initialize global state (full version)
        let app_state = AppState {
            event_bus: Arc::clone(&event_bus),
            gpui_bridge: Arc::clone(&gpui_bridge),
            view_cmd_tx,
            app_store: Arc::clone(&app_store),
        };
        cx.set_global(app_state.clone());

        // Also set the simplified AppState for MainPanel's view initialization
        let main_panel_state = MainPanelAppState {
            gpui_bridge,
            popup_window: None,
            app_store,
        };

        cx.set_global(main_panel_state);

        // Register global keyboard shortcuts as GPUI action keybindings.
        // Only modifier-based shortcuts belong here; bare keys like escape/m/+
        // stay as on_key_down in their respective views to avoid conflicts with typing.
        use personal_agent::ui_gpui::views::main_panel::{
            NavigateBack, NavigateToHistory, NavigateToSettings, NewConversation,
        };
        cx.bind_keys([
            KeyBinding::new("ctrl-h", NavigateToHistory, None),
            KeyBinding::new("ctrl-s", NavigateToSettings, None),
            KeyBinding::new("ctrl-n", NewConversation, None),
            KeyBinding::new("cmd-w", NavigateBack, None),
        ]);

        spawn_runtime_bridge_pump(app_state, cx);

        // Initialize system tray
        let mut tray = SystemTray::new(mtm);
        tray.start_click_listener(cx);

        let auto_open = std::env::var("PA_AUTO_OPEN_POPUP").ok().as_deref() == Some("1");
        let test_popup_onscreen =
            std::env::var("PA_TEST_POPUP_ONSCREEN").ok().as_deref() == Some("1");

        if auto_open {
            tray.toggle_popup(cx);
            info!("GPUI initialized in tray mode; popup auto-opened for automation");
        } else {
            info!("GPUI initialized in tray mode; click the status icon to open popup");
        }

        if test_popup_onscreen {
            info!("PA_TEST_POPUP_ONSCREEN=1 active (automation popup positioning override)");
        }

        cx.set_global(tray);

        // Spawn tokio runtime for services and presenters
        let event_bus_for_tokio = Arc::clone(&event_bus);
        let view_cmd_tx_for_tokio = cx.global::<AppState>().view_cmd_tx.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // Spawn user event forwarder (bridges GPUI events to EventBus)
                let _ = spawn_user_event_forwarder(Arc::clone(&event_bus_for_tokio), user_rx);

                // Resolve runtime directories using OS-standard app data/config locations.
                // We intentionally no longer use ~/.llxprt for runtime service storage.
                let runtime_paths = resolve_runtime_paths().expect(
                    "Could not resolve runtime paths from platform config/data directories",
                );

                tracing::info!(
                    data_dir = %runtime_paths.base_dir.display(),
                    config_dir = %runtime_paths.profiles_dir.parent().unwrap_or(&runtime_paths.profiles_dir).display(),
                    profiles_dir = %runtime_paths.profiles_dir.display(),
                    conversations_dir = %runtime_paths.conversations_dir.display(),
                    mcp_configs_dir = %runtime_paths.mcp_configs_dir.display(),
                    "Using platform-standard runtime directories"
                );

                // Create directories
                let _ = std::fs::create_dir_all(&runtime_paths.profiles_dir);
                let _ = std::fs::create_dir_all(&runtime_paths.secrets_dir);
                let _ = std::fs::create_dir_all(&runtime_paths.conversations_dir);
                let _ = std::fs::create_dir_all(&runtime_paths.mcp_configs_dir);

                if let Err(e) = bootstrap_legacy_runtime_data(&runtime_paths) {
                    tracing::warn!("Legacy bootstrap copy failed: {}", e);
                }

                // Initialize services (following app.rs pattern)
                let _secrets_service: Arc<dyn SecretsService> = Arc::new(
                    SecretsServiceImpl::new(runtime_paths.secrets_dir.clone())
                        .expect("Failed to create SecretsService"),
                );
                let app_settings: Arc<dyn AppSettingsService> = Arc::new(
                    AppSettingsServiceImpl::new(runtime_paths.app_settings_path.clone())
                        .expect("Failed to create AppSettingsService"),
                );
                let conversation_service: Arc<dyn ConversationService> = Arc::new(
                    ConversationServiceImpl::new(runtime_paths.conversations_dir.clone())
                        .expect("Failed to create ConversationService"),
                );
                let profile_service_impl = ProfileServiceImpl::new(runtime_paths.profiles_dir.clone())
                    .expect("Failed to create ProfileService");
                profile_service_impl
                    .initialize()
                    .await
                    .expect("Failed to initialize ProfileService");
                let profile_service: Arc<dyn ProfileService> = Arc::new(profile_service_impl);
                let mcp_service: Arc<dyn McpService> = Arc::new(
                    McpServiceImpl::new(runtime_paths.mcp_configs_dir.clone())
                        .expect("Failed to create McpService"),
                );
                let models_registry_service: Arc<dyn ModelsRegistryService> = Arc::new(
                    ModelsRegistryServiceImpl::new()
                        .expect("Failed to create ModelsRegistryService"),
                );
                let mcp_registry_service: Arc<dyn McpRegistryService> = Arc::new(
                    McpRegistryServiceImpl::new().expect("Failed to create McpRegistryService"),
                );
                let chat_service: Arc<dyn ChatService> = Arc::new(ChatServiceImpl::new(
                    conversation_service.clone(),
                    profile_service.clone(),
                ));

                // Create mpsc channel for ViewCommands (presenter -> view_cmd_tx -> flume)
                let (view_tx, view_rx) = tokio::sync::mpsc::channel(256);

                // Forward mpsc to flume
                let _main_view_cmd_bridge =
                    spawn_mpsc_to_flume_view_command_bridge(view_rx, view_cmd_tx_for_tokio.clone());

                // Create broadcast channels for presenters that emit ViewCommands.
                // Bridge all of them into the shared mpsc -> flume path so MainPanel
                // receives a single unified command stream.
                let (settings_view_tx, _) = tokio::sync::broadcast::channel::<
                    personal_agent::presentation::ViewCommand,
                >(100);
                let (model_selector_view_tx, _) = tokio::sync::broadcast::channel::<
                    personal_agent::presentation::ViewCommand,
                >(100);
                let (profile_editor_view_tx, _) = tokio::sync::broadcast::channel::<
                    personal_agent::presentation::ViewCommand,
                >(100);
                let (mcp_add_view_tx, _) = tokio::sync::broadcast::channel::<
                    personal_agent::presentation::ViewCommand,
                >(100);
                let (mcp_configure_view_tx, _) = tokio::sync::broadcast::channel::<
                    personal_agent::presentation::ViewCommand,
                >(100);
                let (api_key_manager_view_tx, _) = tokio::sync::broadcast::channel::<
                    personal_agent::presentation::ViewCommand,
                >(100);

                let _settings_bridge = spawn_broadcast_to_mpsc_view_command_bridge(
                    settings_view_tx.subscribe(),
                    view_tx.clone(),
                    "SettingsPresenter",
                );
                let _model_selector_bridge = spawn_broadcast_to_mpsc_view_command_bridge(
                    model_selector_view_tx.subscribe(),
                    view_tx.clone(),
                    "ModelSelectorPresenter",
                );
                let _profile_editor_bridge = spawn_broadcast_to_mpsc_view_command_bridge(
                    profile_editor_view_tx.subscribe(),
                    view_tx.clone(),
                    "ProfileEditorPresenter",
                );
                let _mcp_add_bridge = spawn_broadcast_to_mpsc_view_command_bridge(
                    mcp_add_view_tx.subscribe(),
                    view_tx.clone(),
                    "McpAddPresenter",
                );
                let _mcp_configure_bridge = spawn_broadcast_to_mpsc_view_command_bridge(
                    mcp_configure_view_tx.subscribe(),
                    view_tx.clone(),
                    "McpConfigurePresenter",
                );
                let _api_key_manager_bridge = spawn_broadcast_to_mpsc_view_command_bridge(
                    api_key_manager_view_tx.subscribe(),
                    view_tx.clone(),
                    "ApiKeyManagerPresenter",
                );

                // Create and start presenters
                let mut chat_presenter = ChatPresenter::new(
                    Arc::clone(&event_bus_for_tokio),
                    conversation_service.clone(),
                    chat_service.clone(),
                    profile_service.clone(),
                    view_tx.clone(),
                );
                let mut history_presenter = HistoryPresenter::new(
                    Arc::clone(&event_bus_for_tokio),
                    conversation_service.clone(),
                    view_tx.clone(),
                );
                let settings_view_tx_for_snapshot = settings_view_tx.clone();
                let mut settings_presenter = SettingsPresenter::new_with_event_bus(
                    profile_service.clone(),
                    app_settings.clone(),
                    &event_bus_for_tokio,
                    settings_view_tx,
                );

                let mut model_selector_presenter = ModelSelectorPresenter::new_with_event_bus(
                    models_registry_service.clone(),
                    &event_bus_for_tokio,
                    model_selector_view_tx,
                );
                let mut profile_editor_presenter = ProfileEditorPresenter::new_with_event_bus(
                    profile_service.clone(),
                    &event_bus_for_tokio,
                    profile_editor_view_tx,
                );
                let mut mcp_add_presenter = McpAddPresenter::new_with_event_bus(
                    mcp_registry_service.clone(),
                    &event_bus_for_tokio,
                    mcp_add_view_tx,
                );
                let mut mcp_configure_presenter = McpConfigurePresenter::new_with_event_bus(
                    mcp_service.clone(),
                    &event_bus_for_tokio,
                    mcp_configure_view_tx,
                );
                let mut api_key_manager_presenter = ApiKeyManagerPresenter::new_with_event_bus(
                    profile_service.clone(),
                    &event_bus_for_tokio,
                    api_key_manager_view_tx,
                );

                info!("Starting presenters...");
                info!("Starting ChatPresenter...");
                if let Err(e) = chat_presenter.start().await {
                    tracing::error!("Failed to start ChatPresenter: {:?}", e);
                }
                info!("Started ChatPresenter");
                info!("Starting HistoryPresenter...");
                if let Err(e) = history_presenter.start().await {
                    tracing::error!("Failed to start HistoryPresenter: {:?}", e);
                }
                info!("Started HistoryPresenter");
                info!("Starting SettingsPresenter...");
                if let Err(e) = settings_presenter.start().await {
                    tracing::error!("Failed to start SettingsPresenter: {:?}", e);
                }
                info!("Started SettingsPresenter");
                info!("Starting ModelSelectorPresenter...");
                if let Err(e) = model_selector_presenter.start().await {
                    tracing::error!("Failed to start ModelSelectorPresenter: {:?}", e);
                }
                info!("Started ModelSelectorPresenter");
                info!("Starting ProfileEditorPresenter...");
                if let Err(e) = profile_editor_presenter.start().await {
                    tracing::error!("Failed to start ProfileEditorPresenter: {:?}", e);
                }
                info!("Started ProfileEditorPresenter");
                info!("Starting McpAddPresenter...");
                if let Err(e) = mcp_add_presenter.start().await {
                    tracing::error!("Failed to start McpAddPresenter: {:?}", e);
                }
                info!("Started McpAddPresenter");
                info!("Starting McpConfigurePresenter...");
                if let Err(e) = mcp_configure_presenter.start().await {
                    tracing::error!("Failed to start McpConfigurePresenter: {:?}", e);
                }
                info!("Started McpConfigurePresenter");
                info!("Starting ApiKeyManagerPresenter...");
                if let Err(e) = api_key_manager_presenter.start().await {
                    tracing::error!("Failed to start ApiKeyManagerPresenter: {:?}", e);
                }
                info!("Started ApiKeyManagerPresenter");
                info!("All 8 presenters started");

                // Initialize global MCP runtime so chat can discover tools
                info!("Initializing global MCP runtime...");
                {
                    let global = personal_agent::mcp::McpService::global();
                    let mut svc = global.lock().await;
                    if let Err(e) = svc.initialize().await {
                        tracing::error!("Global MCP initialization failed: {e}");
                    } else {
                        info!("Global MCP runtime initialized");
                    }
                }

                // Emit MCP snapshot so settings view shows all configured MCPs
                personal_agent::presentation::SettingsPresenter::emit_mcp_snapshot(
                    &settings_view_tx_for_snapshot,
                );

                // Keep runtime alive
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
                }
            });
        });

        info!("PersonalAgent GPUI initialized - click tray icon to open");
    });
}
