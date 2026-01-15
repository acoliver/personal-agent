//! Add MCP view - enter URL or search registries

use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::Write;

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAppearanceCustomization, NSButton, NSBezelStyle, NSFont, NSLayoutConstraintOrientation, NSStackView, 
    NSStackViewDistribution, NSTextField, NSUserInterfaceLayoutOrientation, NSView, NSViewController, NSScrollView,
    NSButtonType,
};
use objc2_foundation::{NSEdgeInsets, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use objc2_core_graphics::CGColor;

use crate::ui::Theme;
use personal_agent::mcp::{McpConfig, registry::{McpRegistry, McpRegistryServerWrapper}};

fn log_to_file(message: &str) {
    let log_path = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/PersonalAgent/debug.log");
    
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let _ = writeln!(file, "[{timestamp}] McpAddView: {message}");
    }
}

/// Parsed MCP URL result
#[derive(Debug, Clone)]
pub enum ParsedMcp {
    Npm { identifier: String, runtime_hint: String },
    Docker { image: String },
    Http { url: String },
}

/// Parse MCP URL to detect package type
pub fn parse_mcp_url(url: &str) -> Result<ParsedMcp, String> {
    let url = url.trim();
    
    // npx -y @package/name or npx @package/name
    if url.starts_with("npx ") {
        let parts: Vec<&str> = url.split_whitespace().collect();
        // Find the package identifier - it's not "npx" and not a flag (starts with -)
        let identifier = parts.iter()
            .skip(1) // Skip "npx"
            .find(|p| !p.starts_with("-"))
            .ok_or("Invalid npx command")?;
        return Ok(ParsedMcp::Npm {
            identifier: identifier.to_string(),
            runtime_hint: "npx".to_string(),
        });
    }
    
    // docker run image
    if url.starts_with("docker ") {
        let parts: Vec<&str> = url.split_whitespace().collect();
        let image = parts.last().ok_or("Invalid docker command")?;
        return Ok(ParsedMcp::Docker {
            image: image.to_string(),
        });
    }
    
    // HTTP URL
    if url.starts_with("http://") || url.starts_with("https://") {
        return Ok(ParsedMcp::Http {
            url: url.to_string(),
        });
    }
    
    // Bare package name (assume npm)
    if url.starts_with("@") || url.contains("/") {
        return Ok(ParsedMcp::Npm {
            identifier: url.to_string(),
            runtime_hint: "npx".to_string(),
        });
    }
    
    Err("Unrecognized URL format".to_string())
}

// Thread-local storage for passing parsed MCP to configure view
thread_local! {
    pub static PARSED_MCP: RefCell<Option<ParsedMcp>> = const { RefCell::new(None) };
    pub static SELECTED_MCP_CONFIG: RefCell<Option<McpConfig>> = const { RefCell::new(None) };
    static SEARCH_RESULTS: RefCell<Option<Vec<McpRegistryServerWrapper>>> = const { RefCell::new(None) };
}

pub struct McpAddViewIvars {
    search_field: RefCell<Option<Retained<NSTextField>>>,
    results_stack: RefCell<Option<Retained<NSStackView>>>,
    loading_label: RefCell<Option<Retained<NSTextField>>>,
    results_scroll: RefCell<Option<Retained<NSScrollView>>>,
    url_input: RefCell<Option<Retained<NSTextField>>>,
    next_button: RefCell<Option<Retained<NSButton>>>,
}

define_class!(
    #[unsafe(super(NSViewController))]
    #[thread_kind = MainThreadOnly]
    #[name = "McpAddViewController"]
    #[ivars = McpAddViewIvars]
    pub struct McpAddViewController;

    unsafe impl NSObjectProtocol for McpAddViewController {}

    impl McpAddViewController {
        #[unsafe(method(loadView))]
        fn load_view(&self) {
            log_to_file("loadView started");
            let mtm = MainThreadMarker::new().unwrap();

            let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 500.0));
            let root_view = NSView::initWithFrame(NSView::alloc(mtm), frame);
            
            // Force dark appearance
            let dark_appearance_name = unsafe { objc2_app_kit::NSAppearanceNameDarkAqua };
            if let Some(dark_appearance) = objc2_app_kit::NSAppearance::appearanceNamed(dark_appearance_name) {
                root_view.setAppearance(Some(&dark_appearance));
            }
            
            root_view.setWantsLayer(true);
            if let Some(layer) = root_view.layer() {
                let color = CGColor::new_generic_rgb(
                    Theme::BG_DARKEST.0, Theme::BG_DARKEST.1, Theme::BG_DARKEST.2, 1.0,
                );
                layer.setBackgroundColor(Some(&color));
            }

            let main_stack = NSStackView::new(mtm);
            main_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            main_stack.setSpacing(0.0);
            main_stack.setDistribution(NSStackViewDistribution::Fill);
            main_stack.setTranslatesAutoresizingMaskIntoConstraints(false);

            let top_bar = self.build_top_bar(mtm);
            let content = self.build_content(mtm);

            top_bar.setContentHuggingPriority_forOrientation(750.0, NSLayoutConstraintOrientation::Vertical);
            content.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);

            main_stack.addArrangedSubview(&top_bar);
            main_stack.addArrangedSubview(&content);
            root_view.addSubview(&main_stack);

            let leading = main_stack.leadingAnchor().constraintEqualToAnchor(&root_view.leadingAnchor());
            let trailing = main_stack.trailingAnchor().constraintEqualToAnchor(&root_view.trailingAnchor());
            let top = main_stack.topAnchor().constraintEqualToAnchor(&root_view.topAnchor());
            let bottom = main_stack.bottomAnchor().constraintEqualToAnchor(&root_view.bottomAnchor());
            leading.setActive(true);
            trailing.setActive(true);
            top.setActive(true);
            bottom.setActive(true);

            let top_height = top_bar.heightAnchor().constraintEqualToConstant(44.0);
            top_height.setActive(true);

            self.setView(&root_view);
            
            // Register for search complete notification
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentMcpSearchComplete");
            unsafe {
                center.addObserver_selector_name_object(
                    self,
                    sel!(searchCompleteNotification:),
                    Some(&name),
                    None,
                );
            }
            
            log_to_file("loadView completed");
        }

        #[unsafe(method(searchCompleteNotification:))]
        fn search_complete_notification(&self, _notification: Option<&NSObject>) {
            log_to_file("Search complete notification received");
            self.update_search_results();
        }

        #[unsafe(method(backClicked:))]
        fn back_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("Back clicked");
            use objc2_foundation::NSNotificationCenter;
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str("PersonalAgentShowSettingsView");
            unsafe { center.postNotificationName_object(&name, None); }
        }

        #[unsafe(method(nextClicked:))]
        fn next_clicked(&self, _sender: Option<&NSObject>) {
            log_to_file("Next clicked");
            
            let url = if let Some(field) = &*self.ivars().url_input.borrow() {
                field.stringValue().to_string()
            } else {
                String::new()
            };
            
            match parse_mcp_url(&url) {
                Ok(parsed) => {
                    log_to_file(&format!("Parsed MCP: {parsed:?}"));
                    PARSED_MCP.with(|cell| {
                        *cell.borrow_mut() = Some(parsed);
                    });
                    
                    use objc2_foundation::NSNotificationCenter;
                    let center = NSNotificationCenter::defaultCenter();
                    let name = NSString::from_str("PersonalAgentShowConfigureMcp");
                    unsafe { center.postNotificationName_object(&name, None); }
                }
                Err(e) => {
                    log_to_file(&format!("Parse error: {e}"));
                    
                    use objc2_app_kit::NSAlert;
                    let mtm = MainThreadMarker::new().unwrap();
                    let alert = NSAlert::new(mtm);
                    alert.setMessageText(&NSString::from_str("Invalid URL"));
                    alert.setInformativeText(&NSString::from_str(&e));
                    alert.addButtonWithTitle(&NSString::from_str("OK"));
                    unsafe { alert.runModal() };
                }
            }
        }

        #[unsafe(method(urlChanged:))]
        fn url_changed(&self, _sender: Option<&NSObject>) {
            // Enable Next button when URL is non-empty
            let url = if let Some(field) = &*self.ivars().url_input.borrow() {
                field.stringValue().to_string()
            } else {
                String::new()
            };
            
            let is_valid = !url.trim().is_empty();
            
            if let Some(btn) = &*self.ivars().next_button.borrow() {
                btn.setEnabled(is_valid);
            }
        }

        #[unsafe(method(searchFieldAction:))]
        fn search_field_action(&self, _sender: Option<&NSObject>) {
            let query = if let Some(field) = &*self.ivars().search_field.borrow() {
                field.stringValue().to_string()
            } else {
                String::new()
            };
            
            if query.trim().is_empty() {
                return;
            }
            
            log_to_file(&format!("Search triggered: {query}"));
            self.perform_search(query);
        }

        #[unsafe(method(resultClicked:))]
        fn result_clicked(&self, sender: Option<&NSObject>) {
            if let Some(button) = sender.and_then(|s| s.downcast_ref::<NSButton>()) {
                let tag = button.tag() as usize;
                log_to_file(&format!("Result clicked with tag: {tag}"));
                
                // Get the registry entry from the tag
                // We'll store search results in a thread-local
                SEARCH_RESULTS.with(|cell| {
                    let results = cell.borrow();
                    if let Some(ref entries) = *results {
                        if tag < entries.len() {
                            let wrapper = &entries[tag];
                            
                            match McpRegistry::entry_to_config(wrapper) {
                                Ok(config) => {
                                    log_to_file(&format!("Converted to config: {}", config.name));
                                    SELECTED_MCP_CONFIG.with(|cell| {
                                        *cell.borrow_mut() = Some(config);
                                    });
                                    
                                    use objc2_foundation::NSNotificationCenter;
                                    let center = NSNotificationCenter::defaultCenter();
                                    let name = NSString::from_str("PersonalAgentShowConfigureMcp");
                                    unsafe { center.postNotificationName_object(&name, None); }
                                }
                                Err(e) => {
                                    log_to_file(&format!("Failed to convert entry: {e}"));
                                }
                            }
                        }
                    }
                });
            }
        }
    }
);

impl McpAddViewController {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let ivars = McpAddViewIvars {
            search_field: RefCell::new(None),
            results_stack: RefCell::new(None),
            loading_label: RefCell::new(None),
            results_scroll: RefCell::new(None),
            url_input: RefCell::new(None),
            next_button: RefCell::new(None),
        };
        let this = mtm.alloc::<Self>().set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }

    fn perform_search(&self, query: String) {
        log_to_file("perform_search started");
        
        // Show loading state
        self.show_loading(true);
        
        // Clear previous results
        SEARCH_RESULTS.with(|cell| {
            *cell.borrow_mut() = None;
        });
        
        // Spawn thread to do search
        let self_ptr = self as *const Self;
        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Runtime::new() {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to create runtime: {e}");
                    return;
                }
            };
            
            let registry = McpRegistry::new();
            let results = runtime.block_on(async {
                registry.search(&query).await
            });
            
            match results {
                Ok(search_results) => {
                    log_to_file(&format!("Search found {} results", search_results.entries.len()));
                    
                    // Store results in thread-local
                    SEARCH_RESULTS.with(|cell| {
                        *cell.borrow_mut() = Some(search_results.entries.clone());
                    });
                    
                    // Post notification to update UI on main thread
                    use objc2_foundation::NSNotificationCenter;
                    let center = NSNotificationCenter::defaultCenter();
                    let name = NSString::from_str("PersonalAgentMcpSearchComplete");
                    unsafe { center.postNotificationName_object(&name, None); }
                }
                Err(e) => {
                    eprintln!("Search failed: {e}");
                    
                    // Post error notification
                    use objc2_foundation::NSNotificationCenter;
                    let center = NSNotificationCenter::defaultCenter();
                    let name = NSString::from_str("PersonalAgentMcpSearchError");
                    unsafe { center.postNotificationName_object(&name, None); }
                }
            }
        });
    }

    fn show_loading(&self, show: bool) {
        if let Some(label) = &*self.ivars().loading_label.borrow() {
            label.setHidden(!show);
        }
    }

    fn update_search_results(&self) {
        let mtm = MainThreadMarker::new().unwrap();
        
        self.show_loading(false);
        
        if let Some(stack) = &*self.ivars().results_stack.borrow() {
            // Clear existing results
            let subviews = stack.subviews();
            for view in &subviews {
                unsafe {
                    stack.removeArrangedSubview(&view);
                }
                view.removeFromSuperview();
            }
            
            // Add new results
            SEARCH_RESULTS.with(|cell| {
                let results = cell.borrow();
                if let Some(ref entries) = *results {
                    if entries.is_empty() {
                        // Show "no results" message
                        let label = NSTextField::labelWithString(&NSString::from_str("No servers found."), mtm);
                        label.setTextColor(Some(&Theme::text_secondary_color()));
                        label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
                        unsafe {
                            stack.addArrangedSubview(&label);
                        }
                    } else {
                        // Add result rows
                        for (index, wrapper) in entries.iter().enumerate() {
                            let row = self.create_result_row(&wrapper.server, index, mtm);
                            unsafe {
                                stack.addArrangedSubview(&row);
                            }
                        }
                    }
                }
            });
        }
    }

    fn create_result_row(
        &self,
        server: &personal_agent::mcp::registry::McpRegistryServer,
        index: usize,
        mtm: MainThreadMarker,
    ) -> Retained<NSView> {
        // Create button that acts as the row
        let button = NSButton::initWithFrame(
            NSButton::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(360.0, 44.0)),
        );
        button.setButtonType(NSButtonType::MomentaryPushIn);
        button.setBezelStyle(NSBezelStyle::Rounded);
        button.setBordered(true);
        button.setAlignment(objc2_app_kit::NSTextAlignment::Left);
        button.setTag(index as isize);
        
        unsafe {
            button.setTarget(Some(self));
            button.setAction(Some(sel!(resultClicked:)));
            button.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        
        // Create content stack inside button
        let content = NSStackView::new(mtm);
        unsafe {
            content.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            content.setSpacing(2.0);
            content.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
            content.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        
        // Name label (bold)
        let name_label = NSTextField::labelWithString(&NSString::from_str(&server.name), mtm);
        name_label.setFont(Some(&NSFont::boldSystemFontOfSize(12.0)));
        name_label.setTextColor(Some(&Theme::text_primary()));
        unsafe {
            content.addArrangedSubview(&name_label);
        }
        
        // Description label (truncated)
        let desc_text = if server.description.len() > 60 {
            format!("{}...", &server.description[..60])
        } else {
            server.description.clone()
        };
        let desc_label = NSTextField::labelWithString(&NSString::from_str(&desc_text), mtm);
        desc_label.setFont(Some(&NSFont::systemFontOfSize(10.0)));
        desc_label.setTextColor(Some(&Theme::text_secondary_color()));
        unsafe {
            content.addArrangedSubview(&desc_label);
        }
        
        // Add content to button
        button.addSubview(&content);
        
        // Constrain content to fill button with padding
        unsafe {
            let leading = content.leadingAnchor().constraintEqualToAnchor_constant(&button.leadingAnchor(), 8.0);
            let trailing = content.trailingAnchor().constraintEqualToAnchor_constant(&button.trailingAnchor(), -8.0);
            let top = content.topAnchor().constraintEqualToAnchor_constant(&button.topAnchor(), 6.0);
            let bottom = content.bottomAnchor().constraintEqualToAnchor_constant(&button.bottomAnchor(), -6.0);
            leading.setActive(true);
            trailing.setActive(true);
            top.setActive(true);
            bottom.setActive(true);
            
            let width = button.widthAnchor().constraintEqualToConstant(360.0);
            let height = button.heightAnchor().constraintEqualToConstant(44.0);
            width.setActive(true);
            height.setActive(true);
        }
        
        Retained::from(&*button as &NSView)
    }

    fn build_top_bar(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let top_bar = NSStackView::new(mtm);
        top_bar.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        top_bar.setSpacing(8.0);
        top_bar.setEdgeInsets(NSEdgeInsets { top: 8.0, left: 12.0, bottom: 8.0, right: 12.0 });
        top_bar.setTranslatesAutoresizingMaskIntoConstraints(false);

        top_bar.setWantsLayer(true);
        if let Some(layer) = top_bar.layer() {
            let color = CGColor::new_generic_rgb(Theme::BG_DARK.0, Theme::BG_DARK.1, Theme::BG_DARK.2, 1.0);
            layer.setBackgroundColor(Some(&color));
        }

        // Back button
        let back_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("< Add MCP"),
                Some(self),
                Some(sel!(backClicked:)),
                mtm,
            )
        };
        back_btn.setBezelStyle(NSBezelStyle::Rounded);
        let back_width = back_btn.widthAnchor().constraintEqualToConstant(100.0);
        back_width.setActive(true);
        top_bar.addArrangedSubview(&back_btn);

        // Spacer
        let spacer = NSView::new(mtm);
        spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
        top_bar.addArrangedSubview(&spacer);

        Retained::from(&*top_bar as &NSView)
    }

    fn build_content(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let content = NSStackView::new(mtm);
        content.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        content.setSpacing(16.0);
        content.setEdgeInsets(NSEdgeInsets { top: 16.0, left: 16.0, bottom: 16.0, right: 16.0 });
        content.setTranslatesAutoresizingMaskIntoConstraints(false);
        content.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);

        // ===== SEARCH SECTION =====
        
        // Search label
        let search_label = NSTextField::labelWithString(
            &NSString::from_str("Search MCP Registry:"),
            mtm,
        );
        search_label.setTextColor(Some(&Theme::text_primary()));
        search_label.setFont(Some(&NSFont::boldSystemFontOfSize(12.0)));
        content.addArrangedSubview(&search_label);

        // Search field
        let search_field = NSTextField::new(mtm);
        search_field.setPlaceholderString(Some(&NSString::from_str("Search MCP servers...")));
        search_field.setTranslatesAutoresizingMaskIntoConstraints(false);
        unsafe {
            search_field.setTarget(Some(self));
            search_field.setAction(Some(sel!(searchFieldAction:)));
        }
        let width = search_field.widthAnchor().constraintGreaterThanOrEqualToConstant(360.0);
        width.setActive(true);
        content.addArrangedSubview(&search_field);
        *self.ivars().search_field.borrow_mut() = Some(search_field);

        // Loading label
        let loading_label = NSTextField::labelWithString(&NSString::from_str("Searching..."), mtm);
        loading_label.setTextColor(Some(&Theme::text_secondary_color()));
        loading_label.setFont(Some(&NSFont::systemFontOfSize(11.0)));
        loading_label.setHidden(true);
        content.addArrangedSubview(&loading_label);
        *self.ivars().loading_label.borrow_mut() = Some(loading_label);

        // Results scroll view
        let results_scroll = NSScrollView::new(mtm);
        results_scroll.setHasVerticalScroller(true);
        results_scroll.setDrawsBackground(false);
        unsafe {
            results_scroll.setAutohidesScrollers(true);
            results_scroll.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        
        // Results stack (flipped so items start at top)
        let results_stack = super::FlippedStackView::new(mtm);
        unsafe {
            results_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            results_stack.setSpacing(4.0);
            results_stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
            results_stack.setDistribution(NSStackViewDistribution::Fill);
        }
        results_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        
        results_scroll.setDocumentView(Some(&results_stack));
        
        // Constrain results scroll height
        let scroll_height = results_scroll.heightAnchor().constraintEqualToConstant(150.0);
        scroll_height.setActive(true);
        let scroll_width = results_scroll.widthAnchor().constraintEqualToConstant(360.0);
        scroll_width.setActive(true);
        
        content.addArrangedSubview(&results_scroll);
        *self.ivars().results_stack.borrow_mut() = Some(Retained::from(&*results_stack as &NSStackView));
        *self.ivars().results_scroll.borrow_mut() = Some(results_scroll);

        // Separator
        let separator = NSView::new(mtm);
        separator.setWantsLayer(true);
        if let Some(layer) = separator.layer() {
            let color = CGColor::new_generic_rgb(0.3, 0.3, 0.3, 1.0);
            layer.setBackgroundColor(Some(&color));
        }
        unsafe {
            separator.setTranslatesAutoresizingMaskIntoConstraints(false);
            let height = separator.heightAnchor().constraintEqualToConstant(1.0);
            let width = separator.widthAnchor().constraintEqualToConstant(360.0);
            height.setActive(true);
            width.setActive(true);
        }
        content.addArrangedSubview(&separator);

        // ===== MANUAL URL SECTION =====

        // Manual URL label
        let instructions = NSTextField::labelWithString(
            &NSString::from_str("Or enter URL manually:"),
            mtm,
        );
        instructions.setTextColor(Some(&Theme::text_primary()));
        instructions.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        content.addArrangedSubview(&instructions);

        // URL input field
        let url_field = NSTextField::new(mtm);
        url_field.setPlaceholderString(Some(&NSString::from_str("npx -y @modelcontextprotocol/server-filesystem")));
        url_field.setTranslatesAutoresizingMaskIntoConstraints(false);
        unsafe {
            url_field.setTarget(Some(self));
            url_field.setAction(Some(sel!(urlChanged:)));
        }
        let width = url_field.widthAnchor().constraintGreaterThanOrEqualToConstant(360.0);
        width.setActive(true);
        content.addArrangedSubview(&url_field);
        *self.ivars().url_input.borrow_mut() = Some(url_field);

        // Examples section
        let examples_label = NSTextField::labelWithString(&NSString::from_str("Examples:"), mtm);
        examples_label.setTextColor(Some(&Theme::text_secondary_color()));
        examples_label.setFont(Some(&NSFont::boldSystemFontOfSize(11.0)));
        content.addArrangedSubview(&examples_label);

        let example1 = NSTextField::labelWithString(
            &NSString::from_str("  npx -y @modelcontextprotocol/server-filesystem"),
            mtm,
        );
        example1.setTextColor(Some(&Theme::text_secondary_color()));
        example1.setFont(Some(&NSFont::systemFontOfSize(10.0)));
        content.addArrangedSubview(&example1);

        let example2 = NSTextField::labelWithString(
            &NSString::from_str("  docker run mcp/server:latest"),
            mtm,
        );
        example2.setTextColor(Some(&Theme::text_secondary_color()));
        example2.setFont(Some(&NSFont::systemFontOfSize(10.0)));
        content.addArrangedSubview(&example2);

        let example3 = NSTextField::labelWithString(
            &NSString::from_str("  https://example.com/mcp"),
            mtm,
        );
        example3.setTextColor(Some(&Theme::text_secondary_color()));
        example3.setFont(Some(&NSFont::systemFontOfSize(10.0)));
        content.addArrangedSubview(&example3);

        // Spacer to push Next button to bottom
        let spacer = NSView::new(mtm);
        spacer.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Vertical);
        content.addArrangedSubview(&spacer);

        // Next button
        let next_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Next"),
                Some(self),
                Some(sel!(nextClicked:)),
                mtm,
            )
        };
        next_btn.setBezelStyle(NSBezelStyle::Rounded);
        next_btn.setEnabled(false); // Disabled until URL is entered
        let next_width = next_btn.widthAnchor().constraintEqualToConstant(80.0);
        next_width.setActive(true);
        content.addArrangedSubview(&next_btn);
        *self.ivars().next_button.borrow_mut() = Some(next_btn);

        Retained::from(&*content as &NSView)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_npx_with_flags() {
        let result = parse_mcp_url("npx -y @modelcontextprotocol/server-filesystem").unwrap();
        match result {
            ParsedMcp::Npm { identifier, runtime_hint } => {
                assert_eq!(identifier, "@modelcontextprotocol/server-filesystem");
                assert_eq!(runtime_hint, "npx");
            }
            _ => panic!("Expected Npm variant"),
        }
    }

    #[test]
    fn test_parse_npx_without_flags() {
        let result = parse_mcp_url("npx @package/name").unwrap();
        match result {
            ParsedMcp::Npm { identifier, .. } => {
                assert_eq!(identifier, "@package/name");
            }
            _ => panic!("Expected Npm variant"),
        }
    }

    #[test]
    fn test_parse_bare_package() {
        let result = parse_mcp_url("@org/package").unwrap();
        match result {
            ParsedMcp::Npm { identifier, .. } => {
                assert_eq!(identifier, "@org/package");
            }
            _ => panic!("Expected Npm variant"),
        }
    }

    #[test]
    fn test_parse_docker() {
        let result = parse_mcp_url("docker run mcp/server:latest").unwrap();
        match result {
            ParsedMcp::Docker { image } => {
                assert_eq!(image, "mcp/server:latest");
            }
            _ => panic!("Expected Docker variant"),
        }
    }

    #[test]
    fn test_parse_http() {
        let result = parse_mcp_url("https://example.com/mcp").unwrap();
        match result {
            ParsedMcp::Http { url } => {
                assert_eq!(url, "https://example.com/mcp");
            }
            _ => panic!("Expected Http variant"),
        }
    }

    #[test]
    fn test_parse_invalid() {
        let result = parse_mcp_url("invalid");
        assert!(result.is_err());
    }
}
