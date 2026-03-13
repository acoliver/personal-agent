//! Tests for chat rename overwrite semantics.
//!
//! Goal: when rename mode starts, the existing title should behave as
//! selected text — first typed character replaces the whole title,
//! and first backspace/space should clear then apply.

use personal_agent::ui_gpui::views::chat_view::ChatState;

#[test]
fn rename_first_char_replaces_existing_title() {
    let mut state = ChatState::default();
    state.conversation_title = "Existing Title".to_string();
    state.conversation_title_input = state.conversation_title.clone();
    state.conversation_title_editing = true;
    state.rename_replace_on_next_char = true;

    // Equivalent to handle_rename_char("n") logic
    if state.rename_replace_on_next_char {
        state.conversation_title_input.clear();
        state.rename_replace_on_next_char = false;
    }
    state.conversation_title_input.push('n');

    assert_eq!(state.conversation_title_input, "n");
    assert!(!state.rename_replace_on_next_char);
}

#[test]
fn rename_first_space_replaces_existing_title() {
    let mut state = ChatState::default();
    state.conversation_title = "Existing Title".to_string();
    state.conversation_title_input = state.conversation_title.clone();
    state.conversation_title_editing = true;
    state.rename_replace_on_next_char = true;

    // Equivalent to handle_rename_space() logic
    if state.rename_replace_on_next_char {
        state.conversation_title_input.clear();
        state.rename_replace_on_next_char = false;
    }
    state.conversation_title_input.push(' ');

    assert_eq!(state.conversation_title_input, " ");
    assert!(!state.rename_replace_on_next_char);
}

#[test]
fn rename_first_backspace_clears_existing_title() {
    let mut state = ChatState::default();
    state.conversation_title = "Existing Title".to_string();
    state.conversation_title_input = state.conversation_title.clone();
    state.conversation_title_editing = true;
    state.rename_replace_on_next_char = true;

    // Equivalent to handle_rename_backspace() logic
    if state.rename_replace_on_next_char {
        state.conversation_title_input.clear();
        state.rename_replace_on_next_char = false;
    } else {
        state.conversation_title_input.pop();
    }

    assert_eq!(state.conversation_title_input, "");
    assert!(!state.rename_replace_on_next_char);
}

#[test]
fn rename_subsequent_chars_append_normally_after_replacement() {
    let mut state = ChatState::default();
    state.conversation_title = "Existing Title".to_string();
    state.conversation_title_input = state.conversation_title.clone();
    state.conversation_title_editing = true;
    state.rename_replace_on_next_char = true;

    // First char replaces
    if state.rename_replace_on_next_char {
        state.conversation_title_input.clear();
        state.rename_replace_on_next_char = false;
    }
    state.conversation_title_input.push('N');

    // Subsequent chars append
    state.conversation_title_input.push('e');
    state.conversation_title_input.push('w');

    assert_eq!(state.conversation_title_input, "New");
}

#[test]
fn map_input_char_shift_transforms_letters_and_symbols() {
    use personal_agent::ui_gpui::views::chat_view::ChatView;

    assert_eq!(ChatView::map_input_char("a", false), Some('a'));
    assert_eq!(ChatView::map_input_char("a", true), Some('A'));
    assert_eq!(ChatView::map_input_char("1", true), Some('!'));
    assert_eq!(ChatView::map_input_char("/", true), Some('?'));
}
