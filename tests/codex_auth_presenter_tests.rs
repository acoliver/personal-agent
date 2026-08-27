//! `CodexAuthPresenter` behaviour, driven through a faked sign-in.
//!
//! The flow sits behind a trait so none of this touches a browser, a network,
//! or a bound port.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures::FutureExt as _;
use personal_agent::events::types::{CodexSignInMethod, SystemEvent, UserEvent};
use personal_agent::events::{AppEvent, EventBus};
use personal_agent::models::profile::{AuthConfig, ModelProfile};
use personal_agent::presentation::view_command::{CodexSignInFailure, ViewCommand};
use personal_agent::presentation::CodexAuthPresenter;
use personal_agent::services::oauth::flow::{SignInMethod, SignInStart, StartedSignIn};
use personal_agent::services::oauth::{
    now_secs, store, CodexSignIn, OAuthError, TokenSet, CHATGPT_ISSUER,
};
use personal_agent::services::{secure_store, ProfileService};
use tokio::sync::broadcast;
use uuid::Uuid;

mod support;
use support::stub_profile_service::StubProfileService;

/// One scripted `begin` outcome: either it refuses to start, or it starts and
/// later resolves the way the test says.
type ScriptedBegin = Result<(SignInStart, Result<TokenSet, OAuthError>), OAuthError>;

/// A sign-in whose outcome the test decides.
struct FakeSignIn {
    /// What `begin` should do, in order.
    scripted: Mutex<Vec<ScriptedBegin>>,
    /// Methods `begin` was asked for.
    asked: Arc<Mutex<Vec<SignInMethod>>>,
}

impl FakeSignIn {
    fn new(scripted: Vec<ScriptedBegin>) -> (Arc<Self>, Arc<Mutex<Vec<SignInMethod>>>) {
        let asked = Arc::new(Mutex::new(Vec::new()));
        (
            Arc::new(Self {
                scripted: Mutex::new(scripted.into_iter().rev().collect()),
                asked: Arc::clone(&asked),
            }),
            asked,
        )
    }
}

#[async_trait]
impl CodexSignIn for FakeSignIn {
    async fn begin(&self, method: SignInMethod) -> Result<StartedSignIn, OAuthError> {
        self.asked.lock().expect("asked").push(method);
        let next = self
            .scripted
            .lock()
            .expect("script")
            .pop()
            .expect("script exhausted");
        let (start, outcome) = next?;
        Ok(StartedSignIn {
            start,
            complete: async move { outcome }.boxed(),
        })
    }
}

fn browser_start() -> SignInStart {
    SignInStart {
        method: SignInMethod::Browser,
        url: "https://auth.openai.com/oauth/authorize?x=1".to_string(),
        user_code: None,
        copy_to_clipboard: None,
        expires_at: now_secs() + 120,
        fell_back: false,
        listening_port: Some(1455),
    }
}

fn device_start(fell_back: bool) -> SignInStart {
    SignInStart {
        method: SignInMethod::DeviceCode,
        url: "https://auth.openai.com/codex/device".to_string(),
        user_code: Some("BXFD-KM2Q".to_string()),
        copy_to_clipboard: Some("BXFD-KM2Q".to_string()),
        expires_at: now_secs() + 900,
        fell_back,
        listening_port: None,
    }
}

/// An `id_token` carrying a known account, so the slug is predictable.
fn id_token(account_id: &str) -> String {
    use base64::Engine as _;
    let payload = serde_json::json!({
        "email": "andrew@example.com",
        "https://api.openai.com/auth": {
            "chatgpt_account_id": account_id,
            "chatgpt_plan_type": "pro"
        }
    });
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&payload).expect("payload"));
    format!("header.{encoded}.signature")
}

fn tokens(account_id: &str) -> TokenSet {
    TokenSet {
        access_token: "access-1".to_string(),
        refresh_token: Some("refresh-1".to_string()),
        id_token: Some(id_token(account_id)),
        token_type: Some("Bearer".to_string()),
        expires_in: Some(3600),
        scope: None,
    }
}

struct Harness {
    bus: Arc<EventBus>,
    rx: broadcast::Receiver<ViewCommand>,
    _presenter: CodexAuthPresenter,
}

async fn harness(sign_in: Arc<dyn CodexSignIn>, profiles: Vec<ModelProfile>) -> Harness {
    secure_store::use_mock_backend();
    let bus = Arc::new(EventBus::new(256));
    let (view_tx, rx) = broadcast::channel(64);
    let profile_service: Arc<dyn ProfileService> = Arc::new(StubProfileService::new(profiles));
    let mut presenter = CodexAuthPresenter::with_sign_in(profile_service, sign_in, &bus, view_tx);
    presenter.start().await.expect("presenter start");
    Harness {
        bus,
        rx,
        _presenter: presenter,
    }
}

/// Wait for the first command matching a predicate, or give up.
async fn expect_command(
    rx: &mut broadcast::Receiver<ViewCommand>,
    matches: impl Fn(&ViewCommand) -> bool,
) -> ViewCommand {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        assert!(!remaining.is_zero(), "timed out waiting for a view command");
        let received = tokio::time::timeout(remaining, rx.recv())
            .await
            .expect("timed out waiting for a view command");
        match received {
            Ok(command) if matches(&command) => return command,
            // Anything else on the channel, and a lagged receiver, are both
            // just "keep looking".
            Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => panic!("view channel closed"),
        }
    }
}

fn codex_profile(name: &str, account: &str) -> ModelProfile {
    let mut profile = ModelProfile::new(
        name.to_string(),
        "openai-codex".to_string(),
        "gpt-5.6-luna".to_string(),
        "wss://chatgpt.com/backend-api/codex/responses".to_string(),
        AuthConfig::OAuth {
            account: account.to_string(),
        },
    );
    profile.id = Uuid::new_v4();
    profile
}

#[tokio::test]
async fn a_browser_sign_in_reports_a_url_then_stores_the_grant() {
    let (sign_in, asked) = FakeSignIn::new(vec![Ok((browser_start(), Ok(tokens("acct-1"))))]);
    let mut h = harness(sign_in, Vec::new()).await;

    h.bus
        .publish(AppEvent::User(UserEvent::StartCodexSignIn {
            method: CodexSignInMethod::Browser,
        }))
        .expect("publish");

    let started = expect_command(&mut h.rx, |c| {
        matches!(c, ViewCommand::CodexSignInStarted { .. })
    })
    .await;
    match started {
        ViewCommand::CodexSignInStarted {
            method,
            url,
            copy_to_clipboard,
            fell_back,
            ..
        } => {
            assert_eq!(method, CodexSignInMethod::Browser);
            assert!(url.starts_with("https://auth.openai.com/oauth/authorize"));
            assert_eq!(copy_to_clipboard, None);
            assert!(!fell_back);
        }
        other => panic!("expected CodexSignInStarted, got {other:?}"),
    }

    let completed = expect_command(&mut h.rx, |c| {
        matches!(c, ViewCommand::CodexSignInCompleted { .. })
    })
    .await;
    match completed {
        ViewCommand::CodexSignInCompleted {
            account,
            label,
            plan,
        } => {
            assert_eq!(account, "chatgpt-acct-1");
            assert_eq!(label, "andrew@example.com");
            assert_eq!(plan.as_deref(), Some("pro"));
        }
        other => panic!("expected CodexSignInCompleted, got {other:?}"),
    }

    let stored = store::load("chatgpt-acct-1")
        .expect("load")
        .expect("grant stored");
    assert_eq!(stored.access_token, "access-1");
    assert_eq!(stored.refresh_token.as_deref(), Some("refresh-1"));
    assert_eq!(stored.issuer, CHATGPT_ISSUER);
    assert_eq!(
        asked.lock().expect("asked").as_slice(),
        [SignInMethod::Browser]
    );
}

#[tokio::test]
async fn a_device_code_sign_in_carries_the_code_to_the_clipboard() {
    let (sign_in, _asked) = FakeSignIn::new(vec![Ok((device_start(false), Ok(tokens("acct-2"))))]);
    let mut h = harness(sign_in, Vec::new()).await;

    h.bus
        .publish(AppEvent::User(UserEvent::StartCodexSignIn {
            method: CodexSignInMethod::DeviceCode,
        }))
        .expect("publish");

    let started = expect_command(&mut h.rx, |c| {
        matches!(c, ViewCommand::CodexSignInStarted { .. })
    })
    .await;
    match started {
        ViewCommand::CodexSignInStarted {
            method,
            user_code,
            copy_to_clipboard,
            ..
        } => {
            assert_eq!(method, CodexSignInMethod::DeviceCode);
            assert_eq!(user_code.as_deref(), Some("BXFD-KM2Q"));
            assert_eq!(
                copy_to_clipboard.as_deref(),
                Some("BXFD-KM2Q"),
                "the code is handed straight to the clipboard"
            );
        }
        other => panic!("expected CodexSignInStarted, got {other:?}"),
    }
}

#[tokio::test]
async fn a_browser_flow_that_falls_through_reports_a_device_code_not_a_failure() {
    // The flow itself resolves the busy port; the presenter sees only the
    // device-code start it produced, flagged as a fall-through.
    let (sign_in, _asked) = FakeSignIn::new(vec![Ok((device_start(true), Ok(tokens("acct-3"))))]);
    let mut h = harness(sign_in, Vec::new()).await;

    h.bus
        .publish(AppEvent::User(UserEvent::StartCodexSignIn {
            method: CodexSignInMethod::Browser,
        }))
        .expect("publish");

    let started = expect_command(&mut h.rx, |c| {
        matches!(c, ViewCommand::CodexSignInStarted { .. })
    })
    .await;
    match started {
        ViewCommand::CodexSignInStarted {
            method, fell_back, ..
        } => {
            assert_eq!(method, CodexSignInMethod::DeviceCode);
            assert!(fell_back, "the sheet needs to explain the switch");
        }
        other => panic!("expected CodexSignInStarted, got {other:?}"),
    }

    let completed = expect_command(&mut h.rx, |c| {
        matches!(
            c,
            ViewCommand::CodexSignInCompleted { .. } | ViewCommand::CodexSignInFailed { .. }
        )
    })
    .await;
    assert!(
        matches!(completed, ViewCommand::CodexSignInCompleted { .. }),
        "a busy port is not a failure: {completed:?}"
    );
}

#[tokio::test]
async fn a_timeout_is_reported_as_a_timeout() {
    let (sign_in, _asked) = FakeSignIn::new(vec![Ok((browser_start(), Err(OAuthError::TimedOut)))]);
    let mut h = harness(sign_in, Vec::new()).await;

    h.bus
        .publish(AppEvent::User(UserEvent::StartCodexSignIn {
            method: CodexSignInMethod::Browser,
        }))
        .expect("publish");

    let failed = expect_command(&mut h.rx, |c| {
        matches!(c, ViewCommand::CodexSignInFailed { .. })
    })
    .await;
    match failed {
        ViewCommand::CodexSignInFailed { reason, .. } => {
            assert_eq!(reason, CodexSignInFailure::TimedOut);
        }
        other => panic!("expected CodexSignInFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn an_issuer_without_device_login_says_so() {
    let (sign_in, _asked) = FakeSignIn::new(vec![Err(OAuthError::DeviceCodeUnsupported)]);
    let mut h = harness(sign_in, Vec::new()).await;

    h.bus
        .publish(AppEvent::User(UserEvent::StartCodexSignIn {
            method: CodexSignInMethod::DeviceCode,
        }))
        .expect("publish");

    let failed = expect_command(&mut h.rx, |c| {
        matches!(c, ViewCommand::CodexSignInFailed { .. })
    })
    .await;
    match failed {
        ViewCommand::CodexSignInFailed { reason, .. } => {
            assert_eq!(reason, CodexSignInFailure::DeviceCodeUnsupported);
        }
        other => panic!("expected CodexSignInFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn an_expired_device_code_says_so() {
    let (sign_in, _asked) = FakeSignIn::new(vec![Ok((
        device_start(false),
        Err(OAuthError::DeviceCodeExpired),
    ))]);
    let mut h = harness(sign_in, Vec::new()).await;

    h.bus
        .publish(AppEvent::User(UserEvent::StartCodexSignIn {
            method: CodexSignInMethod::DeviceCode,
        }))
        .expect("publish");

    let failed = expect_command(&mut h.rx, |c| {
        matches!(c, ViewCommand::CodexSignInFailed { .. })
    })
    .await;
    match failed {
        ViewCommand::CodexSignInFailed { reason, .. } => {
            assert_eq!(reason, CodexSignInFailure::DeviceCodeExpired);
        }
        other => panic!("expected CodexSignInFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn cancelling_reports_a_cancellation() {
    // The completion never resolves, so only a cancel can end this.
    let (sign_in, _asked) = FakeSignIn::new(vec![Ok((browser_start(), Err(OAuthError::TimedOut)))]);
    let mut h = harness(sign_in, Vec::new()).await;

    h.bus
        .publish(AppEvent::User(UserEvent::StartCodexSignIn {
            method: CodexSignInMethod::Browser,
        }))
        .expect("publish");
    expect_command(&mut h.rx, |c| {
        matches!(c, ViewCommand::CodexSignInStarted { .. })
    })
    .await;

    h.bus
        .publish(AppEvent::User(UserEvent::CancelCodexSignIn))
        .expect("publish");

    let failed = expect_command(&mut h.rx, |c| {
        matches!(
            c,
            ViewCommand::CodexSignInFailed {
                reason: CodexSignInFailure::Cancelled,
                ..
            }
        )
    })
    .await;
    assert!(matches!(failed, ViewCommand::CodexSignInFailed { .. }));
}

#[tokio::test]
async fn the_account_list_names_the_profiles_using_each_account() {
    secure_store::use_mock_backend();
    let account = "chatgpt-listed-1";
    store::save(
        account,
        &store::StoredOAuthToken::from_token_set(tokens("listed-1"), CHATGPT_ISSUER),
    )
    .expect("seed");

    let (sign_in, _asked) = FakeSignIn::new(Vec::new());
    let mut h = harness(
        sign_in,
        vec![
            codex_profile("Codex", account),
            codex_profile("Codex Mini", account),
            codex_profile("Other", "chatgpt-someone-else"),
        ],
    )
    .await;

    h.bus
        .publish(AppEvent::User(UserEvent::ListCodexAccounts))
        .expect("publish");

    let listed = expect_command(&mut h.rx, |c| match c {
        ViewCommand::CodexAccountsListed { accounts } => {
            accounts.iter().any(|a| a.account == account)
        }
        _ => false,
    })
    .await;
    match listed {
        ViewCommand::CodexAccountsListed { accounts } => {
            let entry = accounts
                .iter()
                .find(|a| a.account == account)
                .expect("account listed");
            assert_eq!(entry.label, "andrew@example.com");
            assert_eq!(entry.used_by.len(), 2, "used_by was {:?}", entry.used_by);
            assert!(!entry.needs_reauth);
        }
        other => panic!("expected CodexAccountsListed, got {other:?}"),
    }

    store::delete(account).expect("cleanup");
}

#[tokio::test]
async fn signing_out_forgets_the_account_and_refreshes_the_list() {
    secure_store::use_mock_backend();
    let account = "chatgpt-signout-1";
    store::save(
        account,
        &store::StoredOAuthToken::from_token_set(tokens("signout-1"), CHATGPT_ISSUER),
    )
    .expect("seed");

    let (sign_in, _asked) = FakeSignIn::new(Vec::new());
    let mut h = harness(sign_in, Vec::new()).await;

    h.bus
        .publish(AppEvent::User(UserEvent::SignOutCodexAccount {
            account: account.to_string(),
        }))
        .expect("publish");

    expect_command(&mut h.rx, |c| match c {
        ViewCommand::CodexAccountsListed { accounts } => {
            !accounts.iter().any(|a| a.account == account)
        }
        _ => false,
    })
    .await;

    assert_eq!(store::load(account).expect("load"), None);
}

#[tokio::test]
async fn a_revoked_grant_reaches_the_views_that_can_offer_a_way_back() {
    let (sign_in, _asked) = FakeSignIn::new(Vec::new());
    let mut h = harness(sign_in, Vec::new()).await;

    h.bus
        .publish(AppEvent::System(SystemEvent::OAuthReauthRequired {
            account: "chatgpt-revoked".to_string(),
        }))
        .expect("publish");

    let command = expect_command(&mut h.rx, |c| {
        matches!(c, ViewCommand::CodexReauthRequired { .. })
    })
    .await;
    match command {
        ViewCommand::CodexReauthRequired { account } => {
            assert_eq!(account, "chatgpt-revoked");
        }
        other => panic!("expected CodexReauthRequired, got {other:?}"),
    }
}
