//! Chat view implementation using simple absolute positioning (no NSStackView)
//!
//! Layout (400x500):
//! - Top bar: y=452, height=48 (452-500)
//! - Chat area: y=60, height=392 (60-452) 
//! - Input area: y=0, height=60 (0-60)

use std::cell::RefCell;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly, DefinedClass};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use objc2_app_kit::{
    NSView, NSViewController, NSTextField, NSButton, NSScrollView, NSClipView,
    NSFont, NSBezelStyle, NSColor,
};
use objc2_quartz_core::CALayer;
use uuid::Uuid;

use super::theme::Theme;
use personal_agent::config::Config;
use personal_agent::models::{Conversation, Message as ConvMessage};
use personal_agent::storage::ConversationStorage;

// ============================================================================
// Constants for layout
// ============================================================================
const WIDTH: f64 = 400.0;
const HEIGHT: f64 = 500.0;
const TOP_BAR_HEIGHT: f64 = 48.0;
const INPUT_HEIGHT: f64 = 60.0;
const CHAT_AREA_HEIGHT: f64 = HEIGHT - TOP_BAR_HEIGHT - INPUT_HEIGHT; // 392

// ============================================================================
// Message data structure
// ============================================================================

#[derive(Clone, Debug)]
pub struct Message {
    pub text: String,
    pub is_user: bool,
}

type MessageStore = Rc<RefCell<Vec<Message>>>;

// ============================================================================
// Helper functions
// ============================================================================

fn set_layer_background_color(layer: &CALayer, r: f64, g: f64, b: f64) {
    use objc2_core_graphics::CGColor;
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
    messages_container: RefCell<Option<Retained<NSView>>>,
    input_field: RefCell<Option<Retained<NSTextField>>>,
    conversation: RefCell<Option<Conversation>>,
    title_label: RefCell<Option<Retained<NSTextField>>>,
    model_label: RefCell<Option<Retained<NSTextField>>>,
    thinking_button: RefCell<Option<Retained<NSButton>>>,
}

// ============================================================================
// ChatViewController
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
            let mtm = MainThreadMarker::new().unwrap();

            // Main container
            let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, HEIGHT));
            let main_view = NSView::initWithFrame(NSView::alloc(mtm), frame);
            main_view.setWantsLayer(true);
            if let Some(layer) = main_view.layer() {
                set_layer_background_color(&layer, Theme::BG_DARKEST.0, Theme::BG_DARKEST.1, Theme::BG_DARKEST.2);
            }

            // Build sections
            self.build_top_bar(mtm, &main_view);
            self.build_chat_area(mtm, &main_view);
            self.build_input_area(mtm, &main_view);

            self.setView(&main_view);
            
            // Initialize with sample messages
            self.add_sample_messages();
            
            // Update title from config
            self.update_title_and_model();
        }

        #[unsafe(method(sendMessage:))]
        fn send_message(&self, _sender: Option<&NSObject>) {
            if let Some(input) = &*self.ivars().input_field.borrow() {
                let text = input.stringValue();
                let text_str = text.to_string();
                
                if !text_str.trim().is_empty() {
                    self.add_message_to_store(&text_str, true);
                    input.setStringValue(&NSString::new());
                    
                    if let Some(ref mut conversation) = *self.ivars().conversation.borrow_mut() {
                        conversation.add_message(ConvMessage::user(text_str.clone()));
                    }
                    
                    // Placeholder response
                    self.add_message_to_store("[Thinking... async LLM pending]", false);
                    self.rebuild_messages();
                    
                    // Save
                    if let Some(ref conversation) = *self.ivars().conversation.borrow() {
                        if let Ok(storage) = ConversationStorage::with_default_path() {
                            let _ = storage.save(conversation);
                        }
                    }
                }
            }
        }

        #[unsafe(method(toggleThinking:))]
        fn toggle_thinking(&self, _sender: Option<&NSObject>) {
            println!("Toggle thinking clicked");
        }

        #[unsafe(method(saveConversation:))]
        fn save_conversation(&self, _sender: Option<&NSObject>) {
            println!("Save conversation clicked");
        }

        #[unsafe(method(showHistory:))]
        fn show_history(&self, _sender: Option<&NSObject>) {
            unsafe {
                let center = objc2_foundation::NSNotificationCenter::defaultCenter();
                center.postNotificationName_object(
                    &NSString::from_str("PersonalAgentShowHistory"),
                    None,
                );
            }
        }

        #[unsafe(method(newConversation:))]
        fn new_conversation(&self, _sender: Option<&NSObject>) {
            self.ivars().messages.borrow_mut().clear();
            *self.ivars().conversation.borrow_mut() = Some(Conversation::new());
            self.rebuild_messages();
        }

        #[unsafe(method(showSettings:))]
        fn show_settings(&self, _sender: Option<&NSObject>) {
            unsafe {
                let center = objc2_foundation::NSNotificationCenter::defaultCenter();
                center.postNotificationName_object(
                    &NSString::from_str("PersonalAgentShowSettings"),
                    None,
                );
            }
        }
    }
);

impl ChatViewController {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(ChatViewIvars {
            messages: Rc::new(RefCell::new(Vec::new())),
            messages_container: RefCell::new(None),
            input_field: RefCell::new(None),
            conversation: RefCell::new(Some(Conversation::new())),
            title_label: RefCell::new(None),
            model_label: RefCell::new(None),
            thinking_button: RefCell::new(None),
        });
        unsafe { msg_send![super(this), init] }
    }

    fn build_top_bar(&self, mtm: MainThreadMarker, parent: &NSView) {
        // Top bar: y = HEIGHT - TOP_BAR_HEIGHT = 452
        let top_bar = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, HEIGHT - TOP_BAR_HEIGHT), NSSize::new(WIDTH, TOP_BAR_HEIGHT)),
        );
        top_bar.setWantsLayer(true);
        if let Some(layer) = top_bar.layer() {
            set_layer_background_color(&layer, Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2);
        }

        // Title label
        let title = NSTextField::labelWithString(&NSString::from_str("New Conversation"), mtm);
        title.setTextColor(Some(&Theme::text_primary()));
        title.setFont(Some(&NSFont::boldSystemFontOfSize(13.0)));
        title.setFrame(NSRect::new(NSPoint::new(12.0, 24.0), NSSize::new(200.0, 18.0)));
        top_bar.addSubview(&title);
        *self.ivars().title_label.borrow_mut() = Some(title);

        // Model label
        let model = NSTextField::labelWithString(&NSString::from_str("Loading..."), mtm);
        model.setTextColor(Some(&Theme::text_secondary_color()));
        model.setFont(Some(&NSFont::systemFontOfSize(10.0)));
        model.setFrame(NSRect::new(NSPoint::new(12.0, 8.0), NSSize::new(200.0, 14.0)));
        top_bar.addSubview(&model);
        *self.ivars().model_label.borrow_mut() = Some(model);

        // Buttons on right side
        let buttons = ["T", "S", "H", "+", "G"];
        let actions = [
            sel!(toggleThinking:),
            sel!(saveConversation:),
            sel!(showHistory:),
            sel!(newConversation:),
            sel!(showSettings:),
        ];
        
        for (i, (label, action)) in buttons.iter().zip(actions.iter()).enumerate() {
            let btn = unsafe {
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str(label),
                    Some(self),
                    Some(*action),
                    mtm,
                )
            };
            btn.setBordered(false);
            btn.setWantsLayer(true);
            if let Some(layer) = btn.layer() {
                set_layer_background_color(&layer, Theme::BG_MEDIUM.0, Theme::BG_MEDIUM.1, Theme::BG_MEDIUM.2);
                set_layer_corner_radius(&layer, 4.0);
            }
            // Position from right: start at WIDTH - 40, go left by 32 per button
            let x = WIDTH - 40.0 - (i as f64 * 32.0);
            btn.setFrame(NSRect::new(NSPoint::new(x, 10.0), NSSize::new(28.0, 28.0)));
            top_bar.addSubview(&btn);
            
            if *label == "T" {
                *self.ivars().thinking_button.borrow_mut() = Some(btn);
            }
        }

        parent.addSubview(&top_bar);
    }

    fn build_chat_area(&self, mtm: MainThreadMarker, parent: &NSView) {
        // Chat area: y = INPUT_HEIGHT = 60, height = CHAT_AREA_HEIGHT = 392
        let scroll_view = NSScrollView::initWithFrame(
            NSScrollView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, INPUT_HEIGHT), NSSize::new(WIDTH, CHAT_AREA_HEIGHT)),
        );
        scroll_view.setHasVerticalScroller(true);
        scroll_view.setDrawsBackground(false);
        unsafe { scroll_view.setAutohidesScrollers(true); }

        // Content view for messages
        let content = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH - 20.0, CHAT_AREA_HEIGHT)),
        );
        content.setWantsLayer(true);
        if let Some(layer) = content.layer() {
            set_layer_background_color(&layer, Theme::BG_DARKEST.0, Theme::BG_DARKEST.1, Theme::BG_DARKEST.2);
        }

        scroll_view.setDocumentView(Some(&content));
        *self.ivars().messages_container.borrow_mut() = Some(content);

        parent.addSubview(&scroll_view);
    }

    fn build_input_area(&self, mtm: MainThreadMarker, parent: &NSView) {
        // Input area: y = 0, height = INPUT_HEIGHT = 60
        let input_area = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, INPUT_HEIGHT)),
        );
        input_area.setWantsLayer(true);
        if let Some(layer) = input_area.layer() {
            set_layer_background_color(&layer, Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2);
        }

        // Text field
        let input = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(12.0, 15.0), NSSize::new(WIDTH - 90.0, 30.0)),
        );
        input.setPlaceholderString(Some(&NSString::from_str("Type a message...")));
        input.setBackgroundColor(Some(&Theme::bg_darker()));
        input.setTextColor(Some(&Theme::text_primary()));
        input.setDrawsBackground(true);
        input.setBordered(true);
        input.setFont(Some(&NSFont::systemFontOfSize(13.0)));
        unsafe {
            input.setTarget(Some(self));
            input.setAction(Some(sel!(sendMessage:)));
        }
        input_area.addSubview(&input);
        *self.ivars().input_field.borrow_mut() = Some(input);

        // Send button
        let send_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Send"),
                Some(self),
                Some(sel!(sendMessage:)),
                mtm,
            )
        };
        send_btn.setBezelStyle(NSBezelStyle::Rounded);
        send_btn.setFrame(NSRect::new(NSPoint::new(WIDTH - 70.0, 15.0), NSSize::new(58.0, 30.0)));
        input_area.addSubview(&send_btn);

        parent.addSubview(&input_area);
    }

    fn add_message_to_store(&self, text: &str, is_user: bool) {
        self.ivars().messages.borrow_mut().push(Message {
            text: text.to_string(),
            is_user,
        });
    }

    fn add_sample_messages(&self) {
        self.add_message_to_store("Hello! How can I help you today?", false);
    }

    fn rebuild_messages(&self) {
        let container = self.ivars().messages_container.borrow();
        let Some(container) = container.as_ref() else { return };

        // Remove existing message views
        for subview in container.subviews().to_vec() {
            subview.removeFromSuperview();
        }

        let messages = self.ivars().messages.borrow();
        let mtm = MainThreadMarker::new().unwrap();
        
        // Calculate total height needed
        let msg_height = 40.0;
        let spacing = 8.0;
        let total_height = (messages.len() as f64) * (msg_height + spacing);
        let container_height = total_height.max(CHAT_AREA_HEIGHT);
        
        // Resize container
        container.setFrameSize(NSSize::new(WIDTH - 20.0, container_height));

        // Add messages from top down (newest at bottom in scroll view)
        for (i, msg) in messages.iter().enumerate() {
            let y = container_height - ((i + 1) as f64 * (msg_height + spacing));
            let x = if msg.is_user { 100.0 } else { 10.0 };
            let width = WIDTH - 120.0;
            
            let label = NSTextField::wrappingLabelWithString(&NSString::from_str(&msg.text), mtm);
            label.setFrame(NSRect::new(NSPoint::new(x, y), NSSize::new(width, msg_height)));
            label.setTextColor(Some(&Theme::text_primary()));
            label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
            label.setDrawsBackground(true);
            
            if msg.is_user {
                label.setBackgroundColor(Some(&Theme::bg_light()));
            } else {
                label.setBackgroundColor(Some(&Theme::bg_dark()));
            }
            
            label.setWantsLayer(true);
            if let Some(layer) = label.layer() {
                set_layer_corner_radius(&layer, 8.0);
            }
            
            container.addSubview(&label);
        }
    }

    fn update_title_and_model(&self) {
        // Try to load profile info from config
        if let Ok(config_path) = Config::default_path() {
            if let Ok(config) = Config::load(&config_path) {
                if let Some(profile) = config.get_active_profile() {
                    if let Some(label) = &*self.ivars().model_label.borrow() {
                        let model_text = format!("{} - {}", profile.name, profile.model_id);
                        label.setStringValue(&NSString::from_str(&model_text));
                    }
                }
            }
        }
    }

    pub fn load_conversation(&self, conversation: Conversation) {
        // Clear current messages
        self.ivars().messages.borrow_mut().clear();
        
        // Load messages from conversation
        for msg in &conversation.messages {
            let is_user = matches!(msg.role, personal_agent::models::MessageRole::User);
            self.add_message_to_store(&msg.content, is_user);
        }
        
        *self.ivars().conversation.borrow_mut() = Some(conversation);
        self.rebuild_messages();
        self.update_title_and_model();
    }
}
