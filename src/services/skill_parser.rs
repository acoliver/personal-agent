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

pub(crate) fn parse_skill_content(raw: &str) -> ServiceResult<(SkillMetadata, String)> {
    // Normalize line endings to LF for consistent parsing (handles CRLF on Windows)
    let normalized = raw.replace("\r\n", "\n");

    let Some(rest) = normalized.strip_prefix("---\n") else {
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

    #[test]
    fn parse_skill_content_rejects_missing_closing_delimiter() {
        let raw = "---\nname: example\ndescription: Example skill\nNo closing delimiter";
        let error = parse_skill_content(raw).expect_err("missing closing delimiter should fail");
        assert!(error
            .to_string()
            .contains("must include a closing --- frontmatter delimiter"));
    }

    #[test]
    fn parse_skill_content_rejects_empty_name() {
        let raw = "---\nname: ''\ndescription: Has description\n---\nBody\n";
        let error = parse_skill_content(raw).expect_err("empty name should fail");
        assert!(error.to_string().contains("non-empty name"));
    }

    #[test]
    fn parse_skill_content_rejects_whitespace_only_name() {
        let raw = "---\nname: '   '\ndescription: Has description\n---\nBody\n";
        let error = parse_skill_content(raw).expect_err("whitespace-only name should fail");
        assert!(error.to_string().contains("non-empty name"));
    }

    #[test]
    fn parse_skill_content_rejects_empty_description() {
        let raw = "---\nname: has-name\ndescription: ''\n---\nBody\n";
        let error = parse_skill_content(raw).expect_err("empty description should fail");
        assert!(error.to_string().contains("non-empty description"));
    }

    #[test]
    fn parse_skill_content_rejects_whitespace_only_description() {
        let raw = "---\nname: has-name\ndescription: '   '\n---\nBody\n";
        let error = parse_skill_content(raw).expect_err("whitespace-only description should fail");
        assert!(error.to_string().contains("non-empty description"));
    }

    #[test]
    fn parse_skill_content_rejects_invalid_yaml() {
        let raw = "---\nname: [unclosed\n---\nBody\n";
        let error = parse_skill_content(raw).expect_err("invalid YAML should fail");
        assert!(error.to_string().contains("Invalid skill frontmatter YAML"));
    }

    #[test]
    fn parse_skill_content_allows_optional_metadata() {
        let raw = "---\nname: skill\ndescription: desc\nadditional: value\n---\nBody\n";
        // This should succeed - metadata is flexible and additional fields are ignored
        let result = parse_skill_content(raw);
        assert!(result.is_ok(), "optional metadata fields should be allowed");
    }

    #[test]
    fn parse_skill_content_preserves_body_whitespace() {
        let raw = "---\nname: skill\ndescription: desc\n---\n\n  Indented\n\nTrailing\n\n";
        let (metadata, body) = parse_skill_content(raw).expect("should parse");
        assert_eq!(metadata.name, "skill");
        assert_eq!(body, "\n  Indented\n\nTrailing\n\n");
    }
}
