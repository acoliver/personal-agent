//! Behavioural coverage for naming a conversation from its first prompt.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{mpsc, Mutex as AsyncMutex, Notify};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::chat_test_support::{
    InMemoryAppSettingsService, MockConversationService, MockProfileService,
};
use crate::agent::tool_approval_policy::ToolApprovalPolicy;
use crate::llm::client_agent::ApprovalGate;
use crate::models::{AuthConfig, Conversation, Message, ModelProfile};
use crate::presentation::view_command::{ConversationSummary, ViewCommand};
use crate::services::chat_impl::titling::{generate_and_apply_title, TitleGenerationRequest};
use crate::services::conversation_title::ConversationTitleGenerator;
use crate::services::{
    ChatService, ChatServiceImpl, ConversationService, ServiceError, ServiceResult,
};
use crate::ui_gpui::app_store::{BeginSelectionMode, BeginSelectionResult, StartupInputs};
use crate::ui_gpui::GpuiAppStore;

/// Handle that lets a test hold a proposal open and act while it is in flight.
#[derive(Clone)]
struct TitleGate {
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

impl TitleGate {
    fn new() -> Self {
        Self {
            entered: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
        }
    }

    /// Resolve once the generator has actually started proposing a title.
    async fn wait_until_in_flight(&self) {
        self.entered.notified().await;
    }

    /// Let the held proposal finish.
    fn release(&self) {
        self.release.notify_one();
    }
}

/// Generator driven by the test rather than a provider.
struct ScriptedTitleGenerator {
    response: ServiceResult<String>,
    delay: Duration,
    /// When set, the proposal blocks until the test releases the gate.
    gate: Option<TitleGate>,
    /// Prompts the generator was asked about, so tests can assert the request payload.
    seen_prompts: Arc<AsyncMutex<Vec<String>>>,
}

impl ScriptedTitleGenerator {
    fn responding_with(raw: &str) -> Self {
        Self {
            response: Ok(raw.to_string()),
            delay: Duration::ZERO,
            gate: None,
            seen_prompts: Arc::new(AsyncMutex::new(Vec::new())),
        }
    }

    fn failing_with(message: &str) -> Self {
        Self {
            response: Err(ServiceError::Network(message.to_string())),
            delay: Duration::ZERO,
            gate: None,
            seen_prompts: Arc::new(AsyncMutex::new(Vec::new())),
        }
    }

    fn stalling_for(delay: Duration) -> Self {
        Self {
            response: Ok("Too Late".to_string()),
            delay,
            gate: None,
            seen_prompts: Arc::new(AsyncMutex::new(Vec::new())),
        }
    }

    fn held_by(mut self, gate: TitleGate) -> Self {
        self.gate = Some(gate);
        self
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
        if let Some(gate) = &self.gate {
            gate.entered.notify_one();
            gate.release.notified().await;
        }
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

fn untitled_request() -> TitleGenerationRequest {
    let conversation = conversation_with(
        Some("New Conversation"),
        vec![Message::user(
            "Why does the sidebar lose focus?".to_string(),
        )],
    );
    TitleGenerationRequest::for_untitled_conversation(&conversation, &test_profile())
        .expect("an untitled conversation should be eligible")
}

/// Run the titling task to completion against a throwaway view channel.
async fn apply_title(
    generator: Arc<dyn ConversationTitleGenerator>,
    conversation_service: &Arc<MockConversationService>,
    request: TitleGenerationRequest,
) -> Option<ViewCommand> {
    let (view_tx, mut view_rx) = mpsc::channel(8);
    generate_and_apply_title(
        generator,
        conversation_service.clone() as Arc<dyn ConversationService>,
        view_tx,
        Uuid::new_v4(),
        request,
        CancellationToken::new(),
    )
    .await;
    view_rx.try_recv().ok()
}

/// Wire a chat service whose only stubbed part is the title generator, so tests exercise
/// the real `send_message` path that decides when to auto-name.
async fn wired_chat_service(
    conversation_service: Arc<MockConversationService>,
    generator: Arc<ScriptedTitleGenerator>,
) -> (ChatServiceImpl, mpsc::Receiver<ViewCommand>) {
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

    let app_settings =
        Arc::new(InMemoryAppSettingsService::new()) as Arc<dyn crate::services::AppSettingsService>;
    let skills_service = Arc::new(
        crate::services::SkillsServiceImpl::new(app_settings.clone())
            .expect("skills service should initialize"),
    ) as Arc<dyn crate::services::SkillsService>;
    let (view_tx, view_rx) = mpsc::channel(64);

    let service = ChatServiceImpl::new(
        conversation_service as Arc<dyn ConversationService>,
        profile_service as Arc<dyn crate::services::ProfileService>,
        app_settings,
        skills_service,
        view_tx,
        Arc::new(ApprovalGate::new()),
        Arc::new(AsyncMutex::new(ToolApprovalPolicy::default())),
    )
    .with_title_generator(generator);

    (service, view_rx)
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
async fn untitled_conversation_yields_a_title_request() {
    let profile = test_profile();
    let conversation = conversation_with(
        Some("New Conversation"),
        vec![Message::user("How do I profile a GPUI view?".to_string())],
    );

    assert!(
        TitleGenerationRequest::for_untitled_conversation(&conversation, &profile).is_some(),
        "an untitled conversation with a prompt should be eligible"
    );
}

#[tokio::test]
async fn conversation_with_a_real_title_is_never_retitled() {
    let profile = test_profile();
    let conversation = conversation_with(
        Some("Notes on the tray icon"),
        vec![Message::user("How do I profile a GPUI view?".to_string())],
    );

    assert!(
        TitleGenerationRequest::for_untitled_conversation(&conversation, &profile).is_none(),
        "a user-chosen title must not be replaced"
    );
}

#[tokio::test]
async fn a_still_untitled_conversation_retries_using_its_first_prompt() {
    let profile = test_profile();
    let conversation = conversation_with(
        Some("New Conversation"),
        vec![
            Message::user("Why does the sidebar lose focus?".to_string()),
            Message::assistant("Because of the focus handle.".to_string()),
            Message::user("And what about the composer?".to_string()),
        ],
    );

    let request = TitleGenerationRequest::for_untitled_conversation(&conversation, &profile)
        .expect("a conversation left untitled by a failed attempt should retry");

    let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let generator = Arc::new(ScriptedTitleGenerator::responding_with(
        "Sidebar focus loss",
    ));
    let seen_prompts = generator.seen_prompts();
    apply_title(generator, &conversation_service, request).await;

    assert_eq!(
        seen_prompts.lock().await.as_slice(),
        ["Why does the sidebar lose focus?".to_string()],
        "the retry should name the conversation after its first prompt, not a follow-up"
    );
}

#[tokio::test]
async fn conversation_without_a_user_message_is_not_titled() {
    let profile = test_profile();
    let conversation = conversation_with(
        Some("New Conversation"),
        vec![Message::system("Hi".to_string())],
    );

    assert!(TitleGenerationRequest::for_untitled_conversation(&conversation, &profile).is_none());
}

#[tokio::test]
async fn blank_first_prompt_is_not_titled() {
    let profile = test_profile();
    let conversation = conversation_with(
        Some("New Conversation"),
        vec![Message::user("   \n  ".to_string())],
    );

    assert!(TitleGenerationRequest::for_untitled_conversation(&conversation, &profile).is_none());
}

#[tokio::test]
async fn generated_title_is_persisted_sanitized_and_announced() {
    let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let generator = Arc::new(ScriptedTitleGenerator::responding_with(
        "**Title: \"Sidebar focus loss\".**",
    ));
    let seen_prompts = generator.seen_prompts();

    let announced = apply_title(generator, &conversation_service, untitled_request()).await;

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
    assert!(
        matches!(
            announced,
            Some(ViewCommand::ConversationTitleUpdated { ref title, .. })
                if title == "Sidebar focus loss"
        ),
        "the UI must be told about the new title, got {announced:?}"
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

    let announced = apply_title(
        Arc::new(ScriptedTitleGenerator::failing_with("provider unreachable")),
        &conversation_service,
        untitled_request(),
    )
    .await;

    assert!(
        conversation_service.rename_calls().await.is_empty(),
        "a failed generation must not rename the conversation"
    );
    assert!(announced.is_none(), "nothing should be announced to the UI");
    assert_eq!(
        conversation_service.current_title().await,
        Some("New Conversation".to_string())
    );
}

#[tokio::test(start_paused = true)]
async fn stalled_generation_times_out_without_renaming() {
    let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()));

    apply_title(
        Arc::new(ScriptedTitleGenerator::stalling_for(Duration::from_hours(
            1,
        ))),
        &conversation_service,
        untitled_request(),
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
async fn cancelling_the_turn_abandons_title_generation() {
    let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let cancel = CancellationToken::new();
    let gate = TitleGate::new();
    let (view_tx, mut view_rx) = mpsc::channel(8);

    let titling = tokio::spawn({
        let generator = Arc::new(
            ScriptedTitleGenerator::responding_with("Sidebar focus loss").held_by(gate.clone()),
        ) as Arc<dyn ConversationTitleGenerator>;
        let conversation_service = conversation_service.clone() as Arc<dyn ConversationService>;
        let cancel = cancel.clone();
        async move {
            generate_and_apply_title(
                generator,
                conversation_service,
                view_tx,
                Uuid::new_v4(),
                untitled_request(),
                cancel,
            )
            .await;
        }
    });

    // The user presses Stop while the proposal is still in flight.
    gate.wait_until_in_flight().await;
    cancel.cancel();
    titling.await.expect("title task should not panic");
    gate.release();

    assert!(
        conversation_service.rename_calls().await.is_empty(),
        "a cancelled turn must not rename the conversation afterwards"
    );
    assert!(view_rx.try_recv().is_err(), "nothing should reach the UI");
}

#[tokio::test]
async fn a_manual_rename_during_generation_is_not_overwritten() {
    let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let gate = TitleGate::new();
    let (view_tx, mut view_rx) = mpsc::channel(8);

    let titling = tokio::spawn({
        let generator = Arc::new(
            ScriptedTitleGenerator::responding_with("Sidebar focus loss").held_by(gate.clone()),
        ) as Arc<dyn ConversationTitleGenerator>;
        let conversation_service = conversation_service.clone() as Arc<dyn ConversationService>;
        async move {
            generate_and_apply_title(
                generator,
                conversation_service,
                view_tx,
                Uuid::new_v4(),
                untitled_request(),
                CancellationToken::new(),
            )
            .await;
        }
    });

    // The user renames the conversation while the proposal is genuinely in flight.
    gate.wait_until_in_flight().await;
    conversation_service
        .set_title(Some("My careful notes"))
        .await;
    gate.release();
    titling.await.expect("title task should not panic");

    assert!(
        conversation_service.rename_calls().await.is_empty(),
        "the user's title must win over a late-arriving generated one"
    );
    assert!(view_rx.try_recv().is_err(), "nothing should reach the UI");
    assert_eq!(
        conversation_service.current_title().await,
        Some("My careful notes".to_string())
    );
}

#[tokio::test]
async fn a_conversation_deleted_during_generation_is_not_renamed() {
    let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()));
    conversation_service.set_load_missing(true).await;

    let announced = apply_title(
        Arc::new(ScriptedTitleGenerator::responding_with(
            "Sidebar focus loss",
        )),
        &conversation_service,
        untitled_request(),
    )
    .await;

    assert!(
        conversation_service.rename_calls().await.is_empty(),
        "a deleted conversation must not be renamed"
    );
    assert!(announced.is_none(), "nothing should reach the UI");
}

#[tokio::test]
async fn unusable_model_output_leaves_the_placeholder_title_intact() {
    let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()));

    // A model that only emits an unterminated thinking block leaves nothing to use.
    apply_title(
        Arc::new(ScriptedTitleGenerator::responding_with(
            "<think>still deciding on a title",
        )),
        &conversation_service,
        untitled_request(),
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

#[tokio::test]
async fn sending_the_first_prompt_auto_names_the_conversation() {
    let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let generator = Arc::new(ScriptedTitleGenerator::responding_with(
        r#""Sidebar focus loss""#,
    ));
    let seen_prompts = generator.seen_prompts();
    let (chat_service, _view_rx) =
        wired_chat_service(conversation_service.clone(), generator).await;

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
    let (chat_service, _view_rx) =
        wired_chat_service(conversation_service.clone(), generator).await;

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
async fn the_generated_title_reaches_the_title_bar_and_the_conversation_list() {
    let conversation_id = Uuid::new_v4();
    let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let (chat_service, mut view_rx) = wired_chat_service(
        conversation_service.clone(),
        Arc::new(ScriptedTitleGenerator::responding_with(
            "Sidebar focus loss",
        )),
    )
    .await;

    let _stream = chat_service
        .send_message(
            conversation_id,
            "Why does the sidebar lose focus?".to_string(),
        )
        .await
        .expect("send_message should start");
    assert!(
        !await_rename(&conversation_service).await.is_empty(),
        "the conversation should be auto-named"
    );

    let title_command = next_title_command(&mut view_rx, conversation_id)
        .await
        .expect("the service should publish a title update to the view channel");

    // Feed the service's own command through the real store the UI renders from.
    let store = GpuiAppStore::from_startup_inputs(StartupInputs {
        profiles: Vec::new(),
        selected_profile_id: None,
        conversations: vec![ConversationSummary {
            id: conversation_id,
            title: "New Conversation".to_string(),
            updated_at: chrono::Utc::now(),
            message_count: 1,
            preview: None,
        }],
        selected_conversation: None,
    });
    let generation =
        match store.begin_selection(conversation_id, BeginSelectionMode::BatchNoPublish) {
            BeginSelectionResult::BeganSelection { generation } => generation,
            BeginSelectionResult::NoOpSameSelection => panic!("expected selection to begin"),
        };
    store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
        conversation_id,
        selection_generation: generation,
        messages: Vec::new(),
    }]);

    assert!(store.reduce_batch(vec![title_command]));

    let snapshot = store.current_snapshot();
    assert_eq!(
        snapshot.chat.selected_conversation_title, "Sidebar focus loss",
        "the chat title bar should show the generated title"
    );
    assert_eq!(
        snapshot.history.conversations[0].title, "Sidebar focus loss",
        "the history list should show the generated title"
    );
    assert_eq!(
        snapshot.chat.conversations[0].title, "Sidebar focus loss",
        "the chat sidebar list should show the generated title"
    );

    chat_service.clear_all_streams_for_test();
}

/// Drain `view_rx` for the title update belonging to `conversation_id`.
async fn next_title_command(
    view_rx: &mut mpsc::Receiver<ViewCommand>,
    conversation_id: Uuid,
) -> Option<ViewCommand> {
    for _ in 0..100 {
        while let Ok(command) = view_rx.try_recv() {
            if matches!(
                &command,
                ViewCommand::ConversationTitleUpdated { id, .. } if *id == conversation_id
            ) {
                return Some(command);
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    None
}
