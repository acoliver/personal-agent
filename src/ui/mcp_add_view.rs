//! Add MCP view - enter URL or search registries

use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::Write;

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAppearanceCustomization, NSButton, NSBezelStyle, NSFont, NSLayoutConstraintOrientation, NSStackView, 
    NSStackViewDistribution, NSTextField, NSUserInterfaceLayoutOrientation, NSView, NSViewController,
};
use objc2_foundation::{NSEdgeInsets, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use objc2_core_graphics::CGColor;

use crate::ui::Theme;

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
}

pub struct McpAddViewIvars {
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
            log_to_file("loadView completed");
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
    }
);

impl McpAddViewController {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let ivars = McpAddViewIvars {
            url_input: RefCell::new(None),
            next_button: RefCell::new(None),
        };
        let this = mtm.alloc::<Self>().set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
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

        // Instructions label
        let instructions = NSTextField::labelWithString(
            &NSString::from_str("Enter MCP URL (npx command, docker image, or HTTP URL):"),
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
