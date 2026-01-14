//! Chat view implementation for the popover

use std::cell::RefCell;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly, DefinedClass};
use objc2_foundation::{
    NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use objc2_app_kit::{
    NSView, NSViewController, NSTextField, NSButton, NSScrollView,
    NSFont, NSBezelStyle, NSLineBreakMode, NSStackView, NSUserInterfaceLayoutOrientation,
    NSLayoutConstraintOrientation, NSStackViewDistribution, NSLayoutPriority,
};
use objc2_quartz_core::CALayer;
use uuid::Uuid;

use super::theme::Theme;
use personal_agent::config::Config;
use personal_agent::models::{Conversation, Message as ConvMessage};
use personal_agent::storage::ConversationStorage;

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
    title_label: RefCell<Option<Retained<NSTextField>>>,
    model_label: RefCell<Option<Retained<NSTextField>>>,
    thinking_button: RefCell<Option<Retained<NSButton>>>,
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
            self.add_sample_messages();
            
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
            if let Some(input) = &*self.ivars().input_field.borrow() {
                let text = input.stringValue();
                let text_str = text.to_string();
                
                if !text_str.trim().is_empty() {
                    // Add user message to UI
                    self.add_message_to_store(&text_str, true);
                    
                    // Clear input
                    input.setStringValue(&NSString::new());
                    
                    // Add user message to conversation
                    if let Some(ref mut conversation) = *self.ivars().conversation.borrow_mut() {
                        conversation.add_message(ConvMessage::user(text_str.clone()));
                    }
                    
                    // Show "Thinking..." placeholder
                    // NOTE: Async streaming integration needed for Phase 4
                    // For now, this is a synchronous placeholder
                    let thinking_msg = "[Thinking... (async LLM integration pending)]";
                    self.add_message_to_store(thinking_msg, false);
                    
                    // Rebuild chat area to show new messages
                    self.rebuild_messages();
                    
                    // Save the user message to storage
                    if let Some(ref conversation) = *self.ivars().conversation.borrow() {
                        if let Ok(storage) = ConversationStorage::with_default_path() {
                            if let Err(e) = storage.save(conversation) {
                                eprintln!("Failed to save conversation: {}", e);
                            }
                        }
                    }
                }
            }
        }

        #[unsafe(method(toggleThinking:))]
        fn toggle_thinking(&self, _sender: Option<&NSObject>) {
            // Load config
            let config_path = match Config::default_path() {
                Ok(path) => path,
                Err(e) => {
                    eprintln!("Failed to get config path: {}", e);
                    return;
                }
            };
            
            let mut config = match Config::load(&config_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to load config: {}", e);
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
                        eprintln!("Failed to save config: {}", e);
                    } else {
                        println!("Thinking display toggled to: {}", new_state);
                        // Update button appearance
                        self.update_thinking_button_state();
                    }
                }
            }
        }

        #[unsafe(method(saveConversation:))]
        fn save_conversation(&self, _sender: Option<&NSObject>) {
            // Get current conversation
            if let Some(ref conversation) = *self.ivars().conversation.borrow() {
                // Save to storage
                match ConversationStorage::with_default_path() {
                    Ok(storage) => {
                        match storage.save(conversation) {
                            Ok(()) => {
                                println!("Conversation saved successfully");
                                // Flash the button to show success (visual feedback)
                                // TODO: Add visual feedback animation
                            }
                            Err(e) => {
                                eprintln!("Failed to save conversation: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to create storage: {}", e);
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
            println!("New conversation clicked");
            
            // Get the current profile ID (or use default)
            let config = Config::load(Config::default_path().unwrap()).unwrap_or_default();
            let profile_id = config.default_profile.unwrap_or_else(|| {
                config.profiles.first().map(|p| p.id).unwrap_or_else(Uuid::new_v4)
            });
            
            // Clear messages and create new conversation
            self.ivars().messages.borrow_mut().clear();
            let new_conversation = Conversation::new(profile_id);
            *self.ivars().conversation.borrow_mut() = Some(new_conversation);
            
            self.rebuild_messages();
            self.update_title_and_model();
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
    }
);

impl ChatViewController {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        // Load config and create conversation with active profile
        let config = Config::load(Config::default_path().unwrap()).unwrap_or_default();
        let profile_id = config.default_profile.unwrap_or_else(|| {
            config.profiles.first().map(|p| p.id).unwrap_or_else(Uuid::new_v4)
        });
        let conversation = Conversation::new(profile_id);
        
        let ivars = ChatViewIvars {
            messages: Rc::new(RefCell::new(Vec::new())),
            scroll_view: RefCell::new(None),
            messages_container: RefCell::new(None),
            input_field: RefCell::new(None),
            conversation: RefCell::new(Some(conversation)),
            title_label: RefCell::new(None),
            model_label: RefCell::new(None),
            thinking_button: RefCell::new(None),
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
        
        // Store the conversation
        *self.ivars().conversation.borrow_mut() = Some(conversation);
        
        // Rebuild the UI
        self.rebuild_messages();
        self.update_title_and_model();
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

        // Icon: 24x24 red eye icon (using text "I" as placeholder)
        let icon = NSTextField::labelWithString(&NSString::from_str("I"), mtm);
        icon.setTextColor(Some(&Theme::text_primary()));
        icon.setFont(Some(&NSFont::boldSystemFontOfSize(18.0)));
        unsafe {
            icon.setTranslatesAutoresizingMaskIntoConstraints(false);
            let width_constraint = icon.widthAnchor().constraintEqualToConstant(24.0);
            let height_constraint = icon.heightAnchor().constraintEqualToConstant(24.0);
            width_constraint.setActive(true);
            height_constraint.setActive(true);
            icon.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
        }
        unsafe {
            top_bar.addArrangedSubview(&icon);
        }
        
        // Title: "PersonalAgent"
        let title = NSTextField::labelWithString(&NSString::from_str("PersonalAgent"), mtm);
        title.setTextColor(Some(&Theme::text_primary()));
        title.setFont(Some(&NSFont::boldSystemFontOfSize(13.0)));
        unsafe {
            title.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
        }
        unsafe {
            top_bar.addArrangedSubview(&title);
        }
        *self.ivars().title_label.borrow_mut() = Some(title);
        
        // Spacer (flexible, low priority)
        let spacer = NSView::new(mtm);
        unsafe {
            spacer.setTranslatesAutoresizingMaskIntoConstraints(false);
            spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
        }
        unsafe {
            top_bar.addArrangedSubview(&spacer);
        }
        
        // Icon buttons: T, S, H, +, Gear (28x28 each per wireframe)
        let button_configs: &[(&str, objc2::runtime::Sel)] = &[
            ("T", sel!(toggleThinking:)),
            ("S", sel!(saveConversation:)),
            ("H", sel!(showHistory:)),
            ("+", sel!(newConversation:)),
        ];

        for &(label, action) in button_configs.iter() {
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

        scroll_view.setDocumentView(Some(&messages_stack));

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

    fn add_sample_messages(&self) {
        self.add_message_to_store("How do I create a menu bar app in Rust?", true);
        self.add_message_to_store(
            "To create a menu bar app in Rust for macOS, you'll want to use native Cocoa bindings via objc2.",
            false,
        );
        self.add_message_to_store("Show me an example", true);
        
        self.rebuild_messages();
    }

    fn add_message_to_store(&self, text: &str, is_user: bool) {
        self.ivars().messages.borrow_mut().push(Message {
            text: text.to_string(),
            is_user,
        });
    }

    fn rebuild_messages(&self) {
        let mtm = MainThreadMarker::new().unwrap();
        
        if let Some(container) = &*self.ivars().messages_container.borrow() {
            // Clear existing subviews (for stack view, remove arranged subviews)
            let subviews = container.subviews();
            for view in subviews.iter() {
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
                for msg in messages.iter() {
                    let msg_view = self.create_message_bubble(&msg.text, msg.is_user, mtm);
                    unsafe {
                        stack.addArrangedSubview(&msg_view);
                    }
                }
            }
        }
    }
    
    fn create_message_bubble(
        &self,
        text: &str,
        is_user: bool,
        mtm: MainThreadMarker,
    ) -> Retained<NSView> {
        // Create horizontal container for alignment (user messages right-aligned, assistant left-aligned)
        let row = NSStackView::new(mtm);
        unsafe {
            row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            row.setSpacing(0.0);
            row.setTranslatesAutoresizingMaskIntoConstraints(false);
            row.setDistribution(NSStackViewDistribution::Fill);
        }
        
        // Max bubble width: 300 per wireframe
        let max_width = 300.0;
        
        // Create message bubble
        let bubble = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(max_width, 60.0)),
        );
        bubble.setWantsLayer(true);
        if let Some(layer) = bubble.layer() {
            if is_user {
                // User messages: green-tinted background #2a4a2a per wireframe
                let (r, g, b) = (42.0 / 255.0, 74.0 / 255.0, 42.0 / 255.0);
                set_layer_background_color(&layer, r, g, b);
            } else {
                // Assistant messages: dark gray background #1a1a1a per wireframe
                set_layer_background_color(&layer, Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2);
            }
            set_layer_corner_radius(&layer, 12.0);
        }
        
        // Create label inside bubble with padding
        let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
        label.setFrame(NSRect::new(
            NSPoint::new(10.0, 10.0),
            NSSize::new(max_width - 20.0, 40.0),
        ));
        label.setFont(Some(&NSFont::systemFontOfSize(13.0)));
        unsafe {
            label.setLineBreakMode(NSLineBreakMode::ByWordWrapping);
        }
        label.setTextColor(Some(&Theme::text_primary()));
        bubble.addSubview(&label);
        
        // Set fixed width constraint on bubble
        unsafe {
            bubble.setTranslatesAutoresizingMaskIntoConstraints(false);
            bubble.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
            let width_constraint = bubble.widthAnchor().constraintLessThanOrEqualToConstant(max_width);
            width_constraint.setActive(true);
        }
        
        // Add spacer and bubble to row based on alignment
        if is_user {
            // User messages: right-aligned (spacer on left, bubble on right)
            let spacer = NSView::new(mtm);
            unsafe {
                spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
                row.addArrangedSubview(&spacer);
                row.addArrangedSubview(&bubble);
            }
        } else {
            // Assistant messages: left-aligned (bubble on left, spacer on right)
            let spacer = NSView::new(mtm);
            unsafe {
                spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
                row.addArrangedSubview(&bubble);
                row.addArrangedSubview(&spacer);
            }
        }

        Retained::from(&*row as &NSView)
    }
    
    fn update_title_and_model(&self) {
        // Load config and get active profile
        let config = Config::load(Config::default_path().unwrap()).unwrap_or_default();
        
        if let Some(conversation) = &*self.ivars().conversation.borrow() {
            if let Ok(profile) = config.get_profile(&conversation.profile_id) {
                // Update title label
                if let Some(title_label) = &*self.ivars().title_label.borrow() {
                    let title_text = conversation.title.clone()
                        .unwrap_or_else(|| "New Conversation".to_string());
                    title_label.setStringValue(&NSString::from_str(&title_text));
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
}
