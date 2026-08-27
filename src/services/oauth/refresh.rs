//! Access-token renewal.
//!
//! Renewal is serialized per account. Two conversations starting at the same
//! moment would otherwise both present the same refresh token, and the second
//! exchange invalidates the first, leaving one of them holding a dead grant.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use serdes_ai_providers::{chatgpt_oauth_config, OAuthConfig};
use tokio::sync::Mutex as AsyncMutex;

use super::store::{self, StoredOAuthToken};
use super::{OAuthError, TokenSet};

/// Per-account locks. Held across the network call, so it has to be async.
type AccountLocks = Mutex<HashMap<String, Arc<AsyncMutex<()>>>>;

static LOCKS: OnceLock<AccountLocks> = OnceLock::new();

fn lock_for(account: &str) -> Arc<AsyncMutex<()>> {
    let locks = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = locks.lock().expect("oauth refresh lock map poisoned");
    Arc::clone(
        guard
            .entry(account.to_string())
            .or_insert_with(|| Arc::new(AsyncMutex::new(()))),
    )
}

/// Return a usable access token for an account, refreshing first if the stored
/// one is at or near expiry.
///
/// # Errors
///
/// Returns [`OAuthError::GrantRevoked`] when the account needs a fresh
/// sign-in, [`OAuthError::Storage`] when the record cannot be read or written,
/// and the underlying transport error when the refresh call itself fails.
pub async fn access_token_for(account: &str) -> Result<String, OAuthError> {
    let record = store::load_async(account)
        .await?
        .ok_or(OAuthError::GrantRevoked)?;

    if record.needs_reauth {
        return Err(OAuthError::GrantRevoked);
    }
    if !record.needs_refresh() {
        return Ok(record.access_token);
    }
    if !record.can_refresh() {
        // Expired with nothing to renew it with.
        if record.is_expired() {
            return Err(OAuthError::GrantRevoked);
        }
        return Ok(record.access_token);
    }

    refresh_account(account).await.map(|r| r.access_token)
}

/// Force a refresh for an account and persist the result.
///
/// Concurrent callers for one account collapse onto a single network exchange:
/// whoever loses the race re-reads the record the winner wrote and, finding it
/// fresh, returns it untouched.
///
/// # Errors
///
/// Returns [`OAuthError::GrantRevoked`] when the provider says the grant is
/// gone, and [`OAuthError::Rejected`] or [`OAuthError::Network`] otherwise.
pub async fn refresh_account(account: &str) -> Result<StoredOAuthToken, OAuthError> {
    let lock = lock_for(account);
    let _guard = lock.lock().await;

    let mut record = store::load_async(account)
        .await?
        .ok_or(OAuthError::GrantRevoked)?;

    // Someone else may have refreshed while we waited on the lock.
    if !record.needs_refresh() && !record.access_token.is_empty() {
        return Ok(record);
    }

    let refresh = record
        .refresh_token
        .clone()
        .ok_or(OAuthError::GrantRevoked)?;

    let config = config_for_issuer(&record.issuer);
    let tokens = exchange_refresh(&config, &refresh).await.inspect_err(|e| {
        tracing::warn!(account = %account, error = %e, "OAuth refresh failed");
    })?;

    record.apply_refresh(tokens);
    store::save_async(account, record.clone()).await?;
    Ok(record)
}

/// Perform the refresh exchange, translating a revoked grant into
/// [`OAuthError::GrantRevoked`] so callers can prompt for a new sign-in
/// instead of showing a provider error.
async fn exchange_refresh(config: &OAuthConfig, refresh: &str) -> Result<TokenSet, OAuthError> {
    match serdes_ai_providers::refresh_token(config, refresh).await {
        Ok(tokens) => Ok(tokens.into()),
        Err(error) => Err(classify_refresh_error(&error.to_string())),
    }
}

/// Decide whether a failed refresh is fatal to the grant or merely transient.
fn classify_refresh_error(message: &str) -> OAuthError {
    let lowered = message.to_ascii_lowercase();
    if lowered.contains("invalid_grant")
        || lowered.contains("invalid_request")
        || lowered.contains("unauthorized_client")
        || lowered.contains("http 400")
        || lowered.contains("http 401")
    {
        return OAuthError::GrantRevoked;
    }
    if lowered.contains("http ") {
        return OAuthError::Rejected(message.to_string());
    }
    OAuthError::Network(message.to_string())
}

/// The OAuth client configuration matching a stored record's issuer.
fn config_for_issuer(_issuer: &str) -> OAuthConfig {
    // ChatGPT is the only issuer PersonalAgent signs into today. When a second
    // one appears this dispatches on the stored issuer rather than growing a
    // parallel notion of provider identity.
    chatgpt_oauth_config()
}

/// Mark an account as needing a fresh sign-in and announce it, so any view
/// showing that account can offer the user a way back in.
///
/// # Errors
///
/// Returns [`OAuthError::Storage`] when the record cannot be written back.
pub async fn report_reauth_required_async(account: &str) -> Result<(), OAuthError> {
    let slug = account.to_string();
    store::off_runtime_public("flag a sign-in as expired", move || {
        store::mark_needs_reauth(&slug)
    })
    .await?;
    announce_reauth(account);
    Ok(())
}

/// The synchronous form, for callers that are not on the runtime.
///
/// # Errors
///
/// Returns [`OAuthError::Storage`] when the record cannot be written back.
pub fn report_reauth_required(account: &str) -> Result<(), OAuthError> {
    store::mark_needs_reauth(account)?;
    announce_reauth(account);
    Ok(())
}

/// Tell any view showing this account that it needs a fresh sign-in.
fn announce_reauth(account: &str) {
    // A bus with no subscribers is normal at startup and during tests; the
    // record on disk is what makes the state durable.
    let _ = crate::events::emit(crate::events::AppEvent::System(
        crate::events::SystemEvent::OAuthReauthRequired {
            account: account.to_string(),
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::oauth::{now_secs, CHATGPT_ISSUER};
    use crate::services::secure_store;

    fn stored(account: &str, expires_in: i64, refresh: Option<&str>) -> StoredOAuthToken {
        let mut record = StoredOAuthToken::from_token_set(
            TokenSet {
                access_token: "access-current".into(),
                refresh_token: refresh.map(str::to_owned),
                expires_in: Some(3600),
                ..TokenSet::default()
            },
            CHATGPT_ISSUER,
        );
        record.expires_at = Some(now_secs() + expires_in);
        store::save(account, &record).expect("seed record");
        record
    }

    #[tokio::test]
    async fn a_fresh_token_is_returned_without_a_refresh() {
        secure_store::use_mock_backend();
        let account = "chatgpt-fresh";
        stored(account, 3600, Some("refresh-1"));

        let token = access_token_for(account).await.expect("token");

        assert_eq!(token, "access-current");
    }

    #[tokio::test]
    async fn an_unknown_account_reports_a_revoked_grant() {
        secure_store::use_mock_backend();

        let error = access_token_for("chatgpt-unknown")
            .await
            .expect_err("unknown account");

        assert_eq!(error, OAuthError::GrantRevoked);
    }

    #[tokio::test]
    async fn an_account_flagged_for_reauth_reports_a_revoked_grant() {
        secure_store::use_mock_backend();
        let account = "chatgpt-flagged";
        stored(account, 3600, Some("refresh-1"));
        store::mark_needs_reauth(account).expect("flag");

        let error = access_token_for(account).await.expect_err("flagged");

        assert_eq!(error, OAuthError::GrantRevoked);
    }

    #[tokio::test]
    async fn an_expired_token_with_nothing_to_renew_it_reports_a_revoked_grant() {
        secure_store::use_mock_backend();
        let account = "chatgpt-expired-no-refresh";
        stored(account, -60, None);

        let error = access_token_for(account).await.expect_err("expired");

        assert_eq!(error, OAuthError::GrantRevoked);
    }

    #[tokio::test]
    async fn a_live_token_with_nothing_to_renew_it_is_still_usable() {
        secure_store::use_mock_backend();
        let account = "chatgpt-live-no-refresh";
        stored(account, 60, None);

        let token = access_token_for(account).await.expect("token");

        assert_eq!(token, "access-current");
    }

    #[test]
    fn invalid_grant_is_classified_as_a_revoked_grant() {
        assert_eq!(
            classify_refresh_error("HTTP 400: {\"error\":\"invalid_grant\"}"),
            OAuthError::GrantRevoked
        );
    }

    #[test]
    fn a_server_error_is_classified_as_rejected() {
        assert_eq!(
            classify_refresh_error("HTTP 503: upstream unavailable"),
            OAuthError::Rejected("HTTP 503: upstream unavailable".to_string())
        );
    }

    #[test]
    fn a_transport_failure_is_classified_as_a_network_error() {
        assert_eq!(
            classify_refresh_error("error sending request for url"),
            OAuthError::Network("error sending request for url".to_string())
        );
    }

    #[test]
    fn one_lock_is_handed_out_per_account() {
        let first = lock_for("chatgpt-lock-a");
        let second = lock_for("chatgpt-lock-a");
        let other = lock_for("chatgpt-lock-b");

        assert!(Arc::ptr_eq(&first, &second));
        assert!(!Arc::ptr_eq(&first, &other));
    }
}
