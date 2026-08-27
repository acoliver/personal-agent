//! Persistence for OAuth grants.
//!
//! One record per account slug, stored as JSON in the OS keychain under
//! `oauth:{account}`. The account index that makes enumeration possible lives
//! beside the API-key index; see `secure_store::oauth_tokens`.

use serde::{Deserialize, Serialize};

use super::claims::AccountIdentity;
use super::{now_secs, OAuthError, TokenSet, CHATGPT_ISSUER, REFRESH_LEEWAY_SECS};
use crate::services::secure_store;

/// A stored grant: the tokens plus everything needed to show and renew them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredOAuthToken {
    /// Bearer token sent to the model endpoint.
    pub access_token: String,
    /// Refresh token, absent when the provider did not issue one.
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Identity token, kept so account claims survive a restart.
    #[serde(default)]
    pub id_token: Option<String>,
    /// Identity claims decoded from `id_token` at sign-in time.
    #[serde(default)]
    pub identity: AccountIdentity,
    /// Granted scopes, as reported by the token endpoint.
    #[serde(default)]
    pub scope: Option<String>,
    /// Unix seconds at which the access token stops working.
    #[serde(default)]
    pub expires_at: Option<i64>,
    /// Unix seconds at which this record was written.
    pub obtained_at: i64,
    /// Authorization server that issued the grant.
    pub issuer: String,
    /// Set when a refresh failed permanently; the user must sign in again.
    #[serde(default)]
    pub needs_reauth: bool,
}

impl StoredOAuthToken {
    /// Build a record from a freshly issued token set.
    #[must_use]
    pub fn from_token_set(tokens: TokenSet, issuer: &str) -> Self {
        let now = now_secs();
        let identity = AccountIdentity::from_id_token(tokens.id_token.as_deref());
        Self {
            expires_at: tokens
                .expires_in
                .and_then(|secs| i64::try_from(secs).ok())
                .map(|secs| now + secs),
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            id_token: tokens.id_token,
            identity,
            scope: tokens.scope,
            obtained_at: now,
            issuer: issuer.to_string(),
            needs_reauth: false,
        }
    }

    /// Carry a refreshed token set onto an existing record.
    ///
    /// Providers routinely omit the refresh token and the id token on a
    /// refresh response, so those are kept when the new set does not carry
    /// them. The identity is likewise preserved.
    pub fn apply_refresh(&mut self, tokens: TokenSet) {
        let now = now_secs();
        self.access_token = tokens.access_token;
        if tokens.refresh_token.is_some() {
            self.refresh_token = tokens.refresh_token;
        }
        if let Some(id_token) = tokens.id_token {
            self.identity = AccountIdentity::from_id_token(Some(&id_token));
            self.id_token = Some(id_token);
        }
        if tokens.scope.is_some() {
            self.scope = tokens.scope;
        }
        self.expires_at = tokens
            .expires_in
            .and_then(|secs| i64::try_from(secs).ok())
            .map(|secs| now + secs);
        self.obtained_at = now;
        self.needs_reauth = false;
    }

    /// Whether the access token has already expired.
    ///
    /// A record with no expiry is never considered expired: the provider did
    /// not tell us when it ends, so the endpoint is the authority.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|at| now_secs() >= at)
    }

    /// Whether the access token should be refreshed before the next request.
    #[must_use]
    pub fn needs_refresh(&self) -> bool {
        self.expires_at
            .is_some_and(|at| now_secs() >= at - REFRESH_LEEWAY_SECS)
    }

    /// Whether this record can still be renewed without user interaction.
    #[must_use]
    pub const fn can_refresh(&self) -> bool {
        !self.needs_reauth && self.refresh_token.is_some()
    }

    /// Seconds until expiry, saturating at zero.
    #[must_use]
    pub fn seconds_remaining(&self) -> Option<i64> {
        self.expires_at.map(|at| (at - now_secs()).max(0))
    }
}

/// Load the grant for an account.
///
/// # Errors
///
/// Returns [`OAuthError::Storage`] when the keychain cannot be read or the
/// stored blob is not a valid record.
pub fn load(account: &str) -> Result<Option<StoredOAuthToken>, OAuthError> {
    let blob =
        secure_store::oauth_tokens::get(account).map_err(|e| OAuthError::Storage(e.to_string()))?;
    let Some(blob) = blob else {
        return Ok(None);
    };
    serde_json::from_str(&blob)
        .map(Some)
        .map_err(|e| OAuthError::Storage(format!("stored token for {account} is unreadable: {e}")))
}

/// Write the grant for an account.
///
/// # Errors
///
/// Returns [`OAuthError::Storage`] when the record cannot be serialized or the
/// keychain cannot be written.
pub fn save(account: &str, token: &StoredOAuthToken) -> Result<(), OAuthError> {
    let blob = serde_json::to_string(token)
        .map_err(|e| OAuthError::Storage(format!("token for {account} is unserializable: {e}")))?;
    secure_store::oauth_tokens::store(account, &blob)
        .map_err(|e| OAuthError::Storage(e.to_string()))
}

/// Forget an account entirely.
///
/// # Errors
///
/// Returns [`OAuthError::Storage`] when the keychain entry cannot be removed.
pub fn delete(account: &str) -> Result<(), OAuthError> {
    secure_store::oauth_tokens::delete(account).map_err(|e| OAuthError::Storage(e.to_string()))
}

/// Every account slug with a stored grant.
#[must_use]
pub fn list_accounts() -> Vec<String> {
    secure_store::oauth_tokens::list()
}

// ── Async wrappers ──────────────────────────────────────────────────────
//
// Keychain access is a blocking syscall owned by the OS, and it does not
// always return. On macOS an item whose access control does not name this
// binary puts up a system prompt, and if nobody answers that prompt the read
// never completes. Async callers therefore go through these, which move the
// call off the runtime and give it a deadline.

/// Longest a keychain call may take before we treat it as unavailable.
const STORE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Run a blocking store call off the runtime, with a deadline.
async fn off_runtime<T, F>(what: &'static str, call: F) -> Result<T, OAuthError>
where
    F: FnOnce() -> Result<T, OAuthError> + Send + 'static,
    T: Send + 'static,
{
    off_runtime_within(STORE_TIMEOUT, what, call).await
}

/// Run any blocking store call off the runtime with the standard deadline.
///
/// For sibling modules that need the same protection around a store operation
/// this module does not wrap directly.
///
/// # Errors
///
/// Returns whatever `call` returns, or [`OAuthError::Storage`] if it does not
/// answer within the deadline.
pub async fn off_runtime_public<T, F>(what: &'static str, call: F) -> Result<T, OAuthError>
where
    F: FnOnce() -> Result<T, OAuthError> + Send + 'static,
    T: Send + 'static,
{
    off_runtime(what, call).await
}

/// [`off_runtime`] with an explicit deadline, so tests need not wait one out.
async fn off_runtime_within<T, F>(
    deadline: std::time::Duration,
    what: &'static str,
    call: F,
) -> Result<T, OAuthError>
where
    F: FnOnce() -> Result<T, OAuthError> + Send + 'static,
    T: Send + 'static,
{
    let task = tokio::task::spawn_blocking(call);
    match tokio::time::timeout(deadline, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => Err(OAuthError::Storage(error.to_string())),
        Err(_elapsed) => Err(OAuthError::Storage(format!(
            "the keychain did not answer within {}s while trying to {what}; \
             it may be waiting on a system prompt",
            deadline.as_secs().max(1)
        ))),
    }
}

/// [`load`], off the runtime.
///
/// # Errors
///
/// As [`load`], plus [`OAuthError::Storage`] if the blocking thread is lost.
pub async fn load_async(account: &str) -> Result<Option<StoredOAuthToken>, OAuthError> {
    let account = account.to_string();
    off_runtime("read a saved sign-in", move || load(&account)).await
}

/// [`save`], off the runtime.
///
/// # Errors
///
/// As [`save`], plus [`OAuthError::Storage`] if the blocking thread is lost.
pub async fn save_async(account: &str, token: StoredOAuthToken) -> Result<(), OAuthError> {
    let account = account.to_string();
    off_runtime("save a sign-in", move || save(&account, &token)).await
}

/// [`delete`], off the runtime.
///
/// # Errors
///
/// As [`delete`], plus [`OAuthError::Storage`] if the blocking thread is lost.
pub async fn delete_async(account: &str) -> Result<(), OAuthError> {
    let account = account.to_string();
    off_runtime("forget a sign-in", move || delete(&account)).await
}

/// [`load_all`], off the runtime.
pub async fn load_all_async() -> Vec<(String, StoredOAuthToken)> {
    off_runtime("list saved sign-ins", || Ok(load_all()))
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(%error, "Reading stored OAuth accounts failed");
            Vec::new()
        })
}

/// Load every stored grant, skipping records that cannot be read.
///
/// A corrupt entry should not hide the accounts that are fine, so this reports
/// what it can and logs the rest.
#[must_use]
pub fn load_all() -> Vec<(String, StoredOAuthToken)> {
    list_accounts()
        .into_iter()
        .filter_map(|account| match load(&account) {
            Ok(Some(token)) => Some((account, token)),
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(account = %account, %error, "Skipping unreadable OAuth record");
                None
            }
        })
        .collect()
}

/// Mark an account as requiring a fresh sign-in and drop its dead access token.
///
/// # Errors
///
/// Returns [`OAuthError::Storage`] when the record cannot be written back.
pub fn mark_needs_reauth(account: &str) -> Result<(), OAuthError> {
    let Some(mut token) = load(account)? else {
        return Ok(());
    };
    token.needs_reauth = true;
    token.access_token = String::new();
    save(account, &token)
}

/// A default-shaped record for the `ChatGPT` issuer, used by tests and by the
/// device-code flow once it has exchanged its code.
#[must_use]
pub fn chatgpt_record(tokens: TokenSet) -> StoredOAuthToken {
    StoredOAuthToken::from_token_set(tokens, CHATGPT_ISSUER)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token_set() -> TokenSet {
        TokenSet {
            access_token: "access-1".into(),
            refresh_token: Some("refresh-1".into()),
            id_token: None,
            token_type: Some("Bearer".into()),
            expires_in: Some(3600),
            scope: Some("openid profile".into()),
        }
    }

    #[test]
    fn from_token_set_stamps_expiry_and_issuer() {
        let record = StoredOAuthToken::from_token_set(token_set(), CHATGPT_ISSUER);

        assert_eq!(record.issuer, CHATGPT_ISSUER);
        assert!(!record.needs_reauth);
        let remaining = record.seconds_remaining().expect("expiry recorded");
        assert!((3500..=3600).contains(&remaining), "remaining={remaining}");
    }

    #[test]
    fn record_without_expiry_never_reports_expired() {
        let record = StoredOAuthToken::from_token_set(
            TokenSet {
                expires_in: None,
                ..token_set()
            },
            CHATGPT_ISSUER,
        );

        assert!(!record.is_expired());
        assert!(!record.needs_refresh());
        assert_eq!(record.seconds_remaining(), None);
    }

    #[test]
    fn expiry_inside_the_leeway_asks_for_a_refresh_without_being_expired() {
        let mut record = StoredOAuthToken::from_token_set(token_set(), CHATGPT_ISSUER);
        record.expires_at = Some(now_secs() + REFRESH_LEEWAY_SECS - 1);

        assert!(record.needs_refresh());
        assert!(!record.is_expired());
    }

    #[test]
    fn expiry_outside_the_leeway_asks_for_nothing() {
        let mut record = StoredOAuthToken::from_token_set(token_set(), CHATGPT_ISSUER);
        record.expires_at = Some(now_secs() + REFRESH_LEEWAY_SECS + 60);

        assert!(!record.needs_refresh());
        assert!(!record.is_expired());
    }

    #[test]
    fn past_expiry_reports_expired() {
        let mut record = StoredOAuthToken::from_token_set(token_set(), CHATGPT_ISSUER);
        record.expires_at = Some(now_secs() - 1);

        assert!(record.is_expired());
        assert!(record.needs_refresh());
        assert_eq!(record.seconds_remaining(), Some(0));
    }

    #[test]
    fn apply_refresh_keeps_the_refresh_token_when_the_response_omits_it() {
        let mut record = StoredOAuthToken::from_token_set(token_set(), CHATGPT_ISSUER);

        record.apply_refresh(TokenSet {
            access_token: "access-2".into(),
            refresh_token: None,
            expires_in: Some(3600),
            ..TokenSet::default()
        });

        assert_eq!(record.access_token, "access-2");
        assert_eq!(record.refresh_token.as_deref(), Some("refresh-1"));
        assert_eq!(record.scope.as_deref(), Some("openid profile"));
    }

    #[test]
    fn apply_refresh_clears_the_reauth_flag() {
        let mut record = StoredOAuthToken::from_token_set(token_set(), CHATGPT_ISSUER);
        record.needs_reauth = true;

        record.apply_refresh(TokenSet {
            access_token: "access-2".into(),
            ..TokenSet::default()
        });

        assert!(!record.needs_reauth);
    }

    #[test]
    fn can_refresh_requires_a_token_and_a_healthy_record() {
        let mut record = StoredOAuthToken::from_token_set(token_set(), CHATGPT_ISSUER);
        assert!(record.can_refresh());

        record.needs_reauth = true;
        assert!(!record.can_refresh());

        record.needs_reauth = false;
        record.refresh_token = None;
        assert!(!record.can_refresh());
    }

    #[test]
    fn round_trips_through_the_keychain() {
        secure_store::use_mock_backend();
        let account = "chatgpt-store-round-trip";
        let record = StoredOAuthToken::from_token_set(token_set(), CHATGPT_ISSUER);

        save(account, &record).expect("save");
        let loaded = load(account).expect("load").expect("record present");

        assert_eq!(loaded, record);

        delete(account).expect("delete");
        assert_eq!(load(account).expect("load after delete"), None);
    }

    #[test]
    fn missing_account_loads_as_none() {
        secure_store::use_mock_backend();

        assert_eq!(load("chatgpt-never-stored").expect("load"), None);
    }

    #[test]
    fn corrupt_blob_surfaces_a_storage_error() {
        secure_store::use_mock_backend();
        let account = "chatgpt-corrupt";
        secure_store::oauth_tokens::store(account, "{not json").expect("store raw");

        let error = load(account).expect_err("corrupt blob should not parse");

        assert!(matches!(error, OAuthError::Storage(_)), "got {error:?}");
    }

    #[test]
    fn mark_needs_reauth_drops_the_access_token() {
        secure_store::use_mock_backend();
        let account = "chatgpt-reauth";
        save(
            account,
            &StoredOAuthToken::from_token_set(token_set(), CHATGPT_ISSUER),
        )
        .expect("save");

        mark_needs_reauth(account).expect("mark");
        let loaded = load(account).expect("load").expect("record present");

        assert!(loaded.needs_reauth);
        assert!(loaded.access_token.is_empty());
        assert_eq!(loaded.refresh_token.as_deref(), Some("refresh-1"));
    }

    #[tokio::test]
    async fn the_async_wrappers_round_trip_a_record() {
        secure_store::use_mock_backend();
        let account = "chatgpt-async-round-trip";
        let record = StoredOAuthToken::from_token_set(token_set(), CHATGPT_ISSUER);

        save_async(account, record.clone()).await.expect("save");
        let loaded = load_async(account)
            .await
            .expect("load")
            .expect("record present");
        assert_eq!(loaded, record);

        delete_async(account).await.expect("delete");
        assert_eq!(load_async(account).await.expect("load"), None);
    }

    #[tokio::test]
    async fn a_keychain_that_never_answers_gives_up_instead_of_hanging() {
        // The OS owns this call and does not always return: on macOS an item
        // whose access control does not name this binary raises a system
        // prompt, and an unanswered prompt never completes. Blocking forever
        // would take the panel, and the presenter behind it, with it.
        let outcome: Result<(), OAuthError> =
            off_runtime_within(std::time::Duration::from_millis(20), "stall", || {
                std::thread::sleep(std::time::Duration::from_millis(400));
                Ok(())
            })
            .await;

        let error = outcome.expect_err("a stalled keychain call must not hang");
        assert!(
            matches!(error, OAuthError::Storage(_)),
            "expected a storage error, got {error:?}"
        );
        assert!(
            error.to_string().contains("did not answer"),
            "the message should say what happened, got {error}"
        );
    }

    #[tokio::test]
    async fn a_call_that_answers_in_time_passes_its_result_through() {
        let value: u8 = off_runtime_within(std::time::Duration::from_secs(5), "read", || Ok(7))
            .await
            .expect("a prompt call should succeed");

        assert_eq!(value, 7);
    }

    #[test]
    fn mark_needs_reauth_on_an_unknown_account_is_a_no_op() {
        secure_store::use_mock_backend();

        mark_needs_reauth("chatgpt-absent").expect("no-op should succeed");
    }
}
