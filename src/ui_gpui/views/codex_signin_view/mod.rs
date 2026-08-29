//! The `ChatGPT` sign-in sheet.
//!
//! The browser flow is what runs by default. Device code takes over
//! automatically when the browser flow cannot start, which in practice means
//! the fixed callback port is taken. When it does, the user code goes on the
//! clipboard the moment it arrives, without the user pressing anything,
//! because pasting it is the next thing they will do.

mod render;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::events::types::{CodexSignInMethod, UserEvent};
use crate::presentation::view_command::{CodexSignInFailure, ViewCommand};
use crate::ui_gpui::bridge::GpuiBridge;

/// Where the sheet is in the sign-in lifecycle.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum SignInPhase {
    /// Nothing has started yet.
    #[default]
    Idle,
    /// A flow is running and the user has something to do.
    Pending(PendingSignIn),
    /// A grant was stored.
    Succeeded { label: String, plan: Option<String> },
    /// The attempt failed.
    Failed {
        reason: CodexSignInFailure,
        message: String,
    },
}

/// What the sheet renders while a sign-in is in flight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingSignIn {
    pub method: CodexSignInMethod,
    /// URL to show, and to open.
    pub url: String,
    /// Code the user types, for device-code sign-ins.
    pub user_code: Option<String>,
    /// Seconds left before this attempt expires.
    pub remaining_secs: i64,
    /// Set when the browser flow could not start and this took its place.
    pub fell_back: bool,
    /// Set once the code has been written to the clipboard.
    pub copied: bool,
}

impl PendingSignIn {
    /// The countdown as `m:ss`.
    #[must_use]
    pub fn countdown(&self) -> String {
        let remaining = self.remaining_secs.max(0);
        format!("{}:{:02}", remaining / 60, remaining % 60)
    }

    /// Whether the attempt has run out of time.
    #[must_use]
    pub const fn expired(&self) -> bool {
        self.remaining_secs <= 0
    }
}

/// The `ChatGPT` sign-in sheet.
pub struct CodexSignInView {
    pub(super) state: SignInPhase,
    pub(super) bridge: Option<Arc<GpuiBridge>>,
}

impl CodexSignInView {
    /// Build an empty sheet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: SignInPhase::Idle,
            bridge: None,
        }
    }

    /// Build a sheet wired to the event bridge.
    #[must_use]
    pub const fn with_bridge(bridge: Arc<GpuiBridge>) -> Self {
        Self {
            state: SignInPhase::Idle,
            bridge: Some(bridge),
        }
    }

    /// The current phase, for tests and for the parent view.
    #[must_use]
    pub const fn phase(&self) -> &SignInPhase {
        &self.state
    }

    /// The in-flight sign-in, when there is one.
    #[must_use]
    pub const fn pending(&self) -> Option<&PendingSignIn> {
        match &self.state {
            SignInPhase::Pending(pending) => Some(pending),
            _ => None,
        }
    }

    fn emit(&self, event: &UserEvent) {
        if let Some(bridge) = &self.bridge {
            if !bridge.emit(event.clone()) {
                tracing::error!("Failed to emit event {:?}", event);
            }
        } else {
            tracing::warn!("No bridge set - event not emitted: {:?}", event);
        }
    }

    /// Ask for a device code instead of the browser.
    pub fn use_device_code(&self) {
        self.emit(&UserEvent::StartCodexSignIn {
            method: CodexSignInMethod::DeviceCode,
        });
    }

    /// Ask for the browser flow again.
    pub fn use_browser(&self) {
        self.emit(&UserEvent::StartCodexSignIn {
            method: CodexSignInMethod::Browser,
        });
    }

    /// Retry whatever failed, with the method that failed.
    pub fn retry(&self) {
        // Only an expired code is worth asking for another code; everything
        // else goes back through the browser.
        let method = match &self.state {
            SignInPhase::Failed {
                reason: CodexSignInFailure::DeviceCodeExpired,
                ..
            } => CodexSignInMethod::DeviceCode,
            _ => CodexSignInMethod::Browser,
        };
        self.emit(&UserEvent::StartCodexSignIn { method });
    }

    /// Abandon the in-flight sign-in.
    pub fn cancel(&mut self) {
        self.emit(&UserEvent::CancelCodexSignIn);
        self.state = SignInPhase::Idle;
    }

    /// Apply a presenter command.
    ///
    /// Returns the text that should be put on the clipboard, if any. The
    /// caller owns the clipboard because only it holds the GPUI context.
    pub fn apply(&mut self, command: ViewCommand) -> Option<String> {
        match command {
            ViewCommand::CodexSignInStarted {
                method,
                url,
                user_code,
                copy_to_clipboard,
                expires_in_secs,
                fell_back,
            } => {
                self.state = SignInPhase::Pending(PendingSignIn {
                    method,
                    url,
                    user_code,
                    remaining_secs: expires_in_secs,
                    fell_back,
                    copied: copy_to_clipboard.is_some(),
                });
                copy_to_clipboard
            }
            ViewCommand::CodexSignInProgress { remaining_secs } => {
                if let SignInPhase::Pending(pending) = &mut self.state {
                    pending.remaining_secs = remaining_secs;
                }
                None
            }
            ViewCommand::CodexSignInCompleted { label, plan, .. } => {
                self.state = SignInPhase::Succeeded { label, plan };
                None
            }
            ViewCommand::CodexSignInFailed { reason, message } => {
                self.state = SignInPhase::Failed { reason, message };
                None
            }
            _ => None,
        }
    }

    /// The message shown for a failure, in the user's terms rather than the
    /// provider's.
    #[must_use]
    pub const fn failure_message(reason: CodexSignInFailure) -> &'static str {
        match reason {
            CodexSignInFailure::TimedOut => "Sign-in timed out.",
            CodexSignInFailure::StateMismatch => "Sign-in could not be verified. Start over.",
            CodexSignInFailure::DeviceCodeExpired => "That code expired.",
            CodexSignInFailure::DeviceCodeUnsupported => {
                "This server does not offer device-code sign-in."
            }
            CodexSignInFailure::Rejected => "OpenAI rejected the sign-in.",
            CodexSignInFailure::Offline => "No network connection.",
            CodexSignInFailure::Storage => "The sign-in could not be saved.",
            CodexSignInFailure::Cancelled => "Sign-in cancelled.",
        }
    }

    /// The label on the primary action offered after a failure.
    #[must_use]
    pub const fn failure_action(reason: CodexSignInFailure) -> Option<&'static str> {
        match reason {
            CodexSignInFailure::DeviceCodeExpired => Some("Get a new code"),
            CodexSignInFailure::DeviceCodeUnsupported => Some("Use my browser"),
            _ if reason.is_retryable() => Some("Try again"),
            _ => None,
        }
    }
}

impl Default for CodexSignInView {
    fn default() -> Self {
        Self::new()
    }
}
