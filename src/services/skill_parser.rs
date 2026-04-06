use std::path::Path;

use crate::models::SkillMetadata;

use super::{ServiceError, ServiceResult};

/// Parse a `SKILL.md` file into YAML frontmatter metadata and markdown body.
///
/// # Errors
///
/// Returns `ServiceError::Validation` when the file is missing required
/// frontmatter structure or contains invalid metadata, and `ServiceError::Io`
/// when the file cannot be read.
pub fn parse_skill_file(path: &Path) -> ServiceResult<(SkillMetadata, String)> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| ServiceError::Io(format!("Failed to read {}: {error}", path.display())))?;

    parse_skill_content(&raw)
}

fn parse_skill_content(raw: &str) -> ServiceResult<(SkillMetadata, String)> {
    let Some(rest) = raw.strip_prefix("---\n") else {
        return Err(ServiceError::Validation(
            "Skill file must begin with YAML frontmatter delimited by ---".to_string(),
        ));
    };

    let Some((frontmatter, body)) = rest.split_once("\n---\n") else {
        return Err(ServiceError::Validation(
            "Skill file must include a closing --- frontmatter delimiter".to_string(),
        ));
    };

    let metadata: SkillMetadata = serde_yaml::from_str(frontmatter).map_err(|error| {
        ServiceError::Validation(format!("Invalid skill frontmatter YAML: {error}"))
    })?;

    if metadata.name.trim().is_empty() {
        return Err(ServiceError::Validation(
            "Skill frontmatter must include a non-empty name".to_string(),
        ));
    }

    if metadata.description.trim().is_empty() {
        return Err(ServiceError::Validation(
            "Skill frontmatter must include a non-empty description".to_string(),
        ));
    }

    Ok((metadata, body.to_string()))
}

#[cfg(test)]
mod tests {
    use super::parse_skill_content;

    #[test]
    fn parse_skill_content_extracts_frontmatter_and_body() {
        let raw = "---\nname: example\ndescription: Example skill\nmetadata:\n  source: test\n---\n# Example\nUse this skill.\n";

        let (metadata, body) = parse_skill_content(raw).expect("skill content should parse");

        assert_eq!(metadata.name, "example");
        assert_eq!(metadata.description, "Example skill");
        assert_eq!(metadata.metadata.get("source"), Some(&"test".to_string()));
        assert_eq!(body, "# Example\nUse this skill.\n");
    }

    #[test]
    fn parse_skill_content_rejects_missing_opening_delimiter() {
        let error = parse_skill_content("name: example\nbody")
            .expect_err("content without frontmatter should fail");
        assert!(error
            .to_string()
            .contains("must begin with YAML frontmatter"));
    }
}
