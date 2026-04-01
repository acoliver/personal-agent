//! `ErrorPresenter` - handles error display and logging
//!
//! `ErrorPresenter` subscribes to error events and emits view commands
//! to display user-friendly error messages.
//!
//! @plan PLAN-20250125-REFACTOR.P12
//! @requirement REQ-027.4
//! @pseudocode presenters.md lines 450-505

use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

use super::view_command::ErrorSeverity;
use super::{Presenter, PresenterError, ViewCommand};
use crate::events::{
    types::{ChatEvent, McpEvent, SystemEvent},
    AppEvent,
};
use crate::ui_gpui::error_log::{ErrorLogEntry, ErrorLogStore, ErrorSeverityTag};

/// `ErrorPresenter` - handles error display and logging
///
/// @plan PLAN-20250125-REFACTOR.P12
/// @requirement REQ-027.4
/// @pseudocode presenters.md lines 450-453
pub struct ErrorPresenter {
    /// Event receiver from `EventBus`
    rx: broadcast::Receiver<AppEvent>,

    /// View command sender
    view_tx: mpsc::Sender<ViewCommand>,

    /// Running flag for event loop
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl ErrorPresenter {
    /// Create a new `ErrorPresenter`
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.4
    #[must_use]
    pub fn new(
        event_bus: &broadcast::Sender<AppEvent>,
        view_tx: mpsc::Sender<ViewCommand>,
    ) -> Self {
        let rx = event_bus.subscribe();
        Self {
            rx,
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
    /// @requirement REQ-027.4
    pub async fn start(&mut self) -> Result<(), PresenterError> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let mut rx = self.rx.resubscribe();
        let running = self.running.clone();
        let mut view_tx = self.view_tx.clone();

        tokio::spawn(async move {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(event) => {
                        Self::handle_event(&mut view_tx, event).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("ErrorPresenter lagged: {} events missed", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("ErrorPresenter event stream closed");
                        break;
                    }
                }
            }
            tracing::info!("ErrorPresenter event loop ended");
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
    /// @requirement REQ-027.4
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
    /// @requirement REQ-027.4
    async fn handle_event(view_tx: &mut mpsc::Sender<ViewCommand>, event: AppEvent) {
        match event {
            AppEvent::System(system_evt) => {
                Self::handle_system_event(view_tx, system_evt).await;
            }
            AppEvent::Chat(chat_evt) => {
                Self::handle_chat_error(view_tx, chat_evt).await;
            }
            AppEvent::Mcp(mcp_evt) => {
                Self::handle_mcp_error(view_tx, mcp_evt).await;
            }
            _ => {} // Ignore other events
        }
    }

    /// Handle system error events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.4
    async fn handle_system_event(view_tx: &mut mpsc::Sender<ViewCommand>, event: SystemEvent) {
        if let SystemEvent::Error {
            source,
            error,
            context,
        } = event
        {
            let message = context.map_or_else(
                || format!("{source}: {error}"),
                |ctx| format!("{source}: {error}\n\nContext: {ctx}"),
            );

            let _ = view_tx
                .send(ViewCommand::ShowError {
                    title: format!("{source} Error"),
                    message: message.clone(),
                    severity: ErrorSeverity::Critical,
                })
                .await;

            ErrorLogStore::global().push(|id| ErrorLogEntry {
                id,
                timestamp: chrono::Utc::now(),
                severity: ErrorSeverityTag::Internal,
                source: source.clone(),
                message,
                raw_detail: None,
                conversation_title: None,
                conversation_id: None,
            });
        }
    }

    /// Handle chat error events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.4
    async fn handle_chat_error(view_tx: &mut mpsc::Sender<ViewCommand>, event: ChatEvent) {
        if let ChatEvent::StreamError {
            conversation_id,
            error,
            recoverable,
        } = event
        {
            let _ = view_tx
                .send(ViewCommand::ShowError {
                    title: "Chat Error".to_string(),
                    message: error.clone(),
                    severity: if recoverable {
                        ErrorSeverity::Warning
                    } else {
                        ErrorSeverity::Error
                    },
                })
                .await;

            ErrorLogStore::global().push(|id| ErrorLogEntry {
                id,
                timestamp: chrono::Utc::now(),
                severity: crate::ui_gpui::error_log::classify_error_severity(&error),
                source: "chat".to_string(),
                message: error.clone(),
                raw_detail: None,
                conversation_title: None,
                conversation_id: Some(conversation_id),
            });
        }
    }

    /// Handle MCP error events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.4
    async fn handle_mcp_error(view_tx: &mut mpsc::Sender<ViewCommand>, event: McpEvent) {
        match event {
            McpEvent::StartFailed { id: _, name, error } => {
                let _ = view_tx
                    .send(ViewCommand::ShowError {
                        title: "MCP Server Error".to_string(),
                        message: format!("Failed to start MCP server '{name}': {error}"),
                        severity: ErrorSeverity::Error,
                    })
                    .await;

                ErrorLogStore::global().push(|id| ErrorLogEntry {
                    id,
                    timestamp: chrono::Utc::now(),
                    severity: ErrorSeverityTag::Mcp,
                    source: format!("mcp/{name}"),
                    message: format!("Failed to start: {error}"),
                    raw_detail: None,
                    conversation_title: None,
                    conversation_id: None,
                });
            }
            McpEvent::Unhealthy { id: _, name, error } => {
                let _ = view_tx
                    .send(ViewCommand::ShowError {
                        title: "MCP Server Unhealthy".to_string(),
                        message: format!("MCP server '{name}' is unhealthy: {error}"),
                        severity: ErrorSeverity::Warning,
                    })
                    .await;

                ErrorLogStore::global().push(|id| ErrorLogEntry {
                    id,
                    timestamp: chrono::Utc::now(),
                    severity: ErrorSeverityTag::Mcp,
                    source: format!("mcp/{name}"),
                    message: format!("Unhealthy: {error}"),
                    raw_detail: None,
                    conversation_title: None,
                    conversation_id: None,
                });
            }
            _ => {} // Ignore other MCP events
        }
    }
}

// Implement Presenter trait
//
// @plan PLAN-20250125-REFACTOR.P12
// @requirement REQ-027.4
impl Presenter for ErrorPresenter {
    fn start(&mut self) -> Result<(), PresenterError> {
        // Note: This is a sync wrapper - in real usage, call async start() directly
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
/// @requirement REQ-027.4
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::{broadcast, mpsc};
    use uuid::Uuid;

    /// Test handle system error
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.4
    #[tokio::test]
    async fn test_handle_system_error() {
        let (_event_tx, _) = broadcast::channel::<AppEvent>(100);
        let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);

        let event = SystemEvent::Error {
            source: "TestSource".to_string(),
            error: "Test error message".to_string(),
            context: Some("Test context".to_string()),
        };

        ErrorPresenter::handle_system_event(&mut view_tx.clone(), event).await;

        // Verify ShowError command was sent
        if let Ok(cmd) = view_rx.try_recv() {
            match cmd {
                ViewCommand::ShowError {
                    title,
                    message,
                    severity,
                } => {
                    assert!(title.contains("TestSource"));
                    assert!(message.contains("Test error message"));
                    assert!(message.contains("Test context"));
                    assert_eq!(severity, ErrorSeverity::Critical);
                }
                _ => panic!("Expected ShowError command"),
            }
        } else {
            panic!("Should have received a ViewCommand");
        }
    }

    /// Test handle chat stream error
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.4
    #[tokio::test]
    async fn test_handle_chat_stream_error() {
        let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);

        let event = ChatEvent::StreamError {
            conversation_id: Uuid::new_v4(),
            error: "Stream failed".to_string(),
            recoverable: true,
        };

        ErrorPresenter::handle_chat_error(&mut view_tx.clone(), event).await;

        // Verify ShowError command was sent
        if let Ok(cmd) = view_rx.try_recv() {
            match cmd {
                ViewCommand::ShowError {
                    title,
                    message,
                    severity,
                } => {
                    assert!(title.contains("Chat"));
                    assert_eq!(message, "Stream failed");
                    assert_eq!(severity, ErrorSeverity::Warning);
                }
                _ => panic!("Expected ShowError command"),
            }
        } else {
            panic!("Should have received a ViewCommand");
        }
    }

    /// Test handle MCP start failed
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.4
    #[tokio::test]
    async fn test_handle_mcp_start_failed() {
        let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);

        let event = McpEvent::StartFailed {
            id: Uuid::new_v4(),
            name: "Test MCP".to_string(),
            error: "Connection failed".to_string(),
        };

        ErrorPresenter::handle_mcp_error(&mut view_tx.clone(), event).await;

        // Verify ShowError command was sent
        if let Ok(cmd) = view_rx.try_recv() {
            match cmd {
                ViewCommand::ShowError {
                    title,
                    message,
                    severity,
                } => {
                    assert!(title.contains("MCP"));
                    assert!(message.contains("Test MCP"));
                    assert!(message.contains("Connection failed"));
                    assert_eq!(severity, ErrorSeverity::Error);
                }
                _ => panic!("Expected ShowError command"),
            }
        } else {
            panic!("Should have received a ViewCommand");
        }
    }

    /// Test handle MCP unhealthy
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.4
    #[tokio::test]
    async fn test_handle_mcp_unhealthy() {
        let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);

        let event = McpEvent::Unhealthy {
            id: Uuid::new_v4(),
            name: "Test MCP".to_string(),
            error: "Health check failed".to_string(),
        };

        ErrorPresenter::handle_mcp_error(&mut view_tx.clone(), event).await;

        // Verify ShowError command was sent
        if let Ok(cmd) = view_rx.try_recv() {
            match cmd {
                ViewCommand::ShowError {
                    title, severity, ..
                } => {
                    assert!(title.contains("Unhealthy"));
                    assert_eq!(severity, ErrorSeverity::Warning);
                }
                _ => panic!("Expected ShowError command"),
            }
        } else {
            panic!("Should have received a ViewCommand");
        }
    }

    /// Verify system error pushes an Internal entry to `ErrorLogStore`
    /// @plan PLAN-20250125-REFACTOR.P12
    #[tokio::test]
    async fn test_system_error_pushes_to_error_log() {
        use crate::ui_gpui::error_log::{ErrorLogStore, ErrorSeverityTag};
        ErrorLogStore::global().clear();

        let (view_tx, _view_rx) = mpsc::channel::<ViewCommand>(100);
        let unique_error = format!("Something broke {}", Uuid::new_v4());
        let event = SystemEvent::Error {
            source: "SysSource".to_string(),
            error: unique_error.clone(),
            context: None,
        };

        ErrorPresenter::handle_system_event(&mut view_tx.clone(), event).await;

        // Locate by unique error text to be robust against parallel test execution.
        let entries = ErrorLogStore::global().entries();
        let our_entry = entries
            .iter()
            .find(|e| e.message.contains(&unique_error))
            .expect("entry with our unique error message should be present");
        assert_eq!(our_entry.severity, ErrorSeverityTag::Internal);
        assert_eq!(our_entry.source, "SysSource");
        assert!(ErrorLogStore::global().unviewed_count() >= 1);
    }

    /// Verify chat stream error pushes a Stream entry to `ErrorLogStore` with `conversation_id`
    /// @plan PLAN-20250125-REFACTOR.P12
    #[tokio::test]
    async fn test_chat_stream_error_pushes_to_error_log() {
        use crate::ui_gpui::error_log::{ErrorLogStore, ErrorSeverityTag};
        ErrorLogStore::global().clear();

        let (view_tx, _view_rx) = mpsc::channel::<ViewCommand>(100);
        let conv_id = Uuid::new_v4();
        let event = ChatEvent::StreamError {
            conversation_id: conv_id,
            error: "LLM unavailable".to_string(),
            recoverable: false,
        };

        ErrorPresenter::handle_chat_error(&mut view_tx.clone(), event).await;

        // The global store is shared across test threads; assert the specific entry exists
        // rather than asserting an exact count, to avoid flakiness from parallel tests.
        let entries = ErrorLogStore::global().entries();
        let our_entry = entries
            .iter()
            .find(|e| e.conversation_id == Some(conv_id))
            .expect("entry for our conversation_id should be present");
        assert_eq!(our_entry.severity, ErrorSeverityTag::Stream);
        assert_eq!(our_entry.source, "chat");
        assert_eq!(our_entry.message, "LLM unavailable");
        assert!(ErrorLogStore::global().unviewed_count() >= 1);
    }

    /// Verify MCP `StartFailed` pushes a Mcp entry to `ErrorLogStore`
    /// @plan PLAN-20250125-REFACTOR.P12
    #[tokio::test]
    async fn test_mcp_start_failed_pushes_to_error_log() {
        use crate::ui_gpui::error_log::{ErrorLogStore, ErrorSeverityTag};
        ErrorLogStore::global().clear();

        let (view_tx, _view_rx) = mpsc::channel::<ViewCommand>(100);
        let unique_name = format!("my-mcp-{}", Uuid::new_v4());
        let event = McpEvent::StartFailed {
            id: Uuid::new_v4(),
            name: unique_name.clone(),
            error: "port in use".to_string(),
        };

        ErrorPresenter::handle_mcp_error(&mut view_tx.clone(), event).await;

        // Locate by unique source name to be robust against parallel test execution.
        let entries = ErrorLogStore::global().entries();
        let our_entry = entries
            .iter()
            .find(|e| e.source == format!("mcp/{unique_name}"))
            .expect("entry with our unique mcp source should be present");
        assert_eq!(our_entry.severity, ErrorSeverityTag::Mcp);
        assert!(our_entry.message.contains("Failed to start"));
        assert!(our_entry.message.contains("port in use"));
        assert!(ErrorLogStore::global().unviewed_count() >= 1);
    }

    /// Verify MCP Unhealthy pushes a Mcp entry to `ErrorLogStore`
    /// @plan PLAN-20250125-REFACTOR.P12
    #[tokio::test]
    async fn test_mcp_unhealthy_pushes_to_error_log() {
        use crate::ui_gpui::error_log::{ErrorLogStore, ErrorSeverityTag};
        ErrorLogStore::global().clear();

        let (view_tx, _view_rx) = mpsc::channel::<ViewCommand>(100);
        let unique_name = format!("my-mcp-{}", Uuid::new_v4());
        let event = McpEvent::Unhealthy {
            id: Uuid::new_v4(),
            name: unique_name.clone(),
            error: "timeout".to_string(),
        };

        ErrorPresenter::handle_mcp_error(&mut view_tx.clone(), event).await;

        // Locate by unique source name to be robust against parallel test execution.
        let entries = ErrorLogStore::global().entries();
        let our_entry = entries
            .iter()
            .find(|e| e.source == format!("mcp/{unique_name}"))
            .expect("entry with our unique mcp source should be present");
        assert_eq!(our_entry.severity, ErrorSeverityTag::Mcp);
        assert!(our_entry.message.contains("Unhealthy"));
        assert!(our_entry.message.contains("timeout"));
        assert!(ErrorLogStore::global().unviewed_count() >= 1);
    }
}
