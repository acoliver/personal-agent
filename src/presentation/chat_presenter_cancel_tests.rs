// ============================================================================
// Cancel Pipeline TDD Tests (Phase 04)
// These tests use struct-variant syntax for UserEvent::StopStreaming which
// will not compile until P05 lands the event type change.
// ============================================================================

use super::*;
use crate::events::types::UserEvent;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use uuid::Uuid;

/// Mock `ChatService` that records cancel calls for verification
struct RecordingChatService {
    cancelled_conversations: Arc<Mutex<Vec<Uuid>>>,
}

impl RecordingChatService {
    fn new() -> Self {
        Self {
            cancelled_conversations: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn cancelled_ids(&self) -> Vec<Uuid> {
        self.cancelled_conversations.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl ChatService for RecordingChatService {
    async fn send_message(
        &self,
        _conversation_id: Uuid,
        _content: String,
    ) -> Result<
        Box<dyn futures::Stream<Item = crate::services::ChatStreamEvent> + Send + Unpin>,
        crate::services::ServiceError,
    > {
        let stream = futures::stream::empty::<crate::services::ChatStreamEvent>();
        Ok(Box::new(stream))
    }

    fn cancel(&self, conversation_id: Uuid) {
        self.cancelled_conversations
            .lock()
            .unwrap()
            .push(conversation_id);
    }

    fn is_streaming(&self) -> bool {
        false
    }

    fn is_streaming_for(&self, _conversation_id: Uuid) -> bool {
        false
    }

    async fn resolve_tool_approval(
        &self,
        _request_id: String,
        _decision: crate::events::types::ToolApprovalResponseAction,
    ) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }
}

/// Test that `StopStreaming` event forwards the `conversation_id` to `cancel()`.
///
/// @plan PLAN-20260416-ISSUE173.P04
/// @requirement REQ-173-002.3
#[tokio::test]
async fn handle_stop_streaming_forwards_conversation_id() {
    let chat_service = Arc::new(RecordingChatService::new());
    let conversation_service = Arc::new(MockConversationService) as Arc<dyn ConversationService>;
    let (view_tx, _view_rx) = mpsc::channel::<ViewCommand>(100);

    let conversation_id_a = Uuid::new_v4();

    // Dispatch StopStreaming with conversation_id A using struct-variant syntax.
    // This will fail to compile until P05 changes UserEvent::StopStreaming to struct variant.
    let event = UserEvent::StopStreaming {
        conversation_id: conversation_id_a,
    };

    // Process the event through the presenter
    ChatPresenter::handle_user_event(
        &conversation_service,
        &(chat_service.clone() as Arc<dyn ChatService>),
        &(Arc::new(MockProfileService) as Arc<dyn ProfileService>),
        &(Arc::new(MockAppSettingsService) as Arc<dyn AppSettingsService>),
        &Arc::new(std::sync::Mutex::new(
            crate::models::ConversationExportFormat::Md,
        )),
        &mut view_tx.clone(),
        event,
    )
    .await;

    // Assert the mock received exactly one cancel(A) call
    let cancelled = chat_service.cancelled_ids();
    assert_eq!(cancelled.len(), 1, "Expected exactly one cancel call");
    assert_eq!(
        cancelled[0], conversation_id_a,
        "Expected cancel to be called with conversation_id A"
    );
}

/// Test that `StopStreaming` for conversation A does NOT cancel conversation B.
///
/// @plan PLAN-20260416-ISSUE173.P04
/// @requirement REQ-173-002.3
#[tokio::test]
async fn handle_stop_streaming_does_not_cancel_other_conversations() {
    let chat_service = Arc::new(RecordingChatService::new());
    let conversation_service = Arc::new(MockConversationService) as Arc<dyn ConversationService>;
    let (view_tx, _view_rx) = mpsc::channel::<ViewCommand>(100);

    let conversation_id_a = Uuid::new_v4();
    let conversation_id_b = Uuid::new_v4();

    // Dispatch StopStreaming with conversation_id A using struct-variant syntax.
    // This will fail to compile until P05 changes UserEvent::StopStreaming to struct variant.
    let event = UserEvent::StopStreaming {
        conversation_id: conversation_id_a,
    };

    // Process the event through the presenter
    ChatPresenter::handle_user_event(
        &conversation_service,
        &(chat_service.clone() as Arc<dyn ChatService>),
        &(Arc::new(MockProfileService) as Arc<dyn ProfileService>),
        &(Arc::new(MockAppSettingsService) as Arc<dyn AppSettingsService>),
        &Arc::new(std::sync::Mutex::new(
            crate::models::ConversationExportFormat::Md,
        )),
        &mut view_tx.clone(),
        event,
    )
    .await;

    // Assert cancel(B) was NEVER called
    let cancelled = chat_service.cancelled_ids();
    assert!(
        !cancelled.contains(&conversation_id_b),
        "cancel(B) should never be called when stopping stream for conversation A"
    );
}

/// Mock `AppSettingsService` for testing
struct MockAppSettingsService;

#[async_trait::async_trait]
impl AppSettingsService for MockAppSettingsService {
    async fn get_default_profile_id(&self) -> Result<Option<Uuid>, crate::services::ServiceError> {
        Ok(None)
    }

    async fn set_default_profile_id(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn clear_default_profile_id(&self) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn get_current_conversation_id(
        &self,
    ) -> Result<Option<Uuid>, crate::services::ServiceError> {
        Ok(None)
    }

    async fn set_current_conversation_id(
        &self,
        _id: Uuid,
    ) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn get_hotkey(&self) -> Result<Option<String>, crate::services::ServiceError> {
        Ok(None)
    }

    async fn set_hotkey(&self, _hotkey: String) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn get_theme(&self) -> Result<Option<String>, crate::services::ServiceError> {
        Ok(None)
    }

    async fn set_theme(&self, _theme: String) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn get_filter_emoji(&self) -> Result<Option<bool>, crate::services::ServiceError> {
        Ok(None)
    }

    async fn set_filter_emoji(&self, _enabled: bool) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn get_setting(
        &self,
        _key: &str,
    ) -> Result<Option<String>, crate::services::ServiceError> {
        Ok(None)
    }

    async fn set_setting(
        &self,
        _key: &str,
        _value: String,
    ) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn reset_to_defaults(&self) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }
}
