//! Device-authorization sign-in, as served by the `OpenAI` auth backend.
//!
//! This is not RFC 8628. The server issues a short user code, the user
//! approves it on any device, and the poll that succeeds hands back a PKCE
//! pair the server generated on our behalf plus an authorization code. The
//! final leg is an ordinary PKCE token exchange, so the result is the same
//! [`TokenSet`] the browser flow produces.
//!
//! Wire shape, all against `{issuer}/api/accounts`:
//!
//! ```text
//! POST /deviceauth/usercode  {client_id}            -> {device_auth_id, user_code, interval}
//! POST /deviceauth/token     {device_auth_id, user_code}
//!        403 | 404 -> not approved yet, wait `interval`
//!        200       -> {authorization_code, code_challenge, code_verifier}
//! POST {issuer}/oauth/token  (authorization_code grant, redirect to
//!                             {issuer}/deviceauth/callback)
//! ```

use serde::{Deserialize, Serialize};

use super::{OAuthError, TokenSet};

/// How long the server keeps a user code alive, in seconds.
pub const DEVICE_CODE_TTL_SECS: i64 = 15 * 60;

/// Fallback poll interval when the server does not send one.
const DEFAULT_POLL_INTERVAL_SECS: u64 = 5;

/// Longest poll interval we will honour, so a bad value cannot stall the UI.
const MAX_POLL_INTERVAL_SECS: u64 = 30;

/// A user code awaiting approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCode {
    /// Page the user opens to enter the code.
    pub verification_url: String,
    /// The code itself. Goes on the clipboard the moment it is issued.
    pub user_code: String,
    /// Opaque handle correlating polls with this request.
    pub device_auth_id: String,
    /// Seconds to wait between polls.
    pub interval_secs: u64,
}

/// Request body for the user-code call.
#[derive(Serialize)]
struct UserCodeRequest<'a> {
    client_id: &'a str,
}

/// Response to the user-code call.
///
/// `interval` arrives as a string on the wire.
#[derive(Deserialize)]
struct UserCodeResponse {
    device_auth_id: String,
    #[serde(alias = "user_code", alias = "usercode")]
    user_code: String,
    #[serde(default)]
    interval: Option<String>,
}

/// Request body for each poll.
#[derive(Serialize)]
struct PollRequest<'a> {
    device_auth_id: &'a str,
    user_code: &'a str,
}

/// What a successful poll returns: an authorization code plus the PKCE pair
/// the server generated for it.
#[derive(Deserialize)]
struct ApprovedResponse {
    authorization_code: String,
    code_verifier: String,
}

/// What one poll attempt concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollOutcome {
    /// The user has not approved yet; wait and poll again.
    Pending,
    /// The user approved; the caller now holds tokens.
    Approved,
}

/// Ask the issuer for a user code.
///
/// # Errors
///
/// Returns [`OAuthError::DeviceCodeUnsupported`] when the issuer does not
/// serve device authorization, [`OAuthError::Network`] for transport failures,
/// and [`OAuthError::Rejected`] for any other error status.
pub async fn request_device_code(
    http: &reqwest::Client,
    issuer: &str,
    client_id: &str,
) -> Result<DeviceCode, OAuthError> {
    let issuer = issuer.trim_end_matches('/');
    let url = format!("{issuer}/api/accounts/deviceauth/usercode");

    let response = http
        .post(&url)
        .json(&UserCodeRequest { client_id })
        .send()
        .await
        .map_err(|e| OAuthError::Network(e.to_string()))?;

    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(OAuthError::DeviceCodeUnsupported);
    }
    if !status.is_success() {
        return Err(OAuthError::Rejected(format!(
            "device code request failed with status {status}"
        )));
    }

    let body: UserCodeResponse = response
        .json()
        .await
        .map_err(|e| OAuthError::Rejected(format!("unreadable device code response: {e}")))?;

    Ok(DeviceCode {
        verification_url: format!("{issuer}/codex/device"),
        user_code: body.user_code,
        device_auth_id: body.device_auth_id,
        interval_secs: parse_interval(body.interval.as_deref()),
    })
}

/// Poll once for approval.
///
/// Returns [`PollOutcome::Pending`] while the user has not acted, and the
/// exchanged tokens once they have.
///
/// # Errors
///
/// Returns [`OAuthError::Network`] for transport failures and
/// [`OAuthError::Rejected`] for an unexpected status.
pub async fn poll_once(
    http: &reqwest::Client,
    issuer: &str,
    client_id: &str,
    device_code: &DeviceCode,
) -> Result<(PollOutcome, Option<TokenSet>), OAuthError> {
    let issuer = issuer.trim_end_matches('/');
    let url = format!("{issuer}/api/accounts/deviceauth/token");

    let response = http
        .post(&url)
        .json(&PollRequest {
            device_auth_id: &device_code.device_auth_id,
            user_code: &device_code.user_code,
        })
        .send()
        .await
        .map_err(|e| OAuthError::Network(e.to_string()))?;

    let status = response.status();

    // The backend uses both of these for "the user has not approved yet".
    if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::NOT_FOUND {
        return Ok((PollOutcome::Pending, None));
    }
    if !status.is_success() {
        return Err(OAuthError::Rejected(format!(
            "device authorization failed with status {status}"
        )));
    }

    let approved: ApprovedResponse = response
        .json()
        .await
        .map_err(|e| OAuthError::Rejected(format!("unreadable approval response: {e}")))?;

    let tokens = exchange_code(http, issuer, client_id, &approved).await?;
    Ok((PollOutcome::Approved, Some(tokens)))
}

/// Exchange the server-issued authorization code for tokens.
async fn exchange_code(
    http: &reqwest::Client,
    issuer: &str,
    client_id: &str,
    approved: &ApprovedResponse,
) -> Result<TokenSet, OAuthError> {
    let token_url = format!("{issuer}/oauth/token");
    let redirect_uri = format!("{issuer}/deviceauth/callback");

    let response = http
        .post(&token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", approved.authorization_code.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("client_id", client_id),
            ("code_verifier", approved.code_verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|e| OAuthError::Network(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(OAuthError::Rejected(format!("HTTP {status}: {body}")));
    }

    response
        .json()
        .await
        .map_err(|e| OAuthError::Rejected(format!("unreadable token response: {e}")))
}

/// Clamp the server's poll interval into something sane.
fn parse_interval(raw: Option<&str>) -> u64 {
    raw.and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|secs| *secs > 0)
        .unwrap_or(DEFAULT_POLL_INTERVAL_SECS)
        .min(MAX_POLL_INTERVAL_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const CLIENT_ID: &str = "app_test";

    #[test]
    fn interval_parses_the_string_the_server_sends() {
        assert_eq!(parse_interval(Some("7")), 7);
    }

    #[test]
    fn interval_falls_back_when_absent_or_unusable() {
        assert_eq!(parse_interval(None), DEFAULT_POLL_INTERVAL_SECS);
        assert_eq!(parse_interval(Some("")), DEFAULT_POLL_INTERVAL_SECS);
        assert_eq!(parse_interval(Some("nope")), DEFAULT_POLL_INTERVAL_SECS);
        assert_eq!(parse_interval(Some("0")), DEFAULT_POLL_INTERVAL_SECS);
    }

    #[test]
    fn interval_is_clamped_so_a_bad_value_cannot_stall_the_ui() {
        assert_eq!(parse_interval(Some("6000")), MAX_POLL_INTERVAL_SECS);
    }

    #[tokio::test]
    async fn requests_a_user_code_and_derives_the_verification_url() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/accounts/deviceauth/usercode"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_auth_id": "dev-1",
                "user_code": "BXFD-KM2Q",
                "interval": "5"
            })))
            .mount(&server)
            .await;

        let code = request_device_code(&reqwest::Client::new(), &server.uri(), CLIENT_ID)
            .await
            .expect("device code");

        assert_eq!(code.user_code, "BXFD-KM2Q");
        assert_eq!(code.device_auth_id, "dev-1");
        assert_eq!(code.interval_secs, 5);
        assert_eq!(
            code.verification_url,
            format!("{}/codex/device", server.uri())
        );
    }

    #[tokio::test]
    async fn a_404_on_usercode_means_the_issuer_does_not_offer_device_login() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/accounts/deviceauth/usercode"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let error = request_device_code(&reqwest::Client::new(), &server.uri(), CLIENT_ID)
            .await
            .expect_err("404 should be reported as unsupported");

        assert_eq!(error, OAuthError::DeviceCodeUnsupported);
    }

    #[tokio::test]
    async fn a_500_on_usercode_is_rejected_rather_than_retried() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/accounts/deviceauth/usercode"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let error = request_device_code(&reqwest::Client::new(), &server.uri(), CLIENT_ID)
            .await
            .expect_err("500 should surface");

        assert!(matches!(error, OAuthError::Rejected(_)), "got {error:?}");
    }

    fn pending_code() -> DeviceCode {
        DeviceCode {
            verification_url: "https://example.test/codex/device".into(),
            user_code: "BXFD-KM2Q".into(),
            device_auth_id: "dev-1".into(),
            interval_secs: 1,
        }
    }

    #[tokio::test]
    async fn a_403_poll_means_not_yet_approved() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/accounts/deviceauth/token"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let (outcome, tokens) = poll_once(
            &reqwest::Client::new(),
            &server.uri(),
            CLIENT_ID,
            &pending_code(),
        )
        .await
        .expect("pending poll");

        assert_eq!(outcome, PollOutcome::Pending);
        assert!(tokens.is_none());
    }

    #[tokio::test]
    async fn a_404_poll_also_means_not_yet_approved() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/accounts/deviceauth/token"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let (outcome, _) = poll_once(
            &reqwest::Client::new(),
            &server.uri(),
            CLIENT_ID,
            &pending_code(),
        )
        .await
        .expect("pending poll");

        assert_eq!(outcome, PollOutcome::Pending);
    }

    #[tokio::test]
    async fn an_approved_poll_exchanges_the_code_for_tokens() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/accounts/deviceauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "authorization_code": "auth-code-1",
                "code_challenge": "challenge",
                "code_verifier": "verifier-1"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "access-1",
                "refresh_token": "refresh-1",
                "expires_in": 3600
            })))
            .mount(&server)
            .await;

        let (outcome, tokens) = poll_once(
            &reqwest::Client::new(),
            &server.uri(),
            CLIENT_ID,
            &pending_code(),
        )
        .await
        .expect("approved poll");

        assert_eq!(outcome, PollOutcome::Approved);
        let tokens = tokens.expect("tokens returned on approval");
        assert_eq!(tokens.access_token, "access-1");
        assert_eq!(tokens.refresh_token.as_deref(), Some("refresh-1"));
        assert_eq!(tokens.expires_in, Some(3600));
    }

    #[tokio::test]
    async fn a_rejected_exchange_surfaces_the_status() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/accounts/deviceauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "authorization_code": "auth-code-1",
                "code_challenge": "challenge",
                "code_verifier": "verifier-1"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(400).set_body_string("invalid_grant"))
            .mount(&server)
            .await;

        let error = poll_once(
            &reqwest::Client::new(),
            &server.uri(),
            CLIENT_ID,
            &pending_code(),
        )
        .await
        .expect_err("exchange failure should surface");

        match error {
            OAuthError::Rejected(message) => {
                assert!(message.contains("400"), "message was {message}");
                assert!(message.contains("invalid_grant"), "message was {message}");
            }
            other => panic!("expected Rejected, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_unexpected_poll_status_aborts_rather_than_looping() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/accounts/deviceauth/token"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let error = poll_once(
            &reqwest::Client::new(),
            &server.uri(),
            CLIENT_ID,
            &pending_code(),
        )
        .await
        .expect_err("500 should abort the poll loop");

        assert!(matches!(error, OAuthError::Rejected(_)), "got {error:?}");
    }
}
