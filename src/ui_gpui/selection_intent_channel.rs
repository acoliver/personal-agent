//! Global GPUI-side selection intent channel.
//!
//! Child views use this channel to request conversation selection without minting
//! `selection_generation` themselves. The app-root runtime pump owns the actual
//! authoritative selection transition and presenter handoff.
//!
//! @plan PLAN-20260304-GPUIREMEDIATE.P05
//! @requirement REQ-ARCH-003.6
//! @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:001-087

use std::collections::VecDeque;
use std::sync::Mutex;

use uuid::Uuid;

pub struct SelectionIntentChannel {
    pending: Mutex<VecDeque<Uuid>>,
}

impl SelectionIntentChannel {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(VecDeque::new()),
        }
    }

    /// @plan PLAN-20260304-GPUIREMEDIATE.P05
    /// @requirement REQ-ARCH-003.6
    /// @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:001-087
    pub fn request_select(&self, conversation_id: Uuid) {
        self.pending
            .lock()
            .expect("selection intent channel mutex poisoned")
            .push_back(conversation_id);
    }

    /// @plan PLAN-20260304-GPUIREMEDIATE.P05
    /// @requirement REQ-ARCH-003.6
    /// @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:001-087
    pub fn take_pending(&self) -> Option<Uuid> {
        self.pending
            .lock()
            .expect("selection intent channel mutex poisoned")
            .pop_front()
    }
}

impl Default for SelectionIntentChannel {
    fn default() -> Self {
        Self::new()
    }
}

static SELECTION_INTENT_CHANNEL: once_cell::sync::Lazy<SelectionIntentChannel> =
    once_cell::sync::Lazy::new(SelectionIntentChannel::new);

pub fn selection_intent_channel() -> &'static SelectionIntentChannel {
    &SELECTION_INTENT_CHANNEL
}
