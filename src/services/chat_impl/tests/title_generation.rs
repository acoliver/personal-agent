//! Behavioural coverage for naming a conversation from its first prompt.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

use super::chat_test_support::{MockConversationService, MockProfileService};
use crate::events::types::ConversationEvent;
use crate::events::{subscribe, AppEvent};
use crate::models::{AuthConfig, Conversation, Message, ModelProfile};
use crate::services::chat_impl::titling::{generate_and_apply_title, TitleGenerationRequest};
use crate::services::conversation_title::ConversationTitleGenerator;
use crate::services::{
    ChatService, ChatServiceImpl, ConversationService, ServiceError, ServiceResult,
};

/// Generator driven by the test rather than a provider.
struct ScriptedTitleGenerator {
    response: ServiceResult<String>,
    delay: Duration,
    /// Prompts the generator was asked about, so tests can assert the request payload.
    seen_prompts: Arc<AsyncMutex<Vec<String>>>,
}

impl ScriptedTitleGenerator {
    fn responding_with(raw: &str) -> Self {
        Self {
            response: Ok(raw.to_string()),
            delay: Duration::ZERO,
            seen_prompts: Arc::new(AsyncMutex::new(Vec::new())),
        }
    }

    fn failing_with(message: &str) -> Self {
        Self {
            response: Err(ServiceError::Network(message.to_string())),
            delay: Duration::ZERO,
            seen_prompts: Arc::new(AsyncMutex::new(Vec::new())),
        }
    }

    fn stalling_for(delay: Duration) -> Self {
        Self {
            response: Ok("Too Late".to_string()),
            delay,
            seen_prompts: Arc::new(AsyncMutex::new(Vec::new())),
        }
    }

    fn seen_prompts(&self) -> Arc<AsyncMutex<Vec<String>>> {
        self.seen_prompts.clone()
    }
}

#[async_trait]
impl ConversationTitleGenerator for ScriptedTitleGenerator {
    async fn propose_title(
        &self,
        _profile: &ModelProfile,
        first_prompt: &str,
    ) -> ServiceResult<String> {
        self.seen_prompts
            .lock()
            .await
            .push(first_prompt.to_string());
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        self.response.clone()
    }
}

fn test_profile() -> ModelProfile {
    ModelProfile::new(
        "Test Profile".to_string(),
        "openai".to_string(),
        "gpt-4".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::None,
    )
}

fn conversation_with(title: Option<&str>, messages: Vec<Message>) -> Conversation {
    let mut conversation = Conversation::new(Uuid::new_v4());
    conversation.title = title.map(ToString::to_string);
    conversation.messages = messages;
    conversation
}

/// Drain the global bus for the `TitleUpdated` event belonging to `conversation_id`.
async fn next_title_update(
    rx: &mut tokio::sync::broadcast::Receiver<AppEvent>,
    conversation_id: Uuid,
) -> Option<String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok(AppEvent::Conversation(ConversationEvent::TitleUpdated { id, title })))
                if id == conversation_id =>
            {
                return Some(title)
            }
            // Other tests share the global bus; skip their traffic and any lag it causes.
            Ok(Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {}
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) | Err(_) => return None,
        }
    }
}

/// Wire a chat service whose only stubbed part is the title generator, so the test
/// exercises the real `send_message` path that decides when to auto-name.
async fn wired_chat_service(
    conversation_service: Arc<MockConversationService>,
    generator: Arc<ScriptedTitleGenerator>,
) -> ChatServiceImpl {
    crate::services::secure_store::use_mock_backend();
    crate::services::secure_store::api_keys::store("_test_title_generation", "fake-key")
        .expect("store test key");

    let profile = ModelProfile::new(
        "Test Profile".to_string(),
        "openai".to_string(),
        "gpt-4".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: "_test_title_generation".to_string(),
        },
    );

    let profile_service = Arc::new(MockProfileService::new());
    profile_service.add_profile(profile.clone()).await;
    profile_service.set_default_profile(profile).await;

    ChatServiceImpl::new_for_tests(
        conversation_service as Arc<dyn ConversationService>,
        profile_service as Arc<dyn crate::services::ProfileService>,
    )
    .with_title_generator(generator)
}

/// Poll `conversation_service` until the auto-generated rename lands.
async fn await_rename(conversation_service: &MockConversationService) -> Vec<String> {
    for _ in 0..100 {
        let calls = conversation_service.rename_calls().await;
        if !calls.is_empty() {
            return calls;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Vec::new()
}

#[tokio::test]
async fn sending_the_first_prompt_auto_names_the_conversation() {
    let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let generator = Arc::new(ScriptedTitleGenerator::responding_with(
        r#""Sidebar focus loss""#,
    ));
    let seen_prompts = generator.seen_prompts();
    let chat_service = wired_chat_service(conversation_service.clone(), generator).await;

    // The stream itself is expected to fail against the fake credentials; auto-naming
    // must not depend on it.
    let _stream = chat_service
        .send_message(
            Uuid::new_v4(),
            "Why does the sidebar lose focus?".to_string(),
        )
        .await
        .expect("send_message should start");

    assert_eq!(
        await_rename(&conversation_service).await,
        vec!["Sidebar focus loss".to_string()],
        "the first prompt should rename the untitled conversation"
    );
    assert_eq!(
        seen_prompts.lock().await.as_slice(),
        ["Why does the sidebar lose focus?".to_string()]
    );

    chat_service.clear_all_streams_for_test();
}

#[tokio::test]
async fn sending_a_prompt_into_a_titled_conversation_does_not_rename_it() {
    let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()));
    conversation_service
        .set_title(Some("Tray icon notes"))
        .await;
    let generator = Arc::new(ScriptedTitleGenerator::responding_with(
        "Sidebar focus loss",
    ));
    let seen_prompts = generator.seen_prompts();
    let chat_service = wired_chat_service(conversation_service.clone(), generator).await;

    let _stream = chat_service
        .send_message(
            Uuid::new_v4(),
            "Why does the sidebar lose focus?".to_string(),
        )
        .await
        .expect("send_message should start");

    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(
        seen_prompts.lock().await.is_empty(),
        "a titled conversation should not trigger a title request at all"
    );
    assert_eq!(
        conversation_service.current_title().await,
        Some("Tray icon notes".to_string())
    );

    chat_service.clear_all_streams_for_test();
}

#[tokio::test]
async fn first_prompt_in_untitled_conversation_yields_a_title_request() {
    let profile = test_profile();
    let conversation = conversation_with(
        Some("New Conversation"),
        vec![Message::user("How do I profile a GPUI view?".to_string())],
    );

    let request = TitleGenerationRequest::for_first_prompt(&conversation, &profile);

    assert!(request.is_some(), "first prompt should be eligible");
}

#[tokio::test]
async fn conversation_with_a_real_title_is_never_retitled() {
    let profile = test_profile();
    let conversation = conversation_with(
        Some("Notes on the tray icon"),
        vec![Message::user("How do I profile a GPUI view?".to_string())],
    );

    assert!(
        TitleGenerationRequest::for_first_prompt(&conversation, &profile).is_none(),
        "a user-chosen title must not be replaced"
    );
}

#[tokio::test]
async fn second_prompt_does_not_trigger_another_title_request() {
    let profile = test_profile();
    let conversation = conversation_with(
        Some("New Conversation"),
        vec![
            Message::user("First question".to_string()),
            Message::assistant("First answer".to_string()),
            Message::user("Follow-up question".to_string()),
        ],
    );

    assert!(
        TitleGenerationRequest::for_first_prompt(&conversation, &profile).is_none(),
        "auto-naming must happen at most once per conversation"
    );
}

#[tokio::test]
async fn conversation_without_a_user_message_is_not_titled() {
    let profile = test_profile();
    let conversation = conversation_with(
        Some("New Conversation"),
        vec![Message::system("Hi".to_string())],
    );

    assert!(TitleGenerationRequest::for_first_prompt(&conversation, &profile).is_none());
}

#[tokio::test]
async fn blank_first_prompt_is_not_titled() {
    let profile = test_profile();
    let conversation = conversation_with(
        Some("New Conversation"),
        vec![Message::user("   \n  ".to_string())],
    );

    assert!(TitleGenerationRequest::for_first_prompt(&conversation, &profile).is_none());
}

#[tokio::test]
async fn generated_title_is_persisted_sanitized_and_announced() {
    let conversation_id = Uuid::new_v4();
    let mut events = subscribe();

    let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let profile = test_profile();
    let conversation = conversation_with(
        Some("New Conversation"),
        vec![Message::user(
            "Why does the sidebar lose focus?".to_string(),
        )],
    );
    let request = TitleGenerationRequest::for_first_prompt(&conversation, &profile)
        .expect("first prompt should be eligible");

    let generator = Arc::new(ScriptedTitleGenerator::responding_with(
        "**Title: \"Sidebar focus loss\".**",
    ));
    let seen_prompts = generator.seen_prompts();

    generate_and_apply_title(
        generator,
        conversation_service.clone() as Arc<dyn ConversationService>,
        conversation_id,
        request,
    )
    .await;

    assert_eq!(
        conversation_service.rename_calls().await,
        vec!["Sidebar focus loss".to_string()],
        "the sanitized title should be persisted exactly once"
    );
    assert_eq!(
        conversation_service.current_title().await,
        Some("Sidebar focus loss".to_string()),
        "the stored conversation title should reflect the generated name"
    );
    assert_eq!(
        next_title_update(&mut events, conversation_id).await,
        Some("Sidebar focus loss".to_string()),
        "the UI must be told about the new title"
    );
    assert_eq!(
        seen_prompts.lock().await.as_slice(),
        ["Why does the sidebar lose focus?".to_string()],
        "the generator should receive the user's first prompt"
    );
}

#[tokio::test]
async fn generator_failure_leaves_the_placeholder_title_intact() {
    let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let profile = test_profile();
    let conversation = conversation_with(
        Some("New Conversation"),
        vec![Message::user(
            "Why does the sidebar lose focus?".to_string(),
        )],
    );
    let request = TitleGenerationRequest::for_first_prompt(&conversation, &profile)
        .expect("first prompt should be eligible");

    generate_and_apply_title(
        Arc::new(ScriptedTitleGenerator::failing_with("provider unreachable")),
        conversation_service.clone() as Arc<dyn ConversationService>,
        Uuid::new_v4(),
        request,
    )
    .await;

    assert!(
        conversation_service.rename_calls().await.is_empty(),
        "a failed generation must not rename the conversation"
    );
    assert_eq!(
        conversation_service.current_title().await,
        Some("New Conversation".to_string())
    );
}

#[tokio::test(start_paused = true)]
async fn stalled_generation_times_out_without_renaming() {
    let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let profile = test_profile();
    let conversation = conversation_with(
        Some("New Conversation"),
        vec![Message::user(
            "Why does the sidebar lose focus?".to_string(),
        )],
    );
    let request = TitleGenerationRequest::for_first_prompt(&conversation, &profile)
        .expect("first prompt should be eligible");

    generate_and_apply_title(
        Arc::new(ScriptedTitleGenerator::stalling_for(Duration::from_hours(
            1,
        ))),
        conversation_service.clone() as Arc<dyn ConversationService>,
        Uuid::new_v4(),
        request,
    )
    .await;

    assert!(
        conversation_service.rename_calls().await.is_empty(),
        "a stalled generation must not rename the conversation"
    );
    assert_eq!(
        conversation_service.current_title().await,
        Some("New Conversation".to_string())
    );
}

#[tokio::test]
async fn a_manual_rename_during_generation_is_not_overwritten() {
    let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let profile = test_profile();
    let conversation = conversation_with(
        Some("New Conversation"),
        vec![Message::user(
            "Why does the sidebar lose focus?".to_string(),
        )],
    );
    let request = TitleGenerationRequest::for_first_prompt(&conversation, &profile)
        .expect("first prompt should be eligible");

    // The user renames the conversation while the request is in flight.
    conversation_service
        .set_title(Some("My careful notes"))
        .await;

    generate_and_apply_title(
        Arc::new(ScriptedTitleGenerator::responding_with(
            "Sidebar focus loss",
        )),
        conversation_service.clone() as Arc<dyn ConversationService>,
        Uuid::new_v4(),
        request,
    )
    .await;

    assert!(
        conversation_service.rename_calls().await.is_empty(),
        "the user's title must win over a late-arriving generated one"
    );
    assert_eq!(
        conversation_service.current_title().await,
        Some("My careful notes".to_string())
    );
}

#[tokio::test]
async fn unusable_model_output_leaves_the_placeholder_title_intact() {
    let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let profile = test_profile();
    let conversation = conversation_with(
        Some("New Conversation"),
        vec![Message::user(
            "Why does the sidebar lose focus?".to_string(),
        )],
    );
    let request = TitleGenerationRequest::for_first_prompt(&conversation, &profile)
        .expect("first prompt should be eligible");

    // A model that only emits an unterminated thinking block leaves nothing to use.
    generate_and_apply_title(
        Arc::new(ScriptedTitleGenerator::responding_with(
            "<think>still deciding on a title",
        )),
        conversation_service.clone() as Arc<dyn ConversationService>,
        Uuid::new_v4(),
        request,
    )
    .await;

    assert!(
        conversation_service.rename_calls().await.is_empty(),
        "junk output must not rename the conversation"
    );
    assert_eq!(
        conversation_service.current_title().await,
        Some("New Conversation".to_string())
    );
}
