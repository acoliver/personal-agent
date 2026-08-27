//! `CodexAuthPresenter` — drives `ChatGPT` sign-in, sign-out, and the account
//! list.
//!
//! Sign-in is the one flow here that takes minutes of human time, so it
//! reports twice: once immediately with what to render, then a countdown
//! until it resolves. The flow itself sits behind [`CodexSignIn`] so this
//! presenter can be tested without a browser, a network, or a bound port.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use super::view_command::{CodexAccountInfo, CodexSignInFailure};
use super::{PresenterError, ViewCommand};
use crate::events::types::{SystemEvent, UserEvent};
use crate::events::{AppEvent, EventBus};
use crate::models::profile::AuthConfig;
use crate::services::oauth::{flow, store, ChatGptSignIn, CodexSignIn, OAuthError, CHATGPT_ISSUER};
use crate::services::ProfileService;

/// How often the countdown is pushed to the view while a sign-in is pending.
const TICK: Duration = Duration::from_secs(1);

pub struct CodexAuthPresenter {
    rx: broadcast::Receiver<AppEvent>,
    profile_service: Arc<dyn ProfileService>,
    sign_in: Arc<dyn CodexSignIn>,
    view_tx: broadcast::Sender<ViewCommand>,
    running: Arc<std::sync::atomic::AtomicBool>,
    /// The in-flight sign-in, so cancelling can drop its callback server.
    in_flight: Arc<std::sync::Mutex<Option<JoinHandle<()>>>>,
}

impl CodexAuthPresenter {
    /// Build a presenter driving the real `ChatGPT` sign-in.
    pub fn new(
        profile_service: Arc<dyn ProfileService>,
        event_bus: &Arc<EventBus>,
        view_tx: broadcast::Sender<ViewCommand>,
    ) -> Self {
        Self::with_sign_in(
            profile_service,
            Arc::new(ChatGptSignIn::new()),
            event_bus,
            view_tx,
        )
    }

    /// Build a presenter over a specific sign-in driver.
    pub fn with_sign_in(
        profile_service: Arc<dyn ProfileService>,
        sign_in: Arc<dyn CodexSignIn>,
        event_bus: &Arc<EventBus>,
        view_tx: broadcast::Sender<ViewCommand>,
    ) -> Self {
        let rx = event_bus.subscribe();
        Self {
            rx,
            profile_service,
            sign_in,
            view_tx,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            in_flight: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Start the event loop.
    ///
    /// # Errors
    ///
    /// Returns `PresenterError` if presenter startup becomes fallible.
    pub async fn start(&mut self) -> Result<(), PresenterError> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }
        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        Self::emit_accounts(&self.profile_service, &self.view_tx).await;

        let mut rx = self.rx.resubscribe();
        let running = self.running.clone();
        let view_tx = self.view_tx.clone();
        let profile_service = self.profile_service.clone();
        let sign_in = self.sign_in.clone();
        let in_flight = self.in_flight.clone();

        tokio::spawn(async move {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(event) => {
                        Self::handle_event(&profile_service, &sign_in, &in_flight, &view_tx, event)
                            .await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("CodexAuthPresenter lagged: {n} events missed");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("CodexAuthPresenter event stream closed");
                        break;
                    }
                }
            }
            tracing::info!("CodexAuthPresenter event loop ended");
        });

        Ok(())
    }

    /// Stop the event loop and abandon any in-flight sign-in.
    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Self::abort_in_flight(&self.in_flight);
    }

    async fn handle_event(
        profile_service: &Arc<dyn ProfileService>,
        sign_in: &Arc<dyn CodexSignIn>,
        in_flight: &Arc<std::sync::Mutex<Option<JoinHandle<()>>>>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: AppEvent,
    ) {
        match event {
            AppEvent::User(UserEvent::StartCodexSignIn { method }) => {
                Self::on_start_sign_in(profile_service, sign_in, in_flight, view_tx, method.into())
                    .await;
            }
            AppEvent::User(UserEvent::CancelCodexSignIn) => {
                Self::abort_in_flight(in_flight);
                Self::fail(
                    view_tx,
                    CodexSignInFailure::Cancelled,
                    &OAuthError::Cancelled,
                );
            }
            AppEvent::User(UserEvent::SignOutCodexAccount { account }) => {
                Self::on_sign_out(profile_service, view_tx, &account).await;
            }
            AppEvent::User(UserEvent::ListCodexAccounts) => {
                Self::emit_accounts(profile_service, view_tx).await;
            }
            AppEvent::System(SystemEvent::OAuthReauthRequired { account }) => {
                let _ = view_tx.send(ViewCommand::CodexReauthRequired { account });
                Self::emit_accounts(profile_service, view_tx).await;
            }
            _ => {}
        }
    }

    /// Begin a sign-in, report what to render, then await the result.
    async fn on_start_sign_in(
        profile_service: &Arc<dyn ProfileService>,
        sign_in: &Arc<dyn CodexSignIn>,
        in_flight: &Arc<std::sync::Mutex<Option<JoinHandle<()>>>>,
        view_tx: &broadcast::Sender<ViewCommand>,
        method: flow::SignInMethod,
    ) {
        // Only one sign-in at a time; a second request supersedes the first.
        Self::abort_in_flight(in_flight);

        let started = match sign_in.begin(method).await {
            Ok(started) => started,
            Err(error) => {
                Self::fail(view_tx, Self::classify(&error), &error);
                return;
            }
        };

        let start = started.start;
        let expires_at = start.expires_at;
        let expires_in_secs = start.seconds_remaining();
        let _ = view_tx.send(ViewCommand::CodexSignInStarted {
            method: start.method.into(),
            url: start.url,
            user_code: start.user_code,
            copy_to_clipboard: start.copy_to_clipboard,
            expires_in_secs,
            fell_back: start.fell_back,
        });

        let view_tx = view_tx.clone();
        let profile_service = profile_service.clone();
        let handle = tokio::spawn(async move {
            let mut complete = started.complete;
            let mut ticker = tokio::time::interval(TICK);
            // The first tick fires immediately and would double the countdown
            // the view was just given.
            ticker.tick().await;

            let result = loop {
                tokio::select! {
                    outcome = &mut complete => break outcome,
                    _ = ticker.tick() => {
                        let remaining =
                            (expires_at - crate::services::oauth::now_secs()).max(0);
                        let _ = view_tx.send(ViewCommand::CodexSignInProgress {
                            remaining_secs: remaining,
                        });
                    }
                }
            };

            match result {
                Ok(tokens) => match flow::persist(tokens, CHATGPT_ISSUER) {
                    Ok(outcome) => {
                        let _ = view_tx.send(ViewCommand::CodexSignInCompleted {
                            account: outcome.account,
                            label: outcome.identity.display_label(),
                            plan: outcome.identity.plan,
                        });
                        Self::emit_accounts(&profile_service, &view_tx).await;
                    }
                    Err(error) => Self::fail(&view_tx, Self::classify(&error), &error),
                },
                Err(error) => Self::fail(&view_tx, Self::classify(&error), &error),
            }
        });

        if let Ok(mut guard) = in_flight.lock() {
            *guard = Some(handle);
        }
    }

    /// Forget an account and refresh the list.
    async fn on_sign_out(
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        account: &str,
    ) {
        if let Err(error) = store::delete(account) {
            Self::fail(view_tx, CodexSignInFailure::Storage, &error);
            return;
        }
        // Any live session was built with the token that just went away.
        crate::llm::open_responses::invalidate_all();
        Self::emit_accounts(profile_service, view_tx).await;
    }

    /// Push the current account list to the view.
    async fn emit_accounts(
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &broadcast::Sender<ViewCommand>,
    ) {
        let profiles = profile_service.list().await.unwrap_or_default();
        let accounts = store::load_all()
            .into_iter()
            .map(|(account, record)| {
                let used_by = profiles
                    .iter()
                    .filter(|profile| {
                        matches!(&profile.auth, AuthConfig::OAuth { account: slug } if slug == &account)
                    })
                    .map(|profile| profile.name.clone())
                    .collect();
                CodexAccountInfo {
                    label: record.identity.display_label(),
                    plan: record.identity.plan.clone(),
                    needs_reauth: record.needs_reauth,
                    expires_in_secs: record.seconds_remaining(),
                    used_by,
                    account,
                }
            })
            .collect();

        let _ = view_tx.send(ViewCommand::CodexAccountsListed { accounts });
    }

    /// Drop an in-flight sign-in, releasing its callback server.
    fn abort_in_flight(in_flight: &Arc<std::sync::Mutex<Option<JoinHandle<()>>>>) {
        if let Ok(mut guard) = in_flight.lock() {
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }
    }

    fn fail(
        view_tx: &broadcast::Sender<ViewCommand>,
        reason: CodexSignInFailure,
        error: &OAuthError,
    ) {
        let _ = view_tx.send(ViewCommand::CodexSignInFailed {
            reason,
            message: error.to_string(),
        });
    }

    /// Map a flow error onto the failure the sheet renders.
    const fn classify(error: &OAuthError) -> CodexSignInFailure {
        match error {
            OAuthError::TimedOut => CodexSignInFailure::TimedOut,
            OAuthError::StateMismatch => CodexSignInFailure::StateMismatch,
            OAuthError::DeviceCodeExpired => CodexSignInFailure::DeviceCodeExpired,
            OAuthError::DeviceCodeUnsupported => CodexSignInFailure::DeviceCodeUnsupported,
            OAuthError::Network(_) => CodexSignInFailure::Offline,
            OAuthError::Storage(_) => CodexSignInFailure::Storage,
            OAuthError::Cancelled => CodexSignInFailure::Cancelled,
            // A busy port never reaches here: the flow falls through to a
            // device code rather than failing.
            OAuthError::PortUnavailable(_) | OAuthError::Rejected(_) | OAuthError::GrantRevoked => {
                CodexSignInFailure::Rejected
            }
        }
    }
}
