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
    types::{ConversationEvent, UserEvent},
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

        Self::emit_initial_conversation_list(&self.conversation_service, &self.view_tx).await;
        Self::restore_startup_conversation(&self.conversation_service, &self.view_tx).await;
        self.spawn_event_loop();

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

    async fn emit_initial_conversation_list(
        conversation_service: &Arc<dyn ConversationService>,
        view_tx: &mpsc::Sender<ViewCommand>,
    ) {
        tracing::info!("ChatPresenter: emitting initial conversation list");
        match Self::emit_conversation_list(conversation_service, &mut view_tx.clone()).await {
            Ok(()) => {
                tracing::info!("ChatPresenter: initial conversation list emitted successfully");
            }
            Err(e) => tracing::error!(
                "ChatPresenter: failed to emit initial conversation list: {}",
                e
            ),
        }
    }

    async fn restore_startup_conversation(
        conversation_service: &Arc<dyn ConversationService>,
        view_tx: &mpsc::Sender<ViewCommand>,
    ) {
        match conversation_service.list(Some(1), Some(0)).await {
            Ok(conversations) => {
                if let Some(conversation) = conversations.first() {
                    Self::activate_startup_conversation(
                        conversation_service,
                        view_tx,
                        conversation.id,
                    )
                    .await;
                } else {
                    tracing::info!("ChatPresenter: no conversations at startup");
                }
            }
            Err(e) => tracing::warn!(
                "ChatPresenter: failed to list conversations at startup: {}",
                e
            ),
        }
    }

    async fn activate_startup_conversation(
        conversation_service: &Arc<dyn ConversationService>,
        view_tx: &mpsc::Sender<ViewCommand>,
        startup_id: Uuid,
    ) {
        tracing::info!(
            conversation_id = %startup_id,
            "ChatPresenter: startup selecting newest conversation"
        );
        let _ = conversation_service.set_active(startup_id).await;

        let mut startup_view_tx = view_tx.clone();
        let _ = startup_view_tx
            .send(ViewCommand::ConversationActivated {
                id: startup_id,
                selection_generation: 1,
            })
            .await;

        match Self::replay_conversation_messages(
            conversation_service,
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
    }

    fn spawn_event_loop(&self) {
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
#[path = "chat_presenter_tests.rs"]
mod tests;
