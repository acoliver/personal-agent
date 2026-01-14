//! History view for browsing and loading conversations

use std::cell::{RefCell, Cell};

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly, DefinedClass};
use objc2_foundation::{
    NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use objc2_app_kit::{
    NSView, NSViewController, NSTextField, NSButton, NSScrollView, NSFont, NSBezelStyle,
    NSStackView, NSUserInterfaceLayoutOrientation, NSStackViewDistribution, NSLayoutConstraintOrientation,
};
use objc2_quartz_core::CALayer;

use super::theme::Theme;
use personal_agent::storage::ConversationStorage;
use personal_agent::models::Conversation;

// Thread-local storage for passing conversation data between views
thread_local! {
    pub static LOADED_CONVERSATION_JSON: Cell<Option<String>> = const { Cell::new(None) };
}

// ============================================================================
// Helper functions for CALayer operations
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
// Conversation item for display
// ============================================================================

#[derive(Clone, Debug)]
struct ConversationItem {
    filename: String,
    title: String,
    date: String,
    message_count: usize,
}

// ============================================================================
// HistoryViewController ivars
// ============================================================================

pub struct HistoryViewIvars {
    conversations: RefCell<Vec<ConversationItem>>,
    conversations_container: RefCell<Option<Retained<NSView>>>,
    scroll_view: RefCell<Option<Retained<NSScrollView>>>,
}

// ============================================================================
// HistoryViewController - conversation history view controller
// ============================================================================

define_class!(
    #[unsafe(super(NSViewController))]
    #[thread_kind = MainThreadOnly]
    #[name = "HistoryViewController"]
    #[ivars = HistoryViewIvars]
    pub struct HistoryViewController;

    unsafe impl NSObjectProtocol for HistoryViewController {}

    impl HistoryViewController {
        #[unsafe(method(loadView))]
        fn load_view(&self) {
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
                main_stack.setDistribution(objc2_app_kit::NSStackViewDistribution::Fill);
            }
            
            // Build the UI components
            let top_bar = self.build_top_bar_stack(mtm);
            let content_area = self.build_content_area_stack(mtm);
            
            // Add to stack
            unsafe {
                main_stack.addArrangedSubview(&top_bar);
                main_stack.addArrangedSubview(&content_area);
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
            
            // Load conversations
            self.load_conversations();
        }

        #[unsafe(method(backButtonClicked:))]
        fn back_button_clicked(&self, _sender: Option<&NSObject>) {
            // Post notification to switch back to chat view
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowChatView");
            unsafe {
                center.postNotificationName_object(&name, None);
            }
        }

        #[unsafe(method(conversationSelected:))]
        fn conversation_selected(&self, sender: Option<&NSObject>) {
            // Get the button's tag (conversation index)
            if let Some(button) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                let tag = button.tag();
                let conversations = self.ivars().conversations.borrow();
                
                if let Some(conversation_item) = conversations.get(tag as usize) {
                    println!("Loading conversation: {}", conversation_item.filename);
                    
                    // Load the conversation from storage
                    if let Ok(storage) = ConversationStorage::with_default_path() {
                        match storage.load(&conversation_item.filename) {
                            Ok(conversation) => {
                                // Post notification with conversation data
                                use objc2_foundation::NSNotificationCenter;
                                let center = NSNotificationCenter::defaultCenter();
                                
                                // Serialize conversation to JSON string to pass via notification
                                // Store it as a global variable that can be accessed by the notification handler
                                // This is a simple approach - in production, you'd use proper IPC or shared state
                                if let Ok(json) = serde_json::to_string(&conversation) {
                                    // For now, we'll just use the notification to trigger the load
                                    // and have the main delegate access the conversation directly
                                    // A more proper approach would use NSNotification's object or userInfo
                                    
                                    // Store conversation JSON in a static for the delegate to read
                                    LOADED_CONVERSATION_JSON.with(|cell| {
                                        cell.replace(Some(json));
                                    });
                                    
                                    let name = NSString::from_str("PersonalAgentLoadConversation");
                                    unsafe {
                                        center.postNotificationName_object(&name, None);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to load conversation: {}", e);
                            }
                        }
                    }
                }
            }
        }

        #[unsafe(method(deleteConversation:))]
        fn delete_conversation(&self, sender: Option<&NSObject>) {
            use objc2_app_kit::NSAlert;
            
            // Get the button's tag (conversation index)
            if let Some(button) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                let tag = button.tag();
                let conversations = self.ivars().conversations.borrow();
                
                if let Some(conversation) = conversations.get(tag as usize) {
                    let filename = conversation.filename.clone();
                    drop(conversations); // Release borrow before showing alert
                    
                    let mtm = MainThreadMarker::new().unwrap();
                    
                    // Show confirmation dialog
                    let alert = NSAlert::new(mtm);
                    alert.setMessageText(&NSString::from_str("Delete Conversation?"));
                    alert.setInformativeText(&NSString::from_str("This cannot be undone."));
                    alert.addButtonWithTitle(&NSString::from_str("Delete"));
                    alert.addButtonWithTitle(&NSString::from_str("Cancel"));
                    
                    let response = unsafe { alert.runModal() };
                    
                    // NSAlertFirstButtonReturn = 1000
                    if response == 1000 {
                        println!("Deleting conversation: {}", filename);
                        
                        // Delete the conversation file
                        if let Ok(storage) = ConversationStorage::with_default_path() {
                            if let Err(e) = storage.delete(&filename) {
                                eprintln!("Failed to delete conversation: {}", e);
                            } else {
                                // Reload the list
                                self.load_conversations();
                            }
                        }
                    }
                }
            }
        }
    }
);

impl HistoryViewController {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let ivars = HistoryViewIvars {
            conversations: RefCell::new(Vec::new()),
            conversations_container: RefCell::new(None),
            scroll_view: RefCell::new(None),
        };
        
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
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

        // Back button (w=40 per wireframe)
        let back_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("<"),
                Some(self),
                Some(sel!(backButtonClicked:)),
                mtm,
            )
        };
        back_btn.setBezelStyle(NSBezelStyle::Rounded);
        unsafe {
            back_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
            back_btn.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
            let width_constraint = back_btn.widthAnchor().constraintEqualToConstant(40.0);
            width_constraint.setActive(true);
        }
        unsafe {
            top_bar.addArrangedSubview(&back_btn);
        }

        // Title
        let title = NSTextField::labelWithString(&NSString::from_str("History"), mtm);
        title.setTextColor(Some(&Theme::text_primary()));
        title.setFont(Some(&NSFont::boldSystemFontOfSize(14.0)));
        unsafe {
            title.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
        }
        unsafe {
            top_bar.addArrangedSubview(&title);
        }
        
        // Spacer (flexible, pushes title left)
        let spacer = NSView::new(mtm);
        unsafe {
            spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
            top_bar.addArrangedSubview(&spacer);
        }

        Retained::from(&*top_bar as &NSView)
    }

    fn build_content_area_stack(&self, mtm: MainThreadMarker) -> Retained<NSScrollView> {
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

        // Create vertical stack for conversations inside scroll view
        let convs_stack = NSStackView::new(mtm);
        unsafe {
            convs_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            convs_stack.setSpacing(8.0);
            convs_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
            convs_stack.setDistribution(NSStackViewDistribution::Fill);
        }
        
        convs_stack.setWantsLayer(true);
        if let Some(layer) = convs_stack.layer() {
            set_layer_background_color(&layer, Theme::BG_DARKEST.0, Theme::BG_DARKEST.1, Theme::BG_DARKEST.2);
        }

        scroll_view.setDocumentView(Some(&convs_stack));

        // Store references
        *self.ivars().scroll_view.borrow_mut() = Some(scroll_view.clone());
        *self.ivars().conversations_container.borrow_mut() = Some(Retained::from(&*convs_stack as &NSView));

        scroll_view
    }

    fn load_conversations(&self) {
        let mtm = MainThreadMarker::new().unwrap();
        
        // Load conversations from storage
        let storage = match ConversationStorage::with_default_path() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to create conversation storage: {}", e);
                return;
            }
        };
        
        let conversations = match storage.load_all() {
            Ok(convs) => convs,
            Err(e) => {
                eprintln!("Failed to load conversations: {}", e);
                Vec::new()
            }
        };
        
        // Convert to display items
        let items: Vec<ConversationItem> = conversations
            .iter()
            .map(|conv| {
                let title = conv.title.clone().unwrap_or_else(|| "Untitled Conversation".to_string());
                let date = conv.created_at.format("%Y-%m-%d %H:%M").to_string();
                let filename = conv.filename();
                let message_count = conv.messages.len();
                
                ConversationItem {
                    filename,
                    title,
                    date,
                    message_count,
                }
            })
            .collect();
        
        *self.ivars().conversations.borrow_mut() = items.clone();
        
        if let Some(container) = &*self.ivars().conversations_container.borrow() {
            // Clear existing subviews (for stack view, remove arranged subviews)
            let subviews = container.subviews();
            for view in subviews.iter() {
                if let Some(stack) = container.downcast_ref::<NSStackView>() {
                    unsafe {
                        stack.removeArrangedSubview(&view);
                    }
                }
                view.removeFromSuperview();
            }

            // For stack view, just add conversation views - stack handles positioning
            if let Some(stack) = container.downcast_ref::<NSStackView>() {
                for (index, item) in items.iter().enumerate() {
                    let conv_view = self.create_conversation_card(
                        &item.title,
                        &item.date,
                        item.message_count,
                        index,
                        mtm,
                    );
                    unsafe {
                        stack.addArrangedSubview(&conv_view);
                    }
                }

                // If no conversations, show a message
                if items.is_empty() {
                    let message = NSTextField::labelWithString(
                        &NSString::from_str("No conversations yet.\n\nStart a new conversation to get started."),
                        mtm,
                    );
                    message.setTextColor(Some(&Theme::text_secondary_color()));
                    message.setFont(Some(&NSFont::systemFontOfSize(13.0)));
                    unsafe {
                        stack.addArrangedSubview(&message);
                    }
                }
            }
        }
    }

    fn create_conversation_card(
        &self,
        title: &str,
        date: &str,
        message_count: usize,
        index: usize,
        mtm: MainThreadMarker,
    ) -> Retained<NSView> {
        let width = 380.0;
        let height = 80.0;

        // Create card container
        let card = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height)),
        );
        card.setWantsLayer(true);
        if let Some(layer) = card.layer() {
            set_layer_background_color(&layer, Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2);
            set_layer_corner_radius(&layer, 8.0);
        }
        
        // Set fixed height constraint
        unsafe {
            card.setTranslatesAutoresizingMaskIntoConstraints(false);
            let height_constraint = card.heightAnchor().constraintEqualToConstant(height);
            height_constraint.setActive(true);
        }

        // Left side: vertical stack for labels
        let labels_stack = NSStackView::new(mtm);
        labels_stack.setFrame(NSRect::new(
            NSPoint::new(12.0, 12.0),
            NSSize::new(240.0, 56.0),
        ));
        unsafe {
            labels_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            labels_stack.setSpacing(4.0);
            labels_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
        }

        // Conversation title
        let title_label = NSTextField::labelWithString(&NSString::from_str(title), mtm);
        title_label.setTextColor(Some(&Theme::text_primary()));
        title_label.setFont(Some(&NSFont::boldSystemFontOfSize(14.0)));
        unsafe {
            labels_stack.addArrangedSubview(&title_label);
        }

        // Date
        let date_label = NSTextField::labelWithString(&NSString::from_str(date), mtm);
        date_label.setTextColor(Some(&Theme::text_secondary_color()));
        date_label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        unsafe {
            labels_stack.addArrangedSubview(&date_label);
        }

        // Message count
        let count_text = format!("{} messages", message_count);
        let count_label = NSTextField::labelWithString(&NSString::from_str(&count_text), mtm);
        count_label.setTextColor(Some(&Theme::text_secondary_color()));
        count_label.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        unsafe {
            labels_stack.addArrangedSubview(&count_label);
        }
        
        card.addSubview(&labels_stack);

        // Right side: horizontal stack for buttons
        let buttons_stack = NSStackView::new(mtm);
        buttons_stack.setFrame(NSRect::new(
            NSPoint::new(260.0, 26.0),
            NSSize::new(110.0, 28.0),
        ));
        unsafe {
            buttons_stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            buttons_stack.setSpacing(5.0);
        }

        // Load button
        let load_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Load"),
                Some(self),
                Some(sel!(conversationSelected:)),
                mtm,
            )
        };
        load_btn.setBezelStyle(NSBezelStyle::Rounded);
        load_btn.setTag(index as isize);
        unsafe {
            buttons_stack.addArrangedSubview(&load_btn);
        }

        // Delete button
        let delete_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Delete"),
                Some(self),
                Some(sel!(deleteConversation:)),
                mtm,
            )
        };
        delete_btn.setBezelStyle(NSBezelStyle::Rounded);
        delete_btn.setTag(index as isize);
        unsafe {
            buttons_stack.addArrangedSubview(&delete_btn);
        }
        
        card.addSubview(&buttons_stack);

        card
    }
}
