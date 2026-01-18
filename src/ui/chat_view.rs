//! Chat view implementation for the popover
#![allow(unsafe_code)]
#![allow(unused_unsafe)]
#![allow(clippy::single_match_else)]
#![allow(clippy::format_push_string)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::assigning_clones)]
#![allow(clippy::option_map_or_none)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::manual_is_ascii_check)]
#![allow(clippy::ref_option)]
#![allow(clippy::unused_self)]

use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::Write;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSButton, NSFont, NSLayoutConstraintOrientation, NSScrollView, NSStackView,
    NSStackViewDistribution, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
    NSViewController,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use super::chat_view_helpers::{
    build_llm_messages, collect_profile, fetch_mcp_tools, load_conversation_by_title,
    load_view_layout, rebuild_messages, reset_streaming_buffers, should_show_thinking,
    start_streaming_request, update_thinking_button_state, update_title_and_model,
};

use objc2_quartz_core::CALayer;
use uuid::Uuid;

use super::theme::Theme;
use personal_agent::config::Config;
use personal_agent::mcp::McpService;
use personal_agent::models::{Conversation, Message as ConvMessage};
use personal_agent::storage::ConversationStorage;

/// Logging helper - writes to file
pub(super) fn log_to_file(message: &str) {
    let log_path = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/PersonalAgent/debug.log");

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let _ = writeln!(file, "[{timestamp}] ChatView: {message}");
    }
}

// ============================================================================
// Message data structure
// ============================================================================

#[derive(Clone, Debug)]
pub struct Message {
    pub text: String,
    pub is_user: bool,
}

// ============================================================================
// Shared state for messages
// ============================================================================

type MessageStore = Rc<RefCell<Vec<Message>>>;

// ============================================================================
// Helper functions for CALayer operations
// ============================================================================

pub(super) fn set_layer_background_color(layer: &CALayer, r: f64, g: f64, b: f64) {
    use objc2_core_graphics::CGColor;
    // Create a CGColor using objc2-core-graphics
    let color = CGColor::new_generic_rgb(r, g, b, 1.0);
    layer.setBackgroundColor(Some(&color));
}

pub(super) fn set_layer_corner_radius(layer: &CALayer, radius: f64) {
    layer.setCornerRadius(radius);
}

// ============================================================================
// ChatViewController ivars
// ============================================================================

pub struct ChatViewIvars {
    pub(super) messages: MessageStore,
    pub(super) scroll_view: RefCell<Option<Retained<NSScrollView>>>,
    pub(super) messages_container: RefCell<Option<Retained<NSView>>>,
    pub(super) input_field: RefCell<Option<Retained<NSTextField>>>,
    pub(super) conversation: RefCell<Option<Conversation>>,
    /// Title popup button for conversation selection
    pub(super) title_popup: RefCell<Option<Retained<objc2_app_kit::NSPopUpButton>>>,
    /// Title edit field (shown when renaming)
    pub(super) title_edit_field: RefCell<Option<Retained<NSTextField>>>,
    /// Rename button
    pub(super) rename_button: RefCell<Option<Retained<NSButton>>>,
    pub(super) _model_label: RefCell<Option<Retained<NSTextField>>>,
    pub(super) thinking_button: RefCell<Option<Retained<NSButton>>>,
    /// Shared streaming response text for updating from background thread
    pub(super) streaming_response: Arc<Mutex<String>>,
    /// Shared streaming thinking text for updating from background thread
    pub(super) streaming_thinking: Arc<Mutex<String>>,
    /// Shared tool uses accumulated during streaming
    pub(super) streaming_tool_uses: Arc<Mutex<Vec<personal_agent::llm::tools::ToolUse>>>,
    /// Flag to indicate streaming is in progress
    pub(super) is_streaming: RefCell<bool>,
    /// Flag to indicate we're currently executing tools (waiting for follow-up stream)
    pub(super) executing_tools: Arc<std::sync::atomic::AtomicBool>,
    /// Stop button for canceling streaming
    pub(super) stop_button: RefCell<Option<Retained<NSButton>>>,
    /// Flag to signal cancellation
    pub(super) cancel_streaming: Arc<std::sync::atomic::AtomicBool>,
}

// ============================================================================
// ChatViewController - main view controller
// ============================================================================

define_class!(
    #[unsafe(super(NSViewController))]
    #[thread_kind = MainThreadOnly]
    #[name = "ChatViewController"]
    #[ivars = ChatViewIvars]
    pub struct ChatViewController;

    unsafe impl NSObjectProtocol for ChatViewController {}

    impl ChatViewController {
        #[unsafe(method(loadView))]
        fn load_view(&self) {
            println!("DEBUG - ChatViewController loadView called");
            let mtm = MainThreadMarker::new().unwrap();
            load_view_layout(self, mtm);
        }

        #[unsafe(method(sendMessage:))]
        fn send_message(&self, _sender: Option<&NSObject>) {
            log_to_file("send_message called");

            if *self.ivars().is_streaming.borrow() {
                log_to_file("Already streaming, ignoring");
                return;
            }

            let Some(input) = &*self.ivars().input_field.borrow() else {
                log_to_file("ERROR: No input field reference!");
                return;
            };

            let text_str = input.stringValue().to_string();
            log_to_file(&format!("Input text: '{text_str}'"));

            if text_str.trim().is_empty() {
                log_to_file("Text is empty, ignoring");
                return;
            }

            log_to_file("Text not empty, adding message");
            self.add_message_to_store(&text_str, true);
            input.setStringValue(&NSString::new());

            if let Some(ref mut conversation) = *self.ivars().conversation.borrow_mut() {
                conversation.add_message(ConvMessage::user(text_str));
                log_to_file(&format!(
                    "Added message to conversation, now has {} messages",
                    conversation.messages.len()
                ));
            } else {
                log_to_file("ERROR: No conversation object!");
            }

            let config = Config::load(Config::default_path().unwrap_or_default()).ok();
            let profile = config.as_ref().and_then(collect_profile);

            if let Some(profile) = profile {
                let tools = fetch_mcp_tools();
                let llm_messages = build_llm_messages(&profile, self.ivars().conversation.borrow().as_ref());

                update_thinking_button_state(self);
                *self.ivars().is_streaming.borrow_mut() = true;
                reset_streaming_buffers(
                    &self.ivars().streaming_response,
                    &self.ivars().streaming_thinking,
                    &self.ivars().streaming_tool_uses,
                );

                start_streaming_request(
                    profile,
                    llm_messages,
                    tools,
                    Arc::clone(&self.ivars().streaming_response),
                    Arc::clone(&self.ivars().streaming_thinking),
                    Arc::clone(&self.ivars().streaming_tool_uses),
                    Arc::clone(&self.ivars().cancel_streaming),
                );

                self.schedule_streaming_update();
            } else {
                log_to_file("No profile configured");
                self.add_message_to_store("[No profile configured - go to Settings]", false);
                rebuild_messages(self);
                update_title_and_model(self);
            }
        }

        #[unsafe(method(checkStreamingStatus:))]
        fn check_streaming_status(&self, _sender: Option<&NSObject>) {
            if !*self.ivars().is_streaming.borrow() {
                return;
            }

            let current_text = if let Ok(buf) = self.ivars().streaming_response.lock() {
                buf.clone()
            } else {
                return;
            };

            let current_thinking = if let Ok(buf) = self.ivars().streaming_thinking.lock() {
                buf.clone()
            } else {
                String::new()
            };

            let show_thinking = should_show_thinking(self);
            let display_text = if show_thinking && !current_thinking.is_empty() {
                if current_text.is_empty() {
                    format!(" *Thinking...*\n{current_thinking}\n\n▌")
                } else {
                    format!(" *Thinking:*\n{current_thinking}\n\n---\n\n{current_text}▌")
                }
            } else if current_text.is_empty() {
                "▌".to_string()
            } else {
                format!("{current_text}▌")
            };

            if let Some(last_msg) = self.ivars().messages.borrow_mut().last_mut() {
                if !last_msg.is_user {
                    last_msg.text = display_text;
                }
            }

            rebuild_messages(self);

            if self.check_streaming_done() {
                self.finalize_streaming();
            } else {
                self.schedule_streaming_update();
            }
        }

        #[unsafe(method(toggleThinking:))]
        fn toggle_thinking(&self, _sender: Option<&NSObject>) {
            let config_path = match Config::default_path() {
                Ok(path) => path,
                Err(e) => {
                    eprintln!("Failed to get config path: {e}");
                    return;
                }
            };

            let mut config = match Config::load(&config_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to load config: {e}");
                    return;
                }
            };

            if let Some(conversation) = &*self.ivars().conversation.borrow() {
                let profile_id = conversation.profile_id;
                if let Ok(profile) = config.get_profile_mut(&profile_id) {
                    let new_state = !profile.parameters.show_thinking;
                    profile.parameters.show_thinking = new_state;

                    if let Err(e) = config.save(&config_path) {
                        eprintln!("Failed to save config: {e}");
                    } else {
                        println!("Thinking display toggled to: {new_state}");
                        update_thinking_button_state(self);
                    }
                }
            }
        }

        #[unsafe(method(showHistory:))]
        fn show_history(&self, _sender: Option<&NSObject>) {
            println!("Show history clicked");
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowHistoryView");
            unsafe {
                center.postNotificationName_object(&name, None);
            }
        }

        #[unsafe(method(newConversation:))]
        fn new_conversation(&self, _sender: Option<&NSObject>) {
            log_to_file("New conversation clicked");

            let config = Config::load(Config::default_path().unwrap()).unwrap_or_default();
            let profile_id = config.default_profile.unwrap_or_else(|| {
                config.profiles.first().map_or_else(Uuid::new_v4, |p| p.id)
            });

            log_to_file(&format!("Using profile_id: {profile_id}"));

            self.ivars().messages.borrow_mut().clear();
            let new_conversation = Conversation::new(profile_id);

            Self::save_active_conversation_id(new_conversation.id);

            *self.ivars().conversation.borrow_mut() = Some(new_conversation);

            log_to_file("Cleared messages, rebuilding view");
            rebuild_messages(self);
            update_title_and_model(self);
            log_to_file("New conversation setup complete");
        }

        #[unsafe(method(showSettings:))]
        fn show_settings(&self, _sender: Option<&NSObject>) {
            println!("Show settings clicked");
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowSettingsView");
            unsafe {
                center.postNotificationName_object(&name, None);
            }
        }

        #[unsafe(method(stopStreaming:))]
        fn stop_streaming(&self, _sender: Option<&NSObject>) {
            log_to_file("ChatView: Stop streaming clicked");
            self.ivars()
                .cancel_streaming
                .store(true, std::sync::atomic::Ordering::SeqCst);
            if let Some(btn) = &*self.ivars().stop_button.borrow() {
                btn.setEnabled(false);
            }
        }

        #[unsafe(method(quitApp:))]
        fn quit_app(&self, _sender: Option<&NSObject>) {
            log_to_file("Quit app clicked");
            std::process::exit(0);
        }

        #[unsafe(method(titlePopupChanged:))]
        fn title_popup_changed(&self, _sender: Option<&NSObject>) {
            log_to_file("ChatView: titlePopupChanged: action fired");

            let popup = self.ivars().title_popup.borrow();
            let Some(popup) = popup.as_ref() else { return };

            let selected_title = popup
                .titleOfSelectedItem()
                .map(|s| s.to_string())
                .unwrap_or_default();

            log_to_file(&format!("ChatView: Popup selected: {}", selected_title));
            let _ = popup;

            if selected_title.is_empty() {
                return;
            }

            load_conversation_by_title(self, &selected_title);
        }

        #[unsafe(method(renameConversation:))]
        fn rename_conversation(&self, _sender: Option<&NSObject>) {
            log_to_file("ChatView: Rename conversation clicked");

            let current_title = if let Some(conv) = &*self.ivars().conversation.borrow() {
                conv.title.clone().unwrap_or_else(|| conv.created_at.format("%Y%m%d%H%M%S%3f").to_string())
            } else {
                return;
            };

            if let Some(popup) = &*self.ivars().title_popup.borrow() {
                popup.setHidden(true);
            }
            if let Some(edit_field) = &*self.ivars().title_edit_field.borrow() {
                edit_field.setStringValue(&NSString::from_str(&current_title));
                edit_field.setHidden(false);
                if let Some(window) = edit_field.window() {
                    window.makeFirstResponder(Some(edit_field));
                }
                unsafe {
                    edit_field.selectText(None);
                }
            }
            if let Some(btn) = &*self.ivars().rename_button.borrow() {
                btn.setTitle(&NSString::from_str("ok"));
                unsafe {
                    btn.setAction(Some(sel!(titleEditDone:)));
                }
            }
        }

        #[unsafe(method(titleEditDone:))]
        fn title_edit_done(&self, _sender: Option<&NSObject>) {
            log_to_file("ChatView: Title edit done");

            let new_title = if let Some(edit_field) = &*self.ivars().title_edit_field.borrow() {
                edit_field.stringValue().to_string().trim().to_string()
            } else {
                return;
            };

            if !new_title.is_empty() {
                log_to_file(&format!("ChatView: Renaming to: {}", new_title));

                if let Some(ref mut conv) = *self.ivars().conversation.borrow_mut() {
                    conv.title = Some(new_title.clone());

                    if let Ok(storage) = ConversationStorage::with_default_path() {
                        if let Err(e) = storage.save(conv) {
                            log_to_file(&format!("ChatView: Failed to save renamed conversation: {e}"));
                        } else {
                            log_to_file(&format!("ChatView: Conversation renamed to: {new_title}"));
                        }
                    }
                }
            }

            if let Some(edit_field) = &*self.ivars().title_edit_field.borrow() {
                edit_field.setHidden(true);
            }
            if let Some(popup) = &*self.ivars().title_popup.borrow() {
                popup.setHidden(false);
            }
            if let Some(btn) = &*self.ivars().rename_button.borrow() {
                btn.setTitle(&NSString::from_str("R"));
                unsafe {
                    btn.setAction(Some(sel!(renameConversation:)));
                }
            }

            update_title_and_model(self);
        }
    }

);

impl ChatViewController {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        // Load config and try to load the active conversation
        let config = Config::load(Config::default_path().unwrap()).unwrap_or_default();
        let profile_id = config
            .default_profile
            .unwrap_or_else(|| config.profiles.first().map_or_else(Uuid::new_v4, |p| p.id));

        // Try to load the active conversation from config, or fall back to most recent
        let conversation = if let Ok(storage) = ConversationStorage::with_default_path() {
            // First, try to load the active conversation from config
            if let Some(active_id) = config.active_conversation_id {
                log_to_file(&format!(
                    "ChatView: Trying to load active conversation: {active_id}"
                ));
                // Find the file for this conversation ID
                if let Ok(filenames) = storage.list() {
                    for filename in &filenames {
                        if let Ok(conv) = storage.load(filename) {
                            if conv.id == active_id {
                                log_to_file(&format!(
                                    "ChatView: Loaded active conversation: {:?}",
                                    conv.title
                                ));
                                return Self::create_with_conversation(mtm, conv);
                            }
                        }
                    }
                }
            }

            // Fall back to most recent conversation
            if let Ok(filenames) = storage.list() {
                if let Some(newest) = filenames.first() {
                    if let Ok(conv) = storage.load(newest) {
                        log_to_file(&format!(
                            "ChatView: Loaded most recent conversation: {:?}",
                            conv.title
                        ));
                        conv
                    } else {
                        Conversation::new(profile_id)
                    }
                } else {
                    Conversation::new(profile_id)
                }
            } else {
                Conversation::new(profile_id)
            }
        } else {
            Conversation::new(profile_id)
        };

        Self::create_with_conversation(mtm, conversation)
    }

    fn create_with_conversation(
        mtm: MainThreadMarker,
        conversation: Conversation,
    ) -> Retained<Self> {
        // Initialize MCP service in background thread (not tokio task)
        log_to_file("Initializing MCP service...");
        std::thread::spawn(|| {
            log_to_file("MCP init thread started");
            // Create a new runtime for this thread to avoid blocking the global one
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create MCP init runtime");

            log_to_file("MCP init runtime created");
            rt.block_on(async {
                log_to_file("MCP init block_on started");
                let service_arc = McpService::global();
                log_to_file("MCP service global obtained");
                let result = {
                    log_to_file("MCP acquiring lock...");
                    let mut svc = service_arc.lock().await;
                    log_to_file("MCP lock acquired, calling initialize...");
                    svc.initialize().await
                };

                match result {
                    Ok(()) => {
                        let count = service_arc.lock().await.active_count();
                        log_to_file(&format!("MCP initialized: {} active", count));
                    }
                    Err(e) => log_to_file(&format!("MCP init error: {e}")),
                }
            });
            log_to_file("MCP init thread finished");
        });

        let ivars = ChatViewIvars {
            messages: Rc::new(RefCell::new(Vec::new())),
            scroll_view: RefCell::new(None),
            messages_container: RefCell::new(None),
            input_field: RefCell::new(None),
            conversation: RefCell::new(Some(conversation)),
            title_popup: RefCell::new(None),
            title_edit_field: RefCell::new(None),
            rename_button: RefCell::new(None),
            _model_label: RefCell::new(None),
            thinking_button: RefCell::new(None),
            streaming_response: Arc::new(Mutex::new(String::new())),
            streaming_thinking: Arc::new(Mutex::new(String::new())),
            streaming_tool_uses: Arc::new(Mutex::new(Vec::new())),
            is_streaming: RefCell::new(false),
            executing_tools: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            stop_button: RefCell::new(None),
            cancel_streaming: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };

        let this = Self::alloc(mtm).set_ivars(ivars);
        // SAFETY: Calling super init with correct signature
        unsafe { msg_send![super(this), init] }
    }

    pub fn load_conversation(&self, conversation: Conversation) {
        // Clear existing messages
        self.ivars().messages.borrow_mut().clear();

        // Load messages from conversation
        for msg in &conversation.messages {
            let is_user = matches!(msg.role, personal_agent::models::MessageRole::User);
            self.add_message_to_store(&msg.content, is_user);
        }

        // Save active conversation ID to config
        Self::save_active_conversation_id(conversation.id);

        // Store the conversation
        *self.ivars().conversation.borrow_mut() = Some(conversation);

        // Rebuild the UI
        rebuild_messages(self);
        update_title_and_model(self);
    }

    /// Save the active conversation ID to config
    fn save_active_conversation_id(conversation_id: Uuid) {
        if let Ok(config_path) = Config::default_path() {
            if let Ok(mut config) = Config::load(&config_path) {
                config.active_conversation_id = Some(conversation_id);
                if let Err(e) = config.save(&config_path) {
                    log_to_file(&format!("Failed to save active conversation ID: {e}"));
                } else {
                    log_to_file(&format!("Saved active conversation ID: {conversation_id}"));
                }
            }
        }
    }

    pub(super) fn add_message_to_store(&self, text: &str, is_user: bool) {
        self.ivars().messages.borrow_mut().push(Message {
            text: text.to_string(),
            is_user,
        });
    }

    pub(super) fn set_title_popup(&self, popup: Retained<objc2_app_kit::NSPopUpButton>) {
        *self.ivars().title_popup.borrow_mut() = Some(popup);
    }

    pub(super) fn set_title_edit_field(&self, field: Retained<NSTextField>) {
        *self.ivars().title_edit_field.borrow_mut() = Some(field);
    }

    pub(super) fn set_rename_button(&self, button: Retained<NSButton>) {
        *self.ivars().rename_button.borrow_mut() = Some(button);
    }

    pub(super) fn set_thinking_button(&self, button: Retained<NSButton>) {
        *self.ivars().thinking_button.borrow_mut() = Some(button);
    }

    pub(super) fn set_stop_button(&self, button: Retained<NSButton>) {
        *self.ivars().stop_button.borrow_mut() = Some(button);
    }

    pub(super) fn set_input_field(&self, field: Retained<NSTextField>) {
        *self.ivars().input_field.borrow_mut() = Some(field);
    }

    pub(super) fn set_scroll_view(&self, scroll_view: Retained<NSScrollView>) {
        *self.ivars().scroll_view.borrow_mut() = Some(scroll_view);
    }

    pub(super) fn set_messages_container(&self, container: Retained<NSView>) {
        *self.ivars().messages_container.borrow_mut() = Some(container);
    }

    fn schedule_streaming_update(&self) {
        // Use performSelector:withObject:afterDelay: to schedule UI update on main thread
        // This is a simple polling approach - update every 100ms
        unsafe {
            let delay: f64 = 0.1; // 100ms
            let _: () = msg_send![
                self,
                performSelector: sel!(checkStreamingStatus:),
                withObject: std::ptr::null::<NSObject>(),
                afterDelay: delay
            ];
        }
    }

    fn check_streaming_done(&self) -> bool {
        if let Ok(buf) = self.ivars().streaming_response.lock() {
            buf.contains("[Error:") || buf.ends_with('␄')
        } else {
            true
        }
    }

    fn finalize_streaming(&self) {
        log_to_file("Finalizing streaming response");

        if self
            .ivars()
            .executing_tools
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            log_to_file(
                "Skipping finalize_streaming - currently executing tools, re-scheduling poll",
            );
            self.schedule_streaming_update();
            return;
        }

        super::chat_view_helpers::schedule_follow_up_request(self);
    }

    /// Create a collapsible thinking bubble
    pub(super) fn create_thinking_bubble(
        &self,
        text: &str,
        mtm: MainThreadMarker,
    ) -> Retained<NSView> {
        // Create container for thinking with header and content
        let container = NSStackView::new(mtm);
        container.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        container.setSpacing(4.0);
        container.setTranslatesAutoresizingMaskIntoConstraints(false);

        // Create header row with "Thinking..." label (clickable to collapse)
        let header = NSTextField::labelWithString(&NSString::from_str("Thinking..."), mtm);
        header.setFont(Some(&NSFont::boldSystemFontOfSize(11.0)));
        header.setTextColor(Some(&Theme::text_secondary_color()));
        header.setTranslatesAutoresizingMaskIntoConstraints(false);

        unsafe {
            container.addArrangedSubview(&header);
        }

        // Create the actual thinking content bubble (dimmer style)
        let content_bubble = self.create_message_bubble_styled(text, false, true, mtm);
        unsafe {
            container.addArrangedSubview(&content_bubble);
        }

        // Create horizontal container for alignment (left-aligned like assistant)
        let row = NSStackView::new(mtm);
        row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        row.setSpacing(0.0);
        row.setTranslatesAutoresizingMaskIntoConstraints(false);
        row.setDistribution(NSStackViewDistribution::Fill);

        let spacer = NSView::new(mtm);
        spacer.setTranslatesAutoresizingMaskIntoConstraints(false);
        spacer.setContentHuggingPriority_forOrientation(
            1.0,
            NSLayoutConstraintOrientation::Horizontal,
        );

        unsafe {
            row.addArrangedSubview(&container);
            row.addArrangedSubview(&spacer);
        }

        Retained::from(&*row as &NSView)
    }

    pub(super) fn create_message_bubble(
        &self,
        text: &str,
        is_user: bool,
        mtm: MainThreadMarker,
    ) -> Retained<NSView> {
        self.create_message_bubble_styled(text, is_user, false, mtm)
    }

    /// Create a message bubble with optional thinking style
    fn create_message_bubble_styled(
        &self,
        text: &str,
        is_user: bool,
        is_thinking: bool,
        mtm: MainThreadMarker,
    ) -> Retained<NSView> {
        // Create horizontal container for alignment
        let row = NSStackView::new(mtm);
        row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        row.setSpacing(0.0);
        row.setTranslatesAutoresizingMaskIntoConstraints(false);
        row.setDistribution(NSStackViewDistribution::Fill);

        // Create message bubble
        let bubble = NSTextField::labelWithString(&NSString::from_str(text), mtm);
        bubble.setFont(Some(&NSFont::systemFontOfSize(13.0)));
        bubble.setTextColor(Some(&Theme::text_primary()));
        bubble.setMaximumNumberOfLines(0);
        bubble.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByWordWrapping);
        bubble.setPreferredMaxLayoutWidth(260.0); // Limit bubble width to ~65% of window
        bubble.setTranslatesAutoresizingMaskIntoConstraints(false);

        bubble.setWantsLayer(true);
        if let Some(layer) = bubble.layer() {
            if is_user {
                set_layer_background_color(&layer, 42.0 / 255.0, 74.0 / 255.0, 42.0 / 255.0);
            } else if is_thinking {
                set_layer_background_color(&layer, 42.0 / 255.0, 42.0 / 255.0, 58.0 / 255.0);
            } else {
                set_layer_background_color(
                    &layer,
                    Theme::BG_DARK.0,
                    Theme::BG_DARK.1,
                    Theme::BG_DARK.2,
                );
            }
            set_layer_corner_radius(&layer, 8.0);
        }
        bubble.setDrawsBackground(true);
        bubble.setBordered(false);
        bubble.setSelectable(true);

        // Add padding via NSTextField cell insets
        bubble.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(260.0, 0.0)));

        let bubble_container = NSStackView::new(mtm);
        bubble_container.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        bubble_container.setSpacing(0.0);
        bubble_container.setTranslatesAutoresizingMaskIntoConstraints(false);
        bubble_container.setDistribution(NSStackViewDistribution::Fill);
        bubble_container.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: 8.0,
            left: 12.0,
            bottom: 8.0,
            right: 12.0,
        });

        unsafe {
            bubble_container.addArrangedSubview(&bubble);
        }

        if is_user {
            // User messages right aligned
            let spacer = NSView::new(mtm);
            spacer.setTranslatesAutoresizingMaskIntoConstraints(false);
            spacer.setContentHuggingPriority_forOrientation(
                1.0,
                NSLayoutConstraintOrientation::Horizontal,
            );

            unsafe {
                row.addArrangedSubview(&spacer);
                row.addArrangedSubview(&bubble_container);
            }
        } else {
            // Assistant messages left aligned
            let spacer = NSView::new(mtm);
            spacer.setTranslatesAutoresizingMaskIntoConstraints(false);
            spacer.setContentHuggingPriority_forOrientation(
                1.0,
                NSLayoutConstraintOrientation::Horizontal,
            );

            unsafe {
                row.addArrangedSubview(&bubble_container);
                row.addArrangedSubview(&spacer);
            }
        }

        Retained::from(&*row as &NSView)
    }
}
