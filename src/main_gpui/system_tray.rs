//! System Tray Manager
//!
//! Manages tray click detection and popup window lifecycle.

use std::sync::Arc;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::sync::Mutex;

use gpui::*;
use tracing::info;

use personal_agent::presentation::view_command::AppMode;
use personal_agent::ui_gpui::views::main_panel::{MainPanel, MainPanelAppState};

use super::AppState;

// ============================================================================
// macOS tray state
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
use objc2_foundation::{NSData, NSSize, NSString};

#[cfg(target_os = "macos")]
thread_local! {
    static STATUS_ITEM: std::cell::Cell<Option<Retained<NSStatusItem>>> = const { std::cell::Cell::new(None) };
}

// ============================================================================
// Linux tray adapter (SNI via ksni)
// ============================================================================

#[cfg(target_os = "linux")]
use ksni::blocking::{Handle as KsniHandle, TrayMethods};
#[cfg(target_os = "linux")]
use ksni::{Category as KsniCategory, Status as KsniStatus, Tray as KsniTray};
#[cfg(target_os = "linux")]
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy)]
enum LinuxTrayEvent {
    Activate { x: i32, y: i32 },
}

#[cfg(target_os = "linux")]
struct LinuxTray {
    click_tx: UnboundedSender<LinuxTrayEvent>,
}

#[cfg(target_os = "linux")]
impl KsniTray for LinuxTray {
    fn id(&self) -> String {
        "com.personalagent.gpui".to_string()
    }

    fn title(&self) -> String {
        "PersonalAgent".to_string()
    }

    fn category(&self) -> KsniCategory {
        KsniCategory::ApplicationStatus
    }

    fn status(&self) -> KsniStatus {
        KsniStatus::Active
    }

    fn icon_name(&self) -> String {
        // Standard icon fallback that usually resolves in Linux icon themes.
        "applications-system".to_string()
    }

    fn activate(&mut self, x: i32, y: i32) {
        let _ = self.click_tx.send(LinuxTrayEvent::Activate { x, y });
    }

    fn secondary_activate(&mut self, x: i32, y: i32) {
        let _ = self.click_tx.send(LinuxTrayEvent::Activate { x, y });
    }

    fn watcher_online(&self) {
        info!("Linux SNI watcher online");
    }

    fn watcher_offline(&self, reason: ksni::OfflineReason) -> bool {
        info!(?reason, "Linux SNI watcher offline");
        true
    }
}

// ============================================================================
// SystemTray struct
// ============================================================================

/// System tray manager - holds tray and popup state.
pub struct SystemTray {
    /// Current window handle (popup or popout).
    popup_window: Option<AnyWindowHandle>,
    /// Current application window mode.
    app_mode: AppMode,

    #[cfg(target_os = "linux")]
    click_events: Mutex<Option<UnboundedReceiver<LinuxTrayEvent>>>,

    #[cfg(target_os = "linux")]
    last_click_position: Arc<Mutex<Option<(f32, f32)>>>,

    #[cfg(target_os = "linux")]
    _tray_handle: Option<Arc<KsniHandle<LinuxTray>>>,

    #[cfg(target_os = "windows")]
    _windows_tray: Option<Arc<Mutex<WindowsTrayState>>>,
}

impl Global for SystemTray {}

// ============================================================================
// macOS constructor + click polling
// ============================================================================

#[cfg(target_os = "macos")]
impl SystemTray {
    /// Create a new system tray with menu bar icon.
    pub fn new(mtm: MainThreadMarker) -> Self {
        // Activation policy:
        // - Packaged builds get LSUIElement=true via Info.plist
        //   (scripts/release/package_macos_arm64.sh, Issue #177), which Launch
        //   Services applies before our process starts. We must NOT override
        //   that here: setting `Regular` would re-add the Dock icon and the
        //   Cmd-Tab tile, defeating the whole point of the agent app.
        // - Raw `cargo run` binaries have no Info.plist, so without any policy
        //   override they show up as a normal app. To match the packaged
        //   experience for local dev, fall back to `Accessory` (menu-bar
        //   only, no Dock, but windows can still take focus). Per-window
        //   `cx.activate(true)` calls in `open_popup` / `open_popout` handle
        //   the key-window / first-responder routing for accessory apps.
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        info!("Set activation policy to Accessory (LSUIElement-equivalent)");

        // Create status item.
        let status_bar = NSStatusBar::systemStatusBar();
        let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);

        // Set up icon.
        if let Some(button) = status_item.button(mtm) {
            let icon_data = include_bytes!("../../assets/MenuBarIcon.imageset/icon-32.png");
            let data = NSData::with_bytes(icon_data);
            use objc2::AllocAnyThread;
            if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
                image.setSize(NSSize::new(18.0, 18.0));
                button.setImage(Some(&image));
            } else {
                button.setTitle(&NSString::from_str("PA"));
            }
        }

        STATUS_ITEM.set(Some(status_item));
        info!("Status item created");

        Self::setup_local_event_monitor();

        Self {
            popup_window: None,
            app_mode: AppMode::Popup,
        }
    }

    /// Set up local event monitor - currently informational (polling is used).
    fn setup_local_event_monitor() {
        info!("Event monitoring via polling (local monitor not used)");
    }

    /// Start polling for tray icon clicks.
    #[allow(clippy::option_if_let_else)]
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn start_click_listener(&self, cx: &mut App) {
        cx.spawn(async move |cx| {
            let mut last_buttons: usize = 0;

            loop {
                smol::Timer::after(std::time::Duration::from_millis(50)).await;

                let current_buttons = NSEvent::pressedMouseButtons();
                let was_down = (last_buttons & 1) != 0;
                let is_down = (current_buttons & 1) != 0;
                last_buttons = current_buttons;

                if was_down && !is_down {
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
}

// ============================================================================
// Linux constructor + click listener
// ============================================================================

#[cfg(target_os = "linux")]
impl SystemTray {
    pub fn new() -> anyhow::Result<Self> {
        let (click_tx, click_rx) = unbounded_channel::<LinuxTrayEvent>();
        let tray = LinuxTray { click_tx };

        let tray_handle = tray
            .assume_sni_available(true)
            .spawn()
            .map(Arc::new)
            .map_err(|error| anyhow::anyhow!("Failed to start Linux SNI tray: {error:?}"))?;
        info!("Linux SNI tray started");

        Ok(Self {
            popup_window: None,
            app_mode: AppMode::Popup,
            click_events: Mutex::new(Some(click_rx)),
            last_click_position: Arc::new(Mutex::new(None)),
            _tray_handle: Some(tray_handle),
        })
    }

    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn start_click_listener(&self, cx: &mut App) {
        let mut click_rx = match self.click_events.lock() {
            Ok(mut guard) => guard.take(),
            Err(error) => {
                tracing::warn!(?error, "Failed to lock Linux tray click receiver");
                None
            }
        };

        let Some(mut click_rx) = click_rx.take() else {
            info!("Linux tray click listener already started or unavailable");
            return;
        };

        let last_click_position = Arc::clone(&self.last_click_position);
        cx.spawn(async move |cx| {
            while let Some(event) = click_rx.recv().await {
                match event {
                    LinuxTrayEvent::Activate { x, y } => {
                        if let Ok(mut lock) = last_click_position.lock() {
                            *lock = Some((x as f32, y as f32));
                        }

                        info!(x, y, "Linux tray click received");
                        let _ = cx.update_global::<Self, _>(|tray, cx| {
                            tray.toggle_popup(cx);
                        });
                    }
                }
            }

            info!("Linux tray click listener exited");
        })
        .detach();

        info!("Linux tray click listener started");
    }

    fn popup_display_context(&self, cx: &App) -> Option<(DisplayId, Bounds<Pixels>)> {
        let click_position = self.last_click_position.lock().ok().and_then(|lock| *lock);

        if let Some((x, y)) = click_position {
            for display in cx.displays() {
                let bounds = display.bounds();
                let origin_x = f32::from(bounds.origin.x);
                let origin_y = f32::from(bounds.origin.y);
                let width = f32::from(bounds.size.width);
                let height = f32::from(bounds.size.height);

                let in_x = x >= origin_x && x <= origin_x + width;
                let in_y = y >= origin_y && y <= origin_y + height;
                if in_x && in_y {
                    return Some((display.id(), bounds));
                }
            }
        }

        cx.primary_display()
            .map(|display| (display.id(), display.bounds()))
    }

    fn popup_display_id(&self, cx: &App) -> Option<DisplayId> {
        self.popup_display_context(cx).map(|(id, _)| id)
    }

    fn get_popup_position(&self, menu_width: f32, menu_height: f32, cx: &App) -> (f32, f32) {
        let Some((_display_id, bounds)) = self.popup_display_context(cx) else {
            info!("No Linux display detected; using fallback popup position");
            return (100.0, 30.0);
        };

        let screen_width = f32::from(bounds.size.width);
        let screen_height = f32::from(bounds.size.height);
        let origin_x = f32::from(bounds.origin.x);
        let origin_y = f32::from(bounds.origin.y);

        if std::env::var("PA_TEST_POPUP_ONSCREEN").ok().as_deref() == Some("1") {
            let x = (screen_width - menu_width - 24.0).max(0.0);
            return (x, 36.0);
        }

        let click_position = self.last_click_position.lock().ok().and_then(|lock| *lock);

        if let Some((click_x, click_y)) = click_position {
            let relative_x = click_x - origin_x;
            let relative_y = click_y - origin_y;

            let raw_x = relative_x - (menu_width / 2.0);
            let raw_y = relative_y + 12.0;

            let max_x = (screen_width - menu_width).max(0.0);
            let max_y = (screen_height - menu_height).max(0.0);

            let clamped_x = raw_x.clamp(0.0, max_x);
            let clamped_y = raw_y.clamp(0.0, max_y);

            info!(
                click_x,
                click_y,
                raw_x,
                raw_y,
                clamped_x,
                clamped_y,
                "Computed Linux popup position from tray click"
            );

            (clamped_x, clamped_y)
        } else {
            info!("No Linux tray click position available; using fallback popup position");
            let x = (screen_width - menu_width - 24.0).max(0.0);
            (x, 36.0)
        }
    }
}

// ============================================================================
// Windows tray adapter (using tray-icon and muda)
// ============================================================================

#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

#[cfg(target_os = "windows")]
use tray_icon::{
    menu::MenuEvent, Icon, TrayIcon, TrayIconBuilder, TrayIconEvent, TrayIconEventReceiver,
};

#[cfg(target_os = "windows")]
use muda::{Menu, MenuItem, PredefinedMenuItem};

#[cfg(target_os = "windows")]
static TRAY_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "windows")]
struct WindowsTrayState {
    _tray: TrayIcon,
    menu: Menu,
    last_click_position: Arc<Mutex<Option<(f32, f32)>>>,
}

#[cfg(target_os = "windows")]
impl SystemTray {
    pub fn new() -> Self {
        // Load the icon from embedded PNG data
        let icon_data = include_bytes!("../../assets/MenuBarIcon.imageset/icon-32.png");
        let icon = load_windows_icon(icon_data);

        // Create the context menu
        let menu = Menu::new();
        let open_popup_item = MenuItem::with_id("open_popup", "Open Popup", true, None);
        let open_popout_item = MenuItem::with_id("open_popout", "Open Pop-out", true, None);
        let settings_item = MenuItem::with_id("settings", "Settings", true, None);
        let quit_item = MenuItem::with_id("quit", "Quit", true, None);
        let separator = PredefinedMenuItem::separator();

        menu.append_items(&[
            &open_popup_item,
            &open_popout_item,
            &separator,
            &settings_item,
            &quit_item,
        ]);

        // Create tray icon with menu
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu.clone()))
            .with_tooltip("PersonalAgent")
            .with_icon(icon)
            .build()
            .expect("Failed to create Windows tray icon");

        TRAY_INITIALIZED.store(true, AtomicOrdering::SeqCst);
        info!("Windows tray icon created successfully");

        Self {
            popup_window: None,
            app_mode: AppMode::Popup,
            _windows_tray: Some(Arc::new(Mutex::new(WindowsTrayState {
                _tray: tray,
                menu,
                last_click_position: Arc::new(Mutex::new(None)),
            }))),
        }
    }

    pub fn start_click_listener(&self, cx: &mut App) {
        let Some(tray_state) = self._windows_tray.clone() else {
            info!("Windows tray state not available; skipping click listener");
            return;
        };

        cx.spawn(async move |cx| {
            let last_click_position = {
                let state = tray_state.lock().expect("tray state poisoned");
                state.last_click_position.clone()
            };

            // Get static event receiver
            let tray_rx = TrayIconEvent::receiver();
            let menu_rx = MenuEvent::receiver();

            // Poll tray icon events
            loop {
                smol::Timer::after(std::time::Duration::from_millis(50)).await;

                // Handle tray icon click events
                while let Ok(event) = tray_rx.try_recv() {
                    match event {
                        TrayIconEvent::Click {
                            button: tray_icon::MouseButton::Left,
                            position,
                            ..
                        } => {
                            if let Ok(mut lock) = last_click_position.lock() {
                                *lock = Some((position.x as f32, position.y as f32));
                            }
                            info!("Windows tray left-click detected");
                            let _ = cx.update_global::<Self, _>(|tray, cx| {
                                tray.toggle_popup(cx);
                            });
                        }
                        _ => {}
                    }
                }

                // Handle menu events
                while let Ok(event) = menu_rx.try_recv() {
                    let id_str = event.id.0.as_str();
                    match id_str {
                        "open_popup" => {
                            info!("Windows tray menu: Open Popup");
                            let _ = cx.update_global::<Self, _>(|tray, cx| {
                                if tray.popup_window.is_some() {
                                    tray.close_popup(cx);
                                }
                                tray.app_mode = AppMode::Popup;
                                tray.open_popup(cx);
                            });
                        }
                        "open_popout" => {
                            info!("Windows tray menu: Open Pop-out");
                            let _ = cx.update_global::<Self, _>(|tray, cx| {
                                if tray.popup_window.is_some() {
                                    tray.close_popup(cx);
                                }
                                tray.app_mode = AppMode::Popout;
                                tray.open_popout(cx);
                            });
                        }
                        "settings" => {
                            info!("Windows tray menu: Settings (not yet implemented)");
                            // TODO: Implement settings navigation
                        }
                        "quit" => {
                            info!("Windows tray menu: Quit");
                            let _ = cx.update(|cx| {
                                cx.quit();
                            });
                        }
                        _ => {}
                    }
                }
            }
        })
        .detach();

        info!("Windows tray click listener started");
    }

    fn popup_display_context(&self, cx: &App) -> Option<(DisplayId, Bounds<Pixels>)> {
        // Try to use the click position to determine display
        if let Some(ref tray_state) = self._windows_tray {
            let state = tray_state.lock().expect("tray state poisoned");
            let click_position = state.last_click_position.lock().ok().and_then(|lock| *lock);

            if let Some((x, y)) = click_position {
                for display in cx.displays() {
                    let bounds = display.bounds();
                    let origin_x = f32::from(bounds.origin.x);
                    let origin_y = f32::from(bounds.origin.y);
                    let width = f32::from(bounds.size.width);
                    let height = f32::from(bounds.size.height);

                    let in_x = x >= origin_x && x <= origin_x + width;
                    let in_y = y >= origin_y && y <= origin_y + height;
                    if in_x && in_y {
                        return Some((display.id(), bounds));
                    }
                }
            }
        }

        cx.primary_display()
            .map(|display| (display.id(), display.bounds()))
    }

    fn popup_display_id(&self, cx: &App) -> Option<DisplayId> {
        self.popup_display_context(cx).map(|(id, _)| id)
    }

    fn get_popup_position(&self, menu_width: f32, menu_height: f32, cx: &App) -> (f32, f32) {
        let Some((_display_id, bounds)) = self.popup_display_context(cx) else {
            info!("No Windows display detected; using fallback popup position");
            return (100.0, 30.0);
        };

        let screen_width = f32::from(bounds.size.width);
        let screen_height = f32::from(bounds.size.height);
        let origin_x = f32::from(bounds.origin.x);
        let origin_y = f32::from(bounds.origin.y);

        if std::env::var("PA_TEST_POPUP_ONSCREEN").ok().as_deref() == Some("1") {
            // Place near bottom-right for testing
            let x = (screen_width - menu_width - 24.0).max(0.0);
            let y = (screen_height - menu_height - 48.0).max(0.0);
            return (x, y);
        }

        // Get click position if available
        let click_position = self._windows_tray.as_ref().and_then(|ts| {
            ts.lock()
                .ok()
                .and_then(|state| state.last_click_position.lock().ok().and_then(|lock| *lock))
        });

        if let Some((click_x, click_y)) = click_position {
            let relative_x = click_x - origin_x;
            let relative_y = click_y - origin_y;

            // Position popup near the click, typically above the taskbar
            let raw_x = relative_x - (menu_width / 2.0);
            let raw_y = relative_y - menu_height - 12.0;

            let max_x = (screen_width - menu_width).max(0.0);
            let max_y = (screen_height - menu_height).max(0.0);

            let clamped_x = raw_x.clamp(0.0, max_x);
            let clamped_y = raw_y.clamp(0.0, max_y);

            info!(
                click_x,
                click_y,
                raw_x,
                raw_y,
                clamped_x,
                clamped_y,
                "Computed Windows popup position from tray click"
            );

            (clamped_x, clamped_y)
        } else {
            // Default to bottom-right area near system tray
            info!("No Windows tray click position available; using fallback near taskbar");
            let x = (screen_width - menu_width - 24.0).max(0.0);
            let y = (screen_height - menu_height - 48.0).max(0.0);
            (x, y)
        }
    }
}

/// Load a Windows icon from PNG data.
#[cfg(target_os = "windows")]
fn load_windows_icon(png_data: &[u8]) -> Icon {
    let img = image::load_from_memory(png_data)
        .expect("Failed to load tray icon image from embedded PNG");
    let rgba = img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let rgba_data = rgba.into_raw();

    Icon::from_rgba(rgba_data, width, height)
        .expect("Failed to create Windows tray icon from RGBA data")
}

// ============================================================================
// Shared popup behavior
// ============================================================================

impl SystemTray {
    /// Toggle the popup window (tray click).
    ///
    /// In popup mode: opens or closes the popup.
    /// In popout mode: foregrounds the existing window.

    pub fn toggle_popup(&mut self, cx: &mut App) {
        match self.app_mode {
            AppMode::Popup => {
                if self.popup_window.is_some() {
                    info!("Closing popup...");
                    self.close_popup(cx);
                } else {
                    info!("Opening popup...");
                    self.open_popup(cx);
                }
            }
            AppMode::Popout => {
                if let Some(handle) = self.popup_window {
                    info!("Foregrounding popout...");
                    cx.activate(true);
                    let _ = handle.update(cx, |_, window, _cx| {
                        window.activate_window();
                    });
                } else {
                    info!("Opening popout...");
                    self.open_popout(cx);
                }
            }
        }
    }

    /// Toggle between popup and popout window modes.
    pub fn toggle_window_mode(&mut self, cx: &mut App) {
        let new_mode = match self.app_mode {
            AppMode::Popup => AppMode::Popout,
            AppMode::Popout => AppMode::Popup,
        };
        info!(?new_mode, "Toggling window mode");
        self.app_mode = new_mode;
        self.close_popup(cx);

        // Update the global so views can query the current mode.
        if let Some(state) = cx.try_global::<MainPanelAppState>().cloned() {
            cx.set_global(MainPanelAppState {
                app_mode: new_mode,
                ..state
            });
        }

        match new_mode {
            AppMode::Popup => self.open_popup(cx),
            AppMode::Popout => self.open_popout(cx),
        }
    }

    /// Open the popup window.
    fn open_popup(&mut self, cx: &mut App) {
        self.close_popup(cx);

        // A tray click is an explicit user intent to interact with this app now.
        // Force activation so the popup is not created behind the foreground app.
        cx.activate(true);

        let menu_width = 780.0_f32;
        let menu_height = 600.0_f32;

        let (origin_x, origin_y) = self.get_popup_position(menu_width, menu_height, cx);

        #[cfg(any(target_os = "linux", target_os = "windows"))]
        let display_id = self.popup_display_id(cx);
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        let display_id = None;

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
            kind: WindowKind::Normal,
            focus: true,
            show: true,
            display_id,
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
                        app_mode: self.app_mode,
                    });
                }

                let _ = handle.update(cx, |main_panel, window, cx| {
                    window.activate_window();
                    if !main_panel.is_runtime_started() {
                        tracing::info!("MainPanel: starting runtime from open_popup");
                        main_panel.start_runtime(cx);
                    }
                });

                // Re-emit MCP snapshot so the new settings view is populated.
                // The one-shot startup emission was consumed by the previous
                // (now-dead) MainPanel; this replays it into the flume channel
                // that the new pump will drain.
                if let Some(app_state) = cx.try_global::<AppState>().cloned() {
                    super::emit_mcp_snapshot_to_flume(&app_state.view_cmd_tx);
                    super::emit_backup_snapshot_to_flume(&app_state.view_cmd_tx);
                }

                info!(x = origin_x, y = origin_y, "Popup opened");
            }
            Err(e) => {
                tracing::warn!(error = ?e, "Failed to open popup");
            }
        }
    }

    /// Open a popout window (free-floating, resizable, movable).
    #[allow(clippy::option_if_let_else)]
    fn open_popout(&mut self, cx: &mut App) {
        self.close_popup(cx);
        cx.activate(true);

        let popout_width = 900.0_f32;
        let popout_height = 580.0_f32;
        let bounds = Bounds::centered(None, size(px(popout_width), px(popout_height)), cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::Normal,
            focus: true,
            show: true,
            display_id: None,
            titlebar: Some(TitlebarOptions {
                title: Some(SharedString::from("PersonalAgent")),
                appears_transparent: true,
                traffic_light_position: Some(point(px(9.0), px(9.0))),
            }),
            window_background: WindowBackgroundAppearance::Opaque,
            app_id: Some("com.personalagent.gpui".to_string()),
            window_min_size: Some(size(px(480.0), px(340.0))),
            window_decorations: Some(WindowDecorations::Server),
            is_movable: true,
            is_resizable: true,
            is_minimizable: true,
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
                        app_mode: AppMode::Popout,
                    });
                }
                let _ = handle.update(cx, |main_panel, window, cx| {
                    window.on_window_should_close(cx, |_this, cx| {
                        cx.update_global::<Self, _>(|tray, cx| {
                            tray.app_mode = AppMode::Popup;
                            tray.close_popup(cx);
                        });
                        true
                    });
                    window.activate_window();
                    if !main_panel.is_runtime_started() {
                        tracing::info!("MainPanel: starting runtime from open_popout");
                        main_panel.start_runtime(cx);
                    }
                });
                if let Some(app_state) = cx.try_global::<AppState>().cloned() {
                    super::emit_mcp_snapshot_to_flume(&app_state.view_cmd_tx);
                }
                info!("Popout window opened");
            }
            Err(e) => {
                tracing::warn!(error = ?e, "Failed to open popout window");
            }
        }
    }

    /// Close the popup/popout window and clear the global handle.

    fn close_popup(&mut self, cx: &mut App) {
        if let Some(handle) = self.popup_window.take() {
            let _ = handle.update(cx, |_, window, _cx| {
                window.remove_window();
            });
        }
        // Clear the stale handle from the global state.
        if let Some(state) = cx.try_global::<MainPanelAppState>().cloned() {
            if state.popup_window.is_some() {
                cx.set_global(MainPanelAppState {
                    popup_window: None,
                    app_mode: AppMode::Popup,
                    ..state
                });
            }
        }
    }
}

// ============================================================================
// macOS popup positioning
// ============================================================================

#[cfg(target_os = "macos")]
impl SystemTray {
    /// Get position for popup window (below status item).
    #[allow(clippy::option_if_let_else)]
    fn get_popup_position(&self, menu_width: f32, menu_height: f32, _cx: &App) -> (f32, f32) {
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
