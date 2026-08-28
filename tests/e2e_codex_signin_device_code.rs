//! The one check that cannot be automated: a person approving a real device
//! code.
//!
//! Everything else about the device-code flow is covered without a human. The
//! protocol is exercised against wiremock in
//! `services::oauth::device_code::tests`, the presenter lifecycle against a
//! faked flow in `codex_auth_presenter_tests`, and the sheet's rendering in
//! `codex_signin_view::tests`. What none of those can prove is that the codes
//! this app requests are ones `auth.openai.com` will actually accept.
//!
//! So this asks for a real code, prints it, and waits. It is `#[ignore]`d,
//! stays out of CI, and is the last step of the work rather than a gate on it.
//!
//! ## Run
//!
//! ```text
//! cargo test --test e2e_codex_signin_device_code -- --ignored --nocapture
//! ```
//!
//! Open the printed URL, enter the printed code, approve. The test then
//! asserts the grant that comes back is one the app can actually use: a
//! refresh token so the session renews itself, an expiry so it knows when to,
//! and an account id for the `chatgpt-account-id` header.
//!
//! Nothing is written to your keychain unless `PA_E2E_CODEX_PERSIST=1`, so a
//! run cannot quietly replace the account you already use.

use std::io::Write as _;
use std::time::{Duration, Instant};

use personal_agent::services::oauth::device_code::{self, PollOutcome, DEVICE_CODE_TTL_SECS};
use personal_agent::services::oauth::{flow, store, TokenSet, CHATGPT_ISSUER};

/// Set to `1` to keep the resulting grant in the keychain.
const PERSIST_ENV: &str = "PA_E2E_CODEX_PERSIST";

/// The client id this app signs in with, taken from the same config the real
/// flow uses so a drift there fails here.
fn client_id() -> String {
    serdes_ai_providers::chatgpt_oauth_config().client_id
}

fn announce(verification_url: &str, user_code: &str) {
    println!();
    println!("  ┌─────────────────────────────────────────────────────────┐");
    println!("  │  Sign in to approve this test                           │");
    println!("  └─────────────────────────────────────────────────────────┘");
    println!();
    println!("  1. Open:  {verification_url}");
    println!("  2. Enter: {user_code}");
    println!();
    println!("  Continue only if you started this run.");
    println!();
    let _ = std::io::stdout().flush();
}

/// Poll until the user approves, reporting progress so a long wait does not
/// look like a hang.
async fn wait_for_approval(
    http: &reqwest::Client,
    issuer: &str,
    client_id: &str,
    code: &device_code::DeviceCode,
) -> TokenSet {
    let deadline =
        Instant::now() + Duration::from_secs(u64::try_from(DEVICE_CODE_TTL_SECS).unwrap_or(900));
    let interval = Duration::from_secs(code.interval_secs);
    let mut waited = 0u64;

    loop {
        let (outcome, tokens) = device_code::poll_once(http, issuer, client_id, code)
            .await
            .expect("polling the device-auth endpoint");

        match outcome {
            PollOutcome::Approved => {
                println!("  approved after {waited}s");
                return tokens.expect("approval carries tokens");
            }
            PollOutcome::Pending => {
                assert!(
                    Instant::now() < deadline,
                    "nobody approved the code within {DEVICE_CODE_TTL_SECS}s"
                );
                if waited > 0 && waited.is_multiple_of(30) {
                    println!("  still waiting ({waited}s)");
                    let _ = std::io::stdout().flush();
                }
                tokio::time::sleep(interval).await;
                waited += code.interval_secs;
            }
        }
    }
}

#[tokio::test]
#[ignore = "Requires a human to approve a real device code at auth.openai.com"]
async fn a_real_device_code_yields_a_usable_grant() {
    let http = reqwest::Client::new();
    let client_id = client_id();

    let code = device_code::request_device_code(&http, CHATGPT_ISSUER, &client_id)
        .await
        .expect("request a device code");

    assert!(!code.user_code.trim().is_empty(), "no user code was issued");
    assert!(
        code.verification_url.starts_with("https://"),
        "verification URL was {}",
        code.verification_url
    );
    assert!(
        code.interval_secs > 0,
        "a zero poll interval would spin the loop"
    );

    announce(&code.verification_url, &code.user_code);

    let tokens = wait_for_approval(&http, CHATGPT_ISSUER, &client_id, &code).await;

    // The grant has to be usable, not merely present.
    assert!(
        !tokens.access_token.trim().is_empty(),
        "no access token came back"
    );
    assert!(
        tokens
            .refresh_token
            .as_deref()
            .is_some_and(|t| !t.is_empty()),
        "no refresh token: the session could not renew itself"
    );
    assert!(
        tokens.expires_in.is_some_and(|secs| secs > 0),
        "no expiry: the app would not know when to refresh"
    );

    let record = store::StoredOAuthToken::from_token_set(tokens.clone(), CHATGPT_ISSUER);
    assert!(
        record.identity.account_id.is_some(),
        "no chatgpt_account_id claim; the codex backend wants that header"
    );
    assert!(
        !record.needs_refresh(),
        "a grant issued seconds ago should not already be due for refresh"
    );

    println!();
    println!("  account: {}", record.identity.display_label());
    println!("  slug:    {}", record.identity.account_slug());
    println!("  plan:    {:?}", record.identity.plan);
    println!(
        "  expires: {}s from now",
        record.seconds_remaining().unwrap_or_default()
    );

    if std::env::var(PERSIST_ENV).as_deref() == Ok("1") {
        let outcome = flow::persist(tokens, CHATGPT_ISSUER).expect("persist the grant");
        println!("  stored under {}", outcome.account);
    } else {
        println!("  not stored (set {PERSIST_ENV}=1 to keep it)");
    }
    println!();
}
