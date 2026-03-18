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
