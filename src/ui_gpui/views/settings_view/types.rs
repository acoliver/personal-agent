//! Types used by the settings view.
//!
//! This module contains the data structures used to represent
//! items and options in the settings UI.

use uuid::Uuid;

use crate::models::SkillSource;
use crate::presentation::view_command::SkillSummary;

/// Represents a profile in the settings list
/// @plan PLAN-20250130-GPUIREDUX.P06
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileItem {
    pub id: Uuid,
    pub name: String,
    pub provider: String,
    pub model: String,
    pub is_default: bool,
}

impl ProfileItem {
    #[must_use]
    pub fn new(id: Uuid, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            provider: String::new(),
            model: String::new(),
            is_default: false,
        }
    }

    #[must_use]
    pub fn with_model(mut self, provider: impl Into<String>, model: impl Into<String>) -> Self {
        self.provider = provider.into();
        self.model = model.into();
        self
    }

    #[must_use]
    pub const fn with_default(mut self, is_default: bool) -> Self {
        self.is_default = is_default;
        self
    }

    #[must_use]
    pub fn display_text(&self) -> String {
        if self.provider.is_empty() && self.model.is_empty() {
            self.name.clone()
        } else {
            format!("{} ({}:{})", self.name, self.provider, self.model)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillItem {
    pub name: String,
    pub description: String,
    pub source: SkillSource,
    pub enabled: bool,
    pub path: String,
}

impl From<SkillSummary> for SkillItem {
    fn from(value: SkillSummary) -> Self {
        Self {
            name: value.name,
            description: value.description,
            source: value.source,
            enabled: value.enabled,
            path: value.path,
        }
    }
}

/// Status of an MCP server in the settings view
/// @plan PLAN-20250130-GPUIREDUX.P06
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum McpStatus {
    Running,
    #[default]
    Stopped,
    Error,
}

impl McpStatus {
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Stopped => "Stopped",
            Self::Error => "Error",
        }
    }
}

/// An MCP server item in the settings list
/// @plan PLAN-20250130-GPUIREDUX.P06
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpItem {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub status: McpStatus,
}

impl McpItem {
    #[must_use]
    pub fn new(id: Uuid, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            enabled: false,
            status: McpStatus::Stopped,
        }
    }

    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self.status = if enabled {
            McpStatus::Running
        } else {
            McpStatus::Stopped
        };
        self
    }

    #[must_use]
    pub fn with_status(mut self, status: McpStatus) -> Self {
        self.status = status;
        if status == McpStatus::Error {
            self.enabled = false;
        }
        self
    }
}

/// A theme option as presented in the settings dropdown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeOption {
    pub name: String,
    pub slug: String,
}

/// Which font dropdown is currently open in the Appearance panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontDropdownTarget {
    UiFont,
    MonoFont,
}

/// Categories shown in the settings sidebar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsCategory {
    #[default]
    General,
    Appearance,
    Models,
    Skills,
    Security,
    McpTools,
    Backup,
}

impl SettingsCategory {
    pub const ALL: [Self; 7] = [
        Self::General,
        Self::Appearance,
        Self::Models,
        Self::Skills,
        Self::Security,
        Self::McpTools,
        Self::Backup,
    ];

    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
            Self::Models => "Models",
            Self::Skills => "Skills",
            Self::Security => "Security",
            Self::McpTools => "MCP Tools",
            Self::Backup => "Backup",
        }
    }
}
