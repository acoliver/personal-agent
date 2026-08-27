//! The `OpenAI` Responses transport (codex and Open Responses endpoints).
//!
//! Two things make this path different from every other provider here.
//!
//! It is **not** wrapped in [`NormalizingSseModel`]. That wrapper repairs Chat
//! Completions SSE; `OpenResponsesModel` already emits well-formed stream
//! events ending in a single terminal `StreamComplete`, and running it through
//! the normalizer would corrupt them.
//!
//! The model instance is **cached per conversation**. The websocket session
//! holds `previous_response_id`, which is what lets a turn send only the new
//! input items instead of replaying the whole history. Building a fresh model
//! per turn, which is what every other provider here does, would open a new
//! socket each time and throw that state away.
//!
//! [`NormalizingSseModel`]: crate::llm::normalizing_model::NormalizingSseModel

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use serdes_ai_responses::client::OpenResponsesModel;
use serdes_ai_responses::types::ReasoningSettings;
use uuid::Uuid;

use crate::llm::error::LlmError;
use crate::llm::provider_quirks::ProviderQuirks;
use crate::models::ModelProfile;
use crate::services::oauth::{refresh, store, OAuthError};

/// The transport sentinel that selects this client in the quirks manifest.
pub const TRANSPORT: &str = "open-responses";

/// Live sessions kept at once. Each holds an open socket, so this is a
/// resource bound rather than a performance tweak.
const MAX_LIVE_SESSIONS: usize = 8;

/// Identity of a session. Anything that changes the wire conversation or the
/// endpoint has to produce a different key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SessionKey {
    conversation_id: Uuid,
    profile_id: Uuid,
    endpoint: String,
    model_id: String,
}

struct Entry {
    model: Arc<OpenResponsesModel>,
    /// Hash of the bearer the model was built with. A refresh changes it, and
    /// the session has to be rebuilt because the token is baked in at
    /// construction.
    token_fingerprint: u64,
    last_used: Instant,
}

type Sessions = Mutex<HashMap<SessionKey, Entry>>;

static SESSIONS: OnceLock<Sessions> = OnceLock::new();

fn sessions() -> &'static Sessions {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn fingerprint(token: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    token.hash(&mut hasher);
    hasher.finish()
}

/// Everything needed to build or find a session.
pub struct SessionRequest<'a> {
    pub profile: &'a ModelProfile,
    pub quirks: &'a ProviderQuirks,
    pub endpoint: &'a str,
    /// Conversation this turn belongs to. `None` builds an uncached model,
    /// which is what connection tests and one-shot calls want.
    pub conversation_id: Option<Uuid>,
    /// Bearer for endpoints that take a static key instead of an account.
    pub api_key: &'a str,
}

/// Resolve a model for this turn, reusing the conversation's session when one
/// is alive and still holds the current token.
///
/// # Errors
///
/// Returns [`LlmError::Auth`] when an OAuth account cannot produce a usable
/// token, and [`LlmError::InvalidConfig`] when the endpoint or headers are
/// unusable.
pub async fn model_for(request: SessionRequest<'_>) -> Result<Arc<dyn serdes_ai::Model>, LlmError> {
    let bearer = resolve_bearer(request.profile, request.api_key).await?;
    let account_id = account_header(request.profile).await;

    let Some(conversation_id) = request.conversation_id else {
        let model = build(&request, &bearer, account_id.as_deref())?;
        return Ok(Arc::new(model));
    };

    let key = SessionKey {
        conversation_id,
        profile_id: request.profile.id,
        endpoint: request.endpoint.to_string(),
        model_id: request.profile.model_id.clone(),
    };
    let wanted = fingerprint(&bearer);

    if let Some(model) = take_live(&key, wanted) {
        return Ok(model);
    }

    let model = Arc::new(build(&request, &bearer, account_id.as_deref())?);
    insert(key, &model, wanted);
    Ok(model)
}

/// Return the cached model for a key when it is still valid for this token.
fn take_live(key: &SessionKey, wanted: u64) -> Option<Arc<dyn serdes_ai::Model>> {
    let mut guard = sessions().lock().ok()?;
    let model = take_live_in(&mut guard, key, wanted);
    drop(guard);
    model
}

/// Look a session up in a map. Split out from the global so the eviction and
/// staleness rules can be tested without a process-wide cache.
fn take_live_in(
    map: &mut HashMap<SessionKey, Entry>,
    key: &SessionKey,
    wanted: u64,
) -> Option<Arc<dyn serdes_ai::Model>> {
    let entry = map.get_mut(key)?;
    if entry.token_fingerprint != wanted {
        // The token was refreshed underneath us; the session's bearer is
        // stale and the socket has to be rebuilt.
        map.remove(key);
        return None;
    }
    entry.last_used = Instant::now();
    Some(Arc::clone(&entry.model) as Arc<dyn serdes_ai::Model>)
}

/// Record a new session, evicting the least recently used one if we are at the
/// socket budget.
fn insert(key: SessionKey, model: &Arc<OpenResponsesModel>, token_fingerprint: u64) {
    let Ok(mut guard) = sessions().lock() else {
        return;
    };
    insert_in(&mut guard, key, model, token_fingerprint);
    drop(guard);
}

fn insert_in(
    map: &mut HashMap<SessionKey, Entry>,
    key: SessionKey,
    model: &Arc<OpenResponsesModel>,
    token_fingerprint: u64,
) {
    if map.len() >= MAX_LIVE_SESSIONS && !map.contains_key(&key) {
        if let Some(oldest) = map
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(key, _)| key.clone())
        {
            map.remove(&oldest);
        }
    }
    map.insert(
        key,
        Entry {
            model: Arc::clone(model),
            token_fingerprint,
            last_used: Instant::now(),
        },
    );
}

/// Drop every session belonging to a conversation. Called when a conversation
/// is closed or deleted so its socket does not outlive it.
pub fn invalidate_conversation(conversation_id: Uuid) {
    let Ok(mut guard) = sessions().lock() else {
        return;
    };
    guard.retain(|key, _| key.conversation_id != conversation_id);
    drop(guard);
}

/// Drop every session using a profile. Called when the profile is edited,
/// because the endpoint, model, or parameters may all have moved.
pub fn invalidate_profile(profile_id: Uuid) {
    let Ok(mut guard) = sessions().lock() else {
        return;
    };
    guard.retain(|key, _| key.profile_id != profile_id);
    drop(guard);
}

/// Drop every session. Called on sign-out.
pub fn invalidate_all() {
    if let Ok(mut guard) = sessions().lock() {
        guard.clear();
    }
}

/// Number of live sessions, for tests and diagnostics.
#[must_use]
pub fn live_session_count() -> usize {
    sessions().lock().map_or(0, |guard| guard.len())
}

/// Resolve the bearer token for a profile.
async fn resolve_bearer(profile: &ModelProfile, api_key: &str) -> Result<String, LlmError> {
    let Some(account) = profile.auth.oauth_account() else {
        // An Open Responses endpoint reached with a static key.
        return Ok(api_key.to_string());
    };
    if account.is_empty() {
        return Err(LlmError::Auth(
            "This profile has no signed-in account. Sign in with ChatGPT to use it.".to_string(),
        ));
    }
    match refresh::access_token_for(account).await {
        Ok(token) => Ok(token),
        Err(error) => {
            if matches!(error, OAuthError::GrantRevoked) {
                let _ = refresh::report_reauth_required_async(account).await;
            }
            Err(LlmError::Auth(error.to_string()))
        }
    }
}

/// The `chatgpt-account-id` header value, when the stored grant knows it.
///
/// Reading the keychain blocks, so it goes to a blocking thread rather than
/// stalling the runtime for the length of a turn setup.
async fn account_header(profile: &ModelProfile) -> Option<String> {
    let account = profile.auth.oauth_account()?;
    store::load_async(account)
        .await
        .ok()
        .flatten()
        .and_then(|record| record.identity.account_id)
}

/// Construct a model. Builder methods panic once the model is shared, so every
/// call here works on a fresh instance.
fn build(
    request: &SessionRequest<'_>,
    bearer: &str,
    account_id: Option<&str>,
) -> Result<OpenResponsesModel, LlmError> {
    if request.endpoint.trim().is_empty() {
        return Err(LlmError::InvalidConfig(
            "Responses profiles need an endpoint URL".to_string(),
        ));
    }
    if request.profile.auth.requires_oauth_account() {
        // A ChatGPT grant is scoped to ChatGPT. Sending it to whatever host a
        // profile happens to name would hand the bearer token, and the account
        // id, to that host.
        ensure_oauth_endpoint_is_trusted(request.endpoint)?;
    }

    let mut model = OpenResponsesModel::new(&request.profile.model_id, request.endpoint);

    if !bearer.is_empty() {
        model = model.bearer(bearer);
    }
    for (name, value) in &request.quirks.headers {
        model = model.header(name.clone(), value.clone());
    }
    if let Some(id) = account_id {
        model = model.header("chatgpt-account-id", id.to_string());
    }
    model = model.header("User-Agent", user_agent());

    if let Some(reasoning) = reasoning_for(request.profile) {
        model = model.with_reasoning(reasoning);
    }

    Ok(model)
}

/// Hosts an OAuth grant may be presented to.
///
/// The grant is issued by `OpenAI` for the `ChatGPT` backend. Anything else is a
/// third party as far as that token is concerned.
const OAUTH_ALLOWED_HOSTS: &[&str] = &["chatgpt.com", "api.openai.com", "auth.openai.com"];

/// Reject an endpoint that an OAuth grant must not be sent to.
///
/// Requires TLS and a host in [`OAUTH_ALLOWED_HOSTS`], matching either the host
/// itself or a subdomain of it.
fn ensure_oauth_endpoint_is_trusted(endpoint: &str) -> Result<(), LlmError> {
    let url = url::Url::parse(endpoint)
        .map_err(|e| LlmError::InvalidConfig(format!("endpoint is not a valid URL: {e}")))?;

    if !matches!(url.scheme(), "https" | "wss") {
        return Err(LlmError::InvalidConfig(format!(
            "a ChatGPT account can only be used over an encrypted connection, not {}",
            url.scheme()
        )));
    }

    let host = url
        .host_str()
        .ok_or_else(|| LlmError::InvalidConfig("endpoint has no host".to_string()))?
        .to_ascii_lowercase();

    let trusted = OAUTH_ALLOWED_HOSTS
        .iter()
        .any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}")));
    if !trusted {
        return Err(LlmError::InvalidConfig(format!(
            "a ChatGPT account cannot be used with {host}; sign-in is only valid for OpenAI's own \
             endpoints. Use an API key for this provider."
        )));
    }

    Ok(())
}

/// Identify this client the way the codex backend expects.
fn user_agent() -> String {
    format!("personal-agent/{}", env!("CARGO_PKG_VERSION"))
}

/// Map profile thinking settings onto Responses reasoning settings.
///
/// The Responses API takes an effort level rather than a token budget, so the
/// configured budget buckets into one.
fn reasoning_for(profile: &ModelProfile) -> Option<ReasoningSettings> {
    if !profile.parameters.enable_thinking {
        return None;
    }
    Some(ReasoningSettings {
        effort: Some(effort_for_budget(profile.parameters.thinking_budget).to_string()),
        summary: Some(serde_json::Value::String("auto".to_string())),
    })
}

/// Bucket a token budget into a Responses effort level.
const fn effort_for_budget(budget: Option<u32>) -> &'static str {
    match budget {
        Some(budget) if budget < 4_096 => "low",
        Some(budget) if budget >= 16_384 => "high",
        _ => "medium",
    }
}

#[cfg(test)]
mod tests {
    use super::ensure_oauth_endpoint_is_trusted;

    #[test]
    fn an_oauth_grant_may_reach_the_chatgpt_backend() {
        for endpoint in [
            "wss://chatgpt.com/backend-api/codex/responses",
            "https://api.openai.com/v1/responses",
            "wss://eu.chatgpt.com/backend-api/codex/responses",
        ] {
            assert!(
                ensure_oauth_endpoint_is_trusted(endpoint).is_ok(),
                "{endpoint} should be allowed"
            );
        }
    }

    #[test]
    fn an_oauth_grant_is_not_sent_to_a_third_party() {
        // Otherwise a profile naming any host would be handed the bearer
        // token and the account id.
        let error = ensure_oauth_endpoint_is_trusted("wss://evil.example/responses")
            .expect_err("a foreign host must be refused");

        assert!(
            error.to_string().contains("evil.example"),
            "the message should name the host, got {error}"
        );
    }

    #[test]
    fn a_lookalike_host_is_not_mistaken_for_the_real_one() {
        for endpoint in [
            "wss://chatgpt.com.evil.example/responses",
            "wss://notchatgpt.com/responses",
            "https://api.openai.com.evil.example/v1/responses",
        ] {
            assert!(
                ensure_oauth_endpoint_is_trusted(endpoint).is_err(),
                "{endpoint} should be refused"
            );
        }
    }

    #[test]
    fn an_oauth_grant_requires_an_encrypted_connection() {
        for endpoint in ["ws://chatgpt.com/responses", "http://api.openai.com/v1"] {
            let error =
                ensure_oauth_endpoint_is_trusted(endpoint).expect_err("plaintext must be refused");
            assert!(error.to_string().contains("encrypted"), "got {error}");
        }
    }

    #[test]
    fn a_malformed_endpoint_is_refused_rather_than_panicking() {
        assert!(ensure_oauth_endpoint_is_trusted("not a url").is_err());
    }

    use super::*;
    use crate::models::{AuthConfig, ModelParameters};

    fn profile(enable_thinking: bool, budget: Option<u32>) -> ModelProfile {
        let mut profile = ModelProfile::new(
            "Codex".to_string(),
            "openai-codex".to_string(),
            "gpt-5.6-luna".to_string(),
            "wss://chatgpt.com/backend-api/codex/responses".to_string(),
            AuthConfig::OAuth {
                account: "chatgpt-acct-1".to_string(),
            },
        );
        profile.parameters = ModelParameters {
            enable_thinking,
            thinking_budget: budget,
            ..profile.parameters
        };
        profile
    }

    #[test]
    fn thinking_disabled_sends_no_reasoning_block() {
        assert!(reasoning_for(&profile(false, Some(10_000))).is_none());
    }

    #[test]
    fn thinking_enabled_sends_an_effort_and_a_summary() {
        let reasoning = reasoning_for(&profile(true, Some(10_000))).expect("reasoning");

        assert_eq!(reasoning.effort.as_deref(), Some("medium"));
        assert_eq!(
            reasoning.summary,
            Some(serde_json::Value::String("auto".to_string()))
        );
    }

    #[test]
    fn budget_buckets_into_an_effort_level() {
        assert_eq!(effort_for_budget(Some(1_024)), "low");
        assert_eq!(effort_for_budget(Some(4_096)), "medium");
        assert_eq!(effort_for_budget(Some(10_000)), "medium");
        assert_eq!(effort_for_budget(Some(16_384)), "high");
        assert_eq!(effort_for_budget(Some(64_000)), "high");
        assert_eq!(effort_for_budget(None), "medium");
    }

    #[test]
    fn the_user_agent_names_this_app_and_its_version() {
        let agent = user_agent();

        assert!(agent.starts_with("personal-agent/"), "agent={agent}");
        assert!(agent.len() > "personal-agent/".len(), "agent={agent}");
    }

    #[test]
    fn a_blank_endpoint_is_rejected_rather_than_dialled() {
        let profile = profile(false, None);
        let quirks = ProviderQuirks::default();
        let request = SessionRequest {
            profile: &profile,
            quirks: &quirks,
            endpoint: "   ",
            conversation_id: None,
            api_key: "",
        };

        let Err(error) = build(&request, "token", None) else {
            panic!("a blank endpoint should not build a model");
        };

        assert!(matches!(error, LlmError::InvalidConfig(_)), "got {error:?}");
    }

    /// The live cache is process-global, so the eviction and staleness rules
    /// are exercised against an owned map. Tests that shared the global would
    /// race each other under the default parallel runner.
    fn test_model() -> Arc<OpenResponsesModel> {
        Arc::new(OpenResponsesModel::new(
            "gpt-5.6-luna",
            "wss://example.test/responses",
        ))
    }

    fn test_key(conversation_id: Uuid, profile_id: Uuid) -> SessionKey {
        SessionKey {
            conversation_id,
            profile_id,
            endpoint: "wss://example.test/responses".to_string(),
            model_id: "gpt-5.6-luna".to_string(),
        }
    }

    #[test]
    fn the_same_token_reuses_one_session() {
        let mut map = HashMap::new();
        let key = test_key(Uuid::new_v4(), Uuid::new_v4());
        insert_in(&mut map, key.clone(), &test_model(), fingerprint("token-1"));

        assert!(take_live_in(&mut map, &key, fingerprint("token-1")).is_some());
        assert!(take_live_in(&mut map, &key, fingerprint("token-1")).is_some());
    }

    #[test]
    fn a_different_token_invalidates_a_cached_session() {
        let mut map = HashMap::new();
        let key = test_key(Uuid::new_v4(), Uuid::new_v4());
        insert_in(&mut map, key.clone(), &test_model(), fingerprint("token-1"));

        assert!(take_live_in(&mut map, &key, fingerprint("token-2")).is_none());
        // The stale entry is dropped, not left to be handed out later.
        assert!(take_live_in(&mut map, &key, fingerprint("token-1")).is_none());
        assert!(map.is_empty());
    }

    #[test]
    fn two_conversations_get_two_sessions() {
        let mut map = HashMap::new();
        let profile_id = Uuid::new_v4();
        let first = test_key(Uuid::new_v4(), profile_id);
        let second = test_key(Uuid::new_v4(), profile_id);

        insert_in(&mut map, first.clone(), &test_model(), 7);
        insert_in(&mut map, second.clone(), &test_model(), 7);

        assert_eq!(map.len(), 2);
        assert!(take_live_in(&mut map, &first, 7).is_some());
        assert!(take_live_in(&mut map, &second, 7).is_some());
    }

    #[test]
    fn invalidating_a_conversation_drops_only_its_sessions() {
        let mut map = HashMap::new();
        let conversation = Uuid::new_v4();
        let other = Uuid::new_v4();
        let profile_id = Uuid::new_v4();
        insert_in(
            &mut map,
            test_key(conversation, profile_id),
            &test_model(),
            7,
        );
        insert_in(&mut map, test_key(other, profile_id), &test_model(), 7);

        map.retain(|key, _| key.conversation_id != conversation);

        assert_eq!(map.len(), 1);
        assert!(take_live_in(&mut map, &test_key(other, profile_id), 7).is_some());
    }

    #[test]
    fn invalidating_a_profile_drops_its_sessions_across_conversations() {
        let mut map = HashMap::new();
        let profile_id = Uuid::new_v4();
        for _ in 0..3 {
            insert_in(
                &mut map,
                test_key(Uuid::new_v4(), profile_id),
                &test_model(),
                7,
            );
        }
        assert_eq!(map.len(), 3);

        map.retain(|key, _| key.profile_id != profile_id);

        assert!(map.is_empty());
    }

    #[test]
    fn the_session_cache_is_bounded() {
        let mut map = HashMap::new();

        for _ in 0..(MAX_LIVE_SESSIONS + 4) {
            insert_in(
                &mut map,
                test_key(Uuid::new_v4(), Uuid::new_v4()),
                &test_model(),
                7,
            );
        }

        assert_eq!(map.len(), MAX_LIVE_SESSIONS);
    }

    #[test]
    fn replacing_an_existing_key_does_not_evict_anyone() {
        let mut map = HashMap::new();
        let keys: Vec<_> = (0..MAX_LIVE_SESSIONS)
            .map(|_| test_key(Uuid::new_v4(), Uuid::new_v4()))
            .collect();
        for key in &keys {
            insert_in(&mut map, key.clone(), &test_model(), 7);
        }
        assert_eq!(map.len(), MAX_LIVE_SESSIONS);

        insert_in(&mut map, keys[0].clone(), &test_model(), 9);

        assert_eq!(map.len(), MAX_LIVE_SESSIONS);
        assert!(take_live_in(&mut map, &keys[0], 9).is_some());
    }

    #[test]
    fn the_global_cache_starts_empty_and_clears() {
        invalidate_all();

        assert_eq!(live_session_count(), 0);
    }

    #[tokio::test]
    async fn a_profile_with_an_empty_account_is_rejected_before_any_network_call() {
        let mut profile = profile(false, None);
        profile.auth = AuthConfig::OAuth {
            account: String::new(),
        };

        let error = resolve_bearer(&profile, "")
            .await
            .expect_err("empty account");

        assert!(matches!(error, LlmError::Auth(_)), "got {error:?}");
    }

    #[tokio::test]
    async fn a_key_authenticated_endpoint_uses_the_api_key_as_the_bearer() {
        let mut profile = profile(false, None);
        profile.auth = AuthConfig::Keychain {
            label: "some-label".to_string(),
        };

        let bearer = resolve_bearer(&profile, "sk-test").await.expect("bearer");

        assert_eq!(bearer, "sk-test");
    }
}
