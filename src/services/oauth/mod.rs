//! OAuth sign-in for subscription-authenticated providers.
//!
//! `PersonalAgent` holds its own grant. It does not read or write the Codex
//! CLI's credentials: two clients refreshing one refresh token invalidate each
//! other, and this feature exists to replace that CLI rather than ride on it.
//!
//! Layout:
//!
//! - [`claims`] reads the identity claims out of an `id_token`.
//! - [`store`] persists tokens in the OS keychain, keyed by account slug.
//! - [`device_code`] implements the codex device-authorization protocol.
//! - [`refresh`] renews an access token, serialized per account.
//! - [`flow`] drives a sign-in end to end and is what the presenter talks to.

pub mod claims;
pub mod device_code;
pub mod flow;
pub mod refresh;
pub mod store;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use claims::AccountIdentity;
pub use flow::{
    ChatGptSignIn, CodexSignIn, SignInMethod, SignInOutcome, SignInStart, StartedSignIn,
};
pub use store::StoredOAuthToken;

/// Issuer for `ChatGPT` / Codex sign-in.
pub const CHATGPT_ISSUER: &str = "https://auth.openai.com";

/// Refresh an access token once it is within this many seconds of expiry.
///
/// A turn that starts inside the window would otherwise race the clock and
/// fail mid-stream.
pub const REFRESH_LEEWAY_SECS: i64 = 300;

/// Errors raised by the OAuth layer.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum OAuthError {
    /// The callback port this provider requires is already bound.
    #[error("callback port {0} is already in use")]
    PortUnavailable(u16),
    /// The user did not finish signing in before the deadline.
    #[error("sign-in timed out")]
    TimedOut,
    /// The authorization server returned a state value we did not issue.
    #[error("sign-in could not be verified")]
    StateMismatch,
    /// A device code was issued but expired before it was approved.
    #[error("device code expired")]
    DeviceCodeExpired,
    /// This issuer does not offer device-code sign-in.
    #[error("device-code sign-in is not available for this server")]
    DeviceCodeUnsupported,
    /// The token endpoint rejected the request.
    #[error("the identity provider rejected the sign-in: {0}")]
    Rejected(String),
    /// The grant is gone: the user revoked it, or changed their password.
    #[error("the saved session is no longer valid; sign in again")]
    GrantRevoked,
    /// The flow could not reach the network.
    #[error("network error: {0}")]
    Network(String),
    /// A token could not be read from or written to the keychain.
    #[error("credential storage error: {0}")]
    Storage(String),
    /// The user cancelled.
    #[error("sign-in cancelled")]
    Cancelled,
}

impl OAuthError {
    /// Whether retrying the same flow could plausibly succeed.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::TimedOut | Self::Network(_) | Self::DeviceCodeExpired | Self::StateMismatch
        )
    }
}

/// Tokens as returned by an authorization server, before they are stamped with
/// a fetch time and stored.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TokenSet {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub scope: Option<String>,
}

impl From<serdes_ai_providers::TokenResponse> for TokenSet {
    fn from(value: serdes_ai_providers::TokenResponse) -> Self {
        Self {
            access_token: value.access_token,
            refresh_token: value.refresh_token,
            id_token: value.id_token,
            token_type: value.token_type,
            expires_in: value.expires_in,
            scope: value.scope,
        }
    }
}

/// Current unix time in seconds.
///
/// # Panics
///
/// Panics if the system clock is set before the unix epoch.
#[must_use]
pub fn now_secs() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is set before the unix epoch")
        .as_secs();
    i64::try_from(secs).unwrap_or(i64::MAX)
}
