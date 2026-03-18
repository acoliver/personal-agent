use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use personal_agent::events::{types::UserEvent, AppEvent};
use personal_agent::presentation::view_command::{
    ErrorSeverity, ModelInfo as ViewModelInfo, ViewId,
};
use personal_agent::presentation::{ModelSelectorPresenter, ViewCommand};
use personal_agent::registry::{Limit, ModelInfo};
use personal_agent::services::{ModelsRegistryService, ServiceError, ServiceResult};
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
struct MockModelsRegistryService {
    state: Arc<Mutex<MockRegistryState>>,
}

#[derive(Debug, Default)]
struct MockRegistryState {
    refresh_calls: usize,
    search_queries: Vec<String>,
    provider_queries: Vec<String>,
    api_url_queries: Vec<String>,
    model_queries: Vec<(String, String)>,
    list_providers_calls: usize,
    check_update_calls: usize,
    get_last_refresh_calls: usize,
    list_all_responses: Vec<ServiceResult<Vec<ModelInfo>>>,
    refresh_responses: Vec<ServiceResult<()>>,
    search_responses: HashMap<String, ServiceResult<Vec<ModelInfo>>>,
    provider_responses: HashMap<String, ServiceResult<Vec<ModelInfo>>>,
    provider_api_url_responses: HashMap<String, ServiceResult<Option<String>>>,
}

use std::collections::HashMap;

impl MockModelsRegistryService {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockRegistryState::default())),
        }
    }

    fn with_list_all_responses(self, responses: Vec<ServiceResult<Vec<ModelInfo>>>) -> Self {
        self.state
            .lock()
            .expect("state poisoned")
            .list_all_responses = responses;
        self
    }

    fn with_refresh_responses(self, responses: Vec<ServiceResult<()>>) -> Self {
        self.state.lock().expect("state poisoned").refresh_responses = responses;
        self
    }

    fn with_search_response(self, query: &str, response: ServiceResult<Vec<ModelInfo>>) -> Self {
        self.state
            .lock()
            .expect("state poisoned")
            .search_responses
            .insert(query.to_string(), response);
        self
    }

    fn with_provider_response(
        self,
        provider: &str,
        response: ServiceResult<Vec<ModelInfo>>,
    ) -> Self {
        self.state
            .lock()
            .expect("state poisoned")
            .provider_responses
            .insert(provider.to_string(), response);
        self
    }

    fn with_provider_api_url_response(
        self,
        provider: &str,
        response: ServiceResult<Option<String>>,
    ) -> Self {
        self.state
            .lock()
            .expect("state poisoned")
            .provider_api_url_responses
            .insert(provider.to_string(), response);
        self
    }

    fn snapshot(&self) -> MockRegistryStateSnapshot {
        let state = self.state.lock().expect("state poisoned");
        MockRegistryStateSnapshot {
            refresh_calls: state.refresh_calls,
            search_queries: state.search_queries.clone(),
            provider_queries: state.provider_queries.clone(),
            api_url_queries: state.api_url_queries.clone(),
            model_queries: state.model_queries.clone(),
            list_providers_calls: state.list_providers_calls,
            check_update_calls: state.check_update_calls,
            get_last_refresh_calls: state.get_last_refresh_calls,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct MockRegistryStateSnapshot {
    refresh_calls: usize,
    search_queries: Vec<String>,
    provider_queries: Vec<String>,
    api_url_queries: Vec<String>,
    model_queries: Vec<(String, String)>,
    list_providers_calls: usize,
    check_update_calls: usize,
    get_last_refresh_calls: usize,
}

#[async_trait]
impl ModelsRegistryService for MockModelsRegistryService {
    async fn refresh(&self) -> ServiceResult<()> {
        let mut state = self.state.lock().expect("state poisoned");
        state.refresh_calls += 1;
        if state.refresh_responses.is_empty() {
            Ok(())
        } else {
            state.refresh_responses.remove(0)
        }
    }

    async fn get_model(&self, provider: &str, model: &str) -> ServiceResult<Option<ModelInfo>> {
        let mut state = self.state.lock().expect("state poisoned");
        state
            .model_queries
            .push((provider.to_string(), model.to_string()));
        drop(state);
        Ok(None)
    }

    async fn get_provider(&self, provider: &str) -> ServiceResult<Vec<ModelInfo>> {
        let mut state = self.state.lock().expect("state poisoned");
        state.provider_queries.push(provider.to_string());
        let response = state
            .provider_responses
            .get(provider)
            .cloned()
            .unwrap_or_else(|| Ok(vec![]));
        drop(state);
        response
    }

    async fn get_provider_api_url(&self, provider: &str) -> ServiceResult<Option<String>> {
        let mut state = self.state.lock().expect("state poisoned");
        state.api_url_queries.push(provider.to_string());
        let response = state
            .provider_api_url_responses
            .get(provider)
            .cloned()
            .unwrap_or(Ok(None));
        drop(state);
        response
    }

    #[allow(clippy::significant_drop_tightening)]
    async fn list_providers(&self) -> ServiceResult<Vec<String>> {
        let mut state = self.state.lock().expect("state poisoned");
        state.list_providers_calls += 1;
        Ok(vec![])
    }

    async fn list_all(&self) -> ServiceResult<Vec<ModelInfo>> {
        let mut state = self.state.lock().expect("state poisoned");
        if state.list_all_responses.is_empty() {
            Ok(vec![])
        } else {
            state.list_all_responses.remove(0)
        }
    }

    async fn search(&self, query: &str) -> ServiceResult<Vec<ModelInfo>> {
        let mut state = self.state.lock().expect("state poisoned");
        state.search_queries.push(query.to_string());
        let response = state
            .search_responses
            .get(query)
            .cloned()
            .unwrap_or_else(|| Ok(vec![]));
        drop(state);
        response
    }

    #[allow(clippy::significant_drop_tightening)]
    async fn check_update(&self) -> ServiceResult<bool> {
        let mut state = self.state.lock().expect("state poisoned");
        state.check_update_calls += 1;
        Ok(false)
    }

    #[allow(clippy::significant_drop_tightening)]
    async fn get_last_refresh(&self) -> ServiceResult<Option<chrono::DateTime<chrono::Utc>>> {
        let mut state = self.state.lock().expect("state poisoned");
        state.get_last_refresh_calls += 1;
        Ok(None)
    }
}

fn registry_model(id: &str, name: &str, provider: Option<&str>, context: Option<u64>) -> ModelInfo {
    ModelInfo {
        id: id.to_string(),
        name: name.to_string(),
        family: None,
        attachment: false,
        reasoning: false,
        tool_call: false,
        structured_output: false,
        temperature: true,
        interleaved: false,
        provider: provider.map(str::to_string),
        status: None,
        knowledge: None,
        release_date: None,
        last_updated: None,
        modalities: None,
        open_weights: false,
        cost: None,
        limit: context.map(|value| Limit {
            context: value,
            output: value / 2,
        }),
    }
}

async fn recv_view_command(view_rx: &mut broadcast::Receiver<ViewCommand>) -> ViewCommand {
    tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for view command")
        .expect("view channel closed")
}

async fn expect_no_view_command(view_rx: &mut broadcast::Receiver<ViewCommand>) {
    let result = tokio::time::timeout(Duration::from_millis(100), view_rx.recv()).await;
    assert!(result.is_err(), "unexpected command received: {result:?}");
}

async fn start_presenter(
    service: Arc<dyn ModelsRegistryService>,
) -> (
    ModelSelectorPresenter,
    broadcast::Sender<AppEvent>,
    broadcast::Receiver<ViewCommand>,
) {
    let (event_tx, _) = broadcast::channel::<AppEvent>(64);
    let (view_tx, view_rx) = broadcast::channel::<ViewCommand>(64);
    let mut presenter = ModelSelectorPresenter::new(service, &event_tx, view_tx);
    presenter.start().await.expect("presenter should start");
    tokio::time::sleep(Duration::from_millis(20)).await;
    (presenter, event_tx, view_rx)
}

#[tokio::test]
async fn lifecycle_start_stop_and_idempotent_start_work() {
    let service = Arc::new(MockModelsRegistryService::new());
    let (event_tx, _) = broadcast::channel::<AppEvent>(64);
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(64);
    let mut presenter = ModelSelectorPresenter::new(service, &event_tx, view_tx);

    assert!(!presenter.is_running());

    presenter.start().await.expect("first start should succeed");
    assert!(presenter.is_running());

    presenter
        .start()
        .await
        .expect("second start should succeed");
    assert!(presenter.is_running());
    expect_no_view_command(&mut view_rx).await;

    presenter.stop().await.expect("stop should succeed");
    assert!(!presenter.is_running());
}

#[tokio::test]
async fn open_model_selector_uses_cached_models_when_available() {
    let models = vec![
        registry_model(
            "claude-3-5-sonnet",
            "Claude 3.5 Sonnet",
            Some("anthropic"),
            Some(200_000),
        ),
        registry_model("gpt-4.1", "GPT-4.1", Some("openai"), Some(128_000)),
    ];
    let service_impl = MockModelsRegistryService::new().with_list_all_responses(vec![Ok(models)]);
    let snapshot_handle = service_impl.clone();
    let service: Arc<dyn ModelsRegistryService> = Arc::new(service_impl);
    let (_presenter, event_tx, mut view_rx) = start_presenter(service).await;

    event_tx
        .send(AppEvent::User(UserEvent::OpenModelSelector))
        .expect("send should succeed");

    let command = recv_view_command(&mut view_rx).await;
    assert_eq!(
        command,
        ViewCommand::ModelSearchResults {
            models: vec![
                ViewModelInfo {
                    provider_id: "anthropic".to_string(),
                    model_id: "claude-3-5-sonnet".to_string(),
                    name: "Claude 3.5 Sonnet".to_string(),
                    context_length: Some(200_000),
                },
                ViewModelInfo {
                    provider_id: "openai".to_string(),
                    model_id: "gpt-4.1".to_string(),
                    name: "GPT-4.1".to_string(),
                    context_length: Some(128_000),
                },
            ],
        }
    );

    assert_eq!(
        snapshot_handle.snapshot(),
        MockRegistryStateSnapshot {
            refresh_calls: 0,
            search_queries: vec![],
            provider_queries: vec![],
            api_url_queries: vec![],
            model_queries: vec![],
            list_providers_calls: 0,
            check_update_calls: 0,
            get_last_refresh_calls: 0,
        }
    );
}

#[tokio::test]
async fn open_model_selector_refreshes_when_cache_is_empty() {
    let refreshed_models = vec![registry_model(
        "grok-2",
        "Grok 2",
        Some("xai"),
        Some(64_000),
    )];
    let service_impl = MockModelsRegistryService::new()
        .with_list_all_responses(vec![Ok(vec![]), Ok(refreshed_models)])
        .with_refresh_responses(vec![Ok(())]);
    let snapshot_handle = service_impl.clone();
    let service: Arc<dyn ModelsRegistryService> = Arc::new(service_impl);
    let (_presenter, event_tx, mut view_rx) = start_presenter(service).await;

    event_tx
        .send(AppEvent::User(UserEvent::OpenModelSelector))
        .expect("send should succeed");

    let command = recv_view_command(&mut view_rx).await;
    assert_eq!(
        command,
        ViewCommand::ModelSearchResults {
            models: vec![ViewModelInfo {
                provider_id: "xai".to_string(),
                model_id: "grok-2".to_string(),
                name: "Grok 2".to_string(),
                context_length: Some(64_000),
            }],
        }
    );
    assert_eq!(snapshot_handle.snapshot().refresh_calls, 1);
}

#[tokio::test]
async fn open_model_selector_emits_error_when_loading_fails_after_refresh() {
    let service: Arc<dyn ModelsRegistryService> = Arc::new(
        MockModelsRegistryService::new()
            .with_list_all_responses(vec![
                Err(ServiceError::Network("cache miss".to_string())),
                Err(ServiceError::Internal("registry offline".to_string())),
            ])
            .with_refresh_responses(vec![Err(ServiceError::Network(
                "refresh failed".to_string(),
            ))]),
    );
    let (_presenter, event_tx, mut view_rx) = start_presenter(service).await;

    event_tx
        .send(AppEvent::User(UserEvent::OpenModelSelector))
        .expect("send should succeed");

    let command = recv_view_command(&mut view_rx).await;
    assert_eq!(
        command,
        ViewCommand::ShowError {
            title: "Failed to load models".to_string(),
            message: "Could not load models from registry: Internal(\"registry offline\")"
                .to_string(),
            severity: ErrorSeverity::Warning,
        }
    );
}

#[tokio::test]
async fn search_models_emits_mapped_results() {
    let service_impl = MockModelsRegistryService::new().with_search_response(
        "claude",
        Ok(vec![registry_model(
            "claude-3-7-sonnet",
            "Claude 3.7 Sonnet",
            Some("anthropic"),
            Some(1_000_000),
        )]),
    );
    let snapshot_handle = service_impl.clone();
    let service: Arc<dyn ModelsRegistryService> = Arc::new(service_impl);
    let (_presenter, event_tx, mut view_rx) = start_presenter(service).await;

    event_tx
        .send(AppEvent::User(UserEvent::SearchModels {
            query: "claude".to_string(),
        }))
        .expect("send should succeed");

    let command = recv_view_command(&mut view_rx).await;
    assert_eq!(
        command,
        ViewCommand::ModelSearchResults {
            models: vec![ViewModelInfo {
                provider_id: "anthropic".to_string(),
                model_id: "claude-3-7-sonnet".to_string(),
                name: "Claude 3.7 Sonnet".to_string(),
                context_length: Some(1_000_000),
            }],
        }
    );
    assert_eq!(
        snapshot_handle.snapshot().search_queries,
        vec!["claude".to_string()]
    );
}

#[tokio::test]
async fn search_models_emits_warning_on_error() {
    let service: Arc<dyn ModelsRegistryService> =
        Arc::new(MockModelsRegistryService::new().with_search_response(
            "bad query",
            Err(ServiceError::Network("query failed".to_string())),
        ));
    let (_presenter, event_tx, mut view_rx) = start_presenter(service).await;

    event_tx
        .send(AppEvent::User(UserEvent::SearchModels {
            query: "bad query".to_string(),
        }))
        .expect("send should succeed");

    let command = recv_view_command(&mut view_rx).await;
    assert_eq!(
        command,
        ViewCommand::ShowError {
            title: "Model Search Failed".to_string(),
            message: "Network error: query failed".to_string(),
            severity: ErrorSeverity::Warning,
        }
    );
}

#[tokio::test]
async fn provider_filter_uses_specific_provider_lookup() {
    let service_impl = MockModelsRegistryService::new().with_provider_response(
        "openai",
        Ok(vec![registry_model(
            "gpt-4o",
            "GPT-4o",
            Some("openai"),
            Some(128_000),
        )]),
    );
    let snapshot_handle = service_impl.clone();
    let service: Arc<dyn ModelsRegistryService> = Arc::new(service_impl);
    let (_presenter, event_tx, mut view_rx) = start_presenter(service).await;

    event_tx
        .send(AppEvent::User(UserEvent::FilterModelsByProvider {
            provider_id: Some("openai".to_string()),
        }))
        .expect("send should succeed");

    let command = recv_view_command(&mut view_rx).await;
    assert_eq!(
        command,
        ViewCommand::ModelSearchResults {
            models: vec![ViewModelInfo {
                provider_id: "openai".to_string(),
                model_id: "gpt-4o".to_string(),
                name: "GPT-4o".to_string(),
                context_length: Some(128_000),
            }],
        }
    );
    assert_eq!(
        snapshot_handle.snapshot().provider_queries,
        vec!["openai".to_string()]
    );
}

#[tokio::test]
async fn provider_filter_with_none_lists_all_models() {
    let service: Arc<dyn ModelsRegistryService> = Arc::new(
        MockModelsRegistryService::new().with_list_all_responses(vec![Ok(vec![registry_model(
            "kimi-k2", "Kimi K2", None, None,
        )])]),
    );
    let (_presenter, event_tx, mut view_rx) = start_presenter(service).await;

    event_tx
        .send(AppEvent::User(UserEvent::FilterModelsByProvider {
            provider_id: None,
        }))
        .expect("send should succeed");

    let command = recv_view_command(&mut view_rx).await;
    assert_eq!(
        command,
        ViewCommand::ModelSearchResults {
            models: vec![ViewModelInfo {
                provider_id: "unknown".to_string(),
                model_id: "kimi-k2".to_string(),
                name: "Kimi K2".to_string(),
                context_length: None,
            }],
        }
    );
}

#[tokio::test]
async fn provider_filter_emits_error_on_failure() {
    let service: Arc<dyn ModelsRegistryService> =
        Arc::new(MockModelsRegistryService::new().with_provider_response(
            "anthropic",
            Err(ServiceError::Internal("provider failed".to_string())),
        ));
    let (_presenter, event_tx, mut view_rx) = start_presenter(service).await;

    event_tx
        .send(AppEvent::User(UserEvent::FilterModelsByProvider {
            provider_id: Some("anthropic".to_string()),
        }))
        .expect("send should succeed");

    let command = recv_view_command(&mut view_rx).await;
    assert_eq!(
        command,
        ViewCommand::ShowError {
            title: "Model Filter Failed".to_string(),
            message: "Internal error: provider failed".to_string(),
            severity: ErrorSeverity::Warning,
        }
    );
}

#[tokio::test]
async fn select_model_emits_selection_and_navigation_with_metadata() {
    let selected_model = registry_model(
        "claude-3-5-sonnet",
        "Claude 3.5 Sonnet",
        Some("anthropic"),
        Some(200_000),
    );
    let service_impl = MockModelsRegistryService::new()
        .with_provider_api_url_response(
            "anthropic",
            Ok(Some("https://api.anthropic.com/v1".to_string())),
        )
        .with_list_all_responses(vec![Ok(vec![selected_model])]);
    let snapshot_handle = service_impl.clone();
    let service: Arc<dyn ModelsRegistryService> = Arc::new(service_impl);
    let (_presenter, event_tx, mut view_rx) = start_presenter(service).await;

    event_tx
        .send(AppEvent::User(UserEvent::SelectModel {
            provider_id: "anthropic".to_string(),
            model_id: "claude-3-5-sonnet".to_string(),
        }))
        .expect("send should succeed");

    let first = recv_view_command(&mut view_rx).await;
    let second = recv_view_command(&mut view_rx).await;

    assert_eq!(
        first,
        ViewCommand::ModelSelected {
            provider_id: "anthropic".to_string(),
            model_id: "claude-3-5-sonnet".to_string(),
            provider_api_url: Some("https://api.anthropic.com/v1".to_string()),
            context_length: Some(200_000),
        }
    );
    assert_eq!(
        second,
        ViewCommand::NavigateTo {
            view: ViewId::ProfileEditor
        }
    );
    assert_eq!(
        snapshot_handle.snapshot().api_url_queries,
        vec!["anthropic".to_string()]
    );
}

#[tokio::test]
async fn select_model_continues_when_metadata_lookups_fail() {
    let service: Arc<dyn ModelsRegistryService> = Arc::new(
        MockModelsRegistryService::new()
            .with_provider_api_url_response(
                "openrouter",
                Err(ServiceError::Network(
                    "provider metadata unavailable".to_string(),
                )),
            )
            .with_list_all_responses(vec![Err(ServiceError::Internal(
                "model index unavailable".to_string(),
            ))]),
    );
    let (_presenter, event_tx, mut view_rx) = start_presenter(service).await;

    event_tx
        .send(AppEvent::User(UserEvent::SelectModel {
            provider_id: "openrouter".to_string(),
            model_id: "deepseek-r1".to_string(),
        }))
        .expect("send should succeed");

    let first = recv_view_command(&mut view_rx).await;
    let second = recv_view_command(&mut view_rx).await;

    assert_eq!(
        first,
        ViewCommand::ModelSelected {
            provider_id: "openrouter".to_string(),
            model_id: "deepseek-r1".to_string(),
            provider_api_url: None,
            context_length: None,
        }
    );
    assert_eq!(
        second,
        ViewCommand::NavigateTo {
            view: ViewId::ProfileEditor
        }
    );
}

#[tokio::test]
async fn unrelated_events_are_ignored() {
    let service_impl = MockModelsRegistryService::new();
    let snapshot_handle = service_impl.clone();
    let service: Arc<dyn ModelsRegistryService> = Arc::new(service_impl);
    let (_presenter, event_tx, mut view_rx) = start_presenter(service).await;

    event_tx
        .send(AppEvent::User(UserEvent::RefreshModelsRegistry))
        .expect("send should succeed");
    event_tx
        .send(AppEvent::System(
            personal_agent::events::types::SystemEvent::AppLaunched,
        ))
        .expect("send should succeed");

    expect_no_view_command(&mut view_rx).await;
    assert_eq!(
        snapshot_handle.snapshot(),
        MockRegistryStateSnapshot {
            refresh_calls: 0,
            search_queries: vec![],
            provider_queries: vec![],
            api_url_queries: vec![],
            model_queries: vec![],
            list_providers_calls: 0,
            check_update_calls: 0,
            get_last_refresh_calls: 0,
        }
    );
}
