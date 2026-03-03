//! GPUI Components Library
//!
//! @plan PLAN-20250130-GPUIREDUX.P02

// Existing components
pub mod button;
pub mod input_bar;
pub mod message_bubble;
pub mod tab_bar;

pub use button::Button;
pub use input_bar::InputBar;
pub use message_bubble::{AssistantBubble, UserBubble};
pub use tab_bar::{Tab, TabBar};

// Phase 02: Component Library additions
pub mod divider;
pub mod dropdown;
pub mod icon_button;
pub mod list;
pub mod secure_text_field;
pub mod text_field;
pub mod toggle;
pub mod top_bar;

// Exports - Phase 02
pub use divider::Divider;
pub use dropdown::Dropdown;
pub use icon_button::IconButton;
pub use list::List;
pub use secure_text_field::SecureTextField;
pub use text_field::TextField;
pub use toggle::Toggle;
pub use top_bar::{TopBar, TopBarButton};
