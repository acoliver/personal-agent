//! Identity claims carried in an `OpenID` `id_token`.
//!
//! The Codex backend wants a `chatgpt-account-id` header for `ChatGPT`-plan
//! accounts, and the UI wants something friendlier than a UUID to show. Both
//! come out of the `id_token` payload.
//!
//! The token arrives from the network, so every field is optional and a
//! malformed token yields an empty identity rather than an error: a missing
//! header is a degraded request, not a failed sign-in.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

/// The `OpenAI`-specific claim namespace inside the `id_token` payload.
const OPENAI_AUTH_CLAIM: &str = "https://api.openai.com/auth";

/// Who a stored grant belongs to, as far as the identity provider says.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountIdentity {
    /// `ChatGPT` account id, sent back as the `chatgpt-account-id` header.
    pub account_id: Option<String>,
    /// Email address, used as the display name when present.
    pub email: Option<String>,
    /// Subscription plan (`pro`, `team`, …), shown next to the account.
    pub plan: Option<String>,
}

impl AccountIdentity {
    /// Parse the identity claims out of a JWT `id_token`.
    ///
    /// Returns an empty identity for any token that cannot be decoded.
    #[must_use]
    pub fn from_id_token(id_token: Option<&str>) -> Self {
        let Some(claims) = decode_payload(id_token) else {
            return Self::default();
        };
        let openai = &claims[OPENAI_AUTH_CLAIM];
        Self {
            account_id: string_claim(&openai["chatgpt_account_id"]),
            email: string_claim(&claims["email"]),
            plan: string_claim(&openai["chatgpt_plan_type"]),
        }
    }

    /// The label to show for this account, falling back through email, then a
    /// shortened account id, then a fixed string.
    #[must_use]
    pub fn display_label(&self) -> String {
        if let Some(email) = self.email.as_deref().filter(|s| !s.is_empty()) {
            return email.to_string();
        }
        if let Some(id) = self.account_id.as_deref().filter(|s| !s.is_empty()) {
            // The claim comes off the network and may hold any UTF-8. Slicing
            // by byte offset would panic inside a multi-byte character, and
            // this runs while building the account list.
            let chars: Vec<char> = id.chars().collect();
            let tail: String = chars[chars.len().saturating_sub(4)..].iter().collect();
            return format!("account ending {tail}");
        }
        "ChatGPT account".to_string()
    }

    /// The account slug used as the keychain key and the profile's auth id.
    ///
    /// Derived from the account id when the provider gave us one so that
    /// signing in twice with the same account reuses one record.
    #[must_use]
    pub fn account_slug(&self) -> String {
        self.account_id.as_deref().map_or_else(
            || format!("chatgpt-{}", uuid::Uuid::new_v4()),
            |id| format!("chatgpt-{id}"),
        )
    }
}

/// Decode the payload segment of a JWT. Returns `None` for anything that is
/// not a three-segment token with a base64url JSON payload.
fn decode_payload(id_token: Option<&str>) -> Option<serde_json::Value> {
    let token = id_token?;
    let mut segments = token.split('.');
    let _header = segments.next()?;
    let payload_b64 = segments.next()?;
    // A JWT has exactly three segments; anything else is not one.
    segments.next()?;
    if segments.next().is_some() {
        return None;
    }
    let payload = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
    serde_json::from_slice(&payload).ok()
}

/// Read a claim as a non-empty string.
fn string_claim(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_token(payload: &serde_json::Value) -> String {
        let encoded = URL_SAFE_NO_PAD.encode(serde_json::to_vec(payload).unwrap());
        format!("header.{encoded}.signature")
    }

    #[test]
    fn extracts_account_id_email_and_plan() {
        let token = make_token(&serde_json::json!({
            "email": "andrew@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acct-4f21",
                "chatgpt_plan_type": "pro"
            }
        }));

        let identity = AccountIdentity::from_id_token(Some(&token));

        assert_eq!(identity.account_id.as_deref(), Some("acct-4f21"));
        assert_eq!(identity.email.as_deref(), Some("andrew@example.com"));
        assert_eq!(identity.plan.as_deref(), Some("pro"));
    }

    #[test]
    fn missing_claims_yield_empty_identity() {
        let token = make_token(&serde_json::json!({ "sub": "user-1" }));

        let identity = AccountIdentity::from_id_token(Some(&token));

        assert_eq!(identity, AccountIdentity::default());
    }

    #[test]
    fn absent_token_yields_empty_identity() {
        assert_eq!(
            AccountIdentity::from_id_token(None),
            AccountIdentity::default()
        );
    }

    #[test]
    fn malformed_base64_yields_empty_identity() {
        let identity = AccountIdentity::from_id_token(Some("header.!!!not-base64!!!.signature"));

        assert_eq!(identity, AccountIdentity::default());
    }

    #[test]
    fn two_segment_token_yields_empty_identity() {
        let encoded = URL_SAFE_NO_PAD.encode(br#"{"email":"a@b.c"}"#);
        let identity = AccountIdentity::from_id_token(Some(&format!("header.{encoded}")));

        assert_eq!(identity, AccountIdentity::default());
    }

    #[test]
    fn four_segment_token_yields_empty_identity() {
        let encoded = URL_SAFE_NO_PAD.encode(br#"{"email":"a@b.c"}"#);
        let identity = AccountIdentity::from_id_token(Some(&format!("header.{encoded}.sig.extra")));

        assert_eq!(identity, AccountIdentity::default());
    }

    #[test]
    fn blank_claims_are_treated_as_absent() {
        let token = make_token(&serde_json::json!({
            "email": "   ",
            "https://api.openai.com/auth": { "chatgpt_account_id": "" }
        }));

        let identity = AccountIdentity::from_id_token(Some(&token));

        assert_eq!(identity, AccountIdentity::default());
    }

    #[test]
    fn display_label_prefers_email() {
        let identity = AccountIdentity {
            account_id: Some("acct-4f21".into()),
            email: Some("andrew@example.com".into()),
            plan: None,
        };

        assert_eq!(identity.display_label(), "andrew@example.com");
    }

    #[test]
    fn display_label_falls_back_to_account_tail() {
        let identity = AccountIdentity {
            account_id: Some("acct-4f21".into()),
            email: None,
            plan: None,
        };

        assert_eq!(identity.display_label(), "account ending 4f21");
    }

    #[test]
    fn display_label_does_not_panic_on_a_multi_byte_account_id() {
        // The claim comes off the network, so it can hold anything. Slicing
        // the last four bytes would land mid-character and panic.
        let identity = AccountIdentity {
            account_id: Some("acct-日本語テスト".into()),
            email: None,
            plan: None,
        };

        assert_eq!(identity.display_label(), "account ending 語テスト");
    }

    #[test]
    fn display_label_handles_an_account_id_shorter_than_the_tail() {
        let identity = AccountIdentity {
            account_id: Some("ab".into()),
            email: None,
            plan: None,
        };

        assert_eq!(identity.display_label(), "account ending ab");
    }

    #[test]
    fn display_label_falls_back_to_a_fixed_string() {
        assert_eq!(
            AccountIdentity::default().display_label(),
            "ChatGPT account"
        );
    }

    #[test]
    fn slug_is_stable_for_a_known_account() {
        let identity = AccountIdentity {
            account_id: Some("acct-4f21".into()),
            ..AccountIdentity::default()
        };

        assert_eq!(identity.account_slug(), "chatgpt-acct-4f21");
        assert_eq!(identity.account_slug(), identity.account_slug());
    }

    #[test]
    fn slug_is_unique_when_the_account_is_unknown() {
        let identity = AccountIdentity::default();

        let first = identity.account_slug();
        let second = identity.account_slug();

        assert!(first.starts_with("chatgpt-"));
        assert_ne!(first, second);
    }
}
