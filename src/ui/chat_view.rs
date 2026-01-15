//! Chat view implementation for the popover

use std::cell::RefCell;
use std::rc::Rc;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::thread;

use chrono::Local;
use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly, DefinedClass};
use objc2_foundation::{
    NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use objc2_app_kit::{
    NSView, NSViewController, NSTextField, NSButton, NSScrollView,
    NSFont, NSBezelStyle, NSStackView, NSUserInterfaceLayoutOrientation,
    NSLayoutConstraintOrientation, NSStackViewDistribution,
};
use objc2_quartz_core::CALayer;
use uuid::Uuid;

use super::theme::Theme;
use personal_agent::config::Config;
use personal_agent::models::{Conversation, Message as ConvMessage};
use personal_agent::storage::ConversationStorage;
use personal_agent::{LlmClient, LlmMessage, StreamEvent};

/// Logging helper - writes to file
fn log_to_file(message: &str) {
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

fn set_layer_background_color(layer: &CALayer, r: f64, g: f64, b: f64) {
    use objc2_core_graphics::CGColor;
    // Create a CGColor using objc2-core-graphics
    let color = CGColor::new_generic_rgb(r, g, b, 1.0);
    layer.setBackgroundColor(Some(&color));
}

fn set_layer_corner_radius(layer: &CALayer, radius: f64) {
    layer.setCornerRadius(radius);
}

// ============================================================================
// ChatViewController ivars
// ============================================================================

pub struct ChatViewIvars {
    messages: MessageStore,
    scroll_view: RefCell<Option<Retained<NSScrollView>>>,
    messages_container: RefCell<Option<Retained<NSView>>>,
    input_field: RefCell<Option<Retained<NSTextField>>>,
    conversation: RefCell<Option<Conversation>>,
    title_button: RefCell<Option<Retained<NSButton>>>,
    title_edit_field: RefCell<Option<Retained<NSTextField>>>,
    model_label: RefCell<Option<Retained<NSTextField>>>,
    thinking_button: RefCell<Option<Retained<NSButton>>>,
    /// Shared streaming response text for updating from background thread
    streaming_response: Arc<Mutex<String>>,
    /// Shared streaming thinking text for updating from background thread
    streaming_thinking: Arc<Mutex<String>>,
    /// Flag to indicate streaming is in progress
    is_streaming: RefCell<bool>,
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

            // Create main container (400x500 for popover content)
            let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 500.0));
            let main_view = NSView::initWithFrame(NSView::alloc(mtm), frame);
            main_view.setWantsLayer(true);
            if let Some(layer) = main_view.layer() {
                set_layer_background_color(&layer, Theme::BG_DARKEST.0, Theme::BG_DARKEST.1, Theme::BG_DARKEST.2);
            }

            // Create main vertical stack
            let main_stack = NSStackView::new(mtm);
            unsafe {
                main_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
                main_stack.setSpacing(0.0);
                main_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
                // CRITICAL: Set distribution to Fill so views expand
                main_stack.setDistribution(NSStackViewDistribution::Fill);
            }
            
            // Build the UI components
            let top_bar = self.build_top_bar_stack(mtm);
            let chat_area = self.build_chat_area_stack(mtm);
            let input_area = self.build_input_area_stack(mtm);
            
            // Set content hugging priorities for proper sizing
            unsafe {
                // Top bar: high priority (wants to stay at fixed height)
                top_bar.setContentHuggingPriority_forOrientation(
                    251.0,
                    NSLayoutConstraintOrientation::Vertical
                );
                
                // Chat area (scroll view): low priority (wants to expand)
                chat_area.setContentHuggingPriority_forOrientation(
                    1.0,
                    NSLayoutConstraintOrientation::Vertical
                );
                
                // Input area: high priority (wants to stay at fixed height)
                input_area.setContentHuggingPriority_forOrientation(
                    251.0,
                    NSLayoutConstraintOrientation::Vertical
                );
            }
            
            // Add to stack
            unsafe {
                main_stack.addArrangedSubview(&top_bar);
                main_stack.addArrangedSubview(&chat_area);
                main_stack.addArrangedSubview(&input_area);
            }
            
            // Add stack to main view
            main_view.addSubview(&main_stack);
            
            // Set constraints to fill parent
            unsafe {
                // Activate constraints individually
                let leading = main_stack.leadingAnchor().constraintEqualToAnchor(&main_view.leadingAnchor());
                leading.setActive(true);
                
                let trailing = main_stack.trailingAnchor().constraintEqualToAnchor(&main_view.trailingAnchor());
                trailing.setActive(true);
                
                let top = main_stack.topAnchor().constraintEqualToAnchor(&main_view.topAnchor());
                top.setActive(true);
                
                let bottom = main_stack.bottomAnchor().constraintEqualToAnchor(&main_view.bottomAnchor());
                bottom.setActive(true);
            }

            self.setView(&main_view);
            
            // Add initial sample messages
            // Load messages from the current conversation (if any)
            self.load_initial_messages();
            
            // Force layout to happen so we can see the actual sizes
            main_view.layoutSubtreeIfNeeded();
            
            // Debug: Print frame information after layout
            println!("
=== ChatViewController Frame Debug ===");
            println!("  main_view: {:?}", main_view.frame());
            println!("  main_stack: {:?}", main_stack.frame());
            println!("  top_bar: {:?}", top_bar.frame());
            println!("  chat_area: {:?}", chat_area.frame());
            println!("  input_area: {:?}", input_area.frame());
            println!("=====================================
");
        }

        #[unsafe(method(sendMessage:))]
        fn send_message(&self, _sender: Option<&NSObject>) {
            log_to_file("send_message called");
            
            // Don't allow new messages while streaming
            if *self.ivars().is_streaming.borrow() {
                log_to_file("Already streaming, ignoring");
                return;
            }
            
            if let Some(input) = &*self.ivars().input_field.borrow() {
                let text = input.stringValue();
                let text_str = text.to_string();
                
                log_to_file(&format!("Input text: '{text_str}'"));
                
                if text_str.trim().is_empty() {
                                    log_to_file("Text is empty, ignoring");
                                } else {
                                    log_to_file("Text not empty, adding message");
                                    
                                    // Add user message to UI
                                    self.add_message_to_store(&text_str, true);
                                    
                                    // Clear input
                                    input.setStringValue(&NSString::new());
                                    
                                    // Add user message to conversation
                                    if let Some(ref mut conversation) = *self.ivars().conversation.borrow_mut() {
                                        conversation.add_message(ConvMessage::user(text_str));
                                        log_to_file(&format!("Added message to conversation, now has {} messages", conversation.messages.len()));
                                    } else {
                                        log_to_file("ERROR: No conversation object!");
                                    }
                                    
                                    // Get the current profile
                                    let config = Config::load(Config::default_path().unwrap_or_default()).ok();
                                    let profile = config.as_ref().and_then(|c| {
                                        c.default_profile.and_then(|id| {
                                            c.profiles.iter().find(|p| p.id == id).cloned()
                                        }).or_else(|| c.profiles.first().cloned())
                                    });
                                    
                                    if let Some(profile) = profile {
                                        // Build messages from conversation
                                        let llm_messages: Vec<LlmMessage> = self.ivars()
                                            .conversation
                                            .borrow()
                                            .as_ref()
                                            .map(|c| {
                                                c.messages.iter().map(|m| {
                                                    match m.role {
                                                        personal_agent::models::MessageRole::User => LlmMessage::user(&m.content),
                                                        personal_agent::models::MessageRole::Assistant => LlmMessage::assistant(&m.content),
                                                        personal_agent::models::MessageRole::System => LlmMessage::system(&m.content),
                                                    }
                                                }).collect()
                                            })
                                            .unwrap_or_default();
                                        
                                        // Show empty assistant message placeholder
                                        self.add_message_to_store("", false);
                                        self.rebuild_messages();
                                        
                                        // Mark as streaming
                                        *self.ivars().is_streaming.borrow_mut() = true;
                                        
                                        // Clear the streaming buffers
                                        if let Ok(mut buf) = self.ivars().streaming_response.lock() {
                                            buf.clear();
                                        }
                                        if let Ok(mut buf) = self.ivars().streaming_thinking.lock() {
                                            buf.clear();
                                        }
                                        
                                        // Clone what we need for the background thread
                                        let streaming_response = Arc::clone(&self.ivars().streaming_response);
                                        let streaming_thinking = Arc::clone(&self.ivars().streaming_thinking);
                                        let profile_clone = profile;
                                        
                                        // Spawn background thread for streaming
                                        log_to_file("Starting streaming request in background...");
                                        thread::spawn(move || {
                                            let rt = tokio::runtime::Builder::new_current_thread()
                                                .enable_all()
                                                .build();
                                            
                                            if let Ok(rt) = rt {
                                                rt.block_on(async {
                                                    match LlmClient::from_profile(&profile_clone) {
                                                        Ok(client) => {
                                                            let streaming_response_clone = Arc::clone(&streaming_response);
                                                            let streaming_thinking_clone = Arc::clone(&streaming_thinking);
                                                            let result = client.request_stream(&llm_messages, |event| {
                                                                match event {
                                                                    StreamEvent::TextDelta(delta) => {
                                                                        if let Ok(mut buf) = streaming_response_clone.lock() {
                                                                            buf.push_str(&delta);
                                                                        }
                                                                    }
                                                                    StreamEvent::ThinkingDelta(delta) => {
                                                                        if let Ok(mut buf) = streaming_thinking_clone.lock() {
                                                                            buf.push_str(&delta);
                                                                        }
                                                                    }
                                                                    StreamEvent::Complete => {
                                                                        log_to_file("Streaming complete");
                                                                        // Add completion marker
                                                                        if let Ok(mut buf) = streaming_response_clone.lock() {
                                                                            buf.push('␄'); // EOT marker
                                                                        }
                                                                    }
                                                                    StreamEvent::Error(e) => {
                                                                        log_to_file(&format!("Stream error: {e}"));
                                                                        if let Ok(mut buf) = streaming_response_clone.lock() {
                                                                            buf.push_str(&format!("
                [Error: {e}]"));
                                                                        }
                                                                    }
                                                                }
                                                            }).await;
                                                            
                                                            if let Err(e) = result {
                                                                log_to_file(&format!("Stream request failed: {e}"));
                                                                if let Ok(mut buf) = streaming_response.lock() {
                                                                    buf.push_str(&format!("[Error: {e}]"));
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            log_to_file(&format!("Failed to create client: {e}"));
                                                            if let Ok(mut buf) = streaming_response.lock() {
                                                                buf.push_str(&format!("[Error: {e}]"));
                                                            }
                                                        }
                                                    }
                                                });
                                            }
                                        });
                                        
                                        // Start a timer to poll for updates
                                        self.schedule_streaming_update();
                                        
                                    } else {
                                        log_to_file("No profile configured");
                                        self.add_message_to_store("[No profile configured - go to Settings]", false);
                                        self.rebuild_messages();
                                    }
                                }
            } else {
                log_to_file("ERROR: No input field reference!");
            }
        }
        
        #[unsafe(method(checkStreamingStatus:))]
        fn check_streaming_status(&self, _sender: Option<&NSObject>) {
            if !*self.ivars().is_streaming.borrow() {
                return;
            }
            
            // Get current streaming text
            let current_text = if let Ok(buf) = self.ivars().streaming_response.lock() {
                buf.clone()
            } else {
                return;
            };
            
            // Get current thinking text
            let current_thinking = if let Ok(buf) = self.ivars().streaming_thinking.lock() {
                buf.clone()
            } else {
                String::new()
            };
            
            // Check if thinking should be shown
            let show_thinking = self.should_show_thinking();
            
            // Build the display text
            let display_text = if show_thinking && !current_thinking.is_empty() {
                if current_text.is_empty() {
                    format!(" *Thinking...*
{current_thinking}

▌")
                } else {
                    format!(" *Thinking:*
{current_thinking}

---

{current_text}▌")
                }
            } else if current_text.is_empty() {
                "▌".to_string() // Show cursor while waiting
            } else {
                format!("{current_text}▌") // Show cursor at end while streaming
            };
            
            // Update the last message in the store
            if let Some(last_msg) = self.ivars().messages.borrow_mut().last_mut() {
                if !last_msg.is_user {
                    last_msg.text = display_text;
                }
            }
            
            // Rebuild UI to show updated text
            self.rebuild_messages();
            
            // Check if streaming is complete by seeing if the buffer has stabilized
            // We need a way to know when streaming is done - check thread status
            // For now, use a simple heuristic based on content
            let is_done = self.check_streaming_done();
            
            if is_done {
                self.finalize_streaming();
            } else {
                // Continue polling
                self.schedule_streaming_update();
            }
        }

        #[unsafe(method(toggleThinking:))]
        fn toggle_thinking(&self, _sender: Option<&NSObject>) {
            // Load config
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
            
            // Get active profile
            if let Some(conversation) = &*self.ivars().conversation.borrow() {
                let profile_id = conversation.profile_id;
                if let Ok(profile) = config.get_profile_mut(&profile_id) {
                    // Toggle show_thinking
                    let new_state = !profile.parameters.show_thinking;
                    profile.parameters.show_thinking = new_state;
                    
                    // Save config
                    if let Err(e) = config.save(&config_path) {
                        eprintln!("Failed to save config: {e}");
                    } else {
                        println!("Thinking display toggled to: {new_state}");
                        // Update button appearance
                        self.update_thinking_button_state();
                    }
                }
            }
        }

        #[unsafe(method(showHistory:))]
        fn show_history(&self, _sender: Option<&NSObject>) {
            println!("Show history clicked");
            // Post notification to show history view
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
            
            // Get the current profile ID (or use default)
            let config = Config::load(Config::default_path().unwrap()).unwrap_or_default();
            let profile_id = config.default_profile.unwrap_or_else(|| {
                config.profiles.first().map_or_else(Uuid::new_v4, |p| p.id)
            });
            
            log_to_file(&format!("Using profile_id: {profile_id}"));
            
            // Clear messages and create new conversation
            self.ivars().messages.borrow_mut().clear();
            let new_conversation = Conversation::new(profile_id);
            
            // Save the new conversation ID as active
            Self::save_active_conversation_id(new_conversation.id);
            
            *self.ivars().conversation.borrow_mut() = Some(new_conversation);
            
            log_to_file("Cleared messages, rebuilding view");
            self.rebuild_messages();
            self.update_title_and_model();
            log_to_file("New conversation setup complete");
        }

        #[unsafe(method(showSettings:))]
        fn show_settings(&self, _sender: Option<&NSObject>) {
            println!("Show settings clicked");
            // Post notification to show settings view
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowSettingsView");
            unsafe {
                center.postNotificationName_object(&name, None);
            }
        }
        
        #[unsafe(method(titleClicked:))]
        fn title_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("ChatView: Title clicked, entering edit mode");
            
            // Hide button, show edit field
            if let Some(title_button) = &*self.ivars().title_button.borrow() {
                title_button.setHidden(true);
            }
            if let Some(title_edit) = &*self.ivars().title_edit_field.borrow() {
                title_edit.setHidden(false);
                // Focus the edit field
                if let Some(window) = title_edit.window() {
                    window.makeFirstResponder(Some(title_edit));
                }
                // Select all text
                unsafe { title_edit.selectText(None); }
            }
        }
        
        #[unsafe(method(titleEditingEnded:))]
        fn title_editing_ended(&self, _sender: Option<&NSObject>) {
            log_to_file("ChatView: Title editing ended");
            
            // Get the new title
            let new_title = if let Some(title_edit) = &*self.ivars().title_edit_field.borrow() {
                title_edit.stringValue().to_string().trim().to_string()
            } else {
                String::new()
            };
            
            // If empty, revert to current button title
            let final_title = if new_title.is_empty() {
                if let Some(title_button) = &*self.ivars().title_button.borrow() {
                    title_button.title().to_string()
                } else {
                    Local::now().format("%Y%m%d%H%M%S").to_string()
                }
            } else {
                new_title
            };
            
            // Update the button title and show it
            if let Some(title_button) = &*self.ivars().title_button.borrow() {
                title_button.setTitle(&NSString::from_str(&final_title));
                title_button.setHidden(false);
            }
            // Hide the edit field and sync its value
            if let Some(title_edit) = &*self.ivars().title_edit_field.borrow() {
                title_edit.setHidden(true);
                title_edit.setStringValue(&NSString::from_str(&final_title));
            }
            
            // Update conversation title and save to disk
            if let Some(ref mut conv) = *self.ivars().conversation.borrow_mut() {
                conv.title = Some(final_title.clone());
                // Save the updated conversation
                if let Ok(storage) = ConversationStorage::with_default_path() {
                    if let Err(e) = storage.save(conv) {
                        log_to_file(&format!("ChatView: Failed to save conversation title: {e}"));
                    } else {
                        log_to_file(&format!("ChatView: Conversation saved with title: {final_title}"));
                    }
                }
            }
            
            log_to_file(&format!("ChatView: Title updated to: {final_title}"));
        }
    }
);

impl ChatViewController {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        // Load config and try to load the active conversation
        let config = Config::load(Config::default_path().unwrap()).unwrap_or_default();
        let profile_id = config.default_profile.unwrap_or_else(|| {
            config.profiles.first().map_or_else(Uuid::new_v4, |p| p.id)
        });
        
        // Try to load the active conversation from config, or fall back to most recent
        let conversation = if let Ok(storage) = ConversationStorage::with_default_path() {
            // First, try to load the active conversation from config
            if let Some(active_id) = config.active_conversation_id {
                log_to_file(&format!("ChatView: Trying to load active conversation: {active_id}"));
                // Find the file for this conversation ID
                if let Ok(filenames) = storage.list() {
                    for filename in &filenames {
                        if let Ok(conv) = storage.load(filename) {
                            if conv.id == active_id {
                                log_to_file(&format!("ChatView: Loaded active conversation: {:?}", conv.title));
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
                        log_to_file(&format!("ChatView: Loaded most recent conversation: {:?}", conv.title));
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
    
    fn create_with_conversation(mtm: MainThreadMarker, conversation: Conversation) -> Retained<Self> {
        let ivars = ChatViewIvars {
            messages: Rc::new(RefCell::new(Vec::new())),
            scroll_view: RefCell::new(None),
            messages_container: RefCell::new(None),
            input_field: RefCell::new(None),
            conversation: RefCell::new(Some(conversation)),
            title_button: RefCell::new(None),
            title_edit_field: RefCell::new(None),
            model_label: RefCell::new(None),
            thinking_button: RefCell::new(None),
            streaming_response: Arc::new(Mutex::new(String::new())),
            streaming_thinking: Arc::new(Mutex::new(String::new())),
            is_streaming: RefCell::new(false),
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
        self.rebuild_messages();
        self.update_title_and_model();
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

    fn build_top_bar_stack(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        // Create horizontal stack for top bar (fixed height 44px per wireframe)
        let top_bar = NSStackView::new(mtm);
        unsafe {
            top_bar.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            top_bar.setSpacing(8.0);
            top_bar.setTranslatesAutoresizingMaskIntoConstraints(false);
            top_bar.setDistribution(NSStackViewDistribution::Fill);
            top_bar.setEdgeInsets(objc2_foundation::NSEdgeInsets {
                top: 8.0,
                left: 12.0,
                bottom: 8.0,
                right: 12.0,
            });
        }
        
        top_bar.setWantsLayer(true);
        if let Some(layer) = top_bar.layer() {
            set_layer_background_color(&layer, Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2);
        }
        
        // CRITICAL: Set fixed height and high content hugging priority
        unsafe {
            top_bar.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            top_bar.setContentCompressionResistancePriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            let height_constraint = top_bar.heightAnchor().constraintEqualToConstant(44.0);
            height_constraint.setActive(true);
        }

        // Title: clickable button that shows conversation name (default: timestamp)
        // Generate default title as timestamp
        let default_title = Local::now().format("%Y%m%d%H%M%S").to_string();
        
        // Create a button that looks like a label for clicking
        let title_button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str(&default_title),
                Some(self),
                Some(sel!(titleClicked:)),
                mtm,
            )
        };
        title_button.setBezelStyle(NSBezelStyle::Inline);
        title_button.setBordered(false);
        unsafe {
            title_button.setTranslatesAutoresizingMaskIntoConstraints(false);
            title_button.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
        }
        // Style it to look like a label
        title_button.setFont(Some(&NSFont::boldSystemFontOfSize(13.0)));
        
        // Create editable title field (hidden by default, replaces button when editing)
        let title_edit = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(150.0, 22.0)),
        );
        title_edit.setStringValue(&NSString::from_str(&default_title));
        title_edit.setTextColor(Some(&Theme::text_primary()));
        title_edit.setBackgroundColor(Some(&Theme::bg_darker()));
        title_edit.setFont(Some(&NSFont::boldSystemFontOfSize(13.0)));
        title_edit.setDrawsBackground(true);
        title_edit.setBordered(true);
        title_edit.setEditable(true);
        title_edit.setSelectable(true);
        title_edit.setHidden(true);
        unsafe {
            title_edit.setTranslatesAutoresizingMaskIntoConstraints(false);
            title_edit.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
            // Set target/action for when editing ends (pressing Enter)
            title_edit.setTarget(Some(self));
            title_edit.setAction(Some(sel!(titleEditingEnded:)));
        }
        
        // Add button and edit field to top bar (edit field hidden initially)
        unsafe {
            top_bar.addArrangedSubview(&title_button);
            top_bar.addArrangedSubview(&title_edit);
        }
        *self.ivars().title_button.borrow_mut() = Some(title_button);
        *self.ivars().title_edit_field.borrow_mut() = Some(title_edit);
        
        // Spacer (flexible, low priority)
        let spacer = NSView::new(mtm);
        unsafe {
            spacer.setTranslatesAutoresizingMaskIntoConstraints(false);
            spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
        }
        unsafe {
            top_bar.addArrangedSubview(&spacer);
        }
        
        // Icon buttons: T, H, +, Gear (28x28 each per wireframe)
        let button_configs: &[(&str, objc2::runtime::Sel)] = &[
            ("T", sel!(toggleThinking:)),
            ("H", sel!(showHistory:)),
            ("+", sel!(newConversation:)),
        ];

        for &(label, action) in button_configs {
            let btn = self.create_icon_button_for_stack(label, action, mtm);
            
            // Store reference to thinking button so we can update its appearance
            if label == "T" {
                *self.ivars().thinking_button.borrow_mut() = Some(btn.clone());
                self.update_thinking_button_state();
            }
            
            unsafe {
                top_bar.addArrangedSubview(&btn);
            }
        }

        // Settings gear icon button (using "G" as text placeholder)
        let gear_btn = self.create_gear_button_for_stack(mtm);
        unsafe {
            top_bar.addArrangedSubview(&gear_btn);
        }

        Retained::from(&*top_bar as &NSView)
    }

    fn create_icon_button_for_stack(
        &self,
        label: &str,
        action: objc2::runtime::Sel,
        mtm: MainThreadMarker,
    ) -> Retained<NSButton> {
        let btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str(label),
                Some(self),
                Some(action),
                mtm,
            )
        };
        btn.setBordered(false);
        btn.setWantsLayer(true);
        if let Some(layer) = btn.layer() {
            set_layer_background_color(&layer, Theme::BG_DARKER.0, Theme::BG_DARKER.1, Theme::BG_DARKER.2);
            set_layer_corner_radius(&layer, 6.0);
        }
        
        // Set fixed size constraints
        unsafe {
            btn.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = btn.widthAnchor().constraintEqualToConstant(28.0);
            let height_constraint = btn.heightAnchor().constraintEqualToConstant(28.0);
            width_constraint.setActive(true);
            height_constraint.setActive(true);
        }
        
        btn
    }

    fn create_gear_button_for_stack(&self, mtm: MainThreadMarker) -> Retained<NSButton> {
        // Create a button with the gear text
        let btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("G"),
                Some(self),
                Some(sel!(showSettings:)),
                mtm,
            )
        };
        btn.setBordered(false);
        btn.setWantsLayer(true);
        if let Some(layer) = btn.layer() {
            set_layer_background_color(&layer, Theme::BG_DARKER.0, Theme::BG_DARKER.1, Theme::BG_DARKER.2);
            set_layer_corner_radius(&layer, 6.0);
        }
        
        // Set fixed size constraints
        unsafe {
            btn.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = btn.widthAnchor().constraintEqualToConstant(28.0);
            let height_constraint = btn.heightAnchor().constraintEqualToConstant(28.0);
            width_constraint.setActive(true);
            height_constraint.setActive(true);
        }
        
        btn
    }

    fn build_chat_area_stack(&self, mtm: MainThreadMarker) -> Retained<NSScrollView> {
        // Create scroll view (flexible - takes remaining space)
        let scroll_view = NSScrollView::new(mtm);
        scroll_view.setHasVerticalScroller(true);
        scroll_view.setDrawsBackground(false);
        unsafe {
            scroll_view.setAutohidesScrollers(true);
            scroll_view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        
        // CRITICAL: Set low content hugging priority so it expands to fill space
        unsafe {
            scroll_view.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
            scroll_view.setContentCompressionResistancePriority_forOrientation(250.0, NSLayoutConstraintOrientation::Vertical);
            
            // Add minimum height constraint to prevent collapse
            let min_height = scroll_view.heightAnchor().constraintGreaterThanOrEqualToConstant(100.0);
            min_height.setActive(true);
        }

        // Create vertical stack for messages inside scroll view
        let messages_stack = NSStackView::new(mtm);
        unsafe {
            messages_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            messages_stack.setSpacing(12.0);
            messages_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
            messages_stack.setDistribution(NSStackViewDistribution::Fill);
        }
        
        messages_stack.setWantsLayer(true);
        if let Some(layer) = messages_stack.layer() {
            set_layer_background_color(&layer, Theme::BG_DARKEST.0, Theme::BG_DARKEST.1, Theme::BG_DARKEST.2);
        }
        
        // CRITICAL: Set translatesAutoresizingMaskIntoConstraints to false for proper Auto Layout
        messages_stack.setTranslatesAutoresizingMaskIntoConstraints(false);

        scroll_view.setDocumentView(Some(&messages_stack));
        
        // CRITICAL: Constrain messages_stack width to scroll view's content width
        // This is required for the stack to know its width and lay out content properly
        let content_view = scroll_view.contentView();
        let width_constraint = messages_stack.widthAnchor().constraintEqualToAnchor(&content_view.widthAnchor());
        width_constraint.setActive(true);

        // Store references
        *self.ivars().scroll_view.borrow_mut() = Some(scroll_view.clone());
        *self.ivars().messages_container.borrow_mut() = Some(Retained::from(&*messages_stack as &NSView));

        scroll_view
    }

    fn build_input_area_stack(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        // Create horizontal stack for input area (fixed height 50px per wireframe)
        let input_stack = NSStackView::new(mtm);
        unsafe {
            input_stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            input_stack.setSpacing(8.0);
            input_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
            input_stack.setDistribution(NSStackViewDistribution::Fill);
            input_stack.setEdgeInsets(objc2_foundation::NSEdgeInsets {
                top: 10.0,
                left: 12.0,
                bottom: 10.0,
                right: 12.0,
            });
        }
        
        input_stack.setWantsLayer(true);
        if let Some(layer) = input_stack.layer() {
            set_layer_background_color(&layer, Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2);
        }
        
        // CRITICAL: Set fixed height and high content hugging priority
        unsafe {
            input_stack.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            input_stack.setContentCompressionResistancePriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            let height_constraint = input_stack.heightAnchor().constraintEqualToConstant(50.0);
            height_constraint.setActive(true);
        }

        // Input field (flexible width, low hugging priority)
        let input = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(300.0, 30.0)),
        );
        input.setPlaceholderString(Some(&NSString::from_str("Type a message...")));
        input.setBackgroundColor(Some(&Theme::bg_darker()));
        input.setTextColor(Some(&Theme::text_primary()));
        input.setDrawsBackground(true);
        input.setBordered(true);
        input.setFont(Some(&NSFont::systemFontOfSize(13.0)));
        
        // Set up action for Enter key
        unsafe {
            input.setTarget(Some(self));
            input.setAction(Some(sel!(sendMessage:)));
            
            // Low horizontal hugging so it expands
            input.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
        }
        
        unsafe {
            input_stack.addArrangedSubview(&input);
        }

        // Send button (fixed width >=60 per wireframe, high hugging priority)
        let send_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Send"),
                Some(self),
                Some(sel!(sendMessage:)),
                mtm,
            )
        };
        send_btn.setBezelStyle(NSBezelStyle::Rounded);
        
        // Set fixed width constraint and high hugging priority
        unsafe {
            send_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
            send_btn.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
            let width_constraint = send_btn.widthAnchor().constraintGreaterThanOrEqualToConstant(60.0);
            width_constraint.setActive(true);
        }
        
        unsafe {
            input_stack.addArrangedSubview(&send_btn);
        }

        // Store input reference
        *self.ivars().input_field.borrow_mut() = Some(input);
        
        Retained::from(&*input_stack as &NSView)
    }

    fn load_initial_messages(&self) {
        // Load messages from the current conversation into the message store
        if let Some(conversation) = &*self.ivars().conversation.borrow() {
            log_to_file(&format!("Loading {} messages from conversation", conversation.messages.len()));
            for msg in &conversation.messages {
                let is_user = matches!(msg.role, personal_agent::models::MessageRole::User);
                self.add_message_to_store(&msg.content, is_user);
            }
        }
        self.rebuild_messages();
        self.update_title_and_model();
    }

    fn add_message_to_store(&self, text: &str, is_user: bool) {
        self.ivars().messages.borrow_mut().push(Message {
            text: text.to_string(),
            is_user,
        });
    }
    
    fn schedule_streaming_update(&self) {
        // Use performSelector:withObject:afterDelay: to schedule UI update on main thread
        // This is a simple polling approach - update every 100ms
        unsafe {
            let delay: f64 = 0.1; // 100ms
            let _: () = msg_send![self, performSelector:sel!(checkStreamingStatus:) withObject:std::ptr::null::<NSObject>() afterDelay:delay];
        }
    }
    
fn check_streaming_done(&self) -> bool {
        // Check if the streaming thread has finished
        // We use a special marker that the streaming thread sets when complete
        if let Ok(buf) = self.ivars().streaming_response.lock() {
            // Check for error markers or completion marker
            buf.contains("[Error:") || buf.ends_with("␄") // EOT marker
        } else {
            true // Assume done if we can't lock
        }
    }
    
    fn finalize_streaming(&self) {
        log_to_file("Finalizing streaming response");
        
        // Get the final text and remove the EOT marker
        let final_text = if let Ok(buf) = self.ivars().streaming_response.lock() {
            buf.trim_end_matches('␄').to_string()
        } else {
            "[Error: Failed to get response]".to_string()
        };
        
        // Get the thinking text
        let thinking_text = if let Ok(buf) = self.ivars().streaming_thinking.lock() {
            let t = buf.clone();
            if t.is_empty() { None } else { Some(t) }
        } else {
            None
        };
        
        // Build display text with thinking if enabled
        let show_thinking = self.should_show_thinking();
        let display_text = match (&thinking_text, show_thinking) {
            (Some(thinking), true) => format!("*Thinking:*
{thinking}

---

{final_text}"),
            _ => final_text.clone(),
        };
        
        // Update the last message
        if let Some(last_msg) = self.ivars().messages.borrow_mut().last_mut() {
            if !last_msg.is_user {
                last_msg.text = display_text;
            }
        }
        
        // Create message with thinking content
        let mut assistant_msg = ConvMessage::assistant(final_text);
        assistant_msg.thinking_content = thinking_text;
        
        // Add to conversation and save
        if let Some(ref mut conv) = *self.ivars().conversation.borrow_mut() {
            conv.add_message(assistant_msg);
        }
        
        // Save conversation
        if let Some(ref conversation) = *self.ivars().conversation.borrow() {
            if let Ok(storage) = ConversationStorage::with_default_path() {
                if let Err(e) = storage.save(conversation) {
                    log_to_file(&format!("Failed to save conversation: {e}"));
                } else {
                    log_to_file("Conversation saved successfully");
                }
            }
        }
        
        // Mark streaming as done
        *self.ivars().is_streaming.borrow_mut() = false;
        
        // Final rebuild
        self.rebuild_messages();
    }

    fn rebuild_messages(&self) {
        let mtm = MainThreadMarker::new().unwrap();
        
        let message_count = self.ivars().messages.borrow().len();
        log_to_file(&format!("rebuild_messages called, {message_count} messages in store"));
        
        if let Some(container) = &*self.ivars().messages_container.borrow() {
            log_to_file("Container found, clearing old views");
            
            // Clear existing subviews (for stack view, remove arranged subviews)
            let subviews = container.subviews();
            log_to_file(&format!("Removing {} existing subviews", subviews.len()));
            for view in &subviews {
                // Check if container is a stack view
                if let Some(stack) = container.downcast_ref::<NSStackView>() {
                    unsafe {
                        stack.removeArrangedSubview(&view);
                    }
                }
                view.removeFromSuperview();
            }

            let messages = self.ivars().messages.borrow();
            
            // For stack view, just add message views - stack handles positioning
            if let Some(stack) = container.downcast_ref::<NSStackView>() {
                log_to_file(&format!("Adding {} message bubbles to stack", messages.len()));
                for msg in messages.iter() {
                    let msg_view = self.create_message_bubble(&msg.text, msg.is_user, mtm);
                    unsafe {
                        stack.addArrangedSubview(&msg_view);
                    }
                }
                log_to_file("All message bubbles added");
            } else {
                log_to_file("ERROR: Container is not an NSStackView!");
            }
        } else {
            log_to_file("ERROR: No messages_container reference!");
        }
    }
    
    fn create_message_bubble(
        &self,
        text: &str,
        is_user: bool,
        mtm: MainThreadMarker,
    ) -> Retained<NSView> {
        // Create horizontal container for alignment
        let row = NSStackView::new(mtm);
        row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        row.setSpacing(0.0);
        row.setTranslatesAutoresizingMaskIntoConstraints(false);
        row.setDistribution(NSStackViewDistribution::Fill);
        
        // Max bubble width: 300 per wireframe
        let max_width = 300.0;
        
        // Create message bubble as a stack view (gets intrinsic size from content)
        let bubble = NSStackView::new(mtm);
        bubble.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        bubble.setSpacing(0.0);
        bubble.setTranslatesAutoresizingMaskIntoConstraints(false);
        bubble.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: 10.0, left: 10.0, bottom: 10.0, right: 10.0
        });
        
        bubble.setWantsLayer(true);
        if let Some(layer) = bubble.layer() {
            if is_user {
                // User messages: green-tinted background #2a4a2a
                set_layer_background_color(&layer, 42.0 / 255.0, 74.0 / 255.0, 42.0 / 255.0);
            } else {
                // Assistant messages: dark gray #1a1a1a
                set_layer_background_color(&layer, Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2);
            }
            set_layer_corner_radius(&layer, 12.0);
        }
        
        // Create wrapping label - this provides the intrinsic content size
        let label = NSTextField::wrappingLabelWithString(&NSString::from_str(text), mtm);
        label.setFont(Some(&NSFont::systemFontOfSize(13.0)));
        label.setTextColor(Some(&Theme::text_primary()));
        label.setTranslatesAutoresizingMaskIntoConstraints(false);
        
        // Constrain label width for wrapping
        let label_width = label.widthAnchor().constraintLessThanOrEqualToConstant(max_width - 20.0);
        label_width.setActive(true);
        
        bubble.addArrangedSubview(&label);
        
        // Constrain bubble max width
        let bubble_width = bubble.widthAnchor().constraintLessThanOrEqualToConstant(max_width);
        bubble_width.setActive(true);
        
        // Add spacer and bubble to row based on alignment
        let spacer = NSView::new(mtm);
        spacer.setTranslatesAutoresizingMaskIntoConstraints(false);
        spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
        
        if is_user {
            // User messages: right-aligned
            row.addArrangedSubview(&spacer);
            row.addArrangedSubview(&bubble);
        } else {
            // Assistant messages: left-aligned
            row.addArrangedSubview(&bubble);
            row.addArrangedSubview(&spacer);
        }

        Retained::from(&*row as &NSView)
    }
    
    fn update_title_and_model(&self) {
        // Load config and get active profile
        let config = Config::load(Config::default_path().unwrap()).unwrap_or_default();
        
        if let Some(conversation) = &*self.ivars().conversation.borrow() {
            if let Ok(profile) = config.get_profile(&conversation.profile_id) {
                // Update title button
                if let Some(title_button) = &*self.ivars().title_button.borrow() {
                    let title_text = conversation.title.clone()
                        .unwrap_or_else(|| Local::now().format("%Y%m%d%H%M%S").to_string());
                    title_button.setTitle(&NSString::from_str(&title_text));
                }
                
                // Update model label
                if let Some(model_label) = &*self.ivars().model_label.borrow() {
                    let model_text = format!("{} - {}", profile.name, profile.model_id);
                    model_label.setStringValue(&NSString::from_str(&model_text));
                }
            }
        }
    }
    
    fn update_thinking_button_state(&self) {
        // Load config and get active profile
        let config = Config::load(Config::default_path().unwrap()).unwrap_or_default();
        
        if let Some(conversation) = &*self.ivars().conversation.borrow() {
            if let Ok(profile) = config.get_profile(&conversation.profile_id) {
                if let Some(button) = &*self.ivars().thinking_button.borrow() {
                    // Update button appearance based on show_thinking state
                    // For now, change the title to show state (T vs T*)
                    let label = if profile.parameters.show_thinking {
                        "T*"
                    } else {
                        "T"
                    };
                    button.setTitle(&NSString::from_str(label));
                }
            }
        }
    }
    
    /// Check if thinking should be shown based on profile settings
    fn should_show_thinking(&self) -> bool {
        let config = Config::load(Config::default_path().unwrap()).unwrap_or_default();
        
        if let Some(conversation) = &*self.ivars().conversation.borrow() {
            if let Ok(profile) = config.get_profile(&conversation.profile_id) {
                return profile.parameters.show_thinking;
            }
        }
        false
    }
}
