//! `McpConfigurePresenter` - handles MCP server configuration UI
//!
//! `McpConfigurePresenter` subscribes to MCP configuration events,
//! coordinates with MCP service, and emits view commands.
//!
//! @plan PLAN-20250125-REFACTOR.P10
//! @requirement REQ-025.1

use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use super::{Presenter, PresenterError, ViewCommand};
use crate::events::{
    types::{McpEvent, UserEvent},
    AppEvent, EventBus,
};
use crate::services::McpService;

/// Persisted MCP fields the configure draft needs beyond the service payload.
struct PersistedMcpDraft {
    auth_type: crate::mcp::McpAuthType,
    keyfile_path: String,
    env: Vec<(String, String, bool)>,
    stored_secret_names: Vec<String>,
}

impl PersistedMcpDraft {
    const fn empty() -> Self {
        Self {
            auth_type: crate::mcp::McpAuthType::None,
            keyfile_path: String::new(),
            env: Vec::new(),
            stored_secret_names: Vec::new(),
        }
    }
}

/// `McpConfigurePresenter` - handles MCP server configuration UI
///
/// @plan PLAN-20250125-REFACTOR.P10
/// @requirement REQ-025.1
pub struct McpConfigurePresenter {
    /// Event receiver from `EventBus`
    rx: broadcast::Receiver<AppEvent>,

    /// Reference to MCP service
    mcp_service: Arc<dyn McpService>,

    /// View command sender
    view_tx: broadcast::Sender<ViewCommand>,

    /// Running flag for event loop
    running: Arc<std::sync::atomic::AtomicBool>,

    /// Optional config path override (for testing); `None` → `Config::default_path()`.
    config_path_override: Option<std::path::PathBuf>,
}

impl McpConfigurePresenter {
    /// Create a new `McpConfigurePresenter`
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    pub fn new(
        mcp_service: Arc<dyn McpService>,
        event_bus: &broadcast::Sender<AppEvent>,
        view_tx: broadcast::Sender<ViewCommand>,
    ) -> Self {
        let rx = event_bus.subscribe();
        Self {
            rx,
            mcp_service,
            config_path_override: None,
            view_tx,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Stub constructor using unified global `EventBus` (REQ-WIRE-006 unification path).
    ///
    /// This constructor accepts Arc<EventBus> directly, subscribing to the global event
    /// bus rather than a caller-supplied `broadcast::Sender`. Full wiring deferred to
    /// later implementation phases.
    ///
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P03
    /// @requirement REQ-WIRE-006
    /// @pseudocode component-001-event-pipeline.md lines 090-136
    #[allow(dead_code)]
    pub fn new_with_event_bus(
        mcp_service: Arc<dyn McpService>,
        event_bus: &Arc<EventBus>,
        view_tx: broadcast::Sender<ViewCommand>,
    ) -> Self {
        let rx = event_bus.sender().subscribe();
        Self {
            rx,
            mcp_service,
            config_path_override: None,
            view_tx,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Override the config file path (for testing).
    #[must_use]
    pub fn with_config_path(mut self, path: std::path::PathBuf) -> Self {
        self.config_path_override = Some(path);
        self
    }

    /// Start the presenter event loop
    ///
    /// # Errors
    ///
    /// Returns `PresenterError` if presenter startup becomes fallible in the future.
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    // Signature fixed by the shared call convention, not by the body.
    #[allow(clippy::unused_async_trait_impl)]
    pub async fn start(&mut self) -> Result<(), PresenterError> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let mut rx = self.rx.resubscribe();
        let running = self.running.clone();
        let mcp_service = self.mcp_service.clone();
        let view_tx = self.view_tx.clone();
        let config_path = self.config_path_override.clone();

        tokio::spawn(async move {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(event) => {
                        Self::handle_event(&mcp_service, &view_tx, event, config_path.as_deref())
                            .await;
                    }
                    Err(err) => {
                        if !crate::events::handle_recv_error("McpConfigurePresenter", &err) {
                            tracing::info!("McpConfigurePresenter event stream closed");
                            break;
                        }
                    }
                }
            }
            tracing::info!("McpConfigurePresenter event loop ended");
        });

        Ok(())
    }

    /// Stop the presenter event loop
    ///
    /// # Errors
    ///
    /// Returns `PresenterError` if presenter shutdown becomes fallible in the future.
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    // Signature fixed by the shared call convention, not by the body.
    #[allow(clippy::unused_async_trait_impl)]
    pub async fn stop(&mut self) -> Result<(), PresenterError> {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Check if presenter is running
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Handle incoming events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn handle_event(
        mcp_service: &Arc<dyn McpService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: AppEvent,
        config_path: Option<&std::path::Path>,
    ) {
        match event {
            AppEvent::User(user_evt) => {
                Self::handle_user_event(mcp_service, view_tx, user_evt, config_path).await;
            }
            AppEvent::Mcp(mcp_evt) => {
                Self::handle_mcp_event(view_tx, mcp_evt).await;
            }
            _ => {} // Ignore other events
        }
    }

    /// Handle user events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
    /// @requirement REQ-WIRE-001
    async fn handle_user_event(
        mcp_service: &Arc<dyn McpService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: UserEvent,
        config_path: Option<&std::path::Path>,
    ) {
        match event {
            UserEvent::ConfigureMcp { id } => {
                Self::on_configure_mcp(mcp_service, view_tx, id, config_path).await;
            }
            UserEvent::SaveMcpConfig {
                id,
                config,
                secrets,
            } => {
                Self::on_save_config(mcp_service, view_tx, id, *config, secrets, config_path).await;
            }
            UserEvent::StartMcpOAuth { id, provider } => {
                Self::on_start_oauth(mcp_service, view_tx, id, provider).await;
            }
            _ => {} // Ignore other user events
        }
    }

    /// Handle save MCP config event (full config payload)
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// Handle configure MCP event
    ///
    /// Loads persisted MCP data and projects it into MCP configure draft payload.
    ///
    /// Draft env/auth fields come from the persisted app config; drafts for
    /// MCPs without an app-config entry carry empty defaults.
    async fn on_configure_mcp(
        mcp_service: &Arc<dyn McpService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        id: Uuid,
        config_path: Option<&std::path::Path>,
    ) {
        tracing::info!("Loading MCP config for id: {}", id);

        match mcp_service.get(id).await {
            Ok(cfg) => {
                let name = cfg.name;
                let (command, args, url, package, package_type, runtime_hint) = match cfg.transport
                {
                    serdes_ai_mcp::McpTransportConfig::Stdio { command, args } => {
                        let package = if command == "docker" {
                            args.last().cloned().unwrap_or_default()
                        } else {
                            args.iter()
                                .find(|arg| !arg.starts_with('-'))
                                .cloned()
                                .unwrap_or_else(|| command.clone())
                        };
                        let package_type = if command == "docker" {
                            crate::mcp::McpPackageType::Docker
                        } else {
                            crate::mcp::McpPackageType::Npm
                        };
                        let runtime_hint = if command == "docker" {
                            Some("docker".to_string())
                        } else {
                            Some(command.clone())
                        };
                        (command, args, None, package, package_type, runtime_hint)
                    }
                    serdes_ai_mcp::McpTransportConfig::Http { url }
                    | serdes_ai_mcp::McpTransportConfig::Sse { url } => (
                        String::new(),
                        vec![],
                        Some(url.clone()),
                        url,
                        crate::mcp::McpPackageType::Http,
                        None,
                    ),
                };

                match Self::load_persisted_draft(config_path, id).await {
                    Err(e) => {
                        tracing::error!("Failed to load MCP draft for {}: {}", id, e);
                        let _ = view_tx.send(ViewCommand::ShowError {
                            title: "Load Failed".to_string(),
                            message: e,
                            severity: super::view_command::ErrorSeverity::Error,
                        });
                    }
                    Ok(persisted) => {
                        let persisted = persisted.unwrap_or_else(PersistedMcpDraft::empty);
                        // The typed-key slot targets a secret var; a plain
                        // var name would send the secret to the wrong
                        // keychain entry (or into the config file).
                        let env_var_name = persisted
                            .env
                            .iter()
                            .find(|(_, _, is_secret)| *is_secret)
                            .map_or_else(|| "API_KEY".to_string(), |(var, _, _)| var.clone());

                        let _ = view_tx.send(ViewCommand::McpConfigureDraftLoaded {
                            id: id.to_string(),
                            name,
                            package,
                            package_type,
                            runtime_hint,
                            env_var_name,
                            command,
                            args,
                            auth_type: persisted.auth_type,
                            keyfile_path: persisted.keyfile_path,
                            env: persisted.env,
                            stored_secret_names: persisted.stored_secret_names,
                            url,
                        });

                        let _ = view_tx.send(ViewCommand::NavigateTo {
                            view: super::view_command::ViewId::McpConfigure,
                        });
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to load MCP config {}: {}", id, e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Load Failed".to_string(),
                    message: e.to_string(),
                    severity: super::view_command::ErrorSeverity::Error,
                });
            }
        }
    }

    /// Load persisted draft fields for `id` off the async path.
    ///
    /// Keychain probes and the config read run in `spawn_blocking` (the
    /// keyring can block on OS prompts). `Ok(None)` means the config has no
    /// entry for this id (registry-driven draft); a real load failure is an
    /// `Err` the caller surfaces as `ShowError` instead of silently
    /// degrading the draft.
    async fn load_persisted_draft(
        config_path: Option<&std::path::Path>,
        id: Uuid,
    ) -> Result<Option<PersistedMcpDraft>, String> {
        let path = match config_path {
            Some(p) => p.to_path_buf(),
            None => crate::config::Config::default_path()
                .map_err(|e| format!("Failed to resolve config path: {e}"))?,
        };
        tokio::task::spawn_blocking(move || {
            let app_config = crate::config::Config::load(&path)
                .map_err(|e| format!("Failed to load config: {e}"))?;
            let Some(mcp) = app_config.mcps.iter().find(|m| m.id == id) else {
                return Ok(None);
            };
            let mut stored_secret_names = Vec::new();
            for var in &mcp.env_vars {
                if var.is_secret
                    && matches!(
                        crate::services::secure_store::mcp_keys::get_named(id, &var.name),
                        Ok(Some(_))
                    )
                {
                    stored_secret_names.push(var.name.clone());
                }
            }
            let env = mcp
                .env_vars
                .iter()
                .map(|var| {
                    // Secret values never enter the draft; non-secret vars
                    // carry their configured plain value.
                    let value = if var.is_secret {
                        String::new()
                    } else {
                        var.value.clone().unwrap_or_default()
                    };
                    (var.name.clone(), value, var.is_secret)
                })
                .collect();
            Ok(Some(PersistedMcpDraft {
                auth_type: mcp.auth_type.clone(),
                keyfile_path: mcp
                    .keyfile_path
                    .as_ref()
                    .map_or_else(String::new, |p| p.display().to_string()),
                env,
                stored_secret_names,
            }))
        })
        .await
        .map_err(|e| format!("Draft load task failed: {e}"))?
    }

    // Signature fixed by the shared call convention, not by the body.
    #[allow(clippy::unused_async_trait_impl)]
    async fn on_save_config(
        _mcp_service: &Arc<dyn McpService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        id: Uuid,
        config: crate::events::types::McpConfig,
        secrets: Vec<(String, crate::events::types::SecretValue)>,
        config_path_override: Option<&std::path::Path>,
    ) {
        tracing::info!("Saving MCP config for id: {} name: {}", id, config.name);

        let saved_name = config.name.clone();
        let mut config = config;

        // Generate a stable ID for new MCPs
        if config.id.is_nil() {
            config.id = Uuid::new_v4();
        }
        let saved_id = config.id;
        let config_path = config_path_override.map(std::path::Path::to_path_buf);

        // Keychain writes and the config-file write can block on OS prompts;
        // run the whole synchronous save off the async path.
        let save_result = tokio::task::spawn_blocking(move || {
            Self::persist_save(saved_id, config, &secrets, config_path.as_deref())
        })
        .await
        .unwrap_or_else(|e| Err(format!("MCP save task failed: {e}")));

        match save_result {
            Ok(()) => {
                // Reload global MCP runtime so chat can use the new server.
                // Use lock().await (not try_lock) so the reload waits for any
                // in-progress initialisation to finish before reloading config.
                let global = crate::mcp::McpService::global();
                let reload_config_path = config_path_override.map(std::path::Path::to_path_buf);
                tokio::spawn(async move {
                    let mut svc = global.lock().await;
                    if let Err(e) = svc.reload_with_path(reload_config_path.as_deref()).await {
                        tracing::error!("MCP global reload after save failed: {e}");
                    } else {
                        tracing::info!("MCP global runtime reloaded after save");
                    }
                });

                let _ = view_tx.send(ViewCommand::McpConfigSaved {
                    id: saved_id,
                    name: Some(saved_name),
                });
                let _ = view_tx.send(ViewCommand::NavigateTo {
                    view: super::view_command::ViewId::Settings,
                });
            }
            Err(e) => {
                tracing::error!("MCP config save failed: {}", e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Save Failed".to_string(),
                    message: e,
                    severity: super::view_command::ErrorSeverity::Error,
                });
            }
        }
    }

    /// Store secrets then merge `config` into the app config file.
    ///
    /// Secrets are stored before the config write so a failed store aborts
    /// the save; any failure after stores have happened rolls each stored
    /// entry back. On success, keychain entries for old secret vars the new
    /// config no longer declares are deleted so renames cannot strand
    /// credentials.
    fn persist_save(
        saved_id: Uuid,
        config: crate::events::types::McpConfig,
        secrets: &[(String, crate::events::types::SecretValue)],
        config_path: Option<&std::path::Path>,
    ) -> Result<(), String> {
        let secrets_manager = crate::mcp::SecretsManager::new();
        let mut stored: Vec<String> = Vec::new();

        let result = Self::write_config_and_secrets(
            saved_id,
            config,
            secrets,
            &secrets_manager,
            &mut stored,
            config_path,
        );

        if result.is_err() {
            for name in &stored {
                if let Err(e) =
                    crate::services::secure_store::mcp_keys::delete_named(saved_id, name)
                {
                    tracing::warn!("Failed to roll back MCP secret {name}: {e}");
                }
            }
        }
        result
    }

    /// Store each secret, then merge `config` into the config file and clean
    /// up keychain entries stranded by renamed secret vars.
    fn write_config_and_secrets(
        saved_id: Uuid,
        config: crate::events::types::McpConfig,
        secrets: &[(String, crate::events::types::SecretValue)],
        secrets_manager: &crate::mcp::SecretsManager,
        stored: &mut Vec<String>,
        config_path: Option<&std::path::Path>,
    ) -> Result<(), String> {
        for (var_name, value) in secrets {
            secrets_manager
                .store_api_key_named(saved_id, var_name, value.expose())
                .map_err(|e| format!("Failed to store MCP secret {var_name}: {e}"))?;
            stored.push(var_name.clone());
        }

        let path = match config_path {
            Some(p) => p.to_path_buf(),
            None => crate::config::Config::default_path()
                .map_err(|e| format!("Failed to resolve config path: {e}"))?,
        };
        let mut app_config = crate::config::Config::load(&path)
            .map_err(|e| format!("Failed to load config: {e}"))?;

        let old_secret_names: Vec<String> = app_config
            .mcps
            .iter()
            .find(|m| m.id == saved_id)
            .map(|m| {
                m.env_vars
                    .iter()
                    .filter(|var| var.is_secret)
                    .map(|var| var.name.clone())
                    .collect()
            })
            .unwrap_or_default();

        if let Some(existing) = app_config.mcps.iter_mut().find(|m| m.id == saved_id) {
            *existing = config;
        } else {
            app_config.mcps.push(config);
        }

        app_config
            .save(&path)
            .map_err(|e| format!("Failed to save config: {e}"))?;

        for old_name in &old_secret_names {
            let still_declared = app_config
                .mcps
                .iter()
                .find(|m| m.id == saved_id)
                .is_some_and(|m| {
                    m.env_vars
                        .iter()
                        .any(|var| var.is_secret && &var.name == old_name)
                });
            if !still_declared {
                if let Err(e) =
                    crate::services::secure_store::mcp_keys::delete_named(saved_id, old_name)
                {
                    tracing::warn!("Failed to delete stranded MCP secret {old_name}: {e}");
                }
            }
        }
        Ok(())
    }

    /// Handle start OAuth event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn on_start_oauth(
        _mcp_service: &Arc<dyn McpService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        _id: Uuid,
        provider: String,
    ) {
        tracing::info!("Starting OAuth flow for provider: {}", provider);
        let _ = view_tx.send(ViewCommand::ShowNotification {
            message: format!("Starting OAuth for {provider}"),
        });
    }

    /// Handle MCP domain events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    // Signature fixed by the shared call convention, not by the body.
    #[allow(clippy::unused_async_trait_impl)]
    async fn handle_mcp_event(view_tx: &broadcast::Sender<ViewCommand>, event: McpEvent) {
        if let McpEvent::ConfigSaved { id } = event {
            let _ = view_tx.send(ViewCommand::McpConfigSaved { id, name: None });
        }
    }
}

// Implement Presenter trait
//
// @plan PLAN-20250125-REFACTOR.P10
impl Presenter for McpConfigurePresenter {
    fn start(&mut self) -> Result<(), PresenterError> {
        // Note: This is a sync wrapper - in real usage, call async start() directly
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PresenterError> {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }
}
