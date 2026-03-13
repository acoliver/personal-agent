use personal_agent::presentation::view_command::ViewCommand;

#[test]
fn chat_view_handles_loaded_transcript_before_incremental_appends() {
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");

    let loaded_pos = source
        .find("ViewCommand::ConversationMessagesLoaded")
        .expect("ChatView should handle ConversationMessagesLoaded");
    let append_pos = source
        .find("ViewCommand::MessageAppended")
        .expect("ChatView should handle MessageAppended");

    assert!(
        loaded_pos < append_pos,
        "ChatView should replace the active transcript before processing incremental appends so replayed selections do not accumulate stale messages"
    );
}

#[test]
fn chat_view_loaded_transcript_replaces_messages_for_active_conversation() {
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");

    let loaded_pos = source
        .find("ViewCommand::ConversationMessagesLoaded")
        .expect("ChatView should handle ConversationMessagesLoaded");
    let window = &source[loaded_pos..std::cmp::min(loaded_pos + 900, source.len())];

    assert!(
        window.contains("self.state.messages = Self::messages_from_payload(messages, &current_model);")
            || window.contains("self.state.messages = Self::messages_from_payload(messages"),
        "Selecting or startup-loading a conversation should replace the visible transcript instead of appending into stale state"
    );
}

#[test]
fn replay_payload_preserves_thinking_and_timestamp_fidelity() {
    let presenter_source = include_str!("../src/presentation/chat_presenter.rs");
    let payload_source = include_str!("../src/presentation/view_command.rs");
    let chat_view_source = include_str!("../src/ui_gpui/views/chat_view.rs");

    assert!(
        payload_source.contains("pub thinking_content: Option<String>")
            && payload_source.contains("pub timestamp: Option<u64>"),
        "Conversation replay payload should carry thinking content and timestamp so transcript reloads preserve assistant fidelity"
    );
    assert!(
        presenter_source.contains("thinking_content: message.thinking_content")
            && presenter_source.contains("timestamp: Some(message.timestamp.timestamp_millis() as u64)"),
        "ChatPresenter replay should include persisted thinking content and timestamps in the bulk transcript payload"
    );
    assert!(
        chat_view_source.contains("chat_message = chat_message.with_thinking(thinking);")
            && chat_view_source.contains("chat_message = chat_message.with_timestamp(timestamp);"),
        "ChatView transcript reconstruction should restore replayed thinking content and timestamps"
    );
}

#[test]
fn presenter_replay_uses_bulk_loaded_transcript_command() {
    let source = include_str!("../src/presentation/chat_presenter.rs");

    let replay_pos = source
        .find("async fn replay_conversation_messages")
        .expect("ChatPresenter replay helper should exist");
    let window = &source[replay_pos..std::cmp::min(replay_pos + 1400, source.len())];

    assert!(
        window.contains("ConversationMessagesLoaded"),
        "Conversation replay should emit ConversationMessagesLoaded so the active transcript is replaced atomically"
    );
    assert!(
        !window.contains("send(ViewCommand::MessageAppended"),
        "Conversation replay should not rebuild selected transcripts via repeated MessageAppended commands"
    );
}

#[test]
fn view_command_exposes_conversation_messages_loaded_variant() {
    let source = include_str!("../src/presentation/view_command.rs");
    assert!(
        source.contains("ConversationMessagesLoaded"),
        "ViewCommand should expose a transcript replacement command for conversation replay"
    );

    let _ = std::mem::discriminant(&ViewCommand::ConversationCleared);
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P03
/// @requirement REQ-INT-001.2
/// @requirement REQ-ARCH-003.2
/// @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:001-063
#[test]
fn manual_selection_still_clears_local_transcript_before_runtime_replay_arrives() {
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");

    let select_pos = source
        .find("fn select_conversation_at_index")
        .expect("ChatView selection handler should exist");
    let window = &source[select_pos..std::cmp::min(select_pos + 1200, source.len())];

    assert!(
        !window.contains("self.state.messages.clear();"),
        "auxiliary source/readback guardrail: manual selection should not clear local transcript before runtime replay arrives, but the current ChatView path still does so and therefore remains expected-red until authoritative-store ownership exists"
    );
}
