//! `ViewCommand` routing infrastructure for `MainPanel`.
//!
//! Contains the GPUI action definitions, the `CommandTargets` observable
//! struct, and the `route_view_command` pure dispatch function.
//!
//! Store-managed commands (conversation lifecycle, streaming, thinking,
//! profiles, etc.) are NOT routed through `MainPanel::handle_command`.
//! They flow exclusively through the `AppStore` snapshot subscription.
//!
//! @plan PLAN-20260325-ISSUE11B.P02
//! @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
//! @requirement REQ-WIRE-002

use crate::presentation::view_command::ViewCommand;

// Global navigation actions bound to keyboard shortcuts (registered in main_gpui.rs)
#[allow(clippy::derive_partial_eq_without_eq)]
mod _actions {
    gpui::actions!(
        main_panel,
        [
            NavigateToHistory,
            NavigateToSettings,
            NewConversation,
            NavigateBack
        ]
    );
}
pub use _actions::*;

// ============================================================
// REQ-WIRE-002: ViewCommand routing infrastructure
// These types and function are consumed by the GPUI render loop
// and tested directly in gpui_wiring_command_routing_tests.
// ============================================================

/// Observable state updated by `route_view_command`, used in tests
/// to verify each `ViewCommand` variant reaches its target view.
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
/// @requirement REQ-WIRE-002
/// @pseudocode component-002-main-panel-routing.md lines 089-171
#[derive(Debug, Default)]
pub struct CommandTargets {
    // Chat view counters (ephemeral commands only)
    pub chat_export_format_commands: usize,
    pub chat_export_completed_commands: usize,
    pub chat_notification_commands: usize,
    pub chat_error_commands: usize,
    pub error_log_export_completed_commands: usize,

    // Settings view counters
    pub settings_profile_commands: usize,
    pub settings_theme_commands: usize,
    pub settings_mcp_status_updates: usize,

    // Model selector state
    pub model_selector_results_count: usize,

    // MCP add/configure and settings reaction counters
    pub mcp_config_saved_count: usize,
    pub mcp_deleted_count: usize,
    pub settings_notifications_count: usize,
    pub mcp_error_commands_count: usize,
    pub mcp_registry_results_count: usize,
    pub mcp_configure_draft_loaded_count: usize,

    // Profile prefill state from model selector
    pub profile_prefill_selected_count: usize,

    // Tool approval routing counters
    pub tool_approval_policy_count: usize,
    pub yolo_mode_changed_count: usize,
}

/// Route a single `ViewCommand` to the correct target view state.
///
/// This function forms the core of the `MainPanel` command dispatch matrix
/// (REQ-WIRE-002). Store-managed commands are no longer routed here.
/// Only ephemeral/non-store commands contribute to target counts.
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
/// @requirement REQ-WIRE-002
/// @pseudocode component-002-main-panel-routing.md lines 089-171
pub fn route_view_command(cmd: ViewCommand, targets: &mut CommandTargets) {
    match cmd {
        // ── Chat view (ephemeral only) ──────────────────────────────────
        ViewCommand::ShowConversationExportFormat { .. } => {
            targets.chat_export_format_commands += 1;
        }
        ViewCommand::ExportCompleted { .. } => {
            targets.chat_export_completed_commands += 1;
        }
        ViewCommand::ErrorLogExportCompleted { .. } => {
            targets.error_log_export_completed_commands += 1;
        }
        ViewCommand::ShowNotification { message } => {
            if message.contains("Conversation saved")
                || message.contains("No active conversation to save")
            {
                targets.chat_notification_commands += 1;
            }
            targets.settings_notifications_count += 1;
        }
        ViewCommand::ShowError { title, .. } => {
            if title == "Save Conversation" {
                targets.chat_error_commands += 1;
            }
            targets.mcp_error_commands_count += 1;
        }

        // ── Settings / Profile view ─────────────────────────────────────
        ViewCommand::ShowSettingsTheme { .. } => {
            targets.settings_theme_commands += 1;
        }
        ViewCommand::ProfileCreated { .. }
        | ViewCommand::ProfileUpdated { .. }
        | ViewCommand::ProfileDeleted { .. } => {
            targets.settings_profile_commands += 1;
        }
        ViewCommand::McpStatusChanged { .. }
        | ViewCommand::McpServerStarted { .. }
        | ViewCommand::McpServerFailed { .. } => {
            targets.settings_mcp_status_updates += 1;
        }
        ViewCommand::McpConfigSaved { .. } => {
            targets.mcp_config_saved_count += 1;
        }
        ViewCommand::McpDeleted { .. } => {
            targets.mcp_deleted_count += 1;
        }
        ViewCommand::McpRegistrySearchResults { results } => {
            targets.mcp_registry_results_count += results.len();
        }
        ViewCommand::McpConfigureDraftLoaded { .. } => {
            targets.mcp_configure_draft_loaded_count += 1;
        }

        // ── Model selector ──────────────────────────────────────────────
        ViewCommand::ModelSearchResults { models } => {
            targets.model_selector_results_count += models.len();
        }

        // ── Profile prefill ────────────────────────────────────────────────
        ViewCommand::ModelSelected { .. } | ViewCommand::ProfileEditorLoad { .. } => {
            targets.profile_prefill_selected_count += 1;
        }

        // ── Tool approval ────────────────────────────────────────────────
        ViewCommand::ToolApprovalPolicyUpdated { .. } => {
            targets.tool_approval_policy_count += 1;
        }
        ViewCommand::YoloModeChanged { .. } => {
            targets.yolo_mode_changed_count += 1;
        }

        // All other commands are store-managed, navigation, or ancillary
        _ => {}
    }
}
