use async_trait::async_trait;
use personal_agent::events::{
    bus::EventBus,
    types::{AppEvent, McpEvent, McpRegistrySource, UserEvent},
};
use personal_agent::presentation::{
    mcp_add_presenter::McpAddPresenter,
    view_command::{ErrorSeverity, ViewCommand, ViewId},
};
use personal_agent::services::{McpRegistryEntry, McpRegistryService, ServiceError};
use std::sync::Arc;
use tokio::sync::broadcast;

struct StaticRegistryService {
    entries: Vec<McpRegistryEntry>,
}

#[async_trait]
impl McpRegistryService for StaticRegistryService {
    async fn search(&self, _query: &str) -> Result<Vec<McpRegistryEntry>, ServiceError> {
        Ok(self.entries.clone())
    }

    async fn search_registry(
        &self,
        _query: &str,
        _source: &str,
    ) -> Result<Vec<McpRegistryEntry>, ServiceError> {
        Ok(self.entries.clone())
    }

    async fn get_details(&self, name: &str) -> Result<Option<McpRegistryEntry>, ServiceError> {
        Ok(self
            .entries
            .iter()
            .find(|entry| entry.name == name)
            .cloned())
    }

    async fn list_all(&self) -> Result<Vec<McpRegistryEntry>, ServiceError> {
        Ok(self.entries.clone())
    }

    async fn list_by_tag(&self, tag: &str) -> Result<Vec<McpRegistryEntry>, ServiceError> {
        Ok(self
            .entries
            .iter()
            .filter(|entry| entry.tags.iter().any(|existing| existing.contains(tag)))
            .cloned()
            .collect())
    }

    async fn list_trending(&self) -> Result<Vec<McpRegistryEntry>, ServiceError> {
        Ok(self.entries.clone())
    }

    async fn refresh(&self) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn get_last_refresh(
        &self,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>, ServiceError> {
        Ok(None)
    }

    async fn install(&self, _name: &str, _config_name: Option<String>) -> Result<(), ServiceError> {
        Ok(())
    }
}

fn registry_entry() -> McpRegistryEntry {
    McpRegistryEntry {
        name: "filesystem".to_string(),
        display_name: "Filesystem".to_string(),
        description: "Browse files".to_string(),
        version: "1.0.0".to_string(),
        author: "Test".to_string(),
        license: "MIT".to_string(),
        repository: "https://example.com/filesystem".to_string(),
        command: "npx".to_string(),
        args: vec!["-y".to_string(), "@test/filesystem".to_string()],
        env: Some(vec![("API_KEY".to_string(), String::new())]),
        tags: vec!["files".to_string()],
        source: "official".to_string(),
        package_type: Some(personal_agent::mcp::McpPackageType::Npm),
        runtime_hint: Some("npx".to_string()),
        url: None,
    }
}

async fn collect_commands(view_rx: &mut broadcast::Receiver<ViewCommand>) -> Vec<ViewCommand> {
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    let mut commands = Vec::new();
    while let Ok(command) = view_rx.try_recv() {
        commands.push(command);
    }
    commands
}

#[tokio::test]
async fn mcp_add_presenter_supports_manual_http_and_docker_entries_and_errors() {
    let event_bus = Arc::new(EventBus::new(32));
    let registry_service = Arc::new(StaticRegistryService {
        entries: vec![registry_entry()],
    });
    let (view_tx, mut view_rx) = broadcast::channel(64);

    let mut presenter = McpAddPresenter::new_with_event_bus(registry_service, &event_bus, view_tx);
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::McpAddNext {
            manual_entry: Some("https://mcp.example.com/server".to_string()),
        }))
        .expect("publish http manual entry");
    let http_commands = collect_commands(&mut view_rx).await;
    assert!(http_commands.iter().any(|command| matches!(
        command,
        ViewCommand::McpConfigureDraftLoaded {
            name,
            package,
            package_type,
            command,
            args,
            url,
            ..
        } if name == "server"
            && package == "https://mcp.example.com/server"
            && *package_type == personal_agent::mcp::McpPackageType::Http
            && command.is_empty()
            && args.is_empty()
            && *url == Some("https://mcp.example.com/server".to_string())
    )));
    assert!(http_commands.iter().any(|command| matches!(
        command,
        ViewCommand::NavigateTo {
            view: ViewId::McpConfigure
        }
    )));

    event_bus
        .publish(AppEvent::User(UserEvent::McpAddNext {
            manual_entry: Some("docker run ghcr.io/example/filesystem:latest".to_string()),
        }))
        .expect("publish docker manual entry");
    let docker_commands = collect_commands(&mut view_rx).await;
    assert!(docker_commands.iter().any(|command| matches!(
        command,
        ViewCommand::McpConfigureDraftLoaded {
            name,
            package,
            package_type,
            runtime_hint,
            command,
            args,
            ..
        } if name == "filesystem"
            && package == "ghcr.io/example/filesystem:latest"
            && *package_type == personal_agent::mcp::McpPackageType::Docker
            && *runtime_hint == Some("docker".to_string())
            && command == "docker"
            && args == &vec!["run".to_string(), "ghcr.io/example/filesystem:latest".to_string()]
    )));

    event_bus
        .publish(AppEvent::User(UserEvent::McpAddNext { manual_entry: None }))
        .expect("publish missing manual entry");
    let missing_entry_commands = collect_commands(&mut view_rx).await;
    assert!(missing_entry_commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Warning,
        } if title == "Manual Entry Required" && message.contains("Enter an MCP package")
    )));

    event_bus
        .publish(AppEvent::User(UserEvent::McpAddNext {
            manual_entry: Some("plain-command".to_string()),
        }))
        .expect("publish invalid manual entry");
    let invalid_entry_commands = collect_commands(&mut view_rx).await;
    assert!(invalid_entry_commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Warning,
        } if title == "Invalid Manual Entry" && message.contains("Use a package like")
    )));
}

#[tokio::test]
async fn mcp_add_presenter_selection_failure_and_mcp_event_are_harmless() {
    let event_bus = Arc::new(EventBus::new(32));
    let registry_service = Arc::new(StaticRegistryService { entries: vec![] });
    let (view_tx, mut view_rx) = broadcast::channel(64);

    let mut presenter = McpAddPresenter::new_with_event_bus(registry_service, &event_bus, view_tx);
    presenter.start().await.expect("start presenter");
    let _ = collect_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::SelectMcpFromRegistry {
            source: McpRegistrySource {
                name: "smithery::filesystem".to_string(),
            },
        }))
        .expect("publish selection failure");
    let selection_commands = collect_commands(&mut view_rx).await;
    assert!(selection_commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Warning,
        } if title == "Selection Failed" && message.contains("filesystem")
    )));

    event_bus
        .publish(AppEvent::Mcp(McpEvent::Recovered {
            id: uuid::Uuid::new_v4(),
            name: "ignored".to_string(),
        }))
        .expect("publish unrelated mcp event");
    let mcp_commands = collect_commands(&mut view_rx).await;
    assert!(
        mcp_commands.is_empty(),
        "unhandled MCP events should not emit commands"
    );
}
