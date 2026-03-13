//! Navigation system for GPUI views
//!
//! Implements a stack-based navigation system that allows views to be pushed
//! and popped, supporting forward navigation to new views and back navigation.
//!
//! @plan PLAN-20250130-GPUIREDUX.P01

use crate::presentation::view_command::ViewId;

/// Navigation state managing a stack of views
///
/// The navigation stack always contains at least one view (Chat by default).
/// Navigation pushes new views onto the stack, and back navigation pops them.
#[derive(Debug, Clone)]
pub struct NavigationState {
    stack: Vec<ViewId>,
}

impl NavigationState {
    /// Create a new navigation state with Chat as the root view
    pub fn new() -> Self {
        Self {
            stack: vec![ViewId::Chat],
        }
    }

    /// Get the current (top) view from the navigation stack
    pub fn current(&self) -> ViewId {
        self.stack.last().copied().unwrap_or(ViewId::Chat)
    }

    /// Get the current depth of the navigation stack
    pub fn stack_depth(&self) -> usize {
        self.stack.len()
    }

    /// Check if we can navigate back (stack has more than one view)
    pub fn can_go_back(&self) -> bool {
        self.stack.len() > 1
    }

    /// Navigate to a view.
    ///
    /// If the target is already in the stack, pops back to it instead of
    /// pushing a duplicate. Otherwise pushes the new view on top.
    /// Does nothing if the current view is already the target.
    pub fn navigate(&mut self, to: ViewId) {
        if self.current() == to {
            return;
        }
        if let Some(pos) = self.stack.iter().rposition(|v| *v == to) {
            self.stack.truncate(pos + 1);
        } else {
            self.stack.push(to);
        }
    }

    /// Navigate back to the previous view
    ///
    /// Pops the current view from the stack if there's more than one view.
    /// Returns true if navigation occurred, false if already at root.
    pub fn navigate_back(&mut self) -> bool {
        if self.stack.len() > 1 {
            self.stack.pop();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_chat() {
        let nav = NavigationState::new();
        assert_eq!(nav.current(), ViewId::Chat);
        assert_eq!(nav.stack_depth(), 1);
    }

    #[test]
    fn test_navigate_pushes_to_stack() {
        let mut nav = NavigationState::new();
        nav.navigate(ViewId::Settings);

        assert_eq!(nav.current(), ViewId::Settings);
        assert_eq!(nav.stack_depth(), 2);
    }

    #[test]
    fn test_navigate_back_pops_stack() {
        let mut nav = NavigationState::new();
        nav.navigate(ViewId::Settings);
        nav.navigate(ViewId::ProfileEditor);

        assert_eq!(nav.stack_depth(), 3);

        nav.navigate_back();
        assert_eq!(nav.current(), ViewId::Settings);
        assert_eq!(nav.stack_depth(), 2);
    }

    #[test]
    fn test_navigate_back_at_root_stays_at_root() {
        let mut nav = NavigationState::new();
        nav.navigate_back(); // Already at Chat

        assert_eq!(nav.current(), ViewId::Chat);
        assert_eq!(nav.stack_depth(), 1);
    }

    #[test]
    fn test_navigate_to_same_view_does_nothing() {
        let mut nav = NavigationState::new();
        nav.navigate(ViewId::Chat); // Already at Chat

        assert_eq!(nav.stack_depth(), 1);
    }

    #[test]
    fn test_navigate_to_existing_view_pops_back() {
        let mut nav = NavigationState::new();
        nav.navigate(ViewId::Settings);
        nav.navigate(ViewId::ProfileEditor);
        nav.navigate(ViewId::ModelSelector);

        assert_eq!(nav.stack_depth(), 4);

        // Navigating to Settings should pop back to it, not push a duplicate
        nav.navigate(ViewId::Settings);
        assert_eq!(nav.current(), ViewId::Settings);
        assert_eq!(nav.stack_depth(), 2); // [Chat, Settings]
    }

    #[test]
    fn test_can_go_back_returns_false_at_root() {
        let nav = NavigationState::new();
        assert!(!nav.can_go_back());
    }

    #[test]
    fn test_can_go_back_returns_true_when_stacked() {
        let mut nav = NavigationState::new();
        nav.navigate(ViewId::Settings);
        assert!(nav.can_go_back());
    }
}
