use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::models::{Conversation, ConversationExportFormat, MessageRole};

pub const EXPORT_FORMAT_SETTING_KEY: &str = "chat.export.format";
pub const EXPORT_DIR_SETTING_KEY: &str = "chat.export.dir";

#[must_use]
pub fn sanitize_filename_component(input: &str) -> String {
    let mut result = String::new();
    let mut previous_was_separator = false;

    for ch in input.trim().chars() {
        let valid = ch.is_ascii_alphanumeric() || ch == '-' || ch == '_';
        if valid {
            result.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            result.push('-');
            previous_was_separator = true;
        }
    }

    let trimmed = result.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "conversation".to_string()
    } else {
        trimmed
    }
}

#[must_use]
pub fn build_export_filename(
    conversation: &Conversation,
    format: ConversationExportFormat,
) -> String {
    let timestamp = conversation.updated_at.format("%Y%m%d-%H%M%S");
    let title = conversation
        .title
        .as_deref()
        .map_or_else(|| "conversation".to_string(), sanitize_filename_component);
    format!("{timestamp}-{title}.{}", format.extension())
}

#[must_use]
pub fn resolve_export_directory(configured_dir: Option<&str>) -> PathBuf {
    if let Some(value) = configured_dir {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    dirs::download_dir()
        .or_else(dirs::document_dir)
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn render_export_content(
    conversation: &Conversation,
    format: ConversationExportFormat,
) -> Result<String, String> {
    match format {
        ConversationExportFormat::Json => serde_json::to_string_pretty(conversation)
            .map_err(|error| format!("failed to serialize conversation JSON: {error}")),
        ConversationExportFormat::Txt => Ok(render_txt(conversation)),
        ConversationExportFormat::Md => Ok(render_markdown(conversation)),
    }
}

fn render_txt(conversation: &Conversation) -> String {
    let title = conversation
        .title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Untitled Conversation");

    let mut output = String::new();
    let _ = writeln!(output, "Conversation: {title}");
    let _ = writeln!(output, "ID: {}", conversation.id);
    let _ = writeln!(
        output,
        "Updated: {}\n",
        conversation.updated_at.format("%Y-%m-%d %H:%M:%S UTC")
    );

    for message in &conversation.messages {
        let _ = writeln!(
            output,
            "[{}] {}",
            message.timestamp.format("%Y-%m-%d %H:%M:%S"),
            role_label(&message.role)
        );
        output.push_str(message.content.trim_end());
        output.push('\n');
        if let Some(thinking) = message.thinking_content.as_deref() {
            let thinking = thinking.trim();
            if !thinking.is_empty() {
                output.push_str("Thinking:\n");
                output.push_str(thinking);
                output.push('\n');
            }
        }
        output.push('\n');
    }

    output
}

fn render_markdown(conversation: &Conversation) -> String {
    let title = conversation
        .title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Untitled Conversation");

    let mut output = String::new();
    let _ = writeln!(output, "# {title}\n");
    let _ = writeln!(output, "- Conversation ID: `{}`", conversation.id);
    let _ = writeln!(
        output,
        "- Updated: {}\n",
        conversation.updated_at.format("%Y-%m-%d %H:%M:%S UTC")
    );

    for message in &conversation.messages {
        let _ = writeln!(
            output,
            "## {} ({})\n",
            role_label(&message.role),
            message.timestamp.format("%Y-%m-%d %H:%M:%S")
        );
        output.push_str(message.content.trim_end());
        output.push_str("\n\n");
        if let Some(thinking) = message.thinking_content.as_deref() {
            let thinking = thinking.trim();
            if !thinking.is_empty() {
                output.push_str("### Thinking\n\n");
                output.push_str("```text\n");
                output.push_str(thinking);
                output.push_str("\n```\n\n");
            }
        }
    }

    output
}

const fn role_label(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::System => "System",
        MessageRole::User => "User",
        MessageRole::Assistant => "Assistant",
    }
}

#[must_use]
pub fn resolve_unique_export_path(directory: &Path, filename: &str) -> PathBuf {
    let initial = directory.join(filename);
    if !initial.exists() {
        return initial;
    }

    let stem = Path::new(filename)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("conversation")
        .to_string();
    let extension = Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_string);

    for index in 1..1000 {
        let candidate = extension.as_deref().map_or_else(
            || directory.join(format!("{stem}-{index}")),
            |ext| directory.join(format!("{stem}-{index}.{ext}")),
        );

        if !candidate.exists() {
            return candidate;
        }
    }

    directory.join(filename)
}

pub fn write_export_file(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create export directory {}: {error}",
                parent.display()
            )
        })?;
    }

    std::fs::write(path, content)
        .map_err(|error| format!("failed to write export file {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Conversation, Message};
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn fixture_conversation() -> Conversation {
        let profile_id = Uuid::new_v4();
        let mut conversation = Conversation::new(profile_id);
        conversation.id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        conversation.title = Some("Sprint Planning / Q1".to_string());
        conversation.updated_at = Utc.with_ymd_and_hms(2026, 1, 9, 11, 8, 7).unwrap();

        let mut user = Message::user("What should we ship this sprint?".to_string());
        user.timestamp = Utc.with_ymd_and_hms(2026, 1, 9, 11, 0, 0).unwrap();

        let mut assistant = Message::assistant_with_thinking(
            "Prioritize login reliability and onboarding.".to_string(),
            "Weight impact against implementation risk.".to_string(),
        );
        assistant.timestamp = Utc.with_ymd_and_hms(2026, 1, 9, 11, 1, 0).unwrap();

        conversation.messages = vec![user, assistant];
        conversation
    }

    #[test]
    fn filename_sanitizes_title_and_uses_format_extension() {
        let conversation = fixture_conversation();
        let filename = build_export_filename(&conversation, ConversationExportFormat::Md);
        assert_eq!(filename, "20260109-110807-sprint-planning-q1.md");
    }

    #[test]
    fn txt_render_contains_roles_and_thinking_block() {
        let content = render_export_content(&fixture_conversation(), ConversationExportFormat::Txt)
            .expect("txt render should succeed");

        assert!(content.contains("Conversation: Sprint Planning / Q1"));
        assert!(content.contains("User"));
        assert!(content.contains("Assistant"));
        assert!(content.contains("Thinking:"));
    }

    #[test]
    fn markdown_render_contains_heading_and_code_block() {
        let content = render_export_content(&fixture_conversation(), ConversationExportFormat::Md)
            .expect("md render should succeed");

        assert!(content.contains("# Sprint Planning / Q1"));
        assert!(content.contains("## Assistant"));
        assert!(content.contains("```text"));
    }

    #[test]
    fn json_render_serializes_messages() {
        let content =
            render_export_content(&fixture_conversation(), ConversationExportFormat::Json)
                .expect("json render should succeed");

        assert!(content.contains("\"messages\""));
        assert!(content.contains("\"assistant\""));
    }

    #[test]
    fn resolve_export_directory_prefers_non_empty_setting() {
        let configured = resolve_export_directory(Some("/tmp/exports"));
        assert_eq!(configured, PathBuf::from("/tmp/exports"));
    }

    #[test]
    fn resolve_unique_export_path_appends_suffix_when_target_exists() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let first = temp_dir.path().join("20260109-110807-sprint.md");
        std::fs::write(&first, "already-here").expect("seed existing export");

        let resolved = resolve_unique_export_path(temp_dir.path(), "20260109-110807-sprint.md");
        assert_eq!(
            resolved,
            temp_dir.path().join("20260109-110807-sprint-1.md")
        );
    }
}
