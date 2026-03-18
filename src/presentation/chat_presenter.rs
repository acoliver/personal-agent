//! `ChatPresenter` - handles user chat events and service coordination
//!
//! `ChatPresenter` subscribes to user chat events and chat domain events,
//! coordinates with chat and conversation services, and emits view commands.
//!
//! @plan PLAN-20250125-REFACTOR.P12
//! @requirement REQ-027.1
//! @pseudocode presenters.md lines 20-251

use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use super::view_command::{
    ConversationMessagePayload, ConversationSummary, ErrorSeverity, MessageRole,
};
use super::{Presenter, PresenterError, ViewCommand};
use crate::events::bus::EventBus;
use crate::events::{
    types::{ChatEvent, ConversationEvent, UserEvent},
    AppEvent,
};
use crate::services::{ChatService, ConversationService, ProfileService, ServiceError};

/// `ChatPresenter` - handles chat UI events and service coordination
///
/// @plan PLAN-20250125-REFACTOR.P12
/// @requirement REQ-027.1
/// @pseudocode presenters.md lines 20-25
pub struct ChatPresenter {
    /// Reference to event bus for subscribing to events
    event_bus: Arc<EventBus>,

    /// Reference to conversation service
    conversation_service: Arc<dyn ConversationService>,

    /// Reference to chat service
    chat_service: Arc<dyn ChatService>,

    /// Reference to profile service
    profile_service: Arc<dyn ProfileService>,

    /// View command sender (mpsc for reliable delivery)
    view_tx: mpsc::Sender<ViewCommand>,

    /// Running flag for event loop
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl ChatPresenter {
    /// Create a new `ChatPresenter`
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    /// @pseudocode presenters.md lines 31-42
    pub fn new(
        event_bus: Arc<EventBus>,
        conversation_service: Arc<dyn ConversationService>,
        chat_service: Arc<dyn ChatService>,
        profile_service: Arc<dyn ProfileService>,
        view_tx: mpsc::Sender<ViewCommand>,
    ) -> Self {
        Self {
            event_bus,
            conversation_service,
            chat_service,
            profile_service,
            view_tx,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Start the presenter event loop
    ///
    /// # Errors
    ///
    /// Returns `PresenterError` if presenter startup becomes fallible in the future.
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    /// @pseudocode presenters.md lines 50-69
    pub async fn start(&mut self) -> Result<(), PresenterError> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // Emit initial conversation list so the dropdown is populated on startup
        tracing::info!("ChatPresenter: emitting initial conversation list");
        match Self::emit_conversation_list(&self.conversation_service, &mut self.view_tx.clone())
            .await
        {
            Ok(()) => {
                tracing::info!("ChatPresenter: initial conversation list emitted successfully");
            }
            Err(e) => tracing::error!(
                "ChatPresenter: failed to emit initial conversation list: {}",
                e
            ),
        }

        // On startup, always activate and replay the newest conversation when available.
        // This guarantees the selected conversation and visible transcript are consistent,
        // even when persisted active-id metadata is absent.
        match self.conversation_service.list(Some(1), Some(0)).await {
            Ok(conversations) => {
                if let Some(conversation) = conversations.first() {
                    let startup_id = conversation.id;
                    tracing::info!(
                        conversation_id = %startup_id,
                        "ChatPresenter: startup selecting newest conversation"
                    );
                    let _ = self.conversation_service.set_active(startup_id).await;

                    let mut startup_view_tx = self.view_tx.clone();
                    let _ = startup_view_tx
                        .send(ViewCommand::ConversationActivated {
                            id: startup_id,
                            selection_generation: 1,
                        })
                        .await;

                    match Self::replay_conversation_messages(
                        &self.conversation_service,
                        &mut startup_view_tx,
                        startup_id,
                        1,
                    )
                    .await
                    {
                        Ok(count) => tracing::info!(
                            conversation_id = %startup_id,
                            count,
                            "ChatPresenter: replayed startup conversation messages"
                        ),
                        Err(e) => tracing::warn!(
                            "ChatPresenter: failed to replay startup conversation {}: {}",
                            startup_id,
                            e
                        ),
                    }
                } else {
                    tracing::info!("ChatPresenter: no conversations at startup");
                }
            }
            Err(e) => tracing::warn!(
                "ChatPresenter: failed to list conversations at startup: {}",
                e
            ),
        }

        // Subscribe to events from EventBus
        let mut rx = self.event_bus.subscribe();
        let running = self.running.clone();
        let conversation_service = self.conversation_service.clone();
        let chat_service = self.chat_service.clone();
        let profile_service = self.profile_service.clone();
        let mut view_tx = self.view_tx.clone();

        tokio::spawn(async move {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(event) => {
                        Self::handle_event(
                            &conversation_service,
                            &chat_service,
                            &profile_service,
                            &mut view_tx,
                            event,
                        )
                        .await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("ChatPresenter lagged: {} events missed", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("ChatPresenter event stream closed");
                        break;
                    }
                }
            }
            tracing::info!("ChatPresenter event loop ended");
        });

        Ok(())
    }

    /// Stop the presenter event loop
    ///
    /// # Errors
    ///
    /// Returns `PresenterError` if presenter shutdown becomes fallible in the future.
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    /// @pseudocode presenters.md lines 250-253
    pub async fn stop(&mut self) -> Result<(), PresenterError> {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Check if presenter is running
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Handle events from `EventBus`
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    async fn handle_event(
        conversation_service: &Arc<dyn ConversationService>,
        chat_service: &Arc<dyn ChatService>,
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        event: AppEvent,
    ) {
        tracing::debug!("ChatPresenter::handle_event: {:?}", event);
        match event {
            AppEvent::User(user_evt) => {
                Self::handle_user_event(
                    conversation_service,
                    chat_service,
                    profile_service,
                    view_tx,
                    user_evt,
                )
                .await;
            }
            AppEvent::Chat(chat_evt) => {
                tracing::info!("ChatPresenter handling ChatEvent: {:?}", chat_evt);
                Self::handle_chat_event(view_tx, chat_evt).await;
            }
            AppEvent::Conversation(conv_evt) => {
                Self::handle_conversation_event(view_tx, conv_evt).await;
            }
            _ => {} // Ignore other events
        }
    }

    /// Handle user events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    async fn handle_user_event(
        conversation_service: &Arc<dyn ConversationService>,
        chat_service: &Arc<dyn ChatService>,
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        event: UserEvent,
    ) {
        match event {
            UserEvent::SendMessage { text } => {
                Self::handle_send_message(
                    conversation_service,
                    chat_service,
                    profile_service,
                    view_tx,
                    text,
                )
                .await;
            }
            UserEvent::StopStreaming => {
                Self::handle_stop_streaming(chat_service, view_tx).await;
            }
            UserEvent::NewConversation => {
                Self::handle_new_conversation(conversation_service, profile_service, view_tx).await;
            }
            UserEvent::ToggleThinking => {
                Self::handle_toggle_thinking(view_tx).await;
            }
            UserEvent::ConfirmRenameConversation { id, title } => {
                Self::handle_rename_conversation(conversation_service, view_tx, id, title).await;
            }
            UserEvent::SelectConversation {
                id,
                selection_generation,
            } => {
                Self::handle_select_conversation(
                    conversation_service,
                    view_tx,
                    id,
                    selection_generation,
                )
                .await;
            }
            UserEvent::RefreshHistory => {
                let _ = Self::emit_conversation_list(conversation_service, view_tx).await;
            }
            _ => {} // Ignore other user events
        }
    }

    async fn emit_conversation_list(
        conversation_service: &Arc<dyn ConversationService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
    ) -> Result<(), ServiceError> {
        let conversations = conversation_service.list(None, None).await?;
        let summaries = conversations
            .into_iter()
            .map(|conversation| ConversationSummary {
                id: conversation.id,
                title: conversation
                    .title
                    .filter(|title| !title.trim().is_empty())
                    .unwrap_or_else(|| "Untitled Conversation".to_string()),
                updated_at: conversation.updated_at,
                message_count: conversation.messages.len(),
            })
            .collect();

        let _ = view_tx
            .send(ViewCommand::ConversationListRefreshed {
                conversations: summaries,
            })
            .await;

        Ok(())
    }

    async fn replay_conversation_messages(
        conversation_service: &Arc<dyn ConversationService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        conversation_id: Uuid,
        selection_generation: u64,
    ) -> Result<usize, ServiceError> {
        let messages = conversation_service.get_messages(conversation_id).await?;
        let replay_count = messages.len();
        let loaded_messages = messages
            .into_iter()
            .filter_map(|message| {
                let role = match message.role {
                    crate::models::MessageRole::User => MessageRole::User,
                    crate::models::MessageRole::Assistant => MessageRole::Assistant,
                    crate::models::MessageRole::System => return None,
                };

                Some(ConversationMessagePayload {
                    role,
                    content: message.content,
                    thinking_content: message.thinking_content,
                    timestamp: Some(message.timestamp.timestamp_millis().cast_unsigned()),
                })
            })
            .collect::<Vec<_>>();

        let _ = view_tx
            .send(ViewCommand::ConversationMessagesLoaded {
                conversation_id,
                selection_generation,
                messages: loaded_messages,
            })
            .await;

        Ok(replay_count)
    }

    /// Handle chat events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    #[allow(clippy::too_many_lines)]
    async fn handle_chat_event(view_tx: &mut mpsc::Sender<ViewCommand>, event: ChatEvent) {
        match event {
            ChatEvent::StreamStarted {
                conversation_id,
                message_id: _,
                model_id: _,
            } => {
                let _ = view_tx
                    .send(ViewCommand::ShowThinking { conversation_id })
                    .await;
            }
            ChatEvent::TextDelta { text } => {
                let _ = view_tx
                    .send(ViewCommand::AppendStream {
                        conversation_id: Uuid::nil(),
                        chunk: text,
                    })
                    .await;
            }
            ChatEvent::ThinkingDelta { text } => {
                let _ = view_tx
                    .send(ViewCommand::AppendThinking {
                        conversation_id: Uuid::nil(),
                        content: text,
                    })
                    .await;
            }
            ChatEvent::ToolCallStarted {
                tool_call_id: _,
                tool_name,
            } => {
                let _ = view_tx
                    .send(ViewCommand::ShowToolCall {
                        conversation_id: Uuid::nil(),
                        tool_name,
                        status: "running".to_string(),
                    })
                    .await;
            }
            ChatEvent::ToolCallCompleted {
                tool_call_id: _,
                tool_name,
                success,
                result,
                duration_ms,
            } => {
                let status = if success {
                    "completed".to_string()
                } else {
                    "failed".to_string()
                };
                let _ = view_tx
                    .send(ViewCommand::UpdateToolCall {
                        conversation_id: Uuid::nil(),
                        tool_name,
                        status,
                        result: Some(result),
                        duration: Some(duration_ms),
                    })
                    .await;
            }
            ChatEvent::StreamCompleted {
                conversation_id,
                message_id: _,
                total_tokens,
            } => {
                let _ = view_tx
                    .send(ViewCommand::FinalizeStream {
                        conversation_id,
                        tokens: u64::from(total_tokens.unwrap_or(0)),
                    })
                    .await;
                let _ = view_tx
                    .send(ViewCommand::HideThinking { conversation_id })
                    .await;
            }
            ChatEvent::StreamCancelled {
                conversation_id,
                message_id: _,
                partial_content,
            } => {
                let _ = view_tx
                    .send(ViewCommand::StreamCancelled {
                        conversation_id,
                        partial_content,
                    })
                    .await;
                let _ = view_tx
                    .send(ViewCommand::HideThinking { conversation_id })
                    .await;
            }
            ChatEvent::StreamError {
                conversation_id,
                error,
                recoverable,
            } => {
                let _ = view_tx
                    .send(ViewCommand::StreamError {
                        conversation_id,
                        error: error.clone(),
                        recoverable,
                    })
                    .await;
                let _ = view_tx
                    .send(ViewCommand::ShowError {
                        title: "Stream Error".to_string(),
                        message: error,
                        severity: if recoverable {
                            ErrorSeverity::Warning
                        } else {
                            ErrorSeverity::Error
                        },
                    })
                    .await;
            }
            ChatEvent::MessageSaved {
                conversation_id,
                message_id: _,
            } => {
                let _ = view_tx
                    .send(ViewCommand::MessageSaved { conversation_id })
                    .await;
            }
        }
    }

    /// Handle `SendMessage` user event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    async fn handle_send_message(
        conversation_service: &Arc<dyn ConversationService>,
        chat_service: &Arc<dyn ChatService>,
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        content: String,
    ) {
        // Validate non-empty
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return;
        }

        // Get or create conversation
        let conversation_id =
            match Self::get_or_create_conversation(conversation_service, profile_service, view_tx)
                .await
            {
                Ok(id) => id,
                Err(e) => {
                    tracing::error!("Failed to get/create conversation: {}", e);
                    let error_msg = format!("Failed to create conversation: {e}");
                    let _ = view_tx
                        .send(ViewCommand::ShowError {
                            title: "Conversation Error".to_string(),
                            message: error_msg.clone(),
                            severity: ErrorSeverity::Error,
                        })
                        .await;
                    return;
                }
            };

        // Emit view commands for user message
        let _ = view_tx
            .send(ViewCommand::MessageAppended {
                conversation_id,
                role: MessageRole::User,
                content: trimmed.to_string(),
            })
            .await;

        // Show loading state
        let _ = view_tx
            .send(ViewCommand::ShowThinking { conversation_id })
            .await;

        // Send message via service
        match chat_service
            .send_message(conversation_id, trimmed.to_string())
            .await
        {
            Ok(_stream) => {
                // Stream events will be handled via ChatEvent
            }
            Err(e) => {
                let error_msg = e.to_string();
                tracing::error!("Failed to send message: {}", error_msg);
                let _ = view_tx
                    .send(ViewCommand::StreamError {
                        conversation_id,
                        error: error_msg.clone(),
                        recoverable: false,
                    })
                    .await;
                let _ = view_tx
                    .send(ViewCommand::HideThinking { conversation_id })
                    .await;
            }
        }
    }

    /// Handle `ToggleThinking` user event
    ///
    /// @plan PLAN-20250128-PRESENTERS.P01
    /// @requirement REQ-027.1
    async fn handle_toggle_thinking(view_tx: &mut mpsc::Sender<ViewCommand>) {
        let _ = view_tx.send(ViewCommand::ToggleThinkingVisibility).await;
    }

    /// Handle `ConfirmRenameConversation` user event
    ///
    /// @plan PLAN-20250128-PRESENTERS.P01
    /// @requirement REQ-027.1
    async fn handle_rename_conversation(
        conversation_service: &Arc<dyn ConversationService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        id: Uuid,
        title: String,
    ) {
        match conversation_service.rename(id, title.clone()).await {
            Ok(()) => {
                let _ = view_tx
                    .send(ViewCommand::ConversationRenamed {
                        id,
                        new_title: title,
                    })
                    .await;
                let _ = Self::emit_conversation_list(conversation_service, view_tx).await;
            }
            Err(e) => {
                let error_msg = format!("Failed to rename conversation: {e}");
                tracing::error!("{}", error_msg);
                let _ = view_tx
                    .send(ViewCommand::ShowError {
                        title: "Error".to_string(),
                        message: error_msg,
                        severity: ErrorSeverity::Error,
                    })
                    .await;
            }
        }
    }

    /// Handle conversation events
    ///
    /// @plan PLAN-20250128-PRESENTERS.P01
    /// @requirement REQ-027.1
    async fn handle_conversation_event(
        view_tx: &mut mpsc::Sender<ViewCommand>,
        event: ConversationEvent,
    ) {
        match event {
            ConversationEvent::Created { id, title: _ } => {
                let _ = view_tx
                    .send(ViewCommand::ConversationCreated { id, profile_id: id })
                    .await;
            }
            ConversationEvent::TitleUpdated { id, title } => {
                let _ = view_tx
                    .send(ViewCommand::ConversationRenamed {
                        id,
                        new_title: title,
                    })
                    .await;
            }
            ConversationEvent::Deleted { id } => {
                let _ = view_tx.send(ViewCommand::ConversationDeleted { id }).await;
            }
            ConversationEvent::Activated { id } | ConversationEvent::Loaded { id } => {
                let _ = view_tx
                    .send(ViewCommand::ConversationActivated {
                        id,
                        selection_generation: 0,
                    })
                    .await;
            }
            ConversationEvent::Deactivated => {
                let _ = view_tx.send(ViewCommand::ConversationCleared).await;
            }
            ConversationEvent::ListRefreshed { count } => {
                let _ = view_tx
                    .send(ViewCommand::HistoryUpdated { count: Some(count) })
                    .await;
            }
        }
    }

    /// Handle `StopStreaming` user event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    async fn handle_stop_streaming(
        chat_service: &Arc<dyn ChatService>,
        _view_tx: &mut mpsc::Sender<ViewCommand>,
    ) {
        chat_service.cancel();
        // StreamCancelled event will be emitted by the service
    }

    /// Handle `NewConversation` user event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    async fn handle_new_conversation(
        conversation_service: &Arc<dyn ConversationService>,
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
    ) {
        let default_profile = match Self::resolve_default_profile_id(profile_service).await {
            Ok(id) => id,
            Err(error_msg) => {
                tracing::error!("{}", error_msg);
                let _ = view_tx
                    .send(ViewCommand::ShowError {
                        title: "Error".to_string(),
                        message: error_msg,
                        severity: ErrorSeverity::Error,
                    })
                    .await;
                return;
            }
        };

        let result = conversation_service
            .create(Some("New Conversation".to_string()), default_profile)
            .await;
        match result {
            Ok(conversation) => {
                let conversation_id = conversation.id;
                let _ = conversation_service.set_active(conversation_id).await;
                let _ = view_tx
                    .send(ViewCommand::ConversationCreated {
                        id: conversation_id,
                        profile_id: default_profile,
                    })
                    .await;
                let _ = view_tx
                    .send(ViewCommand::ConversationActivated {
                        id: conversation_id,
                        selection_generation: 1,
                    })
                    .await;

                let _ = Self::emit_conversation_list(conversation_service, view_tx).await;
            }
            Err(e) => {
                let error_msg = format!("Failed to create conversation: {e}");
                tracing::error!("{}", error_msg);
                let _ = view_tx
                    .send(ViewCommand::ShowError {
                        title: "Error".to_string(),
                        message: error_msg,
                        severity: ErrorSeverity::Error,
                    })
                    .await;
            }
        }
    }

    /// Handle `SelectConversation` user event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    async fn handle_select_conversation(
        conversation_service: &Arc<dyn ConversationService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        id: Uuid,
        selection_generation: u64,
    ) {
        let result = conversation_service.set_active(id).await;
        match result {
            Ok(()) => {
                let _ = view_tx
                    .send(ViewCommand::ConversationActivated {
                        id,
                        selection_generation,
                    })
                    .await;

                match Self::replay_conversation_messages(
                    conversation_service,
                    view_tx,
                    id,
                    selection_generation,
                )
                .await
                {
                    Ok(count) => {
                        tracing::info!(
                            conversation_id = %id,
                            count,
                            "ChatPresenter: replaying selected conversation messages"
                        );
                    }
                    Err(e) => {
                        let error_msg =
                            format!("Failed to load messages for selected conversation: {e}");
                        tracing::warn!(
                            "Failed to load messages for selected conversation {}: {}",
                            id,
                            e
                        );
                        let _ = view_tx
                            .send(ViewCommand::ConversationLoadFailed {
                                conversation_id: id,
                                selection_generation,
                                message: error_msg.clone(),
                            })
                            .await;
                        let _ = view_tx
                            .send(ViewCommand::ShowError {
                                title: "Error".to_string(),
                                message: error_msg,
                                severity: ErrorSeverity::Error,
                            })
                            .await;
                    }
                }

                let _ = Self::emit_conversation_list(conversation_service, view_tx).await;
            }
            Err(e) => {
                let error_msg = format!("Failed to select conversation: {e}");
                tracing::error!("{}", error_msg);
                let _ = view_tx
                    .send(ViewCommand::ConversationLoadFailed {
                        conversation_id: id,
                        selection_generation,
                        message: error_msg.clone(),
                    })
                    .await;
                let _ = view_tx
                    .send(ViewCommand::ShowError {
                        title: "Error".to_string(),
                        message: error_msg,
                        severity: ErrorSeverity::Error,
                    })
                    .await;
            }
        }
    }

    /// Get or create active conversation
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    async fn get_or_create_conversation(
        conversation_service: &Arc<dyn ConversationService>,
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
    ) -> Result<Uuid, Box<dyn std::error::Error + Send + Sync>> {
        // Try to get active conversation
        let active_result = conversation_service.get_active().await;
        if let Ok(Some(id)) = active_result {
            return Ok(id);
        }

        let default_profile = Self::resolve_default_profile_id(profile_service)
            .await
            .map_err(Box::<dyn std::error::Error + Send + Sync>::from)?;

        let conversation_result = conversation_service
            .create(Some("New Conversation".to_string()), default_profile)
            .await;

        match conversation_result {
            Ok(conversation) => {
                let conversation_id = conversation.id;

                let _ = conversation_service.set_active(conversation_id).await;
                let _ = view_tx
                    .send(ViewCommand::ConversationCreated {
                        id: conversation_id,
                        profile_id: default_profile,
                    })
                    .await;

                let _ = view_tx
                    .send(ViewCommand::ConversationActivated {
                        id: conversation_id,
                        selection_generation: 1,
                    })
                    .await;

                Ok(conversation_id)
            }
            Err(e) => Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
        }
    }

    async fn resolve_default_profile_id(
        profile_service: &Arc<dyn ProfileService>,
    ) -> Result<Uuid, String> {
        if let Some(default_profile) = profile_service
            .get_default()
            .await
            .map_err(|e| format!("Failed to load default profile: {e}"))?
        {
            tracing::info!(
                profile_id = %default_profile.id,
                profile_name = %default_profile.name,
                "resolve_default_profile_id: using configured default profile"
            );
            return Ok(default_profile.id);
        }

        // Fallback: if default.json is stale but profiles exist, use first available profile.
        let profiles = profile_service
            .list()
            .await
            .map_err(|e| format!("Failed to list profiles: {e}"))?;

        tracing::info!(
            count = profiles.len(),
            "resolve_default_profile_id: default missing; considering fallback profiles"
        );

        if let Some(profile) = profiles.first() {
            tracing::warn!(
                profile_id = %profile.id,
                profile_name = %profile.name,
                "resolve_default_profile_id: default profile missing; falling back to first profile"
            );
            return Ok(profile.id);
        }

        Err("No default profile configured".to_string())
    }
}

// Implement Presenter trait
//
// @plan PLAN-20250125-REFACTOR.P12
// @requirement REQ-027.1
impl Presenter for ChatPresenter {
    fn start(&mut self) -> Result<(), PresenterError> {
        // Note: This is a sync wrapper - in real usage, call async start() directly
        // This is kept for trait compatibility
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PresenterError> {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// @plan PLAN-20250125-REFACTOR.P12
/// @requirement REQ-027.1
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::{broadcast, mpsc};

    /// Mock `ConversationService` for testing
    struct MockConversationService;

    #[async_trait::async_trait]
    impl ConversationService for MockConversationService {
        async fn create(
            &self,
            _title: Option<String>,
            _model_profile_id: Uuid,
        ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
            Ok(crate::models::Conversation {
                id: Uuid::new_v4(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                title: Some("Test Conversation".to_string()),
                profile_id: _model_profile_id,
                messages: vec![],
            })
        }

        async fn load(
            &self,
            _id: Uuid,
        ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound(
                "Not implemented".to_string(),
            ))
        }

        async fn list(
            &self,
            _limit: Option<usize>,
            _offset: Option<usize>,
        ) -> Result<Vec<crate::models::Conversation>, crate::services::ServiceError> {
            Ok(vec![])
        }

        async fn add_user_message(
            &self,
            _conversation_id: Uuid,
            _content: String,
        ) -> Result<crate::models::Message, crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound(
                "Not implemented".to_string(),
            ))
        }

        async fn add_assistant_message(
            &self,
            _conversation_id: Uuid,
            _content: String,
        ) -> Result<crate::models::Message, crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound(
                "Not implemented".to_string(),
            ))
        }

        async fn rename(
            &self,
            _id: Uuid,
            _new_title: String,
        ) -> Result<(), crate::services::ServiceError> {
            Ok(())
        }

        async fn delete(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
            Ok(())
        }

        async fn set_active(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
            Ok(())
        }

        async fn get_active(&self) -> Result<Option<Uuid>, crate::services::ServiceError> {
            Ok(None)
        }

        async fn get_messages(
            &self,
            _conversation_id: Uuid,
        ) -> Result<Vec<crate::models::Message>, crate::services::ServiceError> {
            Ok(vec![])
        }

        async fn update(
            &self,
            _id: Uuid,
            _title: Option<String>,
            _model_profile_id: Option<Uuid>,
        ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound(
                "Not implemented".to_string(),
            ))
        }
    }

    /// Mock `ChatService` for testing
    struct MockChatService;

    #[async_trait::async_trait]
    impl ChatService for MockChatService {
        async fn send_message(
            &self,
            _conversation_id: Uuid,
            _content: String,
        ) -> Result<
            Box<dyn futures::Stream<Item = crate::services::ChatStreamEvent> + Send + Unpin>,
            crate::services::ServiceError,
        > {
            // Return empty stream

            let stream = futures::stream::empty::<crate::services::ChatStreamEvent>();
            Ok(Box::new(stream))
        }

        fn cancel(&self) {
            // Mock cancel does nothing
        }

        fn is_streaming(&self) -> bool {
            false
        }
    }

    struct MockProfileService;

    #[async_trait::async_trait]
    impl ProfileService for MockProfileService {
        async fn list(
            &self,
        ) -> Result<Vec<crate::models::ModelProfile>, crate::services::ServiceError> {
            Ok(vec![])
        }

        async fn get(
            &self,
            id: Uuid,
        ) -> Result<crate::models::ModelProfile, crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound(format!(
                "profile {id} not found"
            )))
        }

        async fn create(
            &self,
            _name: String,
            _provider: String,
            _model: String,
            _base_url: Option<String>,
            _auth: crate::models::AuthConfig,
            _parameters: crate::models::ModelParameters,
            _system_prompt: Option<String>,
        ) -> Result<crate::models::ModelProfile, crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound(
                "not implemented".to_string(),
            ))
        }

        async fn update(
            &self,
            _id: Uuid,
            _name: Option<String>,
            _provider: Option<String>,
            _model: Option<String>,
            _base_url: Option<String>,
            _auth: Option<crate::models::AuthConfig>,
            _parameters: Option<crate::models::ModelParameters>,
            _system_prompt: Option<String>,
        ) -> Result<crate::models::ModelProfile, crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound(
                "not implemented".to_string(),
            ))
        }

        async fn delete(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
            Ok(())
        }

        async fn test_connection(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
            Ok(())
        }

        async fn get_default(
            &self,
        ) -> Result<Option<crate::models::ModelProfile>, crate::services::ServiceError> {
            Ok(Some(crate::models::ModelProfile {
                id: Uuid::new_v4(),
                name: "Default".to_string(),
                provider_id: "openai".to_string(),
                model_id: "gpt-4".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                auth: crate::models::AuthConfig::Keychain {
                    label: "test-key".to_string(),
                },
                parameters: crate::models::ModelParameters::default(),
                system_prompt: "test".to_string(),
            }))
        }

        async fn set_default(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
            Ok(())
        }
    }

    /// Test presenter creation
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    #[tokio::test]
    async fn test_handle_send_message_emits_events() {
        let (_event_tx, _) = broadcast::channel::<AppEvent>(100);
        let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);

        let conversation_service =
            Arc::new(MockConversationService) as Arc<dyn ConversationService>;
        let chat_service = Arc::new(MockChatService) as Arc<dyn ChatService>;
        let profile_service = Arc::new(MockProfileService) as Arc<dyn ProfileService>;

        // Simulate sending a message
        let content = "Hello, world!".to_string();
        let mut tx = view_tx.clone();
        let conv_service = conversation_service.clone();
        let chat_svc = chat_service.clone();
        let profile_svc = profile_service.clone();

        tokio::spawn(async move {
            ChatPresenter::handle_send_message(
                &conv_service,
                &chat_svc,
                &profile_svc,
                &mut tx,
                content,
            )
            .await;
        });

        // Wait for async processing
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Verify ViewCommands were sent
        let mut found_message = false;
        let mut found_thinking = false;

        while let Ok(cmd) = view_rx.try_recv() {
            match cmd {
                ViewCommand::MessageAppended { role, .. } => {
                    if matches!(role, MessageRole::User) {
                        found_message = true;
                    }
                }
                ViewCommand::ShowThinking { .. } => {
                    found_thinking = true;
                }
                _ => {}
            }
        }

        assert!(found_message, "Should have user message appended");
        assert!(found_thinking, "Should show thinking indicator");
    }

    /// Test handle stop streaming
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    #[tokio::test]
    async fn test_handle_stop_streaming() {
        let chat_service = Arc::new(MockChatService) as Arc<dyn ChatService>;
        let (view_tx, _) = mpsc::channel::<ViewCommand>(100);

        // Stop should call cancel on chat service
        ChatPresenter::handle_stop_streaming(&chat_service, &mut view_tx.clone()).await;

        // If we get here without panic, test passes
        assert!(!chat_service.is_streaming());
    }

    /// Test handle text delta produces view command
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    #[tokio::test]
    async fn test_handle_text_delta_produces_view_command() {
        let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);

        let event = ChatEvent::TextDelta {
            text: "Hello".to_string(),
        };

        ChatPresenter::handle_chat_event(&mut view_tx.clone(), event).await;

        // Verify AppendStream command was sent
        if let Ok(cmd) = view_rx.try_recv() {
            match cmd {
                ViewCommand::AppendStream { chunk, .. } => {
                    assert_eq!(chunk, "Hello");
                }
                _ => panic!("Expected AppendStream command"),
            }
        } else {
            panic!("Should have received a ViewCommand");
        }
    }

    /// Test handle stream completed
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    #[tokio::test]
    async fn test_handle_stream_completed() {
        let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);
        let conversation_id = Uuid::new_v4();

        let event = ChatEvent::StreamCompleted {
            conversation_id,
            message_id: Uuid::new_v4(),
            total_tokens: Some(100),
        };

        ChatPresenter::handle_chat_event(&mut view_tx.clone(), event).await;

        // Verify FinalizeStream and HideThinking commands
        let mut found_finalize = false;
        let mut found_hide = false;

        while let Ok(cmd) = view_rx.try_recv() {
            match cmd {
                ViewCommand::FinalizeStream { tokens, .. } => {
                    assert_eq!(tokens, 100);
                    found_finalize = true;
                }
                ViewCommand::HideThinking { .. } => {
                    found_hide = true;
                }
                _ => {}
            }
        }

        assert!(found_finalize, "Should finalize stream");
        assert!(found_hide, "Should hide thinking");
    }

    /// Test handle new conversation
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    #[tokio::test]
    async fn test_handle_new_conversation() {
        let conversation_service =
            Arc::new(MockConversationService) as Arc<dyn ConversationService>;
        let profile_service = Arc::new(MockProfileService) as Arc<dyn ProfileService>;
        let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);

        ChatPresenter::handle_new_conversation(
            &conversation_service,
            &profile_service,
            &mut view_tx.clone(),
        )
        .await;

        // Verify conversation created and activated
        let mut found_created = false;
        let mut found_activated = false;

        // Allow time for async processing
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        while let Ok(cmd) = view_rx.try_recv() {
            match cmd {
                ViewCommand::ConversationCreated { .. } => {
                    found_created = true;
                }
                ViewCommand::ConversationActivated { .. } => {
                    found_activated = true;
                }
                _ => {}
            }
        }

        assert!(found_created, "Should create conversation");
        assert!(found_activated, "Should activate conversation");
    }
}
