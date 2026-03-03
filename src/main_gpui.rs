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

use gpui::*;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

// Use the library crate
use personal_agent::events::EventBus;
use personal_agent::presentation::{
    ChatPresenter, HistoryPresenter, McpAddPresenter, McpConfigurePresenter,
    ModelSelectorPresenter, ProfileEditorPresenter, SettingsPresenter,
};
use personal_agent::services::{
    AppSettingsService, AppSettingsServiceImpl, ChatService, ChatServiceImpl, ConversationService,
    ConversationServiceImpl, McpRegistryService, McpRegistryServiceImpl, McpService,
    McpServiceImpl, ModelsRegistryService, ModelsRegistryServiceImpl, ProfileService,
    ProfileServiceImpl, SecretsService, SecretsServiceImpl,
};
use personal_agent::ui_gpui::bridge::{spawn_user_event_forwarder, GpuiBridge};
use personal_agent::ui_gpui::theme::Theme;
use personal_agent::ui_gpui::views::main_panel::MainPanel;

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

/// Global application state (full version with all fields)
pub struct AppState {
    /// Event bus for the application
    event_bus: Arc<EventBus>,
    /// GPUI bridge for UI events
    gpui_bridge: Arc<GpuiBridge>,
    /// View command sender (to send commands to UI)
    view_cmd_tx: flume::Sender<personal_agent::presentation::ViewCommand>,
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
        tracing::info!("{} bridge: task started, waiting for commands", presenter_name);
        loop {
            match rx.recv().await {
                Ok(cmd) => {
                    tracing::info!("{} bridge: forwarding command {:?}", presenter_name, std::mem::discriminant(&cmd));
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


fn copy_json_files_if_target_empty(source_dir: &std::path::Path, target_dir: &std::path::Path) -> Result<(), String> {
    if !source_dir.exists() {
        return Ok(());
    }

    let source_entries = std::fs::read_dir(source_dir)
        .map_err(|e| format!("Failed reading source directory {}: {}", source_dir.display(), e))?;

    let source_json_files = source_entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect::<Vec<_>>();

    if source_json_files.is_empty() {
        return Ok(());
    }

    std::fs::create_dir_all(target_dir)
        .map_err(|e| format!("Failed creating target directory {}: {}", target_dir.display(), e))?;

    let target_has_json = std::fs::read_dir(target_dir)
        .map_err(|e| format!("Failed reading target directory {}: {}", target_dir.display(), e))?
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
    let home = dirs::home_dir().ok_or_else(|| "Could not determine home directory for bootstrap".to_string())?;
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


// Also set the simplified AppState that MainPanel expects
use personal_agent::ui_gpui::views::main_panel::MainPanelAppState;

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
                        let _ = cx.update_global::<SystemTray, _>(|tray, cx| {
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
    fn open_popup(&mut self, cx: &mut App) {
        self.close_popup(cx);

        let menu_width = 500.0_f32;
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
                self.popup_window = Some(handle.into());
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

        // Initialize global state (full version)
        let app_state = AppState {
            event_bus: Arc::clone(&event_bus),
            gpui_bridge: Arc::clone(&gpui_bridge),
            view_cmd_tx,
        };
        cx.set_global(app_state);

        // Also set the simplified AppState for MainPanel's view initialization
        let main_panel_state = MainPanelAppState { gpui_bridge };
        cx.set_global(main_panel_state);

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
                let mut settings_presenter = SettingsPresenter::new_with_event_bus(
                    profile_service.clone(),
                    app_settings.clone(),
                    Arc::clone(&event_bus_for_tokio),
                    settings_view_tx,
                );

                let mut model_selector_presenter = ModelSelectorPresenter::new_with_event_bus(
                    models_registry_service.clone(),
                    Arc::clone(&event_bus_for_tokio),
                    model_selector_view_tx,
                );
                let mut profile_editor_presenter = ProfileEditorPresenter::new_with_event_bus(
                    profile_service.clone(),
                    Arc::clone(&event_bus_for_tokio),
                    profile_editor_view_tx,
                );
                let mut mcp_add_presenter = McpAddPresenter::new_with_event_bus(
                    mcp_registry_service.clone(),
                    Arc::clone(&event_bus_for_tokio),
                    mcp_add_view_tx,
                );
                let mut mcp_configure_presenter = McpConfigurePresenter::new_with_event_bus(
                    mcp_service.clone(),
                    Arc::clone(&event_bus_for_tokio),
                    mcp_configure_view_tx,
                );

                info!("Starting presenters...");
                if let Err(e) = chat_presenter.start().await {
                    tracing::error!("Failed to start ChatPresenter: {:?}", e);
                }
                if let Err(e) = history_presenter.start().await {
                    tracing::error!("Failed to start HistoryPresenter: {:?}", e);
                }
                if let Err(e) = settings_presenter.start().await {
                    tracing::error!("Failed to start SettingsPresenter: {:?}", e);
                }
                if let Err(e) = model_selector_presenter.start().await {
                    tracing::error!("Failed to start ModelSelectorPresenter: {:?}", e);
                }
                if let Err(e) = profile_editor_presenter.start().await {
                    tracing::error!("Failed to start ProfileEditorPresenter: {:?}", e);
                }
                if let Err(e) = mcp_add_presenter.start().await {
                    tracing::error!("Failed to start McpAddPresenter: {:?}", e);
                }
                if let Err(e) = mcp_configure_presenter.start().await {
                    tracing::error!("Failed to start McpConfigurePresenter: {:?}", e);
                }
                info!("All 7 presenters started");

                // Keep runtime alive
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
                }
            });
        });

        info!("PersonalAgent GPUI initialized - click tray icon to open");
    });
}
