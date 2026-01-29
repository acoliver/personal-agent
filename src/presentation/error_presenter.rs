//! ErrorPresenter - handles error display and logging
//!
//! ErrorPresenter subscribes to error events and emits view commands
//! to display user-friendly error messages.
//!
//! @plan PLAN-20250125-REFACTOR.P12
//! @requirement REQ-027.4
//! @pseudocode presenters.md lines 450-505

use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

use crate::events::{AppEvent, types::{ChatEvent, McpEvent, SystemEvent}};
use super::{Presenter, PresenterError, ViewCommand};
use super::view_command::ErrorSeverity;

/// ErrorPresenter - handles error display and logging
///
/// @plan PLAN-20250125-REFACTOR.P12
/// @requirement REQ-027.4
/// @pseudocode presenters.md lines 450-453
pub struct ErrorPresenter {
    /// Event receiver from EventBus
    rx: broadcast::Receiver<AppEvent>,

    /// View command sender
    view_tx: mpsc::Sender<ViewCommand>,

    /// Running flag for event loop
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl ErrorPresenter {
    /// Create a new ErrorPresenter
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.4
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
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.4
    pub async fn start(&mut self) -> Result<(), PresenterError> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        self.running.store(true, std::sync::atomic::Ordering::Relaxed);

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
                        continue;
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
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.4
    pub async fn stop(&mut self) -> Result<(), PresenterError> {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Check if presenter is running
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Handle events from EventBus
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.4
    async fn handle_event(
        view_tx: &mut mpsc::Sender<ViewCommand>,
        event: AppEvent,
    ) {
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
    async fn handle_system_event(
        view_tx: &mut mpsc::Sender<ViewCommand>,
        event: SystemEvent,
    ) {
        match event {
            SystemEvent::Error { source, error, context } => {
                let message = if let Some(ctx) = context {
                    format!("{}: {}

Context: {}", source, error, ctx)
                } else {
                    format!("{}: {}", source, error)
                };

                let _ = view_tx.send(ViewCommand::ShowError {
                    title: format!("{} Error", source),
                    message,
                    severity: ErrorSeverity::Critical,
                }).await;
            }
            _ => {} // Ignore other system events
        }
    }

    /// Handle chat error events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.4
    async fn handle_chat_error(
        view_tx: &mut mpsc::Sender<ViewCommand>,
        event: ChatEvent,
    ) {
        match event {
            ChatEvent::StreamError { conversation_id, error, recoverable } => {
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Chat Error".to_string(),
                    message: error.clone(),
                    severity: if recoverable { ErrorSeverity::Warning } else { ErrorSeverity::Error },
                }).await;
            }
            _ => {} // Ignore other chat events
        }
    }

    /// Handle MCP error events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.4
    async fn handle_mcp_error(
        view_tx: &mut mpsc::Sender<ViewCommand>,
        event: McpEvent,
    ) {
        match event {
            McpEvent::StartFailed { id, name, error } => {
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "MCP Server Error".to_string(),
                    message: format!("Failed to start MCP server '{}': {}", name, error),
                    severity: ErrorSeverity::Error,
                }).await;
            }
            McpEvent::Unhealthy { id, name, error } => {
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "MCP Server Unhealthy".to_string(),
                    message: format!("MCP server '{}' is unhealthy: {}", name, error),
                    severity: ErrorSeverity::Warning,
                }).await;
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
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
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
        let (event_tx, _) = broadcast::channel::<AppEvent>(100);
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
                ViewCommand::ShowError { title, message, severity } => {
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
                ViewCommand::ShowError { title, message, severity } => {
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
                ViewCommand::ShowError { title, message, severity } => {
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
                ViewCommand::ShowError { title, severity, .. } => {
                    assert!(title.contains("Unhealthy"));
                    assert_eq!(severity, ErrorSeverity::Warning);
                }
                _ => panic!("Expected ShowError command"),
            }
        } else {
            panic!("Should have received a ViewCommand");
        }
    }
}
