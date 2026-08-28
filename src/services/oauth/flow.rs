//! Driving a sign-in from start to finish.
//!
//! The browser flow is what runs by default: open the browser, show the
//! clickable authorize link, wait on the loopback callback. `ChatGPT` registers
//! a fixed callback port, so a port that is already bound cannot be negotiated
//! around. That is an expected condition rather than a failure, and it falls
//! through to a device code with the code already on the clipboard.
//!
//! The [`CodexSignIn`] trait exists so presenter tests can drive the whole
//! lifecycle without a browser, a network, or a bound port.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::future::BoxFuture;
use serdes_ai_providers::{chatgpt_oauth_config, run_pkce_flow, OAuthConfig};

use super::claims::AccountIdentity;
use super::device_code::{self, DeviceCode, PollOutcome, DEVICE_CODE_TTL_SECS};
use super::store::{self, StoredOAuthToken};
use super::{now_secs, OAuthError, TokenSet, CHATGPT_ISSUER};

/// How long to hold the loopback callback open.
///
/// The upstream preset allows two minutes, which assumes the browser is
/// already signed in. Someone entering an email, a password and a second
/// factor runs out of time, and the failure lands after they have done the
/// work. The device-code flow gets fifteen minutes for the same human effort,
/// so the browser gets a comparable window; nothing is consumed by waiting
/// beyond a bound socket the user can cancel.
const BROWSER_SIGN_IN_TIMEOUT_SECS: u64 = 10 * 60;

/// How a sign-in reaches the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignInMethod {
    /// Open a browser and wait on the loopback callback.
    Browser,
    /// Issue a short code the user enters on any device.
    DeviceCode,
}

/// Everything the UI needs to render a sign-in the moment it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignInStart {
    /// Which flow actually started, which is not always what was asked for.
    pub method: SignInMethod,
    /// The URL to show, and to open in a browser.
    pub url: String,
    /// The code the user types, for device-code sign-ins.
    pub user_code: Option<String>,
    /// Text the view puts on the clipboard without being asked.
    pub copy_to_clipboard: Option<String>,
    /// Unix seconds at which this attempt stops being usable.
    pub expires_at: i64,
    /// Set when the browser flow could not start and this took its place.
    pub fell_back: bool,
    /// Loopback port being listened on, for browser sign-ins.
    pub listening_port: Option<u16>,
}

impl SignInStart {
    /// Seconds left before the attempt expires, saturating at zero.
    #[must_use]
    pub fn seconds_remaining(&self) -> i64 {
        (self.expires_at - now_secs()).max(0)
    }
}

/// A sign-in that has started: what to show now, and what to await.
pub struct StartedSignIn {
    /// What the UI renders immediately.
    pub start: SignInStart,
    /// Resolves when the user finishes, gives up, or the attempt expires.
    pub complete: BoxFuture<'static, Result<TokenSet, OAuthError>>,
}

/// Result of a completed sign-in, once the tokens have been stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignInOutcome {
    /// Account slug the grant was stored under.
    pub account: String,
    /// Identity claims for display.
    pub identity: AccountIdentity,
}

/// Starts sign-ins. Implemented for real by [`ChatGptSignIn`] and faked in
/// presenter tests.
#[async_trait]
pub trait CodexSignIn: Send + Sync {
    /// Begin a sign-in, returning what to render and a future to await.
    ///
    /// # Errors
    ///
    /// Returns an [`OAuthError`] when the attempt cannot be started at all.
    async fn begin(&self, method: SignInMethod) -> Result<StartedSignIn, OAuthError>;
}

/// The real `ChatGPT` sign-in: PKCE in a browser, device code as the fallback.
pub struct ChatGptSignIn {
    config: OAuthConfig,
    issuer: String,
    http: reqwest::Client,
    open_browser: Arc<dyn Fn(&str) + Send + Sync>,
}

impl Default for ChatGptSignIn {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatGptSignIn {
    /// Build a sign-in against the `ChatGPT` issuer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: chatgpt_oauth_config().with_timeout(BROWSER_SIGN_IN_TIMEOUT_SECS),
            issuer: CHATGPT_ISSUER.to_string(),
            http: reqwest::Client::new(),
            open_browser: Arc::new(open_in_browser),
        }
    }

    /// Replace the browser-opening side effect. Used by tests.
    #[must_use]
    pub fn with_browser_opener(mut self, opener: Arc<dyn Fn(&str) + Send + Sync>) -> Self {
        self.open_browser = opener;
        self
    }

    /// Point the flow at a different issuer. Used by tests against a local
    /// server.
    #[must_use]
    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = issuer.into();
        self
    }

    /// Start the browser flow, or report why it could not start.
    async fn begin_browser(&self) -> Result<StartedSignIn, OAuthError> {
        let (url, handle) = run_pkce_flow(&self.config)
            .await
            .map_err(|error| classify_flow_start(&error))?;

        let port = handle.port();
        (self.open_browser)(&url);

        let timeout = Duration::from_secs(self.config.callback_timeout_secs);
        let expires_at = now_secs() + i64::try_from(self.config.callback_timeout_secs).unwrap_or(0);

        let complete = Box::pin(async move {
            match tokio::time::timeout(timeout, handle.wait_for_tokens()).await {
                Ok(Ok(tokens)) => Ok(TokenSet::from(tokens)),
                Ok(Err(error)) => Err(classify_wait(&error)),
                Err(_elapsed) => Err(OAuthError::TimedOut),
            }
        });

        Ok(StartedSignIn {
            start: SignInStart {
                method: SignInMethod::Browser,
                url,
                user_code: None,
                copy_to_clipboard: None,
                expires_at,
                fell_back: false,
                listening_port: Some(port),
            },
            complete,
        })
    }

    /// Start the device-code flow.
    async fn begin_device_code(&self, fell_back: bool) -> Result<StartedSignIn, OAuthError> {
        let code =
            device_code::request_device_code(&self.http, &self.issuer, &self.config.client_id)
                .await?;

        let expires_at = now_secs() + DEVICE_CODE_TTL_SECS;
        let start = SignInStart {
            method: SignInMethod::DeviceCode,
            url: code.verification_url.clone(),
            user_code: Some(code.user_code.clone()),
            copy_to_clipboard: Some(code.user_code.clone()),
            expires_at,
            fell_back,
            listening_port: None,
        };

        let http = self.http.clone();
        let issuer = self.issuer.clone();
        let client_id = self.config.client_id.clone();
        let complete = Box::pin(async move {
            poll_until_approved(&http, &issuer, &client_id, &code, expires_at).await
        });

        Ok(StartedSignIn { start, complete })
    }
}

#[async_trait]
impl CodexSignIn for ChatGptSignIn {
    async fn begin(&self, method: SignInMethod) -> Result<StartedSignIn, OAuthError> {
        match method {
            SignInMethod::DeviceCode => self.begin_device_code(false).await,
            SignInMethod::Browser => match self.begin_browser().await {
                // A bound callback port is not a failure. ChatGPT registers a
                // fixed port, so there is nothing to negotiate; hand the user
                // a code instead of an apology.
                Err(OAuthError::PortUnavailable(port)) => {
                    tracing::info!(port, "Callback port busy; falling back to device code");
                    self.begin_device_code(true).await
                }
                other => other,
            },
        }
    }
}

/// Poll for approval until the user acts or the code expires.
async fn poll_until_approved(
    http: &reqwest::Client,
    issuer: &str,
    client_id: &str,
    code: &DeviceCode,
    expires_at: i64,
) -> Result<TokenSet, OAuthError> {
    let interval = Duration::from_secs(code.interval_secs);
    loop {
        // Checked before the request, not after: the interval can be up to
        // thirty seconds, so a sleep can cross the deadline and the next poll
        // would report a network error or a late approval instead of expiry.
        if now_secs() >= expires_at {
            return Err(OAuthError::DeviceCodeExpired);
        }

        match device_code::poll_once(http, issuer, client_id, code).await? {
            (PollOutcome::Approved, Some(tokens)) => return Ok(tokens),
            (PollOutcome::Approved, None) => {
                return Err(OAuthError::Rejected(
                    "device authorization reported approval without tokens".to_string(),
                ))
            }
            (PollOutcome::Pending, _) => {}
        }

        tokio::time::sleep(interval).await;
    }
}

/// Store a completed sign-in and report which account it landed under.
///
/// # Errors
///
/// Returns [`OAuthError::Storage`] when the record cannot be written.
pub fn persist(tokens: TokenSet, issuer: &str) -> Result<SignInOutcome, OAuthError> {
    let record = StoredOAuthToken::from_token_set(tokens, issuer);
    let account = record.identity.account_slug();
    store::save(&account, &record)?;
    Ok(SignInOutcome {
        account,
        identity: record.identity,
    })
}

/// Translate a flow-start failure into something the UI can act on.
fn classify_flow_start(error: &serdes_ai_providers::OAuthError) -> OAuthError {
    match error {
        serdes_ai_providers::OAuthError::ServerStart(io) => {
            OAuthError::PortUnavailable(chatgpt_oauth_config().required_port.unwrap_or_default())
                .tap_log(io)
        }
        other => OAuthError::Network(other.to_string()),
    }
}

/// Translate a callback-wait failure.
fn classify_wait(error: &serdes_ai_providers::OAuthError) -> OAuthError {
    match error {
        serdes_ai_providers::OAuthError::StateMismatch { .. } => OAuthError::StateMismatch,
        serdes_ai_providers::OAuthError::TokenExchange(message) => {
            OAuthError::Rejected(message.clone())
        }
        serdes_ai_providers::OAuthError::Callback(_) => OAuthError::TimedOut,
        other => OAuthError::Network(other.to_string()),
    }
}

/// Small helper so the port-busy branch can record why the bind failed without
/// smuggling the io error into the user-facing message.
trait TapLog {
    fn tap_log(self, error: &std::io::Error) -> Self;
}

impl TapLog for OAuthError {
    fn tap_log(self, error: &std::io::Error) -> Self {
        tracing::debug!(%error, "OAuth callback server could not bind");
        self
    }
}

/// Ask the platform to open a URL.
fn open_in_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let launcher = "open";
    #[cfg(target_os = "linux")]
    let launcher = "xdg-open";
    #[cfg(target_os = "windows")]
    let launcher = "explorer";

    if let Err(error) = std::process::Command::new(launcher).arg(url).spawn() {
        // The URL is on screen and copyable, so a failed launch is recoverable
        // by the user without any further prompting.
        tracing::warn!(%error, "Could not open a browser for sign-in");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::secure_store;

    #[test]
    fn seconds_remaining_saturates_at_zero() {
        let start = SignInStart {
            method: SignInMethod::Browser,
            url: "https://example.test".into(),
            user_code: None,
            copy_to_clipboard: None,
            expires_at: now_secs() - 30,
            fell_back: false,
            listening_port: Some(1455),
        };

        assert_eq!(start.seconds_remaining(), 0);
    }

    #[test]
    fn seconds_remaining_counts_down_from_the_deadline() {
        let start = SignInStart {
            method: SignInMethod::DeviceCode,
            url: "https://example.test".into(),
            user_code: Some("BXFD-KM2Q".into()),
            copy_to_clipboard: Some("BXFD-KM2Q".into()),
            expires_at: now_secs() + 120,
            fell_back: true,
            listening_port: None,
        };

        let remaining = start.seconds_remaining();
        assert!((115..=120).contains(&remaining), "remaining={remaining}");
    }

    #[test]
    fn persist_stores_under_the_account_slug_from_the_id_token() {
        secure_store::use_mock_backend();
        // {"email":"a@b.c","https://api.openai.com/auth":{"chatgpt_account_id":"acct-9"}}
        let payload = base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            br#"{"email":"a@b.c","https://api.openai.com/auth":{"chatgpt_account_id":"acct-9"}}"#,
        );
        let id_token = format!("header.{payload}.signature");

        let outcome = persist(
            TokenSet {
                access_token: "access-1".into(),
                refresh_token: Some("refresh-1".into()),
                id_token: Some(id_token),
                expires_in: Some(3600),
                ..TokenSet::default()
            },
            CHATGPT_ISSUER,
        )
        .expect("persist");

        assert_eq!(outcome.account, "chatgpt-acct-9");
        assert_eq!(outcome.identity.email.as_deref(), Some("a@b.c"));
        let stored = store::load(&outcome.account)
            .expect("load")
            .expect("record present");
        assert_eq!(stored.access_token, "access-1");
        assert_eq!(stored.issuer, CHATGPT_ISSUER);
    }

    #[test]
    fn state_mismatch_is_reported_as_unverifiable() {
        let error = classify_wait(&serdes_ai_providers::OAuthError::StateMismatch {
            expected: "a".into(),
            actual: "b".into(),
        });

        assert_eq!(error, OAuthError::StateMismatch);
    }

    #[test]
    fn a_token_exchange_failure_is_reported_as_rejected() {
        let error = classify_wait(&serdes_ai_providers::OAuthError::TokenExchange(
            "HTTP 400: nope".into(),
        ));

        assert_eq!(error, OAuthError::Rejected("HTTP 400: nope".to_string()));
    }

    #[test]
    fn a_bound_callback_port_is_reported_as_port_unavailable() {
        let error = classify_flow_start(&serdes_ai_providers::OAuthError::ServerStart(
            std::io::Error::new(std::io::ErrorKind::AddrInUse, "address in use"),
        ));

        assert_eq!(error, OAuthError::PortUnavailable(1455));
    }

    #[test]
    fn the_browser_gets_long_enough_to_actually_sign_in() {
        // The upstream preset allows two minutes, which is not enough for an
        // email, a password and a second factor. Observed running out while a
        // real sign-in was in progress.
        let config = ChatGptSignIn::new().config;

        assert_eq!(config.callback_timeout_secs, BROWSER_SIGN_IN_TIMEOUT_SECS);
        assert!(
            config.callback_timeout_secs >= 5 * 60,
            "a sign-in with a second factor needs more than {}s",
            config.callback_timeout_secs
        );
    }

    #[test]
    fn port_unavailable_is_not_offered_as_a_retry() {
        assert!(!OAuthError::PortUnavailable(1455).is_retryable());
        assert!(OAuthError::TimedOut.is_retryable());
        assert!(OAuthError::DeviceCodeExpired.is_retryable());
        assert!(!OAuthError::GrantRevoked.is_retryable());
    }
}
