//! History view for browsing and loading conversations
#![allow(unsafe_code)]
#![allow(unused_unsafe)]
#![allow(unused_variables)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::assigning_clones)]
#![allow(clippy::too_many_lines)]

use std::cell::{Cell, RefCell};
use std::fs::OpenOptions;
use std::io::Write;

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSButtonType, NSFont, NSLayoutConstraintOrientation, NSScrollView,
    NSStackView, NSStackViewDistribution, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
    NSViewController,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::CALayer;

use super::theme::Theme;
use personal_agent::storage::ConversationStorage;

/// Logging helper - writes to file
fn log_to_file(message: &str) {
    let log_path = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/PersonalAgent/debug.log");

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let _ = writeln!(file, "[{timestamp}] HistoryView: {message}");
    }
}

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
                                eprintln!("Failed to load conversation: {e}");
                            }
                        }
                    }
                }
            }
        }

        #[unsafe(method(deleteConversation:))]
        fn delete_conversation(&self, sender: Option<&NSObject>) {
            // Get the button's tag (conversation index)
            if let Some(button) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                let tag = button.tag();
                let conversations = self.ivars().conversations.borrow();

                if let Some(conversation) = conversations.get(tag as usize) {
                    let filename = conversation.filename.clone();
                    drop(conversations); // Release borrow before delete

                    // Delete immediately - no confirmation dialog
                    println!("Deleting conversation: {filename}");

                    // Delete the conversation file
                    if let Ok(storage) = ConversationStorage::with_default_path() {
                        if let Err(e) = storage.delete(&filename) {
                            eprintln!("Failed to delete conversation: {e}");
                        } else {
                            // Reload the list
                            self.load_conversations();
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
            set_layer_background_color(
                &layer,
                Theme::BG_DARK.0,
                Theme::BG_DARK.1,
                Theme::BG_DARK.2,
            );
        }

        // CRITICAL: Set fixed height and high content hugging priority
        unsafe {
            top_bar.setContentHuggingPriority_forOrientation(
                750.0,
                NSLayoutConstraintOrientation::Vertical,
            );
            top_bar.setContentCompressionResistancePriority_forOrientation(
                750.0,
                NSLayoutConstraintOrientation::Vertical,
            );
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
        back_btn.setBezelStyle(NSBezelStyle::Automatic);
        unsafe {
            back_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
            back_btn.setContentHuggingPriority_forOrientation(
                750.0,
                NSLayoutConstraintOrientation::Horizontal,
            );
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
            title.setContentHuggingPriority_forOrientation(
                750.0,
                NSLayoutConstraintOrientation::Horizontal,
            );
        }
        unsafe {
            top_bar.addArrangedSubview(&title);
        }

        // Spacer (flexible, pushes title left)
        let spacer = NSView::new(mtm);
        unsafe {
            spacer.setContentHuggingPriority_forOrientation(
                1.0,
                NSLayoutConstraintOrientation::Horizontal,
            );
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
            scroll_view.setContentHuggingPriority_forOrientation(
                1.0,
                NSLayoutConstraintOrientation::Vertical,
            );
            scroll_view.setContentCompressionResistancePriority_forOrientation(
                250.0,
                NSLayoutConstraintOrientation::Vertical,
            );

            // Add minimum height constraint to prevent collapse
            let min_height = scroll_view
                .heightAnchor()
                .constraintGreaterThanOrEqualToConstant(100.0);
            min_height.setActive(true);
        }

        // Create FLIPPED vertical stack for conversations inside scroll view
        // FlippedStackView overrides isFlipped to return true, which:
        // - Makes origin at TOP-LEFT instead of BOTTOM-LEFT
        // - Content appears at TOP and scrolls DOWN
        // - scrollPoint(0,0) shows the TOP of content
        let convs_stack = super::FlippedStackView::new(mtm);
        unsafe {
            convs_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            convs_stack.setSpacing(1.0);
            convs_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Width);
            // Fill distribution works correctly with flipped coordinates
            convs_stack.setDistribution(NSStackViewDistribution::Fill);
            convs_stack.setEdgeInsets(objc2_foundation::NSEdgeInsets {
                top: 0.0,
                left: 0.0,
                bottom: 0.0,
                right: 0.0,
            });
        }

        convs_stack.setWantsLayer(true);
        if let Some(layer) = convs_stack.layer() {
            set_layer_background_color(
                &layer,
                Theme::BG_DARKEST.0,
                Theme::BG_DARKEST.1,
                Theme::BG_DARKEST.2,
            );
        }

        // CRITICAL: Set translatesAutoresizingMaskIntoConstraints for proper Auto Layout
        convs_stack.setTranslatesAutoresizingMaskIntoConstraints(false);

        scroll_view.setDocumentView(Some(&convs_stack));

        // CRITICAL: Constrain stack width to scroll view's content width (minus padding)
        let content_view = scroll_view.contentView();
        let width_constraint = convs_stack
            .widthAnchor()
            .constraintEqualToAnchor_constant(&content_view.widthAnchor(), -24.0);
        width_constraint.setActive(true);

        // Store references
        *self.ivars().scroll_view.borrow_mut() = Some(scroll_view.clone());
        *self.ivars().conversations_container.borrow_mut() =
            Some(Retained::from(&*convs_stack as &NSView));

        scroll_view
    }

    pub fn reload_conversations(&self) {
        self.load_conversations();
    }

    fn load_conversations(&self) {
        let mtm = MainThreadMarker::new().unwrap();

        log_to_file("load_conversations called");

        let items = Self::load_conversation_items();
        *self.ivars().conversations.borrow_mut() = items.clone();

        self.render_conversation_items(&items, mtm);
        self.scroll_to_top();
    }

    fn load_conversation_items() -> Vec<ConversationItem> {
        let storage = match ConversationStorage::with_default_path() {
            Ok(s) => s,
            Err(e) => {
                log_to_file(&format!("Failed to create conversation storage: {e}"));
                return Vec::new();
            }
        };

        let conversations = match storage.load_all() {
            Ok(convs) => {
                log_to_file(&format!("Loaded {} conversations", convs.len()));
                convs
            }
            Err(e) => {
                log_to_file(&format!("Failed to load conversations: {e}"));
                Vec::new()
            }
        };

        let mut conversations = conversations;
        conversations.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        conversations
            .iter()
            .map(Self::build_conversation_item)
            .collect()
    }

    fn build_conversation_item(conv: &personal_agent::models::Conversation) -> ConversationItem {
        let title = conv
            .title
            .clone()
            .unwrap_or_else(|| conv.created_at.format("%Y%m%d%H%M%S%3f").to_string());
        let date = conv.created_at.format("%Y-%m-%d %H:%M").to_string();
        let message_count = conv.messages.len();

        log_to_file(&format!(
            "Conversation: title='{title}', date='{date}', messages={message_count}"
        ));

        ConversationItem {
            filename: conv.filename(),
            title,
            date,
            message_count,
        }
    }

    fn render_conversation_items(&self, items: &[ConversationItem], mtm: MainThreadMarker) {
        log_to_file(&format!(
            "Container ref valid: {}",
            self.ivars().conversations_container.borrow().is_some()
        ));

        if let Some(container) = &*self.ivars().conversations_container.borrow() {
            log_to_file(&format!("Adding {} items to container", items.len()));
            Self::clear_container(container);

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

                if items.is_empty() {
                    Self::add_empty_state_message(stack, mtm);
                }
            }
        }
    }

    fn clear_container(container: &NSView) {
        let subviews = container.subviews();
        for view in &subviews {
            if let Some(stack) = container.downcast_ref::<NSStackView>() {
                unsafe {
                    stack.removeArrangedSubview(&view);
                }
            }
            view.removeFromSuperview();
        }
    }

    fn add_empty_state_message(stack: &NSStackView, mtm: MainThreadMarker) {
        let message = NSTextField::labelWithString(
            &NSString::from_str(
                "No conversations yet.\n\nStart a new conversation to get started.",
            ),
            mtm,
        );
        message.setTextColor(Some(&Theme::text_secondary_color()));
        message.setFont(Some(&NSFont::systemFontOfSize(13.0)));
        stack.addArrangedSubview(&message);
    }

    fn scroll_to_top(&self) {
        if let Some(scroll_view) = &*self.ivars().scroll_view.borrow() {
            scroll_view.layoutSubtreeIfNeeded();
            let clip_view = scroll_view.contentView();
            clip_view.scrollToPoint(NSPoint::new(0.0, 0.0));
            scroll_view.reflectScrolledClipView(&clip_view);
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
        log_to_file(&format!("Creating row {index}: {title}"));

        let container = Self::build_conversation_container(mtm);
        let row_button = self.build_conversation_row_button(index, mtm);
        let content_stack = Self::build_conversation_content_stack(title, date, message_count, mtm);
        let delete_btn = self.build_delete_button(index, mtm);

        Self::attach_row_content(&row_button, &content_stack);
        Self::layout_conversation_row(&container, &row_button, &delete_btn);

        log_to_file(&format!("Row {index} created successfully"));
        Retained::from(&*container as &NSView)
    }

    fn build_conversation_container(mtm: MainThreadMarker) -> Retained<NSView> {
        let container = NSView::new(mtm);
        container.setTranslatesAutoresizingMaskIntoConstraints(false);
        unsafe {
            let height_constraint = container.heightAnchor().constraintEqualToConstant(40.0);
            height_constraint.setActive(true);
        }
        container
    }

    fn build_conversation_row_button(
        &self,
        index: usize,
        mtm: MainThreadMarker,
    ) -> Retained<NSButton> {
        let row_button = NSButton::new(mtm);
        row_button.setButtonType(NSButtonType::MomentaryPushIn);
        row_button.setBezelStyle(NSBezelStyle::Automatic);
        row_button.setBordered(false);
        row_button.setTitle(&NSString::from_str(""));
        row_button.setTag(index as isize);
        unsafe {
            row_button.setTarget(Some(self));
            row_button.setAction(Some(sel!(conversationSelected:)));
            row_button.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        row_button.setWantsLayer(true);
        if let Some(layer) = row_button.layer() {
            if index.is_multiple_of(2) {
                set_layer_background_color(
                    &layer,
                    Theme::BG_DARK.0,
                    Theme::BG_DARK.1,
                    Theme::BG_DARK.2,
                );
            } else {
                set_layer_background_color(
                    &layer,
                    Theme::BG_DARKEST.0,
                    Theme::BG_DARKEST.1,
                    Theme::BG_DARKEST.2,
                );
            }
        }

        row_button
    }

    fn build_conversation_content_stack(
        title: &str,
        date: &str,
        message_count: usize,
        mtm: MainThreadMarker,
    ) -> Retained<NSStackView> {
        let content_stack = NSStackView::new(mtm);
        content_stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        content_stack.setSpacing(8.0);
        content_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        content_stack.setDistribution(NSStackViewDistribution::Fill);
        content_stack.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: 8.0,
            left: 8.0,
            bottom: 8.0,
            right: 8.0,
        });

        let title_label = Self::build_title_label(title, mtm);
        let date_label = Self::build_date_label(date, mtm);
        let count_label = Self::build_count_label(message_count, mtm);

        content_stack.addArrangedSubview(&title_label);
        content_stack.addArrangedSubview(&date_label);
        content_stack.addArrangedSubview(&count_label);

        content_stack
    }

    fn build_title_label(title: &str, mtm: MainThreadMarker) -> Retained<NSTextField> {
        let title_label = NSTextField::labelWithString(&NSString::from_str(title), mtm);
        title_label.setTextColor(Some(&Theme::text_primary()));
        title_label.setFont(Some(&NSFont::systemFontOfSize(13.0)));
        title_label.setEditable(false);
        title_label.setBordered(false);
        title_label.setDrawsBackground(false);
        unsafe {
            title_label.setTranslatesAutoresizingMaskIntoConstraints(false);
            title_label.setContentHuggingPriority_forOrientation(
                250.0,
                NSLayoutConstraintOrientation::Horizontal,
            );
        }
        title_label
    }

    fn build_date_label(date: &str, mtm: MainThreadMarker) -> Retained<NSTextField> {
        let date_label = NSTextField::labelWithString(&NSString::from_str(date), mtm);
        date_label.setTextColor(Some(&Theme::text_secondary_color()));
        date_label.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        date_label.setEditable(false);
        date_label.setBordered(false);
        date_label.setDrawsBackground(false);
        unsafe {
            date_label.setTranslatesAutoresizingMaskIntoConstraints(false);
            date_label.setContentHuggingPriority_forOrientation(
                750.0,
                NSLayoutConstraintOrientation::Horizontal,
            );
            let width_constraint = date_label.widthAnchor().constraintEqualToConstant(120.0);
            width_constraint.setActive(true);
        }
        date_label
    }

    fn build_count_label(message_count: usize, mtm: MainThreadMarker) -> Retained<NSTextField> {
        let count_text = format!("{message_count}");
        let count_label = NSTextField::labelWithString(&NSString::from_str(&count_text), mtm);
        count_label.setTextColor(Some(&Theme::text_secondary_color()));
        count_label.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        count_label.setAlignment(objc2_app_kit::NSTextAlignment::Right);
        count_label.setEditable(false);
        count_label.setBordered(false);
        count_label.setDrawsBackground(false);
        unsafe {
            count_label.setTranslatesAutoresizingMaskIntoConstraints(false);
            count_label.setContentHuggingPriority_forOrientation(
                750.0,
                NSLayoutConstraintOrientation::Horizontal,
            );
            let width_constraint = count_label.widthAnchor().constraintEqualToConstant(40.0);
            width_constraint.setActive(true);
        }
        count_label
    }

    fn build_delete_button(&self, index: usize, mtm: MainThreadMarker) -> Retained<NSButton> {
        let delete_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("X"),
                Some(self),
                Some(sel!(deleteConversation:)),
                mtm,
            )
        };
        delete_btn.setBezelStyle(NSBezelStyle::Automatic);
        delete_btn.setTag(index as isize);
        unsafe {
            delete_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
            delete_btn.setContentHuggingPriority_forOrientation(
                750.0,
                NSLayoutConstraintOrientation::Horizontal,
            );
            let width_constraint = delete_btn.widthAnchor().constraintEqualToConstant(30.0);
            width_constraint.setActive(true);
        }
        delete_btn
    }

    fn attach_row_content(row_button: &NSButton, content_stack: &NSStackView) {
        row_button.addSubview(content_stack);

        unsafe {
            let leading = content_stack
                .leadingAnchor()
                .constraintEqualToAnchor(&row_button.leadingAnchor());
            let trailing = content_stack
                .trailingAnchor()
                .constraintEqualToAnchor(&row_button.trailingAnchor());
            let top = content_stack
                .topAnchor()
                .constraintEqualToAnchor(&row_button.topAnchor());
            let bottom = content_stack
                .bottomAnchor()
                .constraintEqualToAnchor(&row_button.bottomAnchor());
            leading.setActive(true);
            trailing.setActive(true);
            top.setActive(true);
            bottom.setActive(true);
        }
    }

    fn layout_conversation_row(container: &NSView, row_button: &NSButton, delete_btn: &NSButton) {
        container.addSubview(row_button);
        container.addSubview(delete_btn);

        unsafe {
            let leading = row_button
                .leadingAnchor()
                .constraintEqualToAnchor(&container.leadingAnchor());
            let trailing = row_button
                .trailingAnchor()
                .constraintEqualToAnchor_constant(&delete_btn.leadingAnchor(), -4.0);
            let top = row_button
                .topAnchor()
                .constraintEqualToAnchor(&container.topAnchor());
            let bottom = row_button
                .bottomAnchor()
                .constraintEqualToAnchor(&container.bottomAnchor());
            leading.setActive(true);
            trailing.setActive(true);
            top.setActive(true);
            bottom.setActive(true);
        }

        unsafe {
            let trailing = delete_btn
                .trailingAnchor()
                .constraintEqualToAnchor(&container.trailingAnchor());
            let center_y = delete_btn
                .centerYAnchor()
                .constraintEqualToAnchor(&container.centerYAnchor());
            trailing.setActive(true);
            center_y.setActive(true);
        }
    }
}
