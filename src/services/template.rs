//! Template variable expansion for system prompts
//!
//! This module provides simple template expansion for system prompts, allowing
//! users to include dynamic variables like session date, profile name, etc.
//!
//! Template variables:
//! - `{{session_date}}` - `conversation.created_at` formatted as %Y-%m-%d
//! - `{{session_datetime}}` - `conversation.created_at` formatted as %Y-%m-%dT%H:%M:%SZ
//! - `{{day_of_week}}` - `conversation.created_at` weekday name
//! - `{{profile_name}}` - `profile.name`
//! - `{{model_id}}` - `profile.model_id`
//! - `{{os}}` - `std::env::consts::OS`

use chrono::DateTime;
use chrono::Utc;

use crate::models::Skill;

/// Context for template expansion, sourced from immutable conversation data
/// to ensure determinism for KV cache compatibility.
pub struct TemplateContext<'a> {
    /// Conversation creation timestamp (immutable once created)
    pub created_at: DateTime<Utc>,
    /// Profile name
    pub profile_name: &'a str,
    /// Model ID
    pub model_id: &'a str,
}

impl<'a> TemplateContext<'a> {
    /// Create a new `TemplateContext` from conversation and profile data
    #[must_use]
    pub const fn new(created_at: DateTime<Utc>, profile_name: &'a str, model_id: &'a str) -> Self {
        Self {
            created_at,
            profile_name,
            model_id,
        }
    }
}

/// Expand template variables in a system prompt string.
///
/// Unknown/misspelled variables pass through as literal text — no error,
/// no silent swallowing. The user sees them in LLM output and can spot the mistake.
///
/// # Arguments
///
/// * `template` - The template string with optional {{variable}} placeholders
/// * `ctx` - `TemplateContext` containing the variable values
///
/// # Returns
///
/// The expanded string with all known template variables replaced.
#[must_use]
pub fn expand_system_prompt(template: &str, ctx: &TemplateContext<'_>) -> String {
    let mut result = template.to_string();

    // session_date: %Y-%m-%d
    result = result.replace(
        "{{session_date}}",
        &ctx.created_at.format("%Y-%m-%d").to_string(),
    );

    // session_datetime: %Y-%m-%dT%H:%M:%SZ
    result = result.replace(
        "{{session_datetime}}",
        &ctx.created_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    );

    // day_of_week: full weekday name
    result = result.replace("{{day_of_week}}", &ctx.created_at.format("%A").to_string());

    // profile_name
    result = result.replace("{{profile_name}}", ctx.profile_name);

    // model_id
    result = result.replace("{{model_id}}", ctx.model_id);

    // os: std::env::consts::OS
    result = result.replace("{{os}}", std::env::consts::OS);

    result
}

#[must_use]
pub fn build_skills_prompt_block(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }

    let mut lines = vec![
        "You can activate any of these skills using the activate_skill tool when relevant:"
            .to_string(),
        "<available_skills>".to_string(),
    ];

    for skill in skills {
        lines.push(format!(
            "  <skill name=\"{}\" description=\"{}\" />",
            xml_escape(&skill.name),
            xml_escape(&skill.description)
        ));
    }

    lines.push("</available_skills>".to_string());
    lines.join("\n")
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_context() -> TemplateContext<'static> {
        // Use a fixed timestamp for deterministic tests: 2026-03-30 11:26:48 UTC (Monday)
        let created_at = DateTime::parse_from_rfc3339("2026-03-30T11:26:48Z")
            .unwrap()
            .with_timezone(&Utc);

        TemplateContext {
            created_at,
            profile_name: "My Claude Profile",
            model_id: "claude-sonnet-4-20250514",
        }
    }

    #[test]
    fn test_session_date_expansion() {
        let ctx = fixed_context();
        let result = expand_system_prompt("Today is {{session_date}}.", &ctx);
        assert_eq!(result, "Today is 2026-03-30.");
    }

    #[test]
    fn test_session_datetime_expansion() {
        let ctx = fixed_context();
        let result = expand_system_prompt("Timestamp: {{session_datetime}}", &ctx);
        assert_eq!(result, "Timestamp: 2026-03-30T11:26:48Z");
    }

    #[test]
    fn test_day_of_week_expansion() {
        let ctx = fixed_context();
        let result = expand_system_prompt("It is {{day_of_week}}.", &ctx);
        assert_eq!(result, "It is Monday.");
    }

    #[test]
    fn test_profile_name_expansion() {
        let ctx = fixed_context();
        let result = expand_system_prompt("Profile: {{profile_name}}", &ctx);
        assert_eq!(result, "Profile: My Claude Profile");
    }

    #[test]
    fn test_model_id_expansion() {
        let ctx = fixed_context();
        let result = expand_system_prompt("Model: {{model_id}}", &ctx);
        assert_eq!(result, "Model: claude-sonnet-4-20250514");
    }

    #[test]
    fn test_os_expansion() {
        let ctx = fixed_context();
        let result = expand_system_prompt("OS: {{os}}", &ctx);
        // Result depends on the platform we're running on
        let expected_os = std::env::consts::OS;
        assert_eq!(result, format!("OS: {expected_os}"));
    }

    #[test]
    fn test_all_variables_combined() {
        let ctx = fixed_context();
        let template = "You are {{profile_name}} using {{model_id}} on {{os}}. Today is {{session_date}} ({{day_of_week}}) at {{session_datetime}}.";
        let result = expand_system_prompt(template, &ctx);
        let expected_os = std::env::consts::OS;
        assert_eq!(
            result,
            format!("You are My Claude Profile using claude-sonnet-4-20250514 on {expected_os}. Today is 2026-03-30 (Monday) at 2026-03-30T11:26:48Z.")
        );
    }

    #[test]
    fn test_no_op_on_plain_text() {
        let ctx = fixed_context();
        let template = "You are a helpful assistant.";
        let result = expand_system_prompt(template, &ctx);
        assert_eq!(result, "You are a helpful assistant.");
    }

    #[test]
    fn test_unknown_variables_pass_through() {
        let ctx = fixed_context();
        let template = "Hello {{typo}} and {{unknown_var}}!";
        let result = expand_system_prompt(template, &ctx);
        // Unknown variables pass through unchanged
        assert_eq!(result, "Hello {{typo}} and {{unknown_var}}!");
    }

    #[test]
    fn test_mixed_known_and_unknown_variables() {
        let ctx = fixed_context();
        let template = "Date: {{session_date}}, Unknown: {{not_a_var}}, Model: {{model_id}}";
        let result = expand_system_prompt(template, &ctx);
        assert_eq!(
            result,
            "Date: 2026-03-30, Unknown: {{not_a_var}}, Model: claude-sonnet-4-20250514"
        );
    }

    #[test]
    fn test_determinism_same_input_same_output() {
        let ctx = fixed_context();
        let template =
            "{{session_date}} {{session_datetime}} {{day_of_week}} {{profile_name}} {{model_id}}";

        let result1 = expand_system_prompt(template, &ctx);
        let result2 = expand_system_prompt(template, &ctx);
        let result3 = expand_system_prompt(template, &ctx);

        // All results should be identical
        assert_eq!(result1, result2);
        assert_eq!(result2, result3);
    }

    #[test]
    fn test_empty_template() {
        let ctx = fixed_context();
        let result = expand_system_prompt("", &ctx);
        assert_eq!(result, "");
    }

    #[test]
    fn test_multiple_same_variable() {
        let ctx = fixed_context();
        let template = "{{session_date}} and {{session_date}} again";
        let result = expand_system_prompt(template, &ctx);
        assert_eq!(result, "2026-03-30 and 2026-03-30 again");
    }

    #[test]
    fn test_build_skills_prompt_block_empty_skills() {
        let skills: Vec<Skill> = vec![];
        let result = build_skills_prompt_block(&skills);
        assert!(
            result.is_empty(),
            "empty skills should produce empty string"
        );
    }

    #[test]
    fn test_build_skills_prompt_block_single_skill() {
        use crate::models::SkillSource;
        let skills = vec![Skill::new(
            "docx".to_string(),
            "Word document processing".to_string(),
            std::path::PathBuf::from("/skills/docx"),
            SkillSource::Bundled,
            true,
        )];
        let result = build_skills_prompt_block(&skills);
        assert!(result.contains("activate_skill"));
        assert!(result.contains("<available_skills>"));
        assert!(result.contains("</available_skills>"));
        assert!(result.contains("name=\"docx\""));
        assert!(result.contains("description=\"Word document processing\""));
    }

    #[test]
    fn test_build_skills_prompt_block_multiple_skills() {
        use crate::models::SkillSource;
        let skills = vec![
            Skill::new(
                "alpha".to_string(),
                "First skill".to_string(),
                std::path::PathBuf::from("/skills/alpha"),
                SkillSource::Bundled,
                true,
            ),
            Skill::new(
                "beta".to_string(),
                "Second skill".to_string(),
                std::path::PathBuf::from("/skills/beta"),
                SkillSource::User,
                true,
            ),
        ];
        let result = build_skills_prompt_block(&skills);
        assert!(result.contains("name=\"alpha\""));
        assert!(result.contains("name=\"beta\""));
    }

    #[test]
    fn test_build_skills_prompt_block_escapes_xml_special_chars() {
        use crate::models::SkillSource;
        let skills = vec![Skill::new(
            "test<skill>".to_string(),
            "Has \"quotes\" & 'apostrophes'".to_string(),
            std::path::PathBuf::from("/skills/test"),
            SkillSource::Bundled,
            true,
        )];
        let result = build_skills_prompt_block(&skills);
        // Name should be escaped
        assert!(result.contains("&lt;") && result.contains("&gt;"));
        // Description should escape quotes and ampersand
        assert!(result.contains("&quot;"));
        assert!(result.contains("&amp;"));
        assert!(result.contains("&apos;"));
        // Raw chars should NOT appear
        assert!(!result.contains("<skill>"));
        assert!(!result.contains("\"quotes\""));
    }

    #[test]
    fn test_xml_escape_all_special_chars() {
        let input = "<>&\"'";
        let result = xml_escape(input);
        assert_eq!(result, "&lt;&gt;&amp;&quot;&apos;");
    }

    #[test]
    fn test_xml_escape_no_special_chars() {
        let input = "normal text 123";
        let result = xml_escape(input);
        assert_eq!(result, "normal text 123");
    }

    #[test]
    fn test_default_system_prompt_contains_template_variables() {
        // Verify the default system prompt contains all expected template variables
        use crate::models::profile::DEFAULT_SYSTEM_PROMPT;

        assert!(DEFAULT_SYSTEM_PROMPT.contains("{{session_datetime}}"));
        assert!(DEFAULT_SYSTEM_PROMPT.contains("{{day_of_week}}"));
        assert!(DEFAULT_SYSTEM_PROMPT.contains("{{model_id}}"));
        assert!(DEFAULT_SYSTEM_PROMPT.contains("{{os}}"));
    }

    #[test]
    fn test_default_system_prompt_expansion() {
        // Verify the default system prompt expands correctly with all variables
        use crate::models::profile::DEFAULT_SYSTEM_PROMPT;

        let ctx = fixed_context();
        let expanded = expand_system_prompt(DEFAULT_SYSTEM_PROMPT, &ctx);

        // Should contain expanded values, not template variables
        assert!(!expanded.contains("{{"));
        assert!(expanded.contains("2026-03-30")); // session_date part
        assert!(expanded.contains("Monday")); // day_of_week
        assert!(expanded.contains("claude-sonnet-4-20250514")); // model_id
                                                                // os value depends on platform
        assert!(expanded.contains(std::env::consts::OS));
    }
}
