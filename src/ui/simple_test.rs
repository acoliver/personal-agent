//! Simple test view to debug NSStackView layout

use objc2::rc::Retained;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly, DefinedClass};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use objc2_app_kit::{
    NSView, NSViewController, NSTextField, NSButton, NSColor, NSFont, NSStackView,
    NSUserInterfaceLayoutOrientation, NSStackViewDistribution, NSLayoutConstraintOrientation,
    NSScrollView, NSTextView,
};
use objc2_core_graphics::CGColor;

pub struct SimpleTestIvars;

define_class!(
    #[unsafe(super(NSViewController))]
    #[thread_kind = MainThreadOnly]
    #[name = "SimpleTestViewController"]
    #[ivars = SimpleTestIvars]
    pub struct SimpleTestViewController;

    unsafe impl NSObjectProtocol for SimpleTestViewController {}

    impl SimpleTestViewController {
        #[unsafe(method(loadView))]
        fn load_view(&self) {
            let mtm = MainThreadMarker::new().unwrap();
            
            println!("
=== SimpleTestViewController with NSStackView ===
");
            
            // ================================================================
            // ROOT VIEW (400x500)
            // ================================================================
            let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 500.0));
            let root_view = NSView::initWithFrame(NSView::alloc(mtm), frame);
            root_view.setWantsLayer(true);
            if let Some(layer) = root_view.layer() {
                let color = CGColor::new_generic_rgb(0.05, 0.05, 0.05, 1.0);
                layer.setBackgroundColor(Some(&color));
            }
            
            // ================================================================
            // MAIN VERTICAL STACK
            // ================================================================
            /*
            -------MainStack (vertical)------------------
            | ----TopBar (horizontal)------------------ |
            | | [Title Label]           [Settings Btn] | |
            | ----------------------------------------- |
            | ----ChatArea (scroll view)-------------- |
            | |                                       | |
            | |  (messages go here)                   | |
            | |                                       | |
            | ----------------------------------------- |
            | ----InputBar (horizontal)--------------- |
            | | [Text Field]              [Send Btn]  | |
            | ----------------------------------------- |
            ---------------------------------------------
            */
            
            let main_stack = NSStackView::new(mtm);
            main_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            main_stack.setSpacing(0.0);
            main_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
            main_stack.setDistribution(NSStackViewDistribution::Fill);
            
            // ================================================================
            // TOP BAR (height ~44)
            // ================================================================
            let top_bar = self.build_top_bar(mtm);
            
            // ================================================================
            // CHAT AREA (flexible height - should expand)
            // ================================================================
            let chat_area = self.build_chat_area(mtm);
            
            // ================================================================
            // INPUT BAR (height ~50)
            // ================================================================
            let input_bar = self.build_input_bar(mtm);
            
            // ================================================================
            // ADD TO STACK WITH PRIORITIES
            // ================================================================
            // High hugging = wants to stay small
            // Low hugging = willing to expand
            
            top_bar.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            chat_area.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
            input_bar.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            
            // Compression resistance - how much it resists being squished
            top_bar.setContentCompressionResistancePriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            chat_area.setContentCompressionResistancePriority_forOrientation(250.0, NSLayoutConstraintOrientation::Vertical);
            input_bar.setContentCompressionResistancePriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            
            main_stack.addArrangedSubview(&top_bar);
            main_stack.addArrangedSubview(&chat_area);
            main_stack.addArrangedSubview(&input_bar);
            
            root_view.addSubview(&main_stack);
            
            // ================================================================
            // CONSTRAIN STACK TO FILL ROOT VIEW
            // ================================================================
            let leading = main_stack.leadingAnchor().constraintEqualToAnchor(&root_view.leadingAnchor());
            let trailing = main_stack.trailingAnchor().constraintEqualToAnchor(&root_view.trailingAnchor());
            let top = main_stack.topAnchor().constraintEqualToAnchor(&root_view.topAnchor());
            let bottom = main_stack.bottomAnchor().constraintEqualToAnchor(&root_view.bottomAnchor());
            
            leading.setActive(true);
            trailing.setActive(true);
            top.setActive(true);
            bottom.setActive(true);
            
            // ================================================================
            // HEIGHT CONSTRAINTS FOR FIXED-HEIGHT ITEMS
            // ================================================================
            let top_height = top_bar.heightAnchor().constraintEqualToConstant(44.0);
            top_height.setActive(true);
            
            let input_height = input_bar.heightAnchor().constraintEqualToConstant(50.0);
            input_height.setActive(true);
            
            // Chat area needs minimum height constraint
            let chat_min_height = chat_area.heightAnchor().constraintGreaterThanOrEqualToConstant(100.0);
            chat_min_height.setActive(true);
            
            self.setView(&root_view);
            
            // Force layout
            root_view.layoutSubtreeIfNeeded();
            
            println!("root_view frame: {:?}", root_view.frame());
            println!("main_stack frame: {:?}", main_stack.frame());
            println!("top_bar frame: {:?}", top_bar.frame());
            println!("chat_area frame: {:?}", chat_area.frame());
            println!("input_bar frame: {:?}", input_bar.frame());
            println!("
=== End Layout Debug ===
");
        }
    }
);

impl SimpleTestViewController {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        unsafe { msg_send![Self::alloc(mtm), init] }
    }
    
    fn build_top_bar(&self, mtm: MainThreadMarker) -> Retained<NSStackView> {
        /*
        ----TopBar (horizontal, height=44)--------
        | [PA] [PersonalAgent]     [gear] [hist] |
        ------------------------------------------
        */
        let stack = NSStackView::new(mtm);
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        stack.setSpacing(8.0);
        stack.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: 8.0, left: 12.0, bottom: 8.0, right: 12.0
        });
        stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        
        // Background
        stack.setWantsLayer(true);
        if let Some(layer) = stack.layer() {
            let color = CGColor::new_generic_rgb(0.1, 0.1, 0.1, 1.0);
            layer.setBackgroundColor(Some(&color));
        }
        
        // Title label
        let title = NSTextField::labelWithString(&NSString::from_str("PersonalAgent"), mtm);
        title.setTextColor(Some(&NSColor::whiteColor()));
        title.setFont(Some(&NSFont::boldSystemFontOfSize(16.0)));
        
        // Spacer view (flexible)
        let spacer = NSView::new(mtm);
        spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
        
        // Settings button
        let settings_btn = unsafe {
            NSButton::buttonWithTitle_target_action(&NSString::from_str("[S]"), None, None, mtm)
        };
        
        // History button  
        let history_btn = unsafe {
            NSButton::buttonWithTitle_target_action(&NSString::from_str("[H]"), None, None, mtm)
        };
        
        // History button  
        let history_btn = unsafe {
            NSButton::buttonWithTitle_target_action(&NSString::from_str(""), None, None, mtm)
        };
        
        stack.addArrangedSubview(&title);
        stack.addArrangedSubview(&spacer);
        stack.addArrangedSubview(&settings_btn);
        stack.addArrangedSubview(&history_btn);
        
        stack
    }
    
    fn build_chat_area(&self, mtm: MainThreadMarker) -> Retained<NSScrollView> {
        /*
        ----ChatArea (scroll view, flexible height)----
        | [User]: Hello                               |
        | [Assistant]: Hi there!                      |
        |                                             |
        -----------------------------------------------
        */
        let scroll_view = NSScrollView::new(mtm);
        scroll_view.setHasVerticalScroller(true);
        scroll_view.setTranslatesAutoresizingMaskIntoConstraints(false);
        
        // Background
        scroll_view.setWantsLayer(true);
        if let Some(layer) = scroll_view.layer() {
            let color = CGColor::new_generic_rgb(0.05, 0.05, 0.05, 1.0);
            layer.setBackgroundColor(Some(&color));
        }
        
        // Content view with some sample text
        let content = NSView::new(mtm);
        content.setTranslatesAutoresizingMaskIntoConstraints(false);
        
        let label1 = NSTextField::labelWithString(&NSString::from_str("User: Hello!"), mtm);
        label1.setTextColor(Some(&NSColor::whiteColor()));
        label1.setFrame(NSRect::new(NSPoint::new(10.0, 360.0), NSSize::new(380.0, 20.0)));
        content.addSubview(&label1);
        
        let label2 = NSTextField::labelWithString(&NSString::from_str("Assistant: Hi there! How can I help?"), mtm);
        label2.setTextColor(Some(&NSColor::systemGrayColor()));
        label2.setFrame(NSRect::new(NSPoint::new(10.0, 330.0), NSSize::new(380.0, 20.0)));
        content.addSubview(&label2);
        
        scroll_view.setDocumentView(Some(&content));
        
        scroll_view
    }
    
    fn build_input_bar(&self, mtm: MainThreadMarker) -> Retained<NSStackView> {
        /*
        ----InputBar (horizontal, height=50)------
        | [TextField...................] [Send]  |
        ------------------------------------------
        */
        let stack = NSStackView::new(mtm);
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        stack.setSpacing(8.0);
        stack.setEdgeInsets(objc2_foundation::NSEdgeInsets {
            top: 8.0, left: 12.0, bottom: 8.0, right: 12.0
        });
        stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        
        // Background
        stack.setWantsLayer(true);
        if let Some(layer) = stack.layer() {
            let color = CGColor::new_generic_rgb(0.15, 0.15, 0.15, 1.0);
            layer.setBackgroundColor(Some(&color));
        }
        
        // Text field (should expand)
        let text_field = NSTextField::new(mtm);
        text_field.setPlaceholderString(Some(&NSString::from_str("Type a message...")));
        text_field.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
        
        // Send button (fixed width)
        let send_btn = unsafe {
            NSButton::buttonWithTitle_target_action(&NSString::from_str("Send"), None, None, mtm)
        };
        send_btn.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Horizontal);
        
        stack.addArrangedSubview(&text_field);
        stack.addArrangedSubview(&send_btn);
        
        stack
    }
}
