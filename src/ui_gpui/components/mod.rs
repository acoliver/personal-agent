//! GPUI Components Library
//!
//! @plan PLAN-20250130-GPUIREDUX.P02

// Existing components
pub mod tab_bar;
pub mod message_bubble;
pub mod input_bar;
pub mod button;

pub use tab_bar::{Tab, TabBar};
pub use message_bubble::{UserBubble, AssistantBubble};
pub use input_bar::InputBar;
pub use button::Button;

// Phase 02: Component Library additions
pub mod text_field;
pub mod secure_text_field;
pub mod dropdown;
pub mod toggle;
pub mod list;
pub mod top_bar;
pub mod icon_button;
pub mod divider;

// Exports - Phase 02
pub use text_field::TextField;
pub use secure_text_field::SecureTextField;
pub use dropdown::Dropdown;
pub use toggle::Toggle;
pub use list::List;
pub use top_bar::{TopBar, TopBarButton};
pub use icon_button::IconButton;
pub use divider::Divider;
