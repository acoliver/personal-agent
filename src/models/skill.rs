use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// A discovered skill available to the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub source: SkillSource,
    pub enabled: bool,
}

impl Skill {
    #[must_use]
    pub fn new(
        name: String,
        description: String,
        path: PathBuf,
        source: SkillSource,
        enabled: bool,
    ) -> Self {
        let id = Uuid::new_v5(
            &Uuid::NAMESPACE_URL,
            format!("skill:{}:{}", source.as_str(), path.to_string_lossy()).as_bytes(),
        );

        Self {
            id,
            name,
            description,
            path,
            source,
            enabled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSource {
    Bundled,
    User,
}

impl SkillSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::User => "user",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}
