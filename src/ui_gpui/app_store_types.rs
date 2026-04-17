use std::collections::HashMap;
use std::hash::BuildHasher;

use uuid::Uuid;

use crate::presentation::view_command::{
    ConversationMessagePayload, ConversationSummary, ProfileSummary,
};

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum ConversationLoadState {
    #[default]
    Idle,
    Loading {
        conversation_id: Uuid,
        generation: u64,
    },
    Ready {
        conversation_id: Uuid,
        generation: u64,
    },
    Error {
        conversation_id: Uuid,
        generation: u64,
        message: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StreamingStoreSnapshot {
    pub thinking_visible: bool,
    pub thinking_buffer: String,
    pub stream_buffer: String,
    pub last_error: Option<String>,
    pub active_target: Option<Uuid>,
    pub model_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConversationStreamingState {
    pub thinking_visible: bool,
    pub thinking_buffer: String,
    pub stream_buffer: String,
    pub last_error: Option<String>,
    pub model_id: Option<String>,
}

impl ConversationStreamingState {
    #[must_use]
    pub fn project_for_snapshot(
        self,
        active: bool,
        conversation_id: Uuid,
    ) -> StreamingStoreSnapshot {
        StreamingStoreSnapshot {
            thinking_visible: self.thinking_visible,
            thinking_buffer: self.thinking_buffer,
            stream_buffer: self.stream_buffer,
            last_error: self.last_error,
            active_target: active.then_some(conversation_id),
            model_id: self.model_id,
        }
    }
}

/// Store-owned chat snapshot slice used by mounted GPUI views.
///
/// @plan PLAN-20260304-GPUIREMEDIATE.P05
/// @requirement REQ-ARCH-001.1
/// @requirement REQ-ARCH-003.2
/// @requirement REQ-ARCH-003.4
/// @requirement REQ-ARCH-003.6
/// @requirement REQ-ARCH-006.6
/// @pseudocode analysis/pseudocode/01-app-store.md:001-405
#[derive(Clone, Debug)]
pub struct ChatStoreSnapshot {
    pub selected_conversation_id: Option<Uuid>,
    pub selected_conversation_title: String,
    pub selection_generation: u64,
    pub load_state: ConversationLoadState,
    pub transcript: Vec<ConversationMessagePayload>,
    pub streaming: StreamingStoreSnapshot,
    pub conversations: Vec<ConversationSummary>,
}

impl Default for ChatStoreSnapshot {
    fn default() -> Self {
        Self {
            selected_conversation_id: None,
            selected_conversation_title: "New Conversation".to_string(),
            selection_generation: 0,
            load_state: ConversationLoadState::Idle,
            transcript: Vec::new(),
            streaming: StreamingStoreSnapshot::default(),
            conversations: Vec::new(),
        }
    }
}

#[must_use]
pub fn project_streaming_snapshot<S: BuildHasher>(
    streaming_states: &HashMap<Uuid, ConversationStreamingState, S>,
    selected_conversation_id: Option<Uuid>,
    active_streaming_target: Option<Uuid>,
) -> StreamingStoreSnapshot {
    let Some(conversation_id) = selected_conversation_id else {
        return StreamingStoreSnapshot::default();
    };

    let active = active_streaming_target == Some(conversation_id);
    streaming_states
        .get(&conversation_id)
        .cloned()
        .unwrap_or_default()
        .project_for_snapshot(active, conversation_id)
}

/// Store-owned history snapshot slice used by mounted GPUI views.
///
/// @plan PLAN-20260304-GPUIREMEDIATE.P05
/// @requirement REQ-ARCH-001.1
/// @requirement REQ-ARCH-004.1
/// @pseudocode analysis/pseudocode/01-app-store.md:001-405
#[derive(Clone, Debug, Default)]
pub struct HistoryStoreSnapshot {
    pub conversations: Vec<ConversationSummary>,
    pub selected_conversation_id: Option<Uuid>,
}

/// Store-owned settings/profile snapshot slice.
///
/// @plan PLAN-20260304-GPUIREMEDIATE.P05
/// @requirement REQ-ARCH-001.1
/// @requirement REQ-ARCH-004.1
/// @pseudocode analysis/pseudocode/01-app-store.md:001-405
#[derive(Clone, Debug, Default)]
pub struct SettingsStoreSnapshot {
    pub profiles: Vec<ProfileSummary>,
    pub selected_profile_id: Option<Uuid>,
    pub settings_visible: bool,
}

/// Published GPUI app snapshot.
///
/// @plan PLAN-20260304-GPUIREMEDIATE.P05
/// @requirement REQ-ARCH-001.1
/// @requirement REQ-ARCH-004.1
/// @pseudocode analysis/pseudocode/01-app-store.md:001-405
#[derive(Clone, Debug, Default)]
pub struct GpuiAppSnapshot {
    pub revision: u64,
    pub chat: ChatStoreSnapshot,
    pub history: HistoryStoreSnapshot,
    pub settings: SettingsStoreSnapshot,
}
