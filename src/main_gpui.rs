//! PersonalAgent GPUI
//!
//! A macOS menu bar app with chat interface using GPUI.
//! 
//! Uses NSEvent local monitor to capture button clicks within the app's run loop.

#![allow(unexpected_cfgs)]
#![allow(clippy::all)]
#![allow(clippy::pedantic)]
#![allow(unused_imports)]
#![allow(dead_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::*;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

// Use the library crate
use personal_agent::events::EventBus;
use personal_agent::ui_gpui::views::main_panel::MainPanel;
use personal_agent::ui_gpui::bridge::{GpuiBridge, spawn_user_event_forwarder};
use personal_agent::ui_gpui::theme::Theme;
use personal_agent::presentation::{
    ChatPresenter, HistoryPresenter, SettingsPresenter,
    ModelSelectorPresenter, ProfileEditorPresenter, 
    McpAddPresenter, McpConfigurePresenter,
};
use personal_agent::services::{
    ChatService, ChatServiceImpl, ConversationService, ConversationServiceImpl,
    ProfileService, ProfileServiceImpl, McpService, McpServiceImpl,
    AppSettingsService, AppSettingsServiceImpl, SecretsService, SecretsServiceImpl,
    ModelsRegistryService, ModelsRegistryServiceImpl,
    McpRegistryService, McpRegistryServiceImpl,
};

// ============================================================================
// System Tray using objc2 with NSEvent local monitor
// ============================================================================

#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::{MainThreadMarker};
#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy,
    NSStatusBar, NSStatusItem, NSVariableStatusItemLength, NSImage, NSEvent,
};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSData, NSSize, NSString};

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
    view_cmd_tx: flume::Sender<personal_agent::presentation::view_command::ViewCommand>,
}

impl Global for AppState {}

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
        Self {
            popup_window: None,
        }
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
        unsafe {
            app.activateIgnoringOtherApps(true);
        }
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

        Self {
            popup_window: None,
        }
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
                                    let frame = window.frame();
                                    let in_x = mouse_loc.x >= frame.origin.x 
                                        && mouse_loc.x <= frame.origin.x + frame.size.width;
                                    let in_y = mouse_loc.y >= frame.origin.y 
                                        && mouse_loc.y <= frame.origin.y + frame.size.height;
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
                        println!(">>> POLLING: Click on status item detected! <<<");
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

        let menu_width = 400.0_f32;
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
            kind: WindowKind::Normal,  // Use Normal instead of PopUp to allow interaction
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
        let status_item = STATUS_ITEM.take();
        let result = if let Some(ref item) = status_item {
            if let Some(mtm) = MainThreadMarker::new() {
                if let Some(button) = item.button(mtm) {
                    if let Some(window) = button.window() {
                        let frame = window.frame();
                        let x = frame.origin.x as f32 + (frame.size.width as f32 / 2.0) - (menu_width / 2.0);
                        let y = frame.origin.y as f32 - menu_height - 5.0;
                        (x, y)
                    } else {
                        (100.0, 30.0)
                    }
                } else {
                    (100.0, 30.0)
                }
            } else {
                (100.0, 30.0)
            }
        } else {
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
        let main_panel_state = MainPanelAppState {
            gpui_bridge,
        };
        cx.set_global(main_panel_state);

        // Initialize system tray
        let mut tray = SystemTray::new(mtm);
        tray.start_click_listener(cx);
        cx.set_global(tray);
        
        // Open a simple test window instead of using system tray popup
        // This ensures GPUI event handling works
        info!("Opening test window...");
        let test_window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point {
                    x: px(100.0),
                    y: px(100.0),
                },
                size: Size {
                    width: px(400.0),
                    height: px(600.0),
                },
            })),
            kind: WindowKind::Normal,
            focus: true,
            show: true,
            display_id: None,
            titlebar: None,
            window_background: WindowBackgroundAppearance::Opaque,
            app_id: Some("com.personalagent.gpui".to_string()),
            window_min_size: None,
            window_decorations: Some(WindowDecorations::Client),
            is_movable: true,
            is_resizable: true,
            is_minimizable: true,
            tabbing_identifier: None,
        };
        
        match cx.open_window(test_window_options, |_window, cx| {
            cx.new(|cx| MainPanel::new(cx))
        }) {
            Ok(_) => {
                info!("Test window opened successfully");
            }
            Err(e) => {
                tracing::error!("Failed to open test window: {:?}", e);
            }
        }

        // Spawn tokio runtime for services and presenters
        let event_bus_for_tokio = Arc::clone(&event_bus);
        let view_cmd_tx_for_tokio = cx.global::<AppState>().view_cmd_tx.clone();
        
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // Spawn user event forwarder (bridges GPUI events to EventBus)
                let _ = spawn_user_event_forwarder(Arc::clone(&event_bus_for_tokio), user_rx);
                
                // Get base directories
                let home = dirs::home_dir().expect("Could not find home directory");
                let base_dir = home.join(".llxprt");
                let config_path = base_dir.join("profiles");
                let secrets_path = base_dir.join("secrets");
                let conversations_dir = base_dir.join("conversations");
                
                // Create directories
                let _ = std::fs::create_dir_all(&config_path);
                let _ = std::fs::create_dir_all(&secrets_path);
                let _ = std::fs::create_dir_all(&conversations_dir);
                
                // Initialize services (following app.rs pattern)
                let secrets_service: Arc<dyn SecretsService> = Arc::new(
                    SecretsServiceImpl::new(secrets_path)
                        .expect("Failed to create SecretsService")
                );
                let app_settings: Arc<dyn AppSettingsService> = Arc::new(
                    AppSettingsServiceImpl::new(base_dir.join("app_settings.json"))
                        .expect("Failed to create AppSettingsService")
                );
                let conversation_service: Arc<dyn ConversationService> = Arc::new(
                    ConversationServiceImpl::new(conversations_dir)
                        .expect("Failed to create ConversationService")
                );
                let profile_service_impl = ProfileServiceImpl::new(config_path.clone())
                    .expect("Failed to create ProfileService");
                profile_service_impl.initialize().await.expect("Failed to initialize ProfileService");
                let profile_service: Arc<dyn ProfileService> = Arc::new(profile_service_impl);
                let mcp_service: Arc<dyn McpService> = Arc::new(
                    McpServiceImpl::new(base_dir.join("mcp_configs"))
                        .expect("Failed to create McpService")
                );
                let models_registry_service: Arc<dyn ModelsRegistryService> = Arc::new(
                    ModelsRegistryServiceImpl::new()
                        .expect("Failed to create ModelsRegistryService")
                );
                let mcp_registry_service: Arc<dyn McpRegistryService> = Arc::new(
                    McpRegistryServiceImpl::new()
                        .expect("Failed to create McpRegistryService")
                );
                let chat_service: Arc<dyn ChatService> = Arc::new(ChatServiceImpl::new(
                    conversation_service.clone(),
                    profile_service.clone(),
                ));
                
                // Create mpsc channel for ViewCommands (presenter -> view_cmd_tx -> flume)
                let (view_tx, mut view_rx) = tokio::sync::mpsc::channel(256);
                
                // Forward mpsc to flume
                let view_cmd_tx_clone = view_cmd_tx_for_tokio.clone();
                tokio::spawn(async move {
                    while let Some(cmd) = view_rx.recv().await {
                        let _ = view_cmd_tx_clone.send(cmd);
                    }
                });
                
                // Create broadcast channels for presenters that need them
                let (app_event_tx, _) = tokio::sync::broadcast::channel::<personal_agent::events::AppEvent>(100);
                let (settings_view_tx, _) = tokio::sync::broadcast::channel::<personal_agent::presentation::ViewCommand>(100);
                let (model_selector_view_tx, _) = tokio::sync::broadcast::channel::<personal_agent::presentation::ViewCommand>(100);
                let (profile_editor_view_tx, _) = tokio::sync::broadcast::channel::<personal_agent::presentation::ViewCommand>(100);
                let (mcp_add_view_tx, _) = tokio::sync::broadcast::channel::<personal_agent::presentation::ViewCommand>(100);
                let (mcp_configure_view_tx, _) = tokio::sync::broadcast::channel::<personal_agent::presentation::ViewCommand>(100);
                
                // Create and start presenters
                let mut chat_presenter = ChatPresenter::new(
                    Arc::clone(&event_bus_for_tokio),
                    conversation_service.clone(),
                    chat_service.clone(),
                    view_tx.clone(),
                );
                let mut history_presenter = HistoryPresenter::new(
                    Arc::clone(&event_bus_for_tokio),
                    conversation_service.clone(),
                    view_tx.clone(),
                );
                let mut settings_presenter = SettingsPresenter::new(
                    profile_service.clone(),
                    app_settings.clone(),
                    &app_event_tx,
                    settings_view_tx,
                );
                // For model_selector_presenter, we need to bridge its broadcast to the main view_tx
                // Create a receiver and forward to view_tx mpsc
                let model_selector_rx = model_selector_view_tx.subscribe();
                let view_tx_for_model_selector = view_tx.clone();
                tokio::spawn(async move {
                    let mut rx = model_selector_rx;
                    while let Ok(cmd) = rx.recv().await {
                        let _ = view_tx_for_model_selector.send(cmd).await;
                    }
                });
                
                let mut model_selector_presenter = ModelSelectorPresenter::new(
                    models_registry_service.clone(),
                    event_bus_for_tokio.sender(),  // Use main event bus sender
                    model_selector_view_tx,
                );
                let mut profile_editor_presenter = ProfileEditorPresenter::new(
                    profile_service.clone(),
                    &app_event_tx,
                    profile_editor_view_tx,
                );
                let mut mcp_add_presenter = McpAddPresenter::new(
                    mcp_registry_service.clone(),
                    &app_event_tx,
                    mcp_add_view_tx,
                );
                let mut mcp_configure_presenter = McpConfigurePresenter::new(
                    mcp_service.clone(),
                    &app_event_tx,
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
