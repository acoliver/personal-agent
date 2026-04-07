//! Tool approval policy handlers for `SettingsPresenter`.

use std::sync::Arc;

use tokio::sync::broadcast;

use super::settings_presenter::SettingsPresenter;
use super::view_command::ViewCommand;
use crate::agent::tool_approval_policy::{McpApprovalMode, ToolApprovalPolicy};
use crate::services::app_settings::AppSettingsService;

impl SettingsPresenter {
    pub(super) async fn emit_tool_approval_policy_snapshot(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
    ) {
        match ToolApprovalPolicy::load_from_settings(app_settings_service.as_ref()).await {
            Ok(policy) => {
                let _ = view_tx.send(ViewCommand::ToolApprovalPolicyUpdated {
                    yolo_mode: policy.yolo_mode,
                    auto_approve_reads: policy.auto_approve_reads,
                    skills_auto_approve: policy.skills_auto_approve,
                    mcp_approval_mode: policy.mcp_approval_mode,
                    persistent_allowlist: policy.persistent_allowlist,
                    persistent_denylist: policy.persistent_denylist,
                });
                let _ = view_tx.send(ViewCommand::YoloModeChanged {
                    active: policy.yolo_mode,
                });
            }
            Err(error) => {
                tracing::warn!("Failed to load tool approval policy snapshot: {error}");
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Tool Approval Settings".to_string(),
                    message: "Failed to load tool approval settings".to_string(),
                    severity: super::view_command::ErrorSeverity::Warning,
                });
            }
        }
    }

    pub(super) async fn on_set_tool_approval_yolo_mode(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        enabled: bool,
    ) {
        let mut policy =
            match ToolApprovalPolicy::load_from_settings(app_settings_service.as_ref()).await {
                Ok(policy) => policy,
                Err(error) => {
                    tracing::warn!("Failed to load tool approval policy for YOLO toggle: {error}");
                    let _ = view_tx.send(ViewCommand::ShowError {
                        title: "Tool Approval Settings".to_string(),
                        message: "Failed to update YOLO mode".to_string(),
                        severity: super::view_command::ErrorSeverity::Warning,
                    });
                    return;
                }
            };

        if policy.yolo_mode == enabled {
            let _ = view_tx.send(ViewCommand::YoloModeChanged { active: enabled });
            return;
        }

        policy.yolo_mode = enabled;
        if let Err(error) = policy.save_to_settings(app_settings_service.as_ref()).await {
            tracing::warn!("Failed to persist YOLO mode: {error}");
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Tool Approval Settings".to_string(),
                message: "Failed to persist YOLO mode".to_string(),
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        }

        Self::emit_tool_approval_policy_snapshot(app_settings_service, view_tx).await;
    }

    pub(super) async fn on_set_tool_approval_auto_approve_reads(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        enabled: bool,
    ) {
        let mut policy =
            match ToolApprovalPolicy::load_from_settings(app_settings_service.as_ref()).await {
                Ok(policy) => policy,
                Err(error) => {
                    tracing::warn!("Failed to load tool approval policy for read toggle: {error}");
                    let _ = view_tx.send(ViewCommand::ShowError {
                        title: "Tool Approval Settings".to_string(),
                        message: "Failed to update read-only approval mode".to_string(),
                        severity: super::view_command::ErrorSeverity::Warning,
                    });
                    return;
                }
            };

        if policy.auto_approve_reads == enabled {
            return;
        }

        policy.auto_approve_reads = enabled;
        if let Err(error) = policy.save_to_settings(app_settings_service.as_ref()).await {
            tracing::warn!("Failed to persist read-only approval mode: {error}");
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Tool Approval Settings".to_string(),
                message: "Failed to persist read-only approval mode".to_string(),
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        }

        Self::emit_tool_approval_policy_snapshot(app_settings_service, view_tx).await;
    }

    pub(super) async fn on_set_tool_approval_skills_auto_approve(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        enabled: bool,
    ) {
        let mut policy =
            match ToolApprovalPolicy::load_from_settings(app_settings_service.as_ref()).await {
                Ok(policy) => policy,
                Err(error) => {
                    tracing::warn!("Failed to load tool approval policy for skill toggle: {error}");
                    let _ = view_tx.send(ViewCommand::ShowError {
                        title: "Tool Approval Settings".to_string(),
                        message: "Failed to update skill approval mode".to_string(),
                        severity: super::view_command::ErrorSeverity::Warning,
                    });
                    return;
                }
            };

        if policy.skills_auto_approve == enabled {
            return;
        }

        policy.skills_auto_approve = enabled;
        if let Err(error) = policy.save_to_settings(app_settings_service.as_ref()).await {
            tracing::warn!("Failed to persist skill approval mode: {error}");
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Tool Approval Settings".to_string(),
                message: "Failed to persist skill approval mode".to_string(),
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        }

        Self::emit_tool_approval_policy_snapshot(app_settings_service, view_tx).await;
    }

    pub(super) async fn on_set_tool_approval_mcp_mode(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        mode: McpApprovalMode,
    ) {
        let mut policy = match ToolApprovalPolicy::load_from_settings(app_settings_service.as_ref())
            .await
        {
            Ok(policy) => policy,
            Err(error) => {
                tracing::warn!("Failed to load tool approval policy for MCP mode change: {error}");
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Tool Approval Settings".to_string(),
                    message: "Failed to update MCP approval mode".to_string(),
                    severity: super::view_command::ErrorSeverity::Warning,
                });
                return;
            }
        };

        if policy.mcp_approval_mode == mode {
            return;
        }

        policy.mcp_approval_mode = mode;
        if let Err(error) = policy.save_to_settings(app_settings_service.as_ref()).await {
            tracing::warn!("Failed to persist MCP approval mode: {error}");
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Tool Approval Settings".to_string(),
                message: "Failed to persist MCP approval mode".to_string(),
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        }

        Self::emit_tool_approval_policy_snapshot(app_settings_service, view_tx).await;
    }

    pub(super) async fn on_add_tool_approval_allowlist_prefix(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        prefix: String,
    ) {
        let mut policy = match ToolApprovalPolicy::load_from_settings(app_settings_service.as_ref())
            .await
        {
            Ok(policy) => policy,
            Err(error) => {
                tracing::warn!("Failed to load tool approval policy for allowlist add: {error}");
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Tool Approval Settings".to_string(),
                    message: "Failed to update allowlist".to_string(),
                    severity: super::view_command::ErrorSeverity::Warning,
                });
                return;
            }
        };

        if let Err(error) = policy
            .allow_persistently(prefix.trim().to_string(), app_settings_service.as_ref())
            .await
        {
            tracing::warn!("Failed to persist allowlist entry: {error}");
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Tool Approval Settings".to_string(),
                message: "Failed to persist allowlist entry".to_string(),
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        }

        Self::emit_tool_approval_policy_snapshot(app_settings_service, view_tx).await;
    }

    pub(super) async fn on_remove_tool_approval_allowlist_prefix(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        prefix: String,
    ) {
        let mut policy =
            match ToolApprovalPolicy::load_from_settings(app_settings_service.as_ref()).await {
                Ok(policy) => policy,
                Err(error) => {
                    tracing::warn!(
                        "Failed to load tool approval policy for allowlist removal: {error}"
                    );
                    let _ = view_tx.send(ViewCommand::ShowError {
                        title: "Tool Approval Settings".to_string(),
                        message: "Failed to update allowlist".to_string(),
                        severity: super::view_command::ErrorSeverity::Warning,
                    });
                    return;
                }
            };

        if let Err(error) = policy
            .remove_persistent_allow_prefix(prefix.trim(), app_settings_service.as_ref())
            .await
        {
            tracing::warn!("Failed to remove allowlist entry: {error}");
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Tool Approval Settings".to_string(),
                message: "Failed to remove allowlist entry".to_string(),
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        }

        Self::emit_tool_approval_policy_snapshot(app_settings_service, view_tx).await;
    }

    pub(super) async fn on_add_tool_approval_denylist_prefix(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        prefix: String,
    ) {
        let mut policy =
            match ToolApprovalPolicy::load_from_settings(app_settings_service.as_ref()).await {
                Ok(policy) => policy,
                Err(error) => {
                    tracing::warn!("Failed to load tool approval policy for denylist add: {error}");
                    let _ = view_tx.send(ViewCommand::ShowError {
                        title: "Tool Approval Settings".to_string(),
                        message: "Failed to update denylist".to_string(),
                        severity: super::view_command::ErrorSeverity::Warning,
                    });
                    return;
                }
            };

        if let Err(error) = policy
            .deny_persistently(prefix.trim().to_string(), app_settings_service.as_ref())
            .await
        {
            tracing::warn!("Failed to persist denylist entry: {error}");
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Tool Approval Settings".to_string(),
                message: "Failed to persist denylist entry".to_string(),
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        }

        Self::emit_tool_approval_policy_snapshot(app_settings_service, view_tx).await;
    }

    pub(super) async fn on_remove_tool_approval_denylist_prefix(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        prefix: String,
    ) {
        let mut policy = match ToolApprovalPolicy::load_from_settings(app_settings_service.as_ref())
            .await
        {
            Ok(policy) => policy,
            Err(error) => {
                tracing::warn!("Failed to load tool approval policy for denylist removal: {error}");
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Tool Approval Settings".to_string(),
                    message: "Failed to update denylist".to_string(),
                    severity: super::view_command::ErrorSeverity::Warning,
                });
                return;
            }
        };

        if let Err(error) = policy
            .remove_persistent_deny_prefix(prefix.trim(), app_settings_service.as_ref())
            .await
        {
            tracing::warn!("Failed to remove denylist entry: {error}");
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Tool Approval Settings".to_string(),
                message: "Failed to remove denylist entry".to_string(),
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        }

        Self::emit_tool_approval_policy_snapshot(app_settings_service, view_tx).await;
    }
}
