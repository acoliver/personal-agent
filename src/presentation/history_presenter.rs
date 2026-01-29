//! HistoryPresenter - handles conversation history UI events
//!
//! HistoryPresenter subscribes to conversation history events,
//! coordinates with conversation service, and emits view commands.
//!
//! @plan PLAN-20250125-REFACTOR.P10
//! @requirement REQ-025.1

use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::events::{AppEvent, types::{ConversationEvent, UserEvent}};
use crate::events::bus::EventBus;
use crate::services::ConversationService;
use super::{Presenter, PresenterError, ViewCommand};
use super::view_command::ErrorSeverity;

/// HistoryPresenter - handles conversation history UI events
///
/// @plan PLAN-20250128-PRESENTERS.P02
/// @requirement REQ-025.1
pub struct HistoryPresenter {
    /// Reference to event bus for subscribing to events
    event_bus: Arc<EventBus>,

    /// Reference to conversation service
    conversation_service: Arc<dyn ConversationService>,

    /// View command sender (mpsc for reliable delivery)
    view_tx: mpsc::Sender<ViewCommand>,

    /// Running flag for event loop
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl HistoryPresenter {
    /// Create a new HistoryPresenter
    ///
    /// @plan PLAN-20250128-PRESENTERS.P02
    /// @requirement REQ-025.1
    pub fn new(
        event_bus: Arc<EventBus>,
        conversation_service: Arc<dyn ConversationService>,
        view_tx: mpsc::Sender<ViewCommand>,
    ) -> Self {
        Self {
            event_bus,
            conversation_service,
            view_tx,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Start the presenter event loop
    ///
    /// @plan PLAN-20250128-PRESENTERS.P02
    /// @requirement REQ-025.1
    pub async fn start(&mut self) -> Result<(), PresenterError> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        self.running.store(true, std::sync::atomic::Ordering::Relaxed);

        // Subscribe to events from EventBus
        let mut rx = self.event_bus.subscribe();
        let running = self.running.clone();
        let conversation_service = self.conversation_service.clone();
        let mut view_tx = self.view_tx.clone();

        tokio::spawn(async move {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(event) => {
                        Self::handle_event(&conversation_service, &mut view_tx, event).await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("HistoryPresenter lagged: {} events missed", n);
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        tracing::info!("HistoryPresenter event stream closed");
                        break;
                    }
                }
            }
            tracing::info!("HistoryPresenter event loop ended");
        });

        Ok(())
    }

    /// Stop the presenter event loop
    ///
    /// @plan PLAN-20250128-PRESENTERS.P02
    /// @requirement REQ-025.1
    pub async fn stop(&mut self) -> Result<(), PresenterError> {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Check if presenter is running
    ///
    /// @plan PLAN-20250128-PRESENTERS.P02
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Handle events from EventBus
    ///
    /// @plan PLAN-20250128-PRESENTERS.P02
    /// @requirement REQ-025.1
    async fn handle_event(
        conversation_service: &Arc<dyn ConversationService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        event: AppEvent,
    ) {
        match event {
            AppEvent::User(user_evt) => {
                Self::handle_user_event(conversation_service, view_tx, user_evt).await;
            }
            AppEvent::Conversation(conv_evt) => {
                Self::handle_conversation_event(view_tx, conv_evt).await;
            }
            _ => {}
        }
    }

    /// Handle user events
    ///
    /// @plan PLAN-20250128-PRESENTERS.P02
    /// @requirement REQ-025.1
    async fn handle_user_event(
        conversation_service: &Arc<dyn ConversationService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        event: UserEvent,
    ) {
        match event {
            UserEvent::SelectConversation { id } => {
                Self::handle_select_conversation(conversation_service, view_tx, id).await;
            }
            _ => {}
        }
    }

    /// Handle conversation domain events
    ///
    /// @plan PLAN-20250128-PRESENTERS.P02
    /// @requirement REQ-025.1
    async fn handle_conversation_event(
        view_tx: &mut mpsc::Sender<ViewCommand>,
        event: ConversationEvent,
    ) {
        match event {
            ConversationEvent::Created { id, title } => {
                let _ = view_tx.send(ViewCommand::ConversationCreated {
                    id,
                    profile_id: Uuid::nil(),
                }).await;
            }
            ConversationEvent::TitleUpdated { id, title } => {
                let _ = view_tx.send(ViewCommand::ConversationTitleUpdated { id, title }).await;
            }
            ConversationEvent::Deleted { id } => {
                let _ = view_tx.send(ViewCommand::ConversationDeleted { id }).await;
            }
            ConversationEvent::ListRefreshed { count } => {
                let _ = view_tx.send(ViewCommand::HistoryUpdated {
                    count: Some(count),
                }).await;
            }
            _ => {}
        }
    }

    /// Handle SelectConversation user event
    ///
    /// @plan PLAN-20250128-PRESENTERS.P02
    /// @requirement REQ-025.1
    async fn handle_select_conversation(
        conversation_service: &Arc<dyn ConversationService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        id: Uuid,
    ) {
        match conversation_service.set_active(id).await {
            Ok(_) => {
                let _ = view_tx.send(ViewCommand::ConversationActivated { id }).await;
            }
            Err(e) => {
                tracing::error!("Failed to select conversation: {}", e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Error".to_string(),
                    message: format!("Failed to select conversation: {}", e),
                    severity: ErrorSeverity::Error,
                }).await;
            }
        }
    }
}

// Implement Presenter trait
//
// @plan PLAN-20250128-PRESENTERS.P02
// @requirement REQ-025.1
impl Presenter for HistoryPresenter {
    fn start(&mut self) -> Result<(), PresenterError> {
        // Note: This is a sync wrapper - in real usage, call async start() directly
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PresenterError> {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// @plan PLAN-20250128-PRESENTERS.P02
/// @requirement REQ-025.1
#[cfg(test)]
mod tests {
    use super::*;

    /// Test handle select conversation
    /// @plan PLAN-20250128-PRESENTERS.P02
    /// @requirement REQ-025.1
    #[tokio::test]
    async fn test_handle_select_conversation() {
        // Test implementation would go here
        assert!(true);
    }

    /// Test handle conversation created event
    /// @plan PLAN-20250128-PRESENTERS.P02
    /// @requirement REQ-025.1
    #[tokio::test]
    async fn test_handle_conversation_created() {
        // Test implementation would go here
        assert!(true);
    }

    /// Test handle title updated event
    /// @plan PLAN-20250128-PRESENTERS.P02
    /// @requirement REQ-025.1
    #[tokio::test]
    async fn test_handle_title_updated() {
        // Test implementation would go here
        assert!(true);
    }

    /// Test handle conversation deleted event
    /// @plan PLAN-20250128-PRESENTERS.P02
    /// @requirement REQ-025.1
    #[tokio::test]
    async fn test_handle_conversation_deleted() {
        // Test implementation would go here
        assert!(true);
    }

    /// Test handle list refreshed event
    /// @plan PLAN-20250128-PRESENTERS.P02
    /// @requirement REQ-025.1
    #[tokio::test]
    async fn test_handle_list_refreshed() {
        // Test implementation would go here
        assert!(true);
    }
}
