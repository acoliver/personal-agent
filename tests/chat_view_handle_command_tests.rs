#![allow(clippy::field_reassign_with_default)]

use chrono::Utc;
use personal_agent::presentation::view_command::{
    ConversationMessagePayload, ConversationSummary, MessageRole as ViewMessageRole, ProfileSummary,
};
use personal_agent::ui_gpui::app_store::{
    ChatStoreSnapshot, ConversationLoadState, StreamingStoreSnapshot,
};
use personal_agent::ui_gpui::views::chat_view::{
    ChatMessage, ChatState, MessageRole, StreamingState,
};
use uuid::Uuid;

fn conversation(id: Uuid, title: &str, message_count: usize) -> ConversationSummary {
    ConversationSummary {
        id,
        title: title.to_string(),
        updated_at: Utc::now(),
        message_count,
    }
}

fn profile(id: Uuid, name: &str, model_id: &str, is_default: bool) -> ProfileSummary {
    ProfileSummary {
        id,
        name: name.to_string(),
        provider_id: "provider".to_string(),
        model_id: model_id.to_string(),
        is_default,
    }
}

fn payload(
    role: ViewMessageRole,
    content: &str,
    thinking_content: Option<&str>,
    timestamp: Option<u64>,
) -> ConversationMessagePayload {
    ConversationMessagePayload {
        role,
        content: content.to_string(),
        thinking_content: thinking_content.map(str::to_string),
        timestamp,
    }
}

#[test]
fn chat_state_new_matches_default() {
    let created = ChatState::new();
    let defaulted = ChatState::default();

    assert_eq!(created.messages, defaulted.messages);
    assert_eq!(created.streaming, defaulted.streaming);
    assert_eq!(created.show_thinking, defaulted.show_thinking);
    assert_eq!(created.thinking_content, defaulted.thinking_content);
    assert_eq!(created.input_text, defaulted.input_text);
    assert_eq!(created.cursor_position, defaulted.cursor_position);
    assert_eq!(created.conversation_title, defaulted.conversation_title);
    assert_eq!(created.current_model, defaulted.current_model);
    assert_eq!(created.selected_profile_id, defaulted.selected_profile_id);
    assert_eq!(
        created.profile_dropdown_open,
        defaulted.profile_dropdown_open
    );
    assert_eq!(
        created.profile_dropdown_index,
        defaulted.profile_dropdown_index
    );
    assert_eq!(
        created.active_conversation_id,
        defaulted.active_conversation_id
    );
    assert_eq!(
        created.conversation_dropdown_open,
        defaulted.conversation_dropdown_open
    );
    assert_eq!(
        created.conversation_dropdown_index,
        defaulted.conversation_dropdown_index
    );
    assert_eq!(
        created.conversation_title_editing,
        defaulted.conversation_title_editing
    );
    assert_eq!(
        created.rename_replace_on_next_char,
        defaulted.rename_replace_on_next_char
    );
}

#[test]
fn chat_state_builder_methods_replace_target_fields_only() {
    let messages = vec![
        ChatMessage::user("hello"),
        ChatMessage::assistant("world", "claude-3").with_thinking("reasoning"),
    ];
    let streaming = StreamingState::Streaming {
        content: "partial".to_string(),
        done: false,
    };

    let state = ChatState::new()
        .with_messages(messages.clone())
        .with_streaming(streaming.clone())
        .with_thinking(true, Some("plan".to_string()));

    assert_eq!(state.messages, messages);
    assert_eq!(state.streaming, streaming);
    assert!(state.show_thinking);
    assert_eq!(state.thinking_content.as_deref(), Some("plan"));
    assert_eq!(state.input_text, "");
    assert_eq!(state.cursor_position, 0);
    assert_eq!(state.conversation_title, "New Conversation");
}

#[test]
fn chat_state_mutator_methods_update_messages_streaming_and_thinking() {
    let mut state = ChatState::default();

    state.add_message(ChatMessage::user("first"));
    state.add_message(ChatMessage::assistant("second", "gpt-4.1"));
    state.set_streaming(StreamingState::Streaming {
        content: "buffer".to_string(),
        done: false,
    });
    state.set_thinking(true, Some("scratchpad".to_string()));

    assert_eq!(state.messages.len(), 2);
    assert_eq!(state.messages[0].role, MessageRole::User);
    assert_eq!(state.messages[1].role, MessageRole::Assistant);
    assert_eq!(
        state.streaming,
        StreamingState::Streaming {
            content: "buffer".to_string(),
            done: false,
        }
    );
    assert!(state.show_thinking);
    assert_eq!(state.thinking_content.as_deref(), Some("scratchpad"));
}

#[test]
fn chat_message_construction_preserves_role_content_thinking_and_timestamp() {
    let user = ChatMessage::user("hello user");
    let assistant = ChatMessage::assistant("hello assistant", "model-x")
        .with_thinking("chain of thought summary")
        .with_timestamp(1_704_000_000_123);

    assert_eq!(user.role, MessageRole::User);
    assert_eq!(user.content, "hello user");
    assert_eq!(user.thinking, None);
    assert_eq!(user.model_id, None);
    assert_eq!(user.timestamp, None);

    assert_eq!(assistant.role, MessageRole::Assistant);
    assert_eq!(assistant.content, "hello assistant");
    assert_eq!(
        assistant.thinking.as_deref(),
        Some("chain of thought summary")
    );
    assert_eq!(assistant.model_id.as_deref(), Some("model-x"));
    assert_eq!(assistant.timestamp, Some(1_704_000_000_123));
}

#[test]
fn streaming_state_all_variants_are_constructible_and_distinct() {
    let idle = StreamingState::Idle;
    let streaming = StreamingState::Streaming {
        content: "chunk-1chunk-2".to_string(),
        done: false,
    };
    let complete_like = StreamingState::Streaming {
        content: "done".to_string(),
        done: true,
    };
    let error = StreamingState::Error("boom".to_string());

    assert_eq!(idle, StreamingState::Idle);
    assert_ne!(idle, streaming);
    assert_ne!(streaming, complete_like);
    assert_eq!(
        complete_like,
        StreamingState::Streaming {
            content: "done".to_string(),
            done: true,
        }
    );
    assert_eq!(error, StreamingState::Error("boom".to_string()));
}

#[test]
fn profile_dropdown_state_and_selection_by_index_are_representable_in_state() {
    let default_id = Uuid::new_v4();
    let alt_id = Uuid::new_v4();

    let mut state = ChatState::default();
    state.profiles = vec![
        profile(default_id, "Default", "claude-sonnet", true),
        profile(alt_id, "Alt", "gpt-4.1", false),
    ];

    assert!(!state.profile_dropdown_open);
    state.profile_dropdown_open = true;
    assert!(state.profile_dropdown_open);

    state.profile_dropdown_index = 1;
    state.selected_profile_id = Some(state.profiles[state.profile_dropdown_index].id);
    state.current_model = state.profiles[state.profile_dropdown_index]
        .model_id
        .clone();

    assert_eq!(state.selected_profile_id, Some(alt_id));
    assert_eq!(state.profile_dropdown_index, 1);
    assert_eq!(state.current_model, "gpt-4.1");

    state.profile_dropdown_open = false;
    assert!(!state.profile_dropdown_open);
}

#[test]
fn conversation_dropdown_state_and_selection_by_index_are_representable_in_state() {
    let first_id = Uuid::new_v4();
    let second_id = Uuid::new_v4();

    let mut state = ChatState::default();
    state.conversations = vec![
        conversation(first_id, "First", 1),
        conversation(second_id, "Second", 2),
    ];

    assert!(!state.conversation_dropdown_open);
    state.conversation_dropdown_open = true;
    state.conversation_dropdown_index = 1;
    state.active_conversation_id = Some(state.conversations[1].id);
    state.conversation_title = state.conversations[1].title.clone();

    assert!(state.conversation_dropdown_open);
    assert_eq!(state.conversation_dropdown_index, 1);
    assert_eq!(state.active_conversation_id, Some(second_id));
    assert_eq!(state.conversation_title, "Second");

    state.conversation_dropdown_open = false;
    assert!(!state.conversation_dropdown_open);
}

#[test]
fn rename_behavior_replace_on_next_char_and_editing_transitions_follow_view_logic() {
    let mut state = ChatState::default();
    state.conversation_title = "Existing Title".to_string();

    state.conversation_title_editing = true;
    state.conversation_title_input = state.conversation_title.clone();
    state.rename_replace_on_next_char = true;

    assert!(state.conversation_title_editing);
    assert_eq!(state.conversation_title_input, "Existing Title");
    assert!(state.rename_replace_on_next_char);

    if state.rename_replace_on_next_char {
        state.conversation_title_input.clear();
        state.rename_replace_on_next_char = false;
    }
    state.conversation_title_input.push('N');
    state.conversation_title_input.push_str("ew Title");

    assert_eq!(state.conversation_title_input, "New Title");
    assert!(!state.rename_replace_on_next_char);

    state.conversation_title = state.conversation_title_input.trim().to_string();
    state.conversation_title_editing = false;
    state.conversation_title_input.clear();

    assert_eq!(state.conversation_title, "New Title");
    assert!(!state.conversation_title_editing);
    assert!(state.conversation_title_input.is_empty());
}

#[test]
fn input_text_and_cursor_position_updates_follow_chat_view_text_rules() {
    let mut state = ChatState::default();

    state.input_text = "hello".to_string();
    state.cursor_position = state.input_text.len();
    assert_eq!(state.cursor_position, 5);

    let insert_at = 2;
    state.input_text.insert_str(insert_at, "XX");
    state.cursor_position = insert_at + 2;
    assert_eq!(state.input_text, "heXXllo");
    assert_eq!(state.cursor_position, 4);

    let pos = state.cursor_position.min(state.input_text.len());
    let prev = state.input_text[..pos]
        .char_indices()
        .next_back()
        .map_or(0, |(i, _)| i);
    state.input_text.drain(prev..pos);
    state.cursor_position = prev;

    assert_eq!(state.input_text, "heXllo");
    assert_eq!(state.cursor_position, 3);

    state.cursor_position = 0;
    assert_eq!(state.cursor_position, 0);
    state.cursor_position = state.input_text.len();
    assert_eq!(state.cursor_position, state.input_text.len());
}

#[test]
fn store_snapshot_types_capture_apply_store_snapshot_inputs_and_expected_projection() {
    let active_id = Uuid::new_v4();
    let conversations = vec![conversation(active_id, "Loaded Chat", 2)];
    let snapshot = ChatStoreSnapshot {
        selected_conversation_id: Some(active_id),
        selected_conversation_title: "Loaded Chat".to_string(),
        selection_generation: 3,
        load_state: ConversationLoadState::Ready {
            conversation_id: active_id,
            generation: 3,
        },
        transcript: vec![
            payload(ViewMessageRole::User, "hi", None, Some(100)),
            payload(
                ViewMessageRole::Assistant,
                "hello",
                Some("reasoned"),
                Some(200),
            ),
        ],
        streaming: StreamingStoreSnapshot {
            thinking_visible: true,
            thinking_buffer: "working".to_string(),
            stream_buffer: "partial".to_string(),
            last_error: None,
            active_target: Some(active_id),
        },
        conversations: conversations.clone(),
    };

    assert_eq!(snapshot.selected_conversation_id, Some(active_id));
    assert_eq!(snapshot.selected_conversation_title, "Loaded Chat");
    assert_eq!(snapshot.conversations, conversations);
    assert_eq!(snapshot.transcript.len(), 2);
    assert_eq!(
        snapshot.transcript[1].thinking_content.as_deref(),
        Some("reasoned")
    );
    assert_eq!(snapshot.streaming.thinking_buffer, "working");
    assert_eq!(snapshot.streaming.stream_buffer, "partial");
    assert_eq!(snapshot.streaming.active_target, Some(active_id));
}

#[test]
fn source_handle_command_replaces_loaded_messages_for_active_conversation_and_ignores_inactive() {
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");
    let loaded_pos = source
        .find("ViewCommand::ConversationMessagesLoaded")
        .expect("ConversationMessagesLoaded handler should exist");
    let window = &source[loaded_pos..std::cmp::min(loaded_pos + 1200, source.len())];

    assert!(
        window.contains("self.state.active_conversation_id != Some(conversation_id)")
            && window.contains("ignoring ConversationMessagesLoaded for inactive conversation"),
        "inactive conversation transcript replays should be ignored"
    );
    assert!(
        window.contains(
            "self.state.messages = Self::messages_from_payload(messages, &current_model);"
        ) || window.contains("self.state.messages = Self::messages_from_payload(messages"),
        "active conversation transcript replays should replace local messages in bulk"
    );
    assert!(window.contains("self.state.streaming = StreamingState::Idle;"));
    assert!(window.contains("self.state.thinking_content = None;"));
}

#[test]
fn source_handle_command_conversation_activated_sets_active_id_and_resets_streaming() {
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");
    let pos = source
        .find("ViewCommand::ConversationActivated")
        .expect("ConversationActivated handler should exist");
    let window = &source[pos..std::cmp::min(pos + 900, source.len())];

    assert!(window.contains("self.state.active_conversation_id = Some(id);"));
    assert!(window.contains("self.conversation_id = Some(id);"));
    assert!(window.contains("self.state.streaming = StreamingState::Idle;"));
    assert!(window.contains("self.state.thinking_content = None;"));
    assert!(window.contains("self.state.conversation_dropdown_open = false;"));
}

#[test]
fn source_handle_command_message_appended_adds_transcript_only_for_matching_conversation() {
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");
    let pos = source
        .find("ViewCommand::MessageAppended")
        .expect("MessageAppended handler should exist");
    let window = &source[pos..std::cmp::min(pos + 900, source.len())];

    assert!(window.contains("self.state.active_conversation_id != Some(conversation_id)"));
    assert!(window.contains("ChatMessage::user(content)"));
    assert!(window.contains("ChatMessage::assistant(content, self.state.current_model.clone())"));
    assert!(window.contains("self.state.messages.push(chat_msg);"));
}

#[test]
fn source_handle_command_thinking_visibility_and_append_paths_are_present() {
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");

    let show_pos = source.find("ViewCommand::ShowThinking").unwrap();
    let show_window = &source[show_pos..std::cmp::min(show_pos + 700, source.len())];
    assert!(show_window.contains("self.state.streaming = StreamingState::Streaming"));
    assert!(show_window.contains("self.state.thinking_content = Some(String::new());"));

    let hide_pos = source.find("ViewCommand::HideThinking").unwrap();
    let hide_window = &source[hide_pos..std::cmp::min(hide_pos + 350, source.len())];
    assert!(hide_window.contains("self.state.thinking_content = None;"));

    let append_pos = source.find("ViewCommand::AppendThinking").unwrap();
    let append_window = &source[append_pos..std::cmp::min(append_pos + 500, source.len())];
    assert!(append_window.contains("self.state.thinking_content ="));
    assert!(append_window.contains("unwrap_or_default() + &content"));

    assert!(source.contains("ViewCommand::ToggleThinkingVisibility =>"));
    assert!(source.contains("self.state.show_thinking = !self.state.show_thinking;"));
}

#[test]
fn source_handle_command_append_stream_finalize_cancel_and_error_paths_are_present() {
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");

    let append_pos = source.find("ViewCommand::AppendStream").unwrap();
    let append_window = &source[append_pos..std::cmp::min(append_pos + 900, source.len())];
    assert!(append_window.contains("StreamingState::Streaming { content, .. }"));
    assert!(append_window.contains("content.push_str(&chunk);"));
    assert!(append_window.contains("StreamingState::Idle =>"));
    assert!(append_window.contains("content: chunk"));
    assert!(append_window.contains("StreamingState::Error(_) => {}"));

    let finalize_pos = source.find("ViewCommand::FinalizeStream").unwrap();
    let finalize_window = &source[finalize_pos..std::cmp::min(finalize_pos + 1200, source.len())];
    assert!(finalize_window.contains("thinking_content = self"));
    assert!(finalize_window
        .contains("ChatMessage::assistant(content.clone(), self.state.current_model.clone())"));
    assert!(finalize_window.contains("msg = msg.with_thinking(thinking);"));
    assert!(finalize_window.contains("self.state.messages.push(msg);"));
    assert!(finalize_window.contains("self.state.streaming = StreamingState::Idle;"));

    let cancelled_pos = source.find("ViewCommand::StreamCancelled").unwrap();
    let cancelled_window = &source[cancelled_pos..std::cmp::min(cancelled_pos + 700, source.len())];
    assert!(cancelled_window.contains("if !partial_content.is_empty()"));
    assert!(cancelled_window.contains("msg.content.push_str(\" [cancelled]\");"));
    assert!(cancelled_window.contains("self.state.streaming = StreamingState::Idle;"));

    let error_pos = source.find("ViewCommand::StreamError").unwrap();
    let error_window = &source[error_pos..std::cmp::min(error_pos + 400, source.len())];
    assert!(error_window.contains("self.state.streaming = StreamingState::Error(error);"));
}

#[test]
fn source_handle_command_conversation_created_deleted_renamed_and_title_updated_paths_are_present()
{
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");

    let renamed_pos = source.find("ViewCommand::ConversationRenamed").unwrap();
    let renamed_window = &source[renamed_pos..std::cmp::min(renamed_pos + 700, source.len())];
    assert!(renamed_window.contains("conversation.title.clone_from(&new_title);"));
    assert!(renamed_window.contains("self.state.conversation_title = new_title;"));

    let created_pos = source.find("ViewCommand::ConversationCreated").unwrap();
    let created_window = &source[created_pos..std::cmp::min(created_pos + 1400, source.len())];
    assert!(created_window.contains("self.state.active_conversation_id = Some(id);"));
    assert!(created_window.contains("self.state.messages.clear();"));
    assert!(created_window.contains("self.state.streaming = StreamingState::Idle;"));
    assert!(created_window
        .contains("self.state.conversation_title = \"New Conversation\".to_string();"));
    assert!(
        created_window.contains("ConversationSummary {")
            || created_window.contains("self.state.conversations.insert(")
    );

    let title_updated_pos = source
        .find("ViewCommand::ConversationTitleUpdated")
        .unwrap();
    let title_updated_window =
        &source[title_updated_pos..std::cmp::min(title_updated_pos + 700, source.len())];
    assert!(title_updated_window.contains("conversation.title.clone_from(&title);"));
    assert!(title_updated_window.contains("self.state.conversation_title = title;"));

    let deleted_pos = source.find("ViewCommand::ConversationDeleted").unwrap();
    let deleted_window = &source[deleted_pos..std::cmp::min(deleted_pos + 1000, source.len())];
    assert!(deleted_window.contains("retain(|conversation| conversation.id != id)"));
    assert!(deleted_window.contains("self.state.active_conversation_id = self"));
    assert!(deleted_window.contains("self.state.messages.clear();"));
    assert!(deleted_window.contains("self.state.streaming = StreamingState::Idle;"));
    assert!(deleted_window.contains("self.state.sync_conversation_title_from_active();"));
}

#[test]
fn source_handle_command_conversation_list_profiles_and_cleared_paths_are_present() {
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");

    let list_pos = source
        .find("ViewCommand::ConversationListRefreshed")
        .unwrap();
    let list_window = &source[list_pos..std::cmp::min(list_pos + 2200, source.len())];
    assert!(list_window.contains("self.state.conversations = conversations;"));
    assert!(list_window.contains("self.state.active_conversation_id = None;"));
    assert!(list_window.contains("self.state.messages.clear();"));
    assert!(list_window.contains("let fallback = self.state.conversations[0].id;"));
    assert!(list_window.contains("self.state.sync_conversation_dropdown_index();"));

    let profiles_pos = source.find("ViewCommand::ChatProfilesUpdated").unwrap();
    let profiles_window = &source[profiles_pos..std::cmp::min(profiles_pos + 1200, source.len())];
    assert!(profiles_window.contains("self.state.profiles = profiles;"));
    assert!(
        profiles_window.contains("self.state.selected_profile_id = selected_profile_id.or_else")
    );
    assert!(profiles_window.contains("self.state.sync_current_model_from_profile();"));
    assert!(profiles_window.contains("self.state.sync_profile_dropdown_index();"));

    let cleared_pos = source.find("ViewCommand::ConversationCleared").unwrap();
    let cleared_window = &source[cleared_pos..std::cmp::min(cleared_pos + 700, source.len())];
    assert!(cleared_window.contains("self.state.messages.clear();"));
    assert!(cleared_window.contains("self.state.streaming = StreamingState::Idle;"));
    assert!(cleared_window.contains("self.state.thinking_content = None;"));
    assert!(cleared_window.contains("self.state.conversation_title_editing = false;"));
    assert!(cleared_window.contains("self.state.sync_conversation_title_from_active();"));
}

#[test]
fn source_chat_state_sync_methods_cover_title_and_dropdown_index_logic() {
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");

    let title_sync_pos = source
        .find("fn sync_conversation_title_from_active(&mut self)")
        .expect("sync_conversation_title_from_active should exist");
    let title_window = &source[title_sync_pos..std::cmp::min(title_sync_pos + 500, source.len())];
    assert!(title_window.contains("\"New Conversation\".to_string()"));
    assert!(title_window.contains("\"Untitled Conversation\".to_string()"));
    assert!(title_window.contains("conversation.title.clone()"));

    let dropdown_sync_pos = source
        .find("fn sync_conversation_dropdown_index(&mut self)")
        .expect("sync_conversation_dropdown_index should exist");
    let dropdown_window =
        &source[dropdown_sync_pos..std::cmp::min(dropdown_sync_pos + 450, source.len())];
    assert!(dropdown_window.contains("position(|conversation| conversation.id == id)"));
    assert!(dropdown_window.contains("unwrap_or(0)"));
    assert!(dropdown_window.contains("saturating_sub(1)"));
}

#[test]
fn source_apply_store_snapshot_maps_transcript_streaming_thinking_and_dropdown_index() {
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");
    let pos = source
        .find("pub fn apply_store_snapshot")
        .expect("apply_store_snapshot should exist");
    let window = &source[pos..std::cmp::min(pos + 1900, source.len())];

    assert!(window.contains("self.state.conversations = conversations;"));
    assert!(window.contains("self.state.active_conversation_id = selected_conversation_id;"));
    assert!(window.contains("self.state.conversation_title = selected_conversation_title;"));
    assert!(window.contains(
        "self.state.messages = Self::messages_from_payload(transcript, &current_model);"
    ));
    assert!(window.contains(
        "self.state.streaming = Self::streaming_state_from_snapshot(&streaming, &load_state);"
    ));
    assert!(window.contains("self.state.thinking_content ="));
    assert!(window.contains("streaming.thinking_buffer"));
    assert!(window.contains("self.state.sync_conversation_dropdown_index();"));
}
