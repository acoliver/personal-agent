//! Main panel with navigation-based view routing
//!
//! @plan PLAN-20250130-GPUIREDUX.P11
//! @requirement REQ-GPUI-003

use std::sync::Arc;
use std::time::Duration;
use gpui::{div, px, prelude::*, Entity, Global, FocusHandle, MouseButton};
use crate::presentation::view_command::{ViewCommand, ViewId};
use crate::ui_gpui::navigation::NavigationState;
use crate::ui_gpui::views::chat_view::{ChatView, ChatState};
use crate::ui_gpui::views::settings_view::SettingsView;
use crate::ui_gpui::views::history_view::HistoryView;
use crate::ui_gpui::views::model_selector_view::ModelSelectorView;
use crate::ui_gpui::views::profile_editor_view::ProfileEditorView;
use crate::ui_gpui::views::mcp_add_view::McpAddView;
use crate::ui_gpui::views::mcp_configure_view::McpConfigureView;
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::theme::Theme;

/// Global app state containing the bridge - used by MainPanel to receive ViewCommands
/// @plan PLAN-20250130-GPUIREDUX.P11
#[derive(Clone)]
pub struct MainPanelAppState {
    pub gpui_bridge: Arc<GpuiBridge>,
}

impl Global for MainPanelAppState {}

/// Main panel component with navigation-based view routing
/// @plan PLAN-20250130-GPUIREDUX.P11
pub struct MainPanel {
    navigation: NavigationState,
    pub focus_handle: FocusHandle,
    chat_view: Option<Entity<ChatView>>,
    history_view: Option<Entity<HistoryView>>,
    settings_view: Option<Entity<SettingsView>>,
    model_selector_view: Option<Entity<ModelSelectorView>>,
    profile_editor_view: Option<Entity<ProfileEditorView>>,
    mcp_add_view: Option<Entity<McpAddView>>,
    mcp_configure_view: Option<Entity<McpConfigureView>>,
}

impl MainPanel {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            navigation: NavigationState::new(),
            focus_handle: cx.focus_handle(),
            chat_view: None,
            history_view: None,
            settings_view: None,
            model_selector_view: None,
            profile_editor_view: None,
            mcp_add_view: None,
            mcp_configure_view: None,
        }
    }

    /// Initialize all child views with bridge
    /// @plan PLAN-20250130-GPUIREDUX.P11
    pub fn init(&mut self, cx: &mut gpui::Context<Self>) {
        // Get the bridge from global state
        let bridge = cx.try_global::<MainPanelAppState>().map(|s| s.gpui_bridge.clone());
        tracing::info!("MainPanel::init - bridge is_some: {}", bridge.is_some());
        
        // Set up navigation channel notify callback to trigger MainPanel redraw
        let entity_id = cx.entity_id();
        // We can't directly call cx.notify() from outside, so we use a shared flag
        // that render() will check
        println!(">>> MainPanel::init - setting up navigation notify callback <<<");
        

        
        // Chat view
        let chat_state = ChatState::default();
        self.chat_view = Some(cx.new(|cx: &mut gpui::Context<ChatView>| {
            let mut view = ChatView::new(chat_state, cx);
            if let Some(ref b) = bridge {
                view.set_bridge(b.clone());
            }
            view
        }));

        // History view
        self.history_view = Some(cx.new(|cx: &mut gpui::Context<HistoryView>| {
            let mut view = HistoryView::new(cx);
            if let Some(ref b) = bridge {
                view.set_bridge(b.clone());
            }
            view
        }));

        // Settings view
        self.settings_view = Some(cx.new(|cx: &mut gpui::Context<SettingsView>| {
            let mut view = SettingsView::new(cx);
            if let Some(ref b) = bridge {
                view.set_bridge(b.clone());
            }
            view
        }));

        // Model Selector view
        self.model_selector_view = Some(cx.new(|cx: &mut gpui::Context<ModelSelectorView>| {
            let mut view = ModelSelectorView::new(cx);
            if let Some(ref b) = bridge {
                view.set_bridge(b.clone());
            }
            view
        }));

        // Profile Editor view
        self.profile_editor_view = Some(cx.new(|cx: &mut gpui::Context<ProfileEditorView>| {
            let mut view = ProfileEditorView::new(cx);
            if let Some(ref b) = bridge {
                view.set_bridge(b.clone());
            }
            view
        }));

        // MCP Add view
        self.mcp_add_view = Some(cx.new(|cx: &mut gpui::Context<McpAddView>| {
            let mut view = McpAddView::new(cx);
            if let Some(ref b) = bridge {
                view.set_bridge(b.clone());
            }
            view
        }));

        // MCP Configure view
        self.mcp_configure_view = Some(cx.new(|cx: &mut gpui::Context<McpConfigureView>| {
            let mut view = McpConfigureView::new(cx);
            if let Some(ref b) = bridge {
                view.set_bridge(b.clone());
            }
            view
        }));
        
        // Start a background thread to poll for navigation requests and trigger redraws
        let entity = cx.entity().downgrade();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(100));
                if crate::ui_gpui::navigation_channel().has_pending() {
                    println!(">>> Navigation poll detected pending request <<<");
                    // We can't directly notify, but setting the flag is enough
                    // render() will pick it up on next frame
                }
            }
        });
        let _ = entity; // suppress warning
    }

    /// Check if all views are initialized
    fn is_initialized(&self) -> bool {
        self.chat_view.is_some()
            && self.history_view.is_some()
            && self.settings_view.is_some()
            && self.model_selector_view.is_some()
            && self.profile_editor_view.is_some()
            && self.mcp_add_view.is_some()
            && self.mcp_configure_view.is_some()
    }

    /// Get the current view ID
    pub fn current_view(&self) -> ViewId {
        self.navigation.current()
    }

    /// Handle ViewCommand from the presentation layer
    /// @plan PLAN-20250130-GPUIREDUX.P11
    pub fn handle_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::NavigateTo { view } => {
                tracing::info!("MainPanel: NavigateTo {:?}", view);
                self.navigation.navigate(view);
                cx.notify();
            }
            ViewCommand::NavigateBack => {
                tracing::info!("MainPanel: NavigateBack");
                self.navigation.navigate_back();
                cx.notify();
            }
            // Forward other commands to child views as needed
            _ => {
                tracing::debug!("MainPanel: Unhandled command {:?}", cmd);
            }
        }
    }
}

impl gpui::Focusable for MainPanel {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}




impl gpui::Render for MainPanel {
    fn render(&mut self, window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        // Initialize views if not yet done
        if !self.is_initialized() {
            self.init(cx);
            // Focus MainPanel on first render so keyboard shortcuts work immediately
            window.focus(&self.focus_handle, cx);
            println!(">>> MainPanel first render - focused <<<");
        }

        // Check for pending navigation requests from child views
        // Poll frequently since we can't get async notify from the channel
        if crate::ui_gpui::navigation_channel().has_pending() {
            if let Some(view_id) = crate::ui_gpui::navigation_channel().take_pending() {
                println!(">>> MainPanel::render - Processing navigation to {:?} <<<", view_id);
                tracing::info!("MainPanel: Processing navigation request to {:?}", view_id);
                
                // Special handling: when navigating to ModelSelector, request models
                if view_id == ViewId::ModelSelector {
                    if let Some(ref model_selector) = self.model_selector_view {
                        model_selector.update(cx, |view, _cx| {
                            view.request_models();
                        });
                    }
                }
                
                self.navigation.navigate(view_id);
                cx.notify();
            }
        }
        
        // Check for pending ViewCommands from presenters via bridge
        // Use MainPanelAppState which is set from main_gpui.rs
        if let Some(app_state) = cx.try_global::<MainPanelAppState>() {
            let bridge = app_state.gpui_bridge.clone();
            let commands = bridge.drain_commands();
            for cmd in commands {
                println!(">>> MainPanel: received ViewCommand {:?} <<<", cmd);
                match &cmd {
                    ViewCommand::ModelSearchResults { models } => {
                        // Forward to ModelSelectorView
                        if let Some(ref model_selector) = self.model_selector_view {
                            let models_clone = models.clone();
                            model_selector.update(cx, |view, cx| {
                                view.handle_command(ViewCommand::ModelSearchResults { models: models_clone }, cx);
                            });
                        }
                    }
                    _ => {
                        // Handle other commands as needed
                    }
                }
            }
        }

        let current_view = self.navigation.current();
        
        // Schedule a notify after a brief delay to keep polling for navigation
        // This is a workaround since we can't use async notify from static channel
        let entity_id = cx.entity_id();
        cx.defer(move |cx| {
            cx.notify(entity_id);
        });

        // Request focus on the MainPanel so we receive keyboard events
        let focus_handle = self.focus_handle.clone();
        
        div()
            .id("main-panel")
            .flex()
            .flex_col()
            .size_full()
            .bg(Theme::bg_darkest())
            .track_focus(&self.focus_handle)
            // Click to get focus
            .on_mouse_down(MouseButton::Left, cx.listener(move |_this, _, window, cx| {
                println!(">>> MainPanel clicked - requesting focus <<<");
                window.focus(&focus_handle, cx);
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _window, _cx| {
                let key = &event.keystroke.key;
                let modifiers = &event.keystroke.modifiers;
                let current = this.navigation.current();
                
                println!(">>> MainPanel key_down: key={} platform={} shift={} current={:?} <<<", 
                    key, modifiers.platform, modifiers.shift, current);
                
                // Global keyboard shortcuts - work from any view
                // Using Ctrl+key to avoid conflicts with system shortcuts
                if modifiers.control {
                    match key.as_str() {
                        "h" => {
                            println!(">>> Ctrl+H - navigating to History <<<");
                            crate::ui_gpui::navigation_channel().request_navigate(ViewId::History);
                        }
                        "s" => {
                            println!(">>> Ctrl+S - navigating to Settings <<<");
                            crate::ui_gpui::navigation_channel().request_navigate(ViewId::Settings);
                        }
                        "n" => {
                            println!(">>> Ctrl+N - new conversation <<<");
                            crate::ui_gpui::navigation_channel().request_navigate(ViewId::Chat);
                        }
                        _ => {}
                    }
                }
                // Cmd+W for close/back (standard macOS)
                else if modifiers.platform && key == "w" {
                    println!(">>> Cmd+W - navigate back <<<");
                    if current != ViewId::Chat {
                        this.navigation.navigate_back();
                    }
                }
                // View-specific shortcuts (no Cmd modifier)
                else if current == ViewId::Settings {
                    // Settings view shortcuts
                    if key == "+" || (key == "=" && modifiers.shift) {
                        // "+" key - Add Profile
                        println!(">>> + pressed on Settings - Add Profile <<<");
                        crate::ui_gpui::navigation_channel().request_navigate(ViewId::ModelSelector);
                    } else if key == "m" {
                        // "m" key - Add MCP
                        println!(">>> m pressed on Settings - Add MCP <<<");
                        crate::ui_gpui::navigation_channel().request_navigate(ViewId::McpAdd);
                    } else if key == "escape" {
                        println!(">>> Escape on Settings - back to Chat <<<");
                        this.navigation.navigate_back();
                    }
                }
                // Model Selector view - forward keys for search
                else if current == ViewId::ModelSelector {
                    // Don't consume keys here - let the view handle search
                    // Only handle escape
                    if key == "escape" {
                        println!(">>> Escape on ModelSelector - back to Settings <<<");
                        this.navigation.navigate_back();
                    } else {
                        // Forward all other keys to the model selector for search
                        if let Some(ref model_selector) = this.model_selector_view {
                            model_selector.update(_cx, |view, cx| {
                                if key == "backspace" {
                                    let current = view.get_state().search_query.clone();
                                    let new_query = current[..current.len().saturating_sub(1)].to_string();
                                    println!(">>> ModelSelector search backspace: '{}' -> '{}' <<<", current, new_query);
                                    view.set_search_query(new_query);
                                    cx.notify();
                                } else if key.len() == 1 && !modifiers.platform && !modifiers.control {
                                    let mut query = view.get_state().search_query.clone();
                                    query.push_str(key);
                                    println!(">>> ModelSelector search updated: '{}' <<<", query);
                                    view.set_search_query(query);
                                    cx.notify();
                                }
                            });
                        }
                    }
                }
                // Chat view - forward all keys to ChatView for text input
                else if current == ViewId::Chat {
                    if let Some(ref chat_view) = this.chat_view {
                        chat_view.update(_cx, |view, cx| {
                            // Forward backspace
                            if key == "backspace" {
                                view.handle_backspace(cx);
                            }
                            // Forward enter
                            else if key == "enter" {
                                view.handle_enter(cx);
                            }
                            // Forward space
                            else if key == "space" {
                                view.handle_space(cx);
                            }
                            // Forward single character keys
                            else if key.len() == 1 && !modifiers.platform && !modifiers.control {
                                view.handle_char(key, cx);
                            }
                        });
                    }
                }
                else if key == "escape" {
                    println!(">>> Escape pressed - navigate back <<<");
                    if current != ViewId::Chat {
                        this.navigation.navigate_back();
                    }
                }
            }))
            // Render view based on navigation state
            // @plan PLAN-20250130-GPUIREDUX.P11
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    // Chat view
                    .when(current_view == ViewId::Chat, |d| {
                        if let Some(view) = &self.chat_view {
                            d.child(view.clone())
                        } else {
                            d.child(div().child("Loading chat..."))
                        }
                    })
                    // History view
                    .when(current_view == ViewId::History, |d| {
                        if let Some(view) = &self.history_view {
                            d.child(view.clone())
                        } else {
                            d.child(div().child("Loading history..."))
                        }
                    })
                    // Settings view
                    .when(current_view == ViewId::Settings, |d| {
                        if let Some(view) = &self.settings_view {
                            d.child(view.clone())
                        } else {
                            d.child(div().child("Loading settings..."))
                        }
                    })
                    // Model Selector view
                    .when(current_view == ViewId::ModelSelector, |d| {
                        if let Some(view) = &self.model_selector_view {
                            d.child(view.clone())
                        } else {
                            d.child(div().child("Loading model selector..."))
                        }
                    })
                    // Profile Editor view
                    .when(current_view == ViewId::ProfileEditor, |d| {
                        if let Some(view) = &self.profile_editor_view {
                            d.child(view.clone())
                        } else {
                            d.child(div().child("Loading profile editor..."))
                        }
                    })
                    // MCP Add view
                    .when(current_view == ViewId::McpAdd, |d| {
                        if let Some(view) = &self.mcp_add_view {
                            d.child(view.clone())
                        } else {
                            d.child(div().child("Loading MCP add..."))
                        }
                    })
                    // MCP Configure view
                    .when(current_view == ViewId::McpConfigure, |d| {
                        if let Some(view) = &self.mcp_configure_view {
                            d.child(view.clone())
                        } else {
                            d.child(div().child("Loading MCP configure..."))
                        }
                    })
            )
    }
}
