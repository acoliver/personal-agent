#![allow(clippy::too_many_lines)]
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::float_cmp)]
#![allow(clippy::excessive_precision)]

use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use personal_agent::llm::events::ChatStreamEvent;
use personal_agent::llm::{send_message_stream, LlmClient, Message as LlmMessage, Role as LlmRole};
use personal_agent::mcp::registry::{
    McpRegistry, McpRegistryEnvVar, McpRegistryPackage, McpRegistryPackageArgument,
    McpRegistryRemote, McpRegistryRepository, McpRegistryServer, McpRegistryServerWrapper,
    McpRegistryTransport,
};
use personal_agent::mcp::{
    McpAuthType, McpPackageArgType, McpPackageType, McpSource, McpTransport,
};
use personal_agent::models::{
    AuthConfig, ContextState, Conversation, ConversationMetadata, Message, ModelParameters,
    ModelProfile, SearchResult,
};
use personal_agent::services::chat_impl::ChatServiceImpl;
use personal_agent::services::{ChatService, ConversationService, ProfileService, ServiceError};
use personal_agent::ui_gpui::theme::Theme;
use serde_json::json;
use tokio::sync::Mutex;
use uuid::Uuid;

fn keychain_profile(label: &str) -> ModelProfile {
    ModelProfile {
        id: Uuid::new_v4(),
        name: format!("profile-{label}"),
        provider_id: "openai".to_string(),
        model_id: "gpt-4o-mini".to_string(),
        base_url: "https://api.openai.com/v1".to_string(),
        auth: AuthConfig::Keychain {
            label: label.to_string(),
        },
        parameters: ModelParameters::default(),
        system_prompt: "default system prompt".to_string(),
    }
}

#[derive(Default)]
struct CoverageConversationService {
    conversations: Mutex<Vec<Conversation>>,
}

impl CoverageConversationService {
    fn new(conversations: Vec<Conversation>) -> Self {
        Self {
            conversations: Mutex::new(conversations),
        }
    }
}

#[async_trait]
impl ConversationService for CoverageConversationService {
    async fn create(
        &self,
        title: Option<String>,
        model_profile_id: Uuid,
    ) -> Result<Conversation, ServiceError> {
        let mut conversation = Conversation::new(model_profile_id);
        if let Some(title) = title {
            conversation.set_title(title);
        }
        self.conversations.lock().await.push(conversation.clone());
        Ok(conversation)
    }

    async fn load(&self, id: Uuid) -> Result<Conversation, ServiceError> {
        self.conversations
            .lock()
            .await
            .iter()
            .find(|conversation| conversation.id == id)
            .cloned()
            .ok_or_else(|| ServiceError::NotFound(format!("missing conversation {id}")))
    }

    async fn list_metadata(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ConversationMetadata>, ServiceError> {
        let convs = self.conversations.lock().await;
        let o = offset.unwrap_or(0);
        let l = limit.unwrap_or(convs.len());
        let end = std::cmp::min(o + l, convs.len());
        if o >= convs.len() {
            return Ok(Vec::new());
        }
        Ok(convs[o..end]
            .iter()
            .map(|c| ConversationMetadata {
                id: c.id,
                title: c.title.clone(),
                profile_id: Some(c.profile_id),
                created_at: c.created_at,
                updated_at: c.updated_at,
                message_count: c.messages.len(),
                last_message_preview: c
                    .messages
                    .last()
                    .map(|m| m.content.chars().take(100).collect()),
            })
            .collect())
    }

    async fn add_message(
        &self,
        conversation_id: Uuid,
        message: Message,
    ) -> Result<Message, ServiceError> {
        let mut conversations = self.conversations.lock().await;
        let conversation = conversations
            .iter_mut()
            .find(|conversation| conversation.id == conversation_id)
            .ok_or_else(|| ServiceError::NotFound("conversation missing".to_string()))?;
        conversation.add_message(message.clone());
        Ok(message)
    }

    async fn search(
        &self,
        _query: &str,
        _limit: Option<usize>,
        _offset: Option<usize>,
    ) -> Result<Vec<SearchResult>, ServiceError> {
        Ok(vec![])
    }

    async fn message_count(&self, conversation_id: Uuid) -> Result<usize, ServiceError> {
        let convs = self.conversations.lock().await;
        let conv = convs
            .iter()
            .find(|c| c.id == conversation_id)
            .ok_or_else(|| ServiceError::NotFound("conversation missing".to_string()))?;
        Ok(conv.messages.len())
    }

    async fn update_context_state(
        &self,
        _id: Uuid,
        _state: &ContextState,
    ) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn get_context_state(&self, _id: Uuid) -> Result<Option<ContextState>, ServiceError> {
        Ok(None)
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

    async fn get_messages(&self, conversation_id: Uuid) -> Result<Vec<Message>, ServiceError> {
        Ok(self.load(conversation_id).await?.messages)
    }

    async fn update(
        &self,
        _id: Uuid,
        _title: Option<String>,
        _model_profile_id: Option<Uuid>,
    ) -> Result<Conversation, ServiceError> {
        Err(ServiceError::NotFound("not implemented".to_string()))
    }
}

struct CoverageProfileService {
    default_profile: Mutex<Option<ModelProfile>>,
}

impl CoverageProfileService {
    fn new(default_profile: Option<ModelProfile>) -> Self {
        Self {
            default_profile: Mutex::new(default_profile),
        }
    }
}

#[async_trait]
impl ProfileService for CoverageProfileService {
    async fn list(&self) -> Result<Vec<ModelProfile>, ServiceError> {
        Ok(vec![])
    }

    async fn get(&self, id: Uuid) -> Result<ModelProfile, ServiceError> {
        Err(ServiceError::NotFound(format!("missing profile {id}")))
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
        Err(ServiceError::NotFound("not implemented".to_string()))
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
        Err(ServiceError::NotFound("not implemented".to_string()))
    }

    async fn delete(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn test_connection(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn get_default(&self) -> Result<Option<ModelProfile>, ServiceError> {
        Ok(self.default_profile.lock().await.clone())
    }

    async fn set_default(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }
}

#[test]
fn theme_color_helpers_return_valid_hsla_and_rgba_values() {
    // All accessor calls must return without panicking and produce in-range Hsla values.
    // Exact colors are theme-dependent (runtime-backed); here we verify structural
    // correctness: channels are in [0, 1] and Rgba methods compile and return values.
    let check_hsla = |color: gpui::Hsla, label: &str| {
        assert!(
            color.h >= 0.0 && color.h <= 1.0,
            "{label}: hue out of range"
        );
        assert!(
            color.s >= 0.0 && color.s <= 1.0,
            "{label}: saturation out of range"
        );
        assert!(
            color.l >= 0.0 && color.l <= 1.0,
            "{label}: lightness out of range"
        );
        assert!(
            color.a >= 0.0 && color.a <= 1.0,
            "{label}: alpha out of range"
        );
    };

    check_hsla(Theme::bg_darkest(), "bg_darkest");
    check_hsla(Theme::bg_base(), "bg_base");
    check_hsla(Theme::bg_darker(), "bg_darker");
    check_hsla(Theme::bg_dark(), "bg_dark");
    check_hsla(Theme::text_primary(), "text_primary");
    check_hsla(Theme::text_secondary(), "text_secondary");
    check_hsla(Theme::text_muted(), "text_muted");
    check_hsla(Theme::accent(), "accent");
    check_hsla(Theme::accent_hover(), "accent_hover");
    check_hsla(Theme::border(), "border");
    check_hsla(Theme::user_bubble_bg(), "user_bubble_bg");
    check_hsla(Theme::assistant_bubble_bg(), "assistant_bubble_bg");
    check_hsla(Theme::error(), "error");
    check_hsla(Theme::warning(), "warning");
    check_hsla(Theme::success(), "success");

    assert_eq!(Theme::assistant_bubble(), Theme::bg_darker());
    // Rgba accessors must compile and return a value (exact bits are theme-dependent)
    let _ = Theme::user_bubble();
    let _ = Theme::thinking_bg();
    let _ = Theme::danger();

    assert_eq!(Theme::SPACING_XS, 4.0);
    assert_eq!(Theme::SPACING_SM, 8.0);
    assert_eq!(Theme::SPACING_MD, 12.0);
    assert_eq!(Theme::SPACING_LG, 16.0);
    assert_eq!(Theme::SPACING_XL, 24.0);
    assert_eq!(Theme::RADIUS_SM, 4.0);
    assert_eq!(Theme::RADIUS_MD, 6.0);
    assert_eq!(Theme::RADIUS_LG, 8.0);
    assert_eq!(Theme::FONT_SIZE_XS, 11.0);
    assert_eq!(Theme::FONT_SIZE_SM, 12.0);
    assert_eq!(Theme::FONT_SIZE_MD, 13.0);
    assert_eq!(Theme::FONT_SIZE_BASE, 14.0);
    assert_eq!(Theme::FONT_SIZE_LG, 16.0);
}

#[test]
fn mcp_registry_entry_to_config_maps_package_variants_and_detects_errors() {
    let package_wrapper = McpRegistryServerWrapper {
        server: McpRegistryServer {
            name: "npm-tool".to_string(),
            description: "npm server".to_string(),
            repository: McpRegistryRepository {
                url: Some("https://github.com/example/npm-tool".to_string()),
                source: Some("github".to_string()),
            },
            version: "1.2.3".to_string(),
            packages: vec![McpRegistryPackage {
                registry_type: "npm".to_string(),
                identifier: "@example/npm-tool".to_string(),
                version: Some("1.2.3".to_string()),
                transport: McpRegistryTransport {
                    transport_type: "stdio".to_string(),
                },
                environment_variables: vec![
                    McpRegistryEnvVar {
                        name: "OPENAI_API_KEY".to_string(),
                        description: Some("api key".to_string()),
                        is_secret: true,
                        is_required: true,
                    },
                    McpRegistryEnvVar {
                        name: "LOG_LEVEL".to_string(),
                        description: Some("verbosity".to_string()),
                        is_secret: false,
                        is_required: false,
                    },
                ],
                package_arguments: vec![
                    McpRegistryPackageArgument {
                        argument_type: "named".to_string(),
                        name: "workspace".to_string(),
                        description: Some("workspace path".to_string()),
                        is_required: true,
                        default: None,
                    },
                    McpRegistryPackageArgument {
                        argument_type: "positional".to_string(),
                        name: "repo".to_string(),
                        description: Some("repository".to_string()),
                        is_required: false,
                        default: Some(".".to_string()),
                    },
                ],
            }],
            remotes: vec![McpRegistryRemote {
                remote_type: "http".to_string(),
                url: "https://ignored.example".to_string(),
            }],
        },
        meta: json!({"source": "official"}),
    };

    let package_config = McpRegistry::entry_to_config(&package_wrapper).unwrap();
    assert_eq!(package_config.name, "npm-tool");
    assert!(package_config.enabled);
    assert_eq!(package_config.transport, McpTransport::Stdio);
    assert_eq!(package_config.package.package_type, McpPackageType::Npm);
    assert_eq!(package_config.package.identifier, "@example/npm-tool");
    assert_eq!(package_config.package.runtime_hint.as_deref(), Some("npx"));
    assert_eq!(
        package_config.source,
        McpSource::Official {
            name: "npm-tool".to_string(),
            version: "1.2.3".to_string(),
        }
    );
    assert_eq!(package_config.auth_type, McpAuthType::ApiKey);
    assert_eq!(package_config.env_vars.len(), 2);
    assert!(package_config.env_vars[0].required);
    assert_eq!(package_config.package_args.len(), 2);
    assert_eq!(
        package_config.package_args[0].arg_type,
        McpPackageArgType::Named
    );
    assert_eq!(
        package_config.package_args[1].arg_type,
        McpPackageArgType::Positional
    );
    assert_eq!(package_config.package_args[1].default.as_deref(), Some("."));

    let docker_wrapper = McpRegistryServerWrapper {
        server: McpRegistryServer {
            name: "docker-tool".to_string(),
            description: "docker server".to_string(),
            repository: McpRegistryRepository::default(),
            version: "2025.03.09".to_string(),
            packages: vec![McpRegistryPackage {
                registry_type: "oci".to_string(),
                identifier: "ghcr.io/example/docker-tool".to_string(),
                version: None,
                transport: McpRegistryTransport {
                    transport_type: "streamable-http".to_string(),
                },
                environment_variables: vec![],
                package_arguments: vec![],
            }],
            remotes: vec![],
        },
        meta: json!({}),
    };
    let docker_config = McpRegistry::entry_to_config(&docker_wrapper).unwrap();
    assert_eq!(docker_config.package.package_type, McpPackageType::Docker);
    assert_eq!(
        docker_config.package.runtime_hint.as_deref(),
        Some("docker")
    );
    assert_eq!(docker_config.transport, McpTransport::Http);
    assert_eq!(docker_config.auth_type, McpAuthType::None);

    let remote_wrapper = McpRegistryServerWrapper {
        server: McpRegistryServer {
            name: "remote-tool".to_string(),
            description: "remote server".to_string(),
            repository: McpRegistryRepository::default(),
            version: "latest".to_string(),
            packages: vec![],
            remotes: vec![
                McpRegistryRemote {
                    remote_type: "http".to_string(),
                    url: "https://remote.example/mcp".to_string(),
                },
                McpRegistryRemote {
                    remote_type: "smithery-oauth".to_string(),
                    url: "https://server.smithery.ai/example/remote-tool".to_string(),
                },
            ],
        },
        meta: json!({}),
    };
    let remote_config = McpRegistry::entry_to_config(&remote_wrapper).unwrap();
    assert_eq!(remote_config.transport, McpTransport::Http);
    assert_eq!(remote_config.auth_type, McpAuthType::None);
    assert_eq!(
        remote_config.source,
        McpSource::Manual {
            url: "https://remote.example/mcp".to_string()
        }
    );
}

#[tokio::test]
async fn send_message_stream_reports_missing_and_empty_keychain_values() {
    personal_agent::services::secure_store::use_mock_backend();

    let missing_label = format!("missing-{}", Uuid::new_v4());
    let empty_label = format!("empty-{}", Uuid::new_v4());
    personal_agent::services::secure_store::api_keys::store(&empty_label, "   ").unwrap();

    let mut conversation = Conversation::new(Uuid::new_v4());
    conversation
        .messages
        .push(Message::system("system guidance".to_string()));
    conversation
        .messages
        .push(Message::user("previous question".to_string()));
    conversation
        .messages
        .push(Message::assistant("previous answer".to_string()));

    let missing_profile = keychain_profile(&missing_label);
    let empty_profile = keychain_profile(&empty_label);
    let missing_client = LlmClient::from_profile(&missing_profile);
    assert!(missing_client.is_err());

    personal_agent::services::secure_store::api_keys::store(&missing_label, "real-key").unwrap();
    let client = LlmClient::from_profile(&missing_profile).unwrap();
    let stream_result =
        send_message_stream(&client, &conversation, "new message".to_string()).await;
    match stream_result {
        Ok(mut stream) => {
            let first_event = stream
                .next()
                .await
                .expect("stream should yield at least one event");
            assert!(
                matches!(
                    first_event,
                    ChatStreamEvent::Error { .. }
                        | ChatStreamEvent::Complete { .. }
                        | ChatStreamEvent::TextDelta { .. }
                ),
                "unexpected stream event: {first_event:?}"
            );
        }
        Err(error) => {
            let error_text = error.to_string();
            assert!(
                error_text.contains("SerdesAI error")
                    || error_text.contains("Authentication error"),
                "unexpected error: {error_text}"
            );
        }
    }

    let empty_result = LlmClient::from_profile(&empty_profile);
    assert!(empty_result.is_err());
    let Err(empty_error) = empty_result else {
        unreachable!("expected empty key to be rejected")
    };
    assert_eq!(empty_error.to_string(), "No API key configured for profile");

    personal_agent::services::secure_store::api_keys::delete(&missing_label).unwrap();
    personal_agent::services::secure_store::api_keys::delete(&empty_label).unwrap();
}

#[test]
fn llm_message_helpers_and_stream_event_accessors_cover_basic_paths() {
    let system = LlmMessage::system("rules".to_string());
    let user = LlmMessage::user("hello".to_string());
    let assistant = LlmMessage::assistant("hi".to_string());

    assert_eq!(system.role, LlmRole::System);
    assert_eq!(system.content, "rules");
    assert_eq!(user.role, LlmRole::User);
    assert_eq!(assistant.role, LlmRole::Assistant);

    let text = ChatStreamEvent::text("chunk".to_string());
    let thinking = ChatStreamEvent::thinking("reasoning".to_string());
    let done = ChatStreamEvent::complete(Some(11), Some(29));
    let error = ChatStreamEvent::error("boom".to_string(), false);

    assert!(text.is_text());
    assert_eq!(text.as_text(), Some("chunk"));
    assert!(thinking.is_thinking());
    assert_eq!(thinking.as_thinking(), Some("reasoning"));
    assert!(done.is_complete());
    assert!(error.is_error());
    assert_eq!(error.as_text(), None);
}

#[tokio::test]
async fn chat_service_streaming_flag_prevents_overlapping_send_message_calls() {
    personal_agent::services::secure_store::use_mock_backend();
    let label = format!("chat-overlap-{}", Uuid::new_v4());
    personal_agent::services::secure_store::api_keys::store(&label, "test-api-key").unwrap();

    let profile = keychain_profile(&label);
    let mut conversation = Conversation::new(profile.id);
    conversation
        .messages
        .push(Message::system("from conversation".to_string()));
    let conversation_id = conversation.id;

    let conversation_service = Arc::new(CoverageConversationService::new(vec![conversation]));
    let profile_service = Arc::new(CoverageProfileService::new(Some(profile)));
    let chat_service = ChatServiceImpl::new_for_tests(conversation_service, profile_service);

    let first = chat_service
        .send_message(conversation_id, "hello there".to_string())
        .await
        .expect("first send should return a stream even if background work later fails");
    assert!(chat_service.is_streaming());

    let second = chat_service
        .send_message(conversation_id, "second request".to_string())
        .await;
    assert!(matches!(
        second,
        Err(ServiceError::Internal(message)) if message == "Stream already in progress"
    ));

    let events: Vec<_> = first.take(4).collect().await;
    assert!(!events.is_empty());
    assert!(events
        .into_iter()
        .any(|event| matches!(event, personal_agent::services::ChatStreamEvent::Error(_))));

    for _ in 0..50 {
        if !chat_service.is_streaming() {
            break;
        }
        tokio::task::yield_now().await;
    }
    chat_service.cancel();
    assert!(!chat_service.is_streaming());

    personal_agent::services::secure_store::api_keys::delete(&label).unwrap();
}
