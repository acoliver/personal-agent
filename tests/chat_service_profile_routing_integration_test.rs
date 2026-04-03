//! Integration test proving that `ChatServiceImpl::send_message` uses the
//! conversation's stored `profile_id` to resolve the profile — and that the
//! resulting HTTP request carries the correct provider-specific headers.
//!
//! Setup: two profiles (`OpenAI` default, Kimi non-default) backed by mock
//! keychain entries, a wiremock server that **requires** `User-Agent: RooCode/1.0`,
//! and a conversation whose `profile_id` points to the Kimi profile.
//!
//! Expected: the HTTP request hits the User-Agent-gated mock (proving Kimi quirks
//! were applied), not the fallback 403 mock.
//!
//! This test would have FAILED before the fix (because `prepare_message_context`
//! always used the global default `OpenAI` profile) and PASSES after it.

use futures::StreamExt;
use personal_agent::services::{
    ChatService, ChatServiceImpl, ConversationService, ProfileService, ServiceError,
};
use personal_agent::{AuthConfig, ModelParameters, ModelProfile};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn sse_ok_response(model: &str, content: &str) -> String {
    let chunk = serde_json::json!({
        "id": "chatcmpl-test",
        "object": "chat.completion.chunk",
        "created": 1,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": { "role": "assistant", "content": content },
            "finish_reason": null
        }]
    });
    let done_chunk = serde_json::json!({
        "id": "chatcmpl-test",
        "object": "chat.completion.chunk",
        "created": 1,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
    });
    format!("data: {chunk}\n\ndata: {done_chunk}\n\ndata: [DONE]\n\n")
}

// ─── In-process mock services ─────────────────────────────────────────────────

/// Conversation service that always returns a conversation bound to a specific profile.
struct StubConversationService {
    profile_id: Uuid,
}

#[async_trait::async_trait]
impl ConversationService for StubConversationService {
    async fn create(
        &self,
        _title: Option<String>,
        profile_id: Uuid,
    ) -> Result<personal_agent::models::Conversation, ServiceError> {
        Ok(personal_agent::models::Conversation::new(profile_id))
    }

    async fn load(&self, _id: Uuid) -> Result<personal_agent::models::Conversation, ServiceError> {
        Ok(personal_agent::models::Conversation::new(self.profile_id))
    }

    async fn list(
        &self,
        _limit: Option<usize>,
        _offset: Option<usize>,
    ) -> Result<Vec<personal_agent::models::Conversation>, ServiceError> {
        Ok(vec![])
    }

    async fn add_user_message(
        &self,
        _conversation_id: Uuid,
        content: String,
    ) -> Result<personal_agent::models::Message, ServiceError> {
        Ok(personal_agent::models::Message::user(content))
    }

    async fn add_assistant_message(
        &self,
        _conversation_id: Uuid,
        content: String,
    ) -> Result<personal_agent::models::Message, ServiceError> {
        Ok(personal_agent::models::Message::assistant(content))
    }

    async fn rename(&self, _id: Uuid, _new_title: String) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn delete(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn set_active(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn get_active(&self) -> Result<Option<Uuid>, ServiceError> {
        Ok(None)
    }

    async fn get_messages(
        &self,
        _conversation_id: Uuid,
    ) -> Result<Vec<personal_agent::models::Message>, ServiceError> {
        Ok(vec![])
    }

    async fn update(
        &self,
        _id: Uuid,
        _title: Option<String>,
        _model_profile_id: Option<Uuid>,
    ) -> Result<personal_agent::models::Conversation, ServiceError> {
        Err(ServiceError::NotFound("stub".to_string()))
    }
}

/// Profile service with explicit profiles-by-id lookup and a configurable default.
struct StubProfileService {
    default: RwLock<Option<ModelProfile>>,
    profiles: RwLock<std::collections::HashMap<Uuid, ModelProfile>>,
}

impl StubProfileService {
    fn new() -> Self {
        Self {
            default: RwLock::new(None),
            profiles: RwLock::new(std::collections::HashMap::new()),
        }
    }

    async fn set_default_profile(&self, profile: ModelProfile) {
        *self.default.write().await = Some(profile);
    }

    async fn add_profile(&self, profile: ModelProfile) {
        self.profiles.write().await.insert(profile.id, profile);
    }
}

#[async_trait::async_trait]
impl ProfileService for StubProfileService {
    async fn list(&self) -> Result<Vec<ModelProfile>, ServiceError> {
        Ok(vec![])
    }

    async fn get(&self, id: Uuid) -> Result<ModelProfile, ServiceError> {
        self.profiles
            .read()
            .await
            .get(&id)
            .cloned()
            .ok_or_else(|| ServiceError::NotFound(format!("profile {id}")))
    }

    async fn create(
        &self,
        _name: String,
        _provider: String,
        _model: String,
        _base_url: Option<String>,
        _auth: AuthConfig,
        _parameters: ModelParameters,
        _system_prompt: Option<String>,
    ) -> Result<ModelProfile, ServiceError> {
        Err(ServiceError::Internal("stub".to_string()))
    }

    async fn update(
        &self,
        _id: Uuid,
        _name: Option<String>,
        _provider: Option<String>,
        _model: Option<String>,
        _base_url: Option<String>,
        _auth: Option<AuthConfig>,
        _parameters: Option<ModelParameters>,
        _system_prompt: Option<String>,
    ) -> Result<ModelProfile, ServiceError> {
        Err(ServiceError::Internal("stub".to_string()))
    }

    async fn delete(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn test_connection(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn get_default(&self) -> Result<Option<ModelProfile>, ServiceError> {
        Ok(self.default.read().await.clone())
    }

    async fn set_default(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }
}

// ─── The test ─────────────────────────────────────────────────────────────────

/// Proves the full send path routes through the conversation's Kimi profile,
/// causing the HTTP request to carry `User-Agent: RooCode/1.0`.
///
/// Before the fix, this test would fail because `prepare_message_context`
/// resolved the `OpenAI` default profile (no User-Agent quirk) and the request
/// hit the 403 fallback mock.
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn send_message_uses_conversation_profile_and_sends_kimi_headers() {
    // ── Wiremock: requires User-Agent: RooCode/1.0 ──────────────────────
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("user-agent", "RooCode/1.0"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse_ok_response("kimi-k2-0711-preview", "pong")),
        )
        .named("kimi-with-user-agent")
        .mount(&mock_server)
        .await;

    // Fallback: if User-Agent is missing, Kimi returns 403
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "error": {
                "message": "Kimi For Coding is currently only available for Coding Agents",
                "type": "access_terminated_error"
            }
        })))
        .named("fallback-403")
        .with_priority(10) // lower priority than the User-Agent mock
        .mount(&mock_server)
        .await;

    // ── Mock keychain ───────────────────────────────────────────────────
    personal_agent::services::secure_store::use_mock_backend();
    personal_agent::services::secure_store::api_keys::store("_test_routing", "sk-fake-key")
        .expect("store test key");

    // ── Profiles ────────────────────────────────────────────────────────
    let openai_profile = ModelProfile::new(
        "OpenAI Default".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        mock_server.uri(),
        AuthConfig::Keychain {
            label: "_test_routing".to_string(),
        },
    );

    let kimi_profile = ModelProfile::new(
        "Kimi".to_string(),
        "kimi-for-coding".to_string(),
        "kimi-k2-0711-preview".to_string(),
        mock_server.uri(),
        AuthConfig::Keychain {
            label: "_test_routing".to_string(),
        },
    );
    let kimi_profile_id = kimi_profile.id;

    // ── Wire up services ────────────────────────────────────────────────
    // Conversation is bound to the Kimi profile
    let conv_service: Arc<dyn ConversationService> = Arc::new(StubConversationService {
        profile_id: kimi_profile_id,
    });

    let profile_service = Arc::new(StubProfileService::new());
    // Default is OpenAI — but the conversation points to Kimi
    profile_service.set_default_profile(openai_profile).await;
    profile_service.add_profile(kimi_profile).await;
    let profile_service: Arc<dyn ProfileService> = profile_service;

    let chat_service = ChatServiceImpl::new_for_tests(conv_service, profile_service);

    // ── Send ────────────────────────────────────────────────────────────
    let conversation_id = Uuid::new_v4();
    let result = chat_service
        .send_message(conversation_id, "ping".to_string())
        .await;

    assert!(
        result.is_ok(),
        "send_message should succeed, got: {:?}",
        result.err()
    );

    // Drain the stream to let the background task complete the HTTP request
    let mut stream = result.unwrap();
    let mut got_token = false;
    let mut got_error: Option<String> = None;
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(10);

    loop {
        let next = tokio::time::timeout_at(deadline, stream.next()).await;
        match next {
            Ok(Some(personal_agent::services::ChatStreamEvent::Token(t))) => {
                println!("Token: {t}");
                got_token = true;
            }
            Ok(Some(personal_agent::services::ChatStreamEvent::Error(e))) => {
                got_error = Some(e.to_string());
                break;
            }
            Ok(Some(personal_agent::services::ChatStreamEvent::Complete) | None) => break,
            Err(_) => {
                got_error = Some("stream timed out".to_string());
                break;
            }
        }
    }

    // ── Assertions ──────────────────────────────────────────────────────
    let received = mock_server.received_requests().await.unwrap_or_default();
    println!("Mock server received {} request(s):", received.len());
    for (i, req) in received.iter().enumerate() {
        let ua = req
            .headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("MISSING");
        println!("  Request {i}: User-Agent = {ua}, path = {}", req.url);
    }

    // THE KEY ASSERTION: at least one request had User-Agent: RooCode/1.0
    let has_kimi_ua = received
        .iter()
        .any(|r| r.headers.get("user-agent").and_then(|v| v.to_str().ok()) == Some("RooCode/1.0"));

    assert!(
        has_kimi_ua,
        "Expected at least one request with User-Agent: RooCode/1.0 \
         (proving the Kimi profile was used, not the OpenAI default). \
         Error: {got_error:?}"
    );

    if let Some(ref err) = got_error {
        assert!(
            !err.contains("Coding Agents"),
            "BUG: send path used the wrong profile — Kimi rejected the request \
             because User-Agent was missing: {err}"
        );
    }

    assert!(
        got_token,
        "Should have received at least one token from the stream. Error: {got_error:?}"
    );

    let _ = personal_agent::services::secure_store::api_keys::delete("_test_routing");
}
