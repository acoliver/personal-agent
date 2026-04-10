//! GPUI Components Library
//!
//! @plan PLAN-20250130-GPUIREDUX.P02

// Existing components
pub mod approval_bubble;
pub mod button;
pub mod input_bar;
pub mod message_bubble;
pub mod tab_bar;

pub use approval_bubble::ApprovalBubble;
pub use button::Button;
pub use input_bar::InputBar;
pub use message_bubble::{AssistantBubble, UserBubble};
pub use tab_bar::{Tab, TabBar};

// Issue 51: Error Log Viewer
pub mod bug_icon;

// Issue 57: Window mode icons (popout/popin/sidebar)
pub mod window_icons;

// Issue 139: Emoji filter toggle icons
pub mod emoji_filter_icon;

// Issue 152: Message bubble copy action icons
pub mod copy_icons;

// Issue 62: Markdown Rendering (TDD Phase)
pub(crate) mod markdown_content;

// Phase 02: Component Library additions
pub mod divider;
pub mod dropdown;
pub mod icon_button;
pub mod secure_text_field;
pub mod selectable_text;
pub mod text_field;
pub mod toggle;
pub mod top_bar;

// Exports - Phase 02
pub use divider::Divider;
pub use dropdown::Dropdown;
pub use icon_button::IconButton;
pub use secure_text_field::SecureTextField;
pub use selectable_text::{find_paragraph_boundaries, find_word_boundaries};
pub use text_field::TextField;
pub use toggle::Toggle;
pub use top_bar::{TopBar, TopBarButton};
