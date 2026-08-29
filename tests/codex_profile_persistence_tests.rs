//! A `ChatGPT` profile has to survive being saved.
//!
//! The editor holds an account slug, the save event carries it, the presenter
//! turns it into an [`AuthConfig`], and the service writes JSON to disk. If any
//! link drops the slug the profile still looks fine on screen and then cannot
//! authenticate, because `resolve_bearer` has no account to look up.
//!
//! These tests walk that chain with the real service against a temp directory.

use personal_agent::events::types::ModelProfileAuth;
use personal_agent::models::profile::{AuthConfig, ModelParameters};
use personal_agent::services::profile::ProfileService;
use personal_agent::services::profile_impl::ProfileServiceImpl;
use tempfile::TempDir;

const ACCOUNT: &str = "chatgpt-acct-4f21";

fn service() -> (ProfileServiceImpl, TempDir) {
    let dir = TempDir::new().expect("temp dir");
    let service = ProfileServiceImpl::new(dir.path().to_path_buf()).expect("profile service");
    (service, dir)
}

/// Read the directory back through a fresh service, so every assertion about
/// what was saved goes through disk rather than an in-memory copy.
///
/// `new` deliberately does not touch the filesystem; `initialize` is what
/// loads.
async fn reload(dir: &TempDir) -> ProfileServiceImpl {
    let service = ProfileServiceImpl::new(dir.path().to_path_buf()).expect("second service");
    service.initialize().await.expect("initialize");
    service
}

/// The mapping the presenter applies between the save event and the profile.
///
/// Mirrors `ProfileEditorPresenter::on_save_profile` so the translation is
/// covered without standing up the whole presenter.
fn auth_from_event(auth: Option<ModelProfileAuth>) -> AuthConfig {
    match auth {
        Some(ModelProfileAuth::Keychain { label }) => AuthConfig::Keychain { label },
        Some(ModelProfileAuth::OAuth { account }) => AuthConfig::OAuth { account },
        Some(ModelProfileAuth::None) => AuthConfig::None,
        None => AuthConfig::Keychain {
            label: String::new(),
        },
    }
}

#[tokio::test]
async fn a_chatgpt_profile_keeps_its_account_across_a_save_and_reload() {
    let (service, dir) = service();
    let auth = auth_from_event(Some(ModelProfileAuth::OAuth {
        account: ACCOUNT.to_string(),
    }));

    let created = service
        .create(
            "Codex".to_string(),
            "openai-codex".to_string(),
            "gpt-5.6-luna".to_string(),
            Some("wss://chatgpt.com/backend-api/codex/responses".to_string()),
            auth,
            ModelParameters::default(),
            None,
        )
        .await
        .expect("create");

    assert_eq!(created.auth.oauth_account(), Some(ACCOUNT));

    // Re-read through a second service so the assertion goes through disk
    // rather than any in-memory copy.
    let reloaded = reload(&dir).await.get(created.id).await.expect("reload");

    assert_eq!(
        reloaded.auth,
        AuthConfig::OAuth {
            account: ACCOUNT.to_string()
        },
        "an OAuth profile must come back as one"
    );
    assert!(reloaded.auth.requires_oauth_account());
    assert!(!reloaded.auth.requires_api_key());
}

#[tokio::test]
async fn the_account_reaches_disk_in_the_documented_shape() {
    let (service, dir) = service();

    let created = service
        .create(
            "Codex".to_string(),
            "openai-codex".to_string(),
            "gpt-5.6-luna".to_string(),
            None,
            AuthConfig::OAuth {
                account: ACCOUNT.to_string(),
            },
            ModelParameters::default(),
            None,
        )
        .await
        .expect("create");

    let raw = std::fs::read_to_string(dir.path().join(format!("{}.json", created.id)))
        .expect("profile file");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("valid json");

    assert_eq!(json["auth"]["type"], "oauth");
    assert_eq!(json["auth"]["account"], ACCOUNT);
}

#[tokio::test]
async fn switching_a_profile_to_chatgpt_replaces_its_api_key() {
    let (service, dir) = service();

    let created = service
        .create(
            "Was an API key profile".to_string(),
            "openai".to_string(),
            "gpt-4o".to_string(),
            None,
            AuthConfig::Keychain {
                label: "some-key".to_string(),
            },
            ModelParameters::default(),
            None,
        )
        .await
        .expect("create");

    service
        .update(
            created.id,
            None,
            Some("openai-codex".to_string()),
            None,
            None,
            Some(AuthConfig::OAuth {
                account: ACCOUNT.to_string(),
            }),
            None,
            None,
        )
        .await
        .expect("update");

    let reloaded = reload(&dir).await.get(created.id).await.expect("reload");

    assert_eq!(reloaded.auth.oauth_account(), Some(ACCOUNT));
    assert!(
        !reloaded.auth.requires_api_key(),
        "the old key label must not survive the switch"
    );
}

#[tokio::test]
async fn a_keychain_profile_is_unaffected() {
    let (service, dir) = service();

    let created = service
        .create(
            "Anthropic".to_string(),
            "anthropic".to_string(),
            "claude".to_string(),
            None,
            AuthConfig::Keychain {
                label: "anthropic-key".to_string(),
            },
            ModelParameters::default(),
            None,
        )
        .await
        .expect("create");

    let reloaded = reload(&dir).await.get(created.id).await.expect("reload");

    assert_eq!(
        reloaded.auth,
        AuthConfig::Keychain {
            label: "anthropic-key".to_string()
        }
    );
    assert_eq!(reloaded.auth.oauth_account(), None);
}

/// The write path against the real OS keychain.
///
/// Every unit test for the store runs on the in-memory mock, so the keyring
/// crate, the account index file, and the round trip through a real credential
/// store are otherwise never exercised. A sign-in performs exactly this write.
///
/// Ignored by default: it touches the developer's login keychain and may
/// prompt. Run with
/// `cargo test --test codex_profile_persistence_tests -- --ignored`.
#[tokio::test]
#[ignore = "Writes to the real OS keychain"]
async fn a_grant_round_trips_through_the_real_keychain() {
    use personal_agent::services::oauth::store::{self, StoredOAuthToken};
    use personal_agent::services::oauth::{TokenSet, CHATGPT_ISSUER};

    let account = "chatgpt-real-keychain-probe";
    // Leave nothing behind from an earlier interrupted run.
    let _ = store::delete(account);

    let record = StoredOAuthToken::from_token_set(
        TokenSet {
            access_token: "probe-access".into(),
            refresh_token: Some("probe-refresh".into()),
            expires_in: Some(3600),
            ..TokenSet::default()
        },
        CHATGPT_ISSUER,
    );

    store::save(account, &record).expect("save to the real keychain");

    let loaded = store::load(account)
        .expect("read back")
        .expect("the record we just wrote");
    assert_eq!(loaded.access_token, "probe-access");
    assert_eq!(loaded.refresh_token.as_deref(), Some("probe-refresh"));

    assert!(
        store::list_accounts().iter().any(|a| a == account),
        "the account index must list what was written"
    );

    store::delete(account).expect("delete");
    assert_eq!(store::load(account).expect("read after delete"), None);
    assert!(
        !store::list_accounts().iter().any(|a| a == account),
        "the account index must drop what was deleted"
    );
}
