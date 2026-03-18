//! Authoritative GPUI app store.
//!
//! Phase 05 extends the startup-seeded skeleton into the runtime reducer/publisher
//! that owns selection freshness, transcript durability, and streaming/thinking
//! state for GPUI-mounted views.
//!
//! @plan PLAN-20260304-GPUIREMEDIATE.P05
//! @requirement REQ-ARCH-001.1
//! @requirement REQ-ARCH-003.2
//! @requirement REQ-ARCH-003.3
//! @requirement REQ-ARCH-003.4
//! @requirement REQ-ARCH-003.6
//! @requirement REQ-ARCH-004.1
//! @requirement REQ-ARCH-006.6
//! @requirement REQ-ARCH-006.7
//! @pseudocode analysis/pseudocode/01-app-store.md:001-405
//! @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:001-087

use std::sync::Mutex;

use uuid::Uuid;

use crate::presentation::view_command::{
    ConversationMessagePayload, ConversationSummary, MessageRole, ProfileSummary, ViewCommand,
};

/// Startup hydration inputs.
///
/// @plan PLAN-20260304-GPUIREMEDIATE.P06
/// @requirement REQ-ARCH-002.1
/// @requirement REQ-ARCH-002.2
/// @requirement REQ-ARCH-002.5
/// @requirement REQ-ARCH-006.3
/// @pseudocode analysis/pseudocode/01-app-store.md:133-195
#[derive(Clone, Debug)]
pub struct StartupInputs {
    pub profiles: Vec<ProfileSummary>,
    pub selected_profile_id: Option<Uuid>,
    pub conversations: Vec<ConversationSummary>,
    pub selected_conversation: Option<StartupSelectedConversation>,
}

/// Startup-selected conversation metadata.
///
/// @plan PLAN-20260304-GPUIREMEDIATE.P06
/// @requirement REQ-ARCH-002.1
/// @requirement REQ-ARCH-002.2
/// @requirement REQ-ARCH-002.5
/// @requirement REQ-ARCH-006.3
/// @pseudocode analysis/pseudocode/01-app-store.md:133-195
#[derive(Clone, Debug)]
pub struct StartupSelectedConversation {
    pub conversation_id: Uuid,
    pub mode: StartupMode,
}

/// Startup hydration mode.
///
/// @plan PLAN-20260304-GPUIREMEDIATE.P06
/// @requirement REQ-ARCH-002.1
/// @requirement REQ-ARCH-002.2
/// @requirement REQ-ARCH-002.5
/// @requirement REQ-ARCH-006.3
/// @pseudocode analysis/pseudocode/01-app-store.md:133-195
#[derive(Clone, Debug)]
pub enum StartupMode {
    ModeA {
        transcript_result: StartupTranscriptResult,
    },
}

/// Startup transcript load result.
///
/// @plan PLAN-20260304-GPUIREMEDIATE.P06
/// @requirement REQ-ARCH-002.1
/// @requirement REQ-ARCH-002.2
/// @requirement REQ-ARCH-002.5
/// @requirement REQ-ARCH-006.3
/// @pseudocode analysis/pseudocode/01-app-store.md:133-195
#[derive(Clone, Debug)]
pub enum StartupTranscriptResult {
    Success(Vec<ConversationMessagePayload>),
    Failure(String),
}

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

#[derive(Clone, Debug, PartialEq, Eq)]
struct FinalizedStreamGuard {
    conversation_id: Uuid,
    transcript_len_after_finalize: usize,
}

#[derive(Clone, Debug, Default)]
enum SelectedTitleProvenance {
    HistoryBacked,
    #[default]
    LiteralFallback,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeginSelectionMode {
    PublishImmediately,
    BatchNoPublish,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeginSelectionResult {
    NoOpSameSelection,
    BeganSelection { generation: u64 },
}

#[derive(Default)]
struct AppStoreInner {
    snapshot: GpuiAppSnapshot,
    subscribers: Vec<flume::Sender<GpuiAppSnapshot>>,
    title_provenance: SelectedTitleProvenance,
    last_finalized_stream_guard: Option<FinalizedStreamGuard>,
}

/// Process-lifetime authoritative store handle.
///
/// @plan PLAN-20260304-GPUIREMEDIATE.P05
/// @requirement REQ-ARCH-001.1
/// @requirement REQ-ARCH-001.3
/// @requirement REQ-ARCH-003.6
/// @requirement REQ-ARCH-004.1
/// @pseudocode analysis/pseudocode/01-app-store.md:001-405
pub struct GpuiAppStore {
    inner: Mutex<AppStoreInner>,
}

impl GpuiAppStore {
    /// @plan PLAN-20260304-GPUIREMEDIATE.P06
    /// @requirement REQ-ARCH-002.1
    /// @requirement REQ-ARCH-002.2
    /// @requirement REQ-ARCH-002.5
    /// @requirement REQ-ARCH-006.3
    /// @pseudocode analysis/pseudocode/01-app-store.md:133-195
    #[must_use]
    pub fn from_startup_inputs(inputs: StartupInputs) -> Self {
        let mut inner = AppStoreInner::default();
        let _ = reduce_startup_batch(&mut inner, inputs);

        Self {
            inner: Mutex::new(inner),
        }
    }

    /// Return the latest published app snapshot.
    ///
    /// @plan PLAN-20260304-GPUIREMEDIATE.P05
    /// @requirement REQ-ARCH-001.3
    /// @pseudocode analysis/pseudocode/01-app-store.md:075-088
    ///
    /// # Panics
    ///
    /// Panics if the store mutex is poisoned.
    pub fn current_snapshot(&self) -> GpuiAppSnapshot {
        self.inner
            .lock()
            .expect("gpui app store mutex poisoned")
            .snapshot
            .clone()
    }

    /// Subscribe to snapshot publications.
    ///
    /// @plan PLAN-20260304-GPUIREMEDIATE.P05
    /// @requirement REQ-ARCH-004.1
    /// @pseudocode analysis/pseudocode/03-main-panel-integration.md:022-031
    ///
    /// # Panics
    ///
    /// Panics if the store mutex is poisoned.
    pub fn subscribe(&self) -> flume::Receiver<GpuiAppSnapshot> {
        let (tx, rx) = flume::unbounded();
        self.inner
            .lock()
            .expect("gpui app store mutex poisoned")
            .subscribers
            .push(tx);
        rx
    }

    /// Count connected snapshot subscribers.
    ///
    /// @plan PLAN-20260304-GPUIREMEDIATE.P07
    /// @requirement REQ-ARCH-004.1
    /// @pseudocode analysis/pseudocode/03-main-panel-integration.md:057-061
    ///
    /// # Panics
    ///
    /// Panics if the store mutex is poisoned.
    pub fn subscriber_count(&self) -> usize {
        self.inner
            .lock()
            .expect("gpui app store mutex poisoned")
            .subscribers
            .iter()
            .filter(|subscriber| !subscriber.is_disconnected())
            .count()
    }

    /// Remove disconnected subscribers from the publication list.
    ///
    /// @plan PLAN-20260304-GPUIREMEDIATE.P07
    /// @requirement REQ-ARCH-004.1
    /// @pseudocode analysis/pseudocode/03-main-panel-integration.md:057-061
    ///
    /// # Panics
    ///
    /// Panics if the store mutex is poisoned.
    pub fn prune_disconnected_subscribers(&self) {
        let mut inner = self.inner.lock().expect("gpui app store mutex poisoned");
        prune_disconnected_subscribers_locked(&mut inner);
    }

    /// Begin a generation-tracked conversation selection.
    ///
    /// @plan PLAN-20260304-GPUIREMEDIATE.P05
    /// @requirement REQ-ARCH-003.6
    /// @pseudocode analysis/pseudocode/01-app-store.md:099-123
    ///
    /// # Panics
    ///
    /// Panics if the store mutex is poisoned.
    pub fn begin_selection(
        &self,
        conversation_id: Uuid,
        mode: BeginSelectionMode,
    ) -> BeginSelectionResult {
        let mut inner = self.inner.lock().expect("gpui app store mutex poisoned");
        begin_selection_locked(&mut inner, conversation_id, mode)
    }

    /// Reduce a batch of runtime commands into the store snapshot.
    ///
    /// @plan PLAN-20260304-GPUIREMEDIATE.P05
    /// @requirement REQ-ARCH-003.4
    /// @requirement REQ-ARCH-003.6
    /// @requirement REQ-ARCH-006.6
    /// @requirement REQ-ARCH-006.7
    /// @pseudocode analysis/pseudocode/01-app-store.md:217-405
    ///
    /// # Panics
    ///
    /// Panics if the store mutex is poisoned.
    pub fn reduce_batch(&self, commands: Vec<ViewCommand>) -> bool {
        if commands.is_empty() {
            return false;
        }

        let mut inner = self.inner.lock().expect("gpui app store mutex poisoned");
        let mut changed = false;
        for command in commands {
            changed = reduce_view_command_without_publish(&mut inner, command) || changed;
        }
        if changed {
            bump_revision_and_publish(&mut inner);
        }
        changed
    }

    /// @plan PLAN-20260304-GPUIREMEDIATE.P05
    /// @requirement REQ-ARCH-004.1
    /// @pseudocode analysis/pseudocode/01-app-store.md:186-190
    pub fn restore_after_clear(&self) -> GpuiAppSnapshot {
        self.current_snapshot()
    }
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P06
/// @requirement REQ-ARCH-002.1
/// @requirement REQ-ARCH-002.2
/// @requirement REQ-ARCH-002.5
/// @requirement REQ-ARCH-006.3
/// @pseudocode analysis/pseudocode/01-app-store.md:133-195
fn reduce_startup_batch(inner: &mut AppStoreInner, inputs: StartupInputs) -> bool {
    let mut changed = if inner.snapshot.history.conversations == inputs.conversations {
        mutate_profiles_snapshot(inner, inputs.profiles, inputs.selected_profile_id)
    } else {
        inner.snapshot.history.conversations = inputs.conversations.clone();
        inner.snapshot.chat.conversations = inputs.conversations;
        true
    };
    changed |= maybe_sync_selected_title(inner);

    if let Some(selection) = inputs.selected_conversation {
        if matches!(
            begin_selection_locked(
                inner,
                selection.conversation_id,
                BeginSelectionMode::BatchNoPublish,
            ),
            BeginSelectionResult::BeganSelection { generation: 1 }
        ) {
            changed = true;
        }

        let completion_changed = match selection.mode {
            StartupMode::ModeA {
                transcript_result: StartupTranscriptResult::Success(messages),
            } => reduce_view_command_without_publish(
                inner,
                ViewCommand::ConversationMessagesLoaded {
                    conversation_id: selection.conversation_id,
                    selection_generation: inner.snapshot.chat.selection_generation,
                    messages,
                },
            ),
            StartupMode::ModeA {
                transcript_result: StartupTranscriptResult::Failure(message),
            } => reduce_view_command_without_publish(
                inner,
                ViewCommand::ConversationLoadFailed {
                    conversation_id: selection.conversation_id,
                    selection_generation: inner.snapshot.chat.selection_generation,
                    message,
                },
            ),
        };
        changed = completion_changed || changed;
    }

    if changed {
        inner.snapshot.revision = 1;
    }

    changed
}

fn begin_selection_locked(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
    mode: BeginSelectionMode,
) -> BeginSelectionResult {
    let same_selection_loading_or_ready = match &inner.snapshot.chat.load_state {
        ConversationLoadState::Loading {
            conversation_id: active_id,
            generation,
        }
        | ConversationLoadState::Ready {
            conversation_id: active_id,
            generation,
        } => {
            inner.snapshot.chat.selected_conversation_id == Some(conversation_id)
                && *active_id == conversation_id
                && *generation == inner.snapshot.chat.selection_generation
        }
        ConversationLoadState::Idle | ConversationLoadState::Error { .. } => false,
    };

    if same_selection_loading_or_ready {
        return BeginSelectionResult::NoOpSameSelection;
    }

    let next_generation = inner.snapshot.chat.selection_generation + 1;
    inner.snapshot.chat.selected_conversation_id = Some(conversation_id);
    inner.snapshot.history.selected_conversation_id = Some(conversation_id);
    inner.snapshot.chat.selection_generation = next_generation;
    inner.snapshot.chat.load_state = ConversationLoadState::Loading {
        conversation_id,
        generation: next_generation,
    };
    clear_streaming_ephemera_only(inner);
    inner.last_finalized_stream_guard = None;

    if !apply_selected_title_from_history(inner, conversation_id) {
        inner.snapshot.chat.selected_conversation_title = "Untitled Conversation".to_string();
        inner.title_provenance = SelectedTitleProvenance::LiteralFallback;
    }

    if mode == BeginSelectionMode::PublishImmediately {
        bump_revision_and_publish(inner);
    }

    BeginSelectionResult::BeganSelection {
        generation: next_generation,
    }
}

#[allow(clippy::too_many_lines)]
fn reduce_view_command_without_publish(inner: &mut AppStoreInner, command: ViewCommand) -> bool {
    match command {
        ViewCommand::ConversationListRefreshed { conversations } => {
            if inner.snapshot.history.conversations == conversations {
                return false;
            }
            inner.snapshot.history.conversations = conversations.clone();
            inner.snapshot.chat.conversations = conversations;
            maybe_sync_selected_title(inner)
        }
        ViewCommand::ConversationActivated {
            id,
            selection_generation,
        } => {
            if inner.snapshot.chat.selected_conversation_id == Some(id)
                && inner.snapshot.chat.selection_generation == selection_generation
                && matches!(
                    inner.snapshot.chat.load_state,
                    ConversationLoadState::Loading {
                        conversation_id,
                        generation,
                    } if conversation_id == id && generation == selection_generation
                )
            {
                return false;
            }
            if inner.snapshot.chat.selected_conversation_id == Some(id)
                && inner.snapshot.chat.selection_generation == selection_generation
            {
                return maybe_upgrade_selected_title_from_history(inner, id);
            }
            false
        }
        ViewCommand::ConversationMessagesLoaded {
            conversation_id,
            selection_generation,
            messages,
        } => {
            if inner.snapshot.chat.selected_conversation_id != Some(conversation_id) {
                return false;
            }
            if selection_generation != inner.snapshot.chat.selection_generation {
                return false;
            }
            if load_state_targets_different_conversation(
                &inner.snapshot.chat.load_state,
                conversation_id,
            ) {
                return false;
            }
            if inner.snapshot.chat.transcript == messages
                && inner.snapshot.chat.load_state
                    == (ConversationLoadState::Ready {
                        conversation_id,
                        generation: selection_generation,
                    })
            {
                return false;
            }
            inner.snapshot.chat.transcript = messages;
            inner.snapshot.chat.load_state = ConversationLoadState::Ready {
                conversation_id,
                generation: selection_generation,
            };
            clear_streaming_ephemera_only(inner);
            inner.last_finalized_stream_guard = None;
            true
        }
        ViewCommand::ConversationLoadFailed {
            conversation_id,
            selection_generation,
            message,
        } => {
            if inner.snapshot.chat.selected_conversation_id != Some(conversation_id) {
                return false;
            }
            if selection_generation != inner.snapshot.chat.selection_generation {
                return false;
            }
            if load_state_targets_different_conversation(
                &inner.snapshot.chat.load_state,
                conversation_id,
            ) {
                return false;
            }
            let next_state = ConversationLoadState::Error {
                conversation_id,
                generation: selection_generation,
                message,
            };
            if inner.snapshot.chat.load_state == next_state {
                return false;
            }
            inner.snapshot.chat.load_state = next_state;
            clear_streaming_ephemera_only(inner);
            true
        }
        ViewCommand::MessageAppended {
            conversation_id,
            role,
            content,
        } => {
            if role == MessageRole::Assistant
                && inner
                    .last_finalized_stream_guard
                    .as_ref()
                    .is_some_and(|guard| {
                        conversation_id == guard.conversation_id
                            && inner.snapshot.chat.transcript.len()
                                == guard.transcript_len_after_finalize
                            && inner.snapshot.chat.transcript.last().is_some_and(|tail| {
                                tail.role == MessageRole::Assistant && tail.content == content
                            })
                    })
            {
                return false;
            }
            append_persisted_message_if_target_matches_selected(
                inner,
                conversation_id,
                role,
                content,
            )
        }
        ViewCommand::ShowThinking { conversation_id } => {
            show_thinking_if_target_matches_selected_or_nil(inner, conversation_id)
        }
        ViewCommand::HideThinking { conversation_id } => {
            hide_thinking_if_target_matches_selected_or_nil(inner, conversation_id)
        }
        ViewCommand::AppendThinking {
            conversation_id,
            content,
        } => append_thinking_buffer_if_target_matches_selected_or_nil(
            inner,
            conversation_id,
            &content,
        ),
        ViewCommand::AppendStream {
            conversation_id,
            chunk,
        } => append_stream_buffer_if_target_matches_selected_or_nil(inner, conversation_id, &chunk),
        ViewCommand::FinalizeStream {
            conversation_id,
            tokens: _,
        } => finalize_stream_if_target_matches_selected_or_nil(inner, conversation_id),
        ViewCommand::StreamCancelled {
            conversation_id,
            partial_content: _,
        } => clear_streaming_ephemera_for_target(inner, conversation_id, None),
        ViewCommand::StreamError {
            conversation_id,
            error,
            recoverable: _,
        } => clear_streaming_ephemera_for_target(inner, conversation_id, Some(error)),
        ViewCommand::ChatProfilesUpdated {
            profiles,
            selected_profile_id,
        } => mutate_profiles_snapshot(inner, profiles, selected_profile_id),
        ViewCommand::ShowSettings {
            profiles,
            selected_profile_id,
        } => {
            if inner.snapshot.settings.settings_visible {
                mutate_profiles_snapshot(inner, profiles, selected_profile_id)
            } else {
                inner.snapshot.settings.settings_visible = true;
                true
            }
        }
        ViewCommand::ConversationRenamed { id, new_title } => {
            mutate_history_and_selected_title_if_targeted(inner, id, &new_title)
        }
        ViewCommand::ConversationTitleUpdated { id, title } => {
            mutate_history_and_selected_title_if_targeted(inner, id, &title)
        }
        ViewCommand::ConversationDeleted { id } => {
            mutate_history_and_selected_selection_if_targeted(inner, id)
        }
        ViewCommand::ConversationCreated { id, .. } => {
            let already_listed = inner
                .snapshot
                .history
                .conversations
                .iter()
                .any(|conversation| conversation.id == id);
            if !already_listed {
                let conversation = ConversationSummary {
                    id,
                    title: "New Conversation".to_string(),
                    updated_at: chrono::Utc::now(),
                    message_count: 0,
                };
                inner
                    .snapshot
                    .history
                    .conversations
                    .insert(0, conversation.clone());
                inner.snapshot.chat.conversations.insert(0, conversation);
            }
            // Activate the new conversation through the proper selection path
            begin_selection_locked(inner, id, BeginSelectionMode::BatchNoPublish);
            // Immediately mark it Ready with empty transcript (new conversation has no messages)
            let gen = inner.snapshot.chat.selection_generation;
            inner.snapshot.chat.transcript.clear();
            inner.snapshot.chat.load_state = ConversationLoadState::Ready {
                conversation_id: id,
                generation: gen,
            };
            inner.snapshot.chat.selected_conversation_title = "New Conversation".to_string();
            true
        }
        _ => false,
    }
}

fn bump_revision_and_publish(inner: &mut AppStoreInner) {
    inner.snapshot.revision += 1;
    publish_snapshot_to_subscribers(inner);
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P07
/// @requirement REQ-ARCH-004.1
/// @pseudocode analysis/pseudocode/03-main-panel-integration.md:057-061
fn publish_snapshot_to_subscribers(inner: &mut AppStoreInner) {
    prune_disconnected_subscribers_locked(inner);
    let snapshot = inner.snapshot.clone();
    inner
        .subscribers
        .retain(|subscriber| subscriber.send(snapshot.clone()).is_ok());
}

fn prune_disconnected_subscribers_locked(inner: &mut AppStoreInner) {
    inner
        .subscribers
        .retain(|subscriber| !subscriber.is_disconnected());
}

fn clear_streaming_ephemera_only(inner: &mut AppStoreInner) {
    inner.snapshot.chat.streaming = StreamingStoreSnapshot::default();
}

fn maybe_sync_selected_title(inner: &mut AppStoreInner) -> bool {
    let Some(conversation_id) = inner.snapshot.chat.selected_conversation_id else {
        return false;
    };
    maybe_upgrade_selected_title_from_history(inner, conversation_id)
}

fn maybe_upgrade_selected_title_from_history(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
) -> bool {
    if inner.snapshot.chat.selected_conversation_id != Some(conversation_id) {
        return false;
    }

    if let Some(history_title) =
        authoritative_history_title(&inner.snapshot.history.conversations, conversation_id)
    {
        if matches!(
            inner.title_provenance,
            SelectedTitleProvenance::LiteralFallback
        ) && inner.snapshot.chat.selected_conversation_title != history_title
        {
            inner.snapshot.chat.selected_conversation_title = history_title;
            inner.title_provenance = SelectedTitleProvenance::HistoryBacked;
            return true;
        }
    }

    false
}

fn apply_selected_title_from_history(inner: &mut AppStoreInner, conversation_id: Uuid) -> bool {
    if let Some(history_title) =
        authoritative_history_title(&inner.snapshot.history.conversations, conversation_id)
    {
        inner.snapshot.chat.selected_conversation_title = history_title;
        inner.title_provenance = SelectedTitleProvenance::HistoryBacked;
        return true;
    }
    false
}

fn authoritative_history_title(
    conversations: &[ConversationSummary],
    conversation_id: Uuid,
) -> Option<String> {
    conversations
        .iter()
        .find(|conversation| conversation.id == conversation_id)
        .map(|conversation| normalize_title(&conversation.title))
}

fn load_state_targets_different_conversation(
    load_state: &ConversationLoadState,
    conversation_id: Uuid,
) -> bool {
    match load_state {
        ConversationLoadState::Loading {
            conversation_id: active_id,
            ..
        }
        | ConversationLoadState::Ready {
            conversation_id: active_id,
            ..
        }
        | ConversationLoadState::Error {
            conversation_id: active_id,
            ..
        } => *active_id != conversation_id,
        ConversationLoadState::Idle => false,
    }
}

fn append_persisted_message_if_target_matches_selected(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
    role: MessageRole,
    content: String,
) -> bool {
    if matches!(role, MessageRole::User | MessageRole::Assistant)
        && inner.snapshot.chat.selected_conversation_id != Some(conversation_id)
    {
        return false;
    }

    inner
        .snapshot
        .chat
        .transcript
        .push(ConversationMessagePayload {
            role,
            content,
            thinking_content: None,
            timestamp: None,
        });
    true
}

fn resolve_nil_or_explicit_target(inner: &AppStoreInner, conversation_id: Uuid) -> Option<Uuid> {
    if conversation_id == Uuid::nil() {
        inner
            .snapshot
            .chat
            .streaming
            .active_target
            .or(inner.snapshot.chat.selected_conversation_id)
    } else {
        Some(conversation_id)
    }
}

fn show_thinking_if_target_matches_selected_or_nil(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
) -> bool {
    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };
    if inner.snapshot.chat.selected_conversation_id != Some(target) {
        return false;
    }
    let mut changed = if inner.snapshot.chat.streaming.thinking_visible {
        false
    } else {
        inner.snapshot.chat.streaming.thinking_visible = true;
        true
    };
    changed |= if inner.snapshot.chat.streaming.active_target == Some(target) {
        false
    } else {
        inner.snapshot.chat.streaming.active_target = Some(target);
        true
    };
    if inner.snapshot.chat.streaming.stream_buffer.is_empty()
        && inner.snapshot.chat.streaming.thinking_buffer.is_empty()
        && inner.snapshot.chat.streaming.last_error.is_none()
    {
        changed = true;
    }
    changed
}

fn hide_thinking_if_target_matches_selected_or_nil(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
) -> bool {
    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };
    if inner.snapshot.chat.selected_conversation_id != Some(target) {
        return false;
    }
    if !inner.snapshot.chat.streaming.thinking_visible {
        return false;
    }
    inner.snapshot.chat.streaming.thinking_visible = false;
    true
}

fn append_thinking_buffer_if_target_matches_selected_or_nil(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
    content: &str,
) -> bool {
    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };
    if inner.snapshot.chat.selected_conversation_id != Some(target) || content.is_empty() {
        return false;
    }
    inner.snapshot.chat.streaming.active_target = Some(target);
    inner.snapshot.chat.streaming.thinking_visible = true;
    inner
        .snapshot
        .chat
        .streaming
        .thinking_buffer
        .push_str(content);
    true
}

fn append_stream_buffer_if_target_matches_selected_or_nil(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
    chunk: &str,
) -> bool {
    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };
    if inner.snapshot.chat.selected_conversation_id != Some(target) || chunk.is_empty() {
        return false;
    }
    inner.snapshot.chat.streaming.active_target = Some(target);
    inner.snapshot.chat.streaming.stream_buffer.push_str(chunk);
    true
}

fn finalize_stream_if_target_matches_selected_or_nil(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
) -> bool {
    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };
    if inner.snapshot.chat.selected_conversation_id != Some(target) {
        return false;
    }
    if inner.snapshot.chat.streaming.active_target != Some(target) {
        return false;
    }
    if inner.snapshot.chat.streaming.stream_buffer.is_empty() {
        return false;
    }

    let assistant_payload = ConversationMessagePayload {
        role: MessageRole::Assistant,
        content: inner.snapshot.chat.streaming.stream_buffer.clone(),
        thinking_content: non_empty_or_none(&inner.snapshot.chat.streaming.thinking_buffer),
        timestamp: None,
    };
    inner.snapshot.chat.transcript.push(assistant_payload);
    inner.last_finalized_stream_guard = Some(FinalizedStreamGuard {
        conversation_id: target,
        transcript_len_after_finalize: inner.snapshot.chat.transcript.len(),
    });
    clear_streaming_ephemera_only(inner);
    true
}

fn clear_streaming_ephemera_for_target(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
    error: Option<String>,
) -> bool {
    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };
    clear_streaming_ephemera_if_selected_target_matches(inner, target, error)
}

fn clear_streaming_ephemera_if_selected_target_matches(
    inner: &mut AppStoreInner,
    target: Uuid,
    error: Option<String>,
) -> bool {
    if inner.snapshot.chat.selected_conversation_id != Some(target) {
        return false;
    }

    let previous = inner.snapshot.chat.streaming.clone();
    let mut next = previous.clone();
    next.active_target = None;
    next.stream_buffer.clear();
    next.thinking_buffer.clear();
    next.thinking_visible = false;
    next.last_error = error;
    if previous == next {
        return false;
    }
    inner.snapshot.chat.streaming = next;
    true
}

fn mutate_profiles_snapshot(
    inner: &mut AppStoreInner,
    profiles: Vec<ProfileSummary>,
    selected_profile_id: Option<Uuid>,
) -> bool {
    if inner.snapshot.settings.profiles == profiles
        && inner.snapshot.settings.selected_profile_id == selected_profile_id
    {
        return false;
    }
    inner.snapshot.settings.profiles = profiles;
    inner.snapshot.settings.selected_profile_id = selected_profile_id;
    true
}

fn mutate_history_and_selected_title_if_targeted(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
    title: &str,
) -> bool {
    update_conversation_title(inner, conversation_id, title)
}

fn mutate_history_and_selected_selection_if_targeted(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
) -> bool {
    let previous_history_len = inner.snapshot.history.conversations.len();
    let previous_chat_len = inner.snapshot.chat.conversations.len();
    inner
        .snapshot
        .history
        .conversations
        .retain(|conversation| conversation.id != conversation_id);
    inner
        .snapshot
        .chat
        .conversations
        .retain(|conversation| conversation.id != conversation_id);

    let changed = if inner.snapshot.chat.selected_conversation_id == Some(conversation_id) {
        inner.snapshot.chat.selected_conversation_id = inner
            .snapshot
            .history
            .conversations
            .first()
            .map(|conversation| conversation.id);
        inner.snapshot.history.selected_conversation_id =
            inner.snapshot.chat.selected_conversation_id;
        if let Some(next_selected) = inner.snapshot.chat.selected_conversation_id {
            apply_selected_title_from_history(inner, next_selected);
        } else {
            inner.snapshot.chat.selected_conversation_title = "New Conversation".to_string();
            inner.snapshot.chat.load_state = ConversationLoadState::Idle;
            inner.snapshot.chat.transcript.clear();
            clear_streaming_ephemera_only(inner);
        }
        true
    } else {
        inner.snapshot.history.conversations.len() != previous_history_len
            || inner.snapshot.chat.conversations.len() != previous_chat_len
    };

    changed
}

fn update_conversation_title(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
    title: &str,
) -> bool {
    let normalized = normalize_title(title);
    let mut changed = false;

    if let Some(conversation) = inner
        .snapshot
        .history
        .conversations
        .iter_mut()
        .find(|conversation| conversation.id == conversation_id)
    {
        if conversation.title != normalized {
            conversation.title.clone_from(&normalized);
            changed = true;
        }
    }

    if let Some(conversation) = inner
        .snapshot
        .chat
        .conversations
        .iter_mut()
        .find(|conversation| conversation.id == conversation_id)
    {
        if conversation.title != normalized {
            conversation.title.clone_from(&normalized);
            changed = true;
        }
    }

    if inner.snapshot.chat.selected_conversation_id == Some(conversation_id)
        && inner.snapshot.chat.selected_conversation_title != normalized
    {
        inner.snapshot.chat.selected_conversation_title = normalized;
        inner.title_provenance = SelectedTitleProvenance::HistoryBacked;
        changed = true;
    }

    changed
}

fn non_empty_or_none(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn normalize_title(title: &str) -> String {
    if title.trim().is_empty() {
        "Untitled Conversation".to_string()
    } else {
        title.to_string()
    }
}
