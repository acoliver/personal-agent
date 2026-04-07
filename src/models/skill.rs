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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_new_generates_stable_uuid() {
        let skill1 = Skill::new(
            "test".to_string(),
            "Test skill".to_string(),
            std::path::PathBuf::from("/skills/test"),
            SkillSource::Bundled,
            true,
        );
        let skill2 = Skill::new(
            "test".to_string(),
            "Test skill".to_string(),
            std::path::PathBuf::from("/skills/test"),
            SkillSource::Bundled,
            true,
        );
        // Same inputs should produce same UUID (deterministic)
        assert_eq!(skill1.id, skill2.id);
    }

    #[test]
    fn skill_new_different_paths_produce_different_ids() {
        let skill1 = Skill::new(
            "test".to_string(),
            "Test skill".to_string(),
            std::path::PathBuf::from("/skills/test1"),
            SkillSource::Bundled,
            true,
        );
        let skill2 = Skill::new(
            "test".to_string(),
            "Test skill".to_string(),
            std::path::PathBuf::from("/skills/test2"),
            SkillSource::Bundled,
            true,
        );
        assert_ne!(skill1.id, skill2.id);
    }

    #[test]
    fn skill_new_different_sources_produce_different_ids() {
        let skill1 = Skill::new(
            "test".to_string(),
            "Test skill".to_string(),
            std::path::PathBuf::from("/skills/test"),
            SkillSource::Bundled,
            true,
        );
        let skill2 = Skill::new(
            "test".to_string(),
            "Test skill".to_string(),
            std::path::PathBuf::from("/skills/test"),
            SkillSource::User,
            true,
        );
        assert_ne!(skill1.id, skill2.id);
    }

    #[test]
    fn skill_source_as_str() {
        assert_eq!(SkillSource::Bundled.as_str(), "bundled");
        assert_eq!(SkillSource::User.as_str(), "user");
    }

    #[test]
    fn skill_metadata_defaults_empty_map() {
        let yaml = "name: test\ndescription: desc\n";
        let meta: SkillMetadata = serde_yaml::from_str(yaml).expect("parse");
        assert!(meta.metadata.is_empty());
    }

    #[test]
    fn skill_metadata_preserves_extra_fields() {
        let yaml = "name: test\ndescription: desc\nmetadata:\n  key: value\n";
        let meta: SkillMetadata = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(meta.metadata.get("key"), Some(&"value".to_string()));
    }
}
