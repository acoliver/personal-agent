use crate::models::{Conversation, Message, MessageRole as ConversationMessageRole};
use crate::presentation::render_export_content;
use chrono::TimeZone;

pub(super) fn build_conversation_export_content(
    conversation_id: uuid::Uuid,
    conversation_title: &str,
    selected_title: Option<&str>,
    selected_profile_id: Option<uuid::Uuid>,
    messages: &[super::state::ChatMessage],
    format: crate::models::ConversationExportFormat,
) -> Result<String, String> {
    let updated_at = messages
        .iter()
        .filter_map(|message| message.timestamp)
        .max()
        .and_then(|timestamp| {
            chrono::Utc
                .timestamp_millis_opt(timestamp.cast_signed())
                .single()
        })
        .unwrap_or_else(chrono::Utc::now);

    let title = selected_title
        .filter(|title| !title.trim().is_empty())
        .unwrap_or(conversation_title)
        .to_string();

    let messages = messages
        .iter()
        .map(|message| {
            let role = match message.role {
                super::state::MessageRole::User => ConversationMessageRole::User,
                super::state::MessageRole::Assistant => ConversationMessageRole::Assistant,
            };

            let mut export_message = match role {
                ConversationMessageRole::User => Message::user(message.content.clone()),
                ConversationMessageRole::Assistant => message.thinking.clone().map_or_else(
                    || Message::assistant(message.content.clone()),
                    |thinking| Message::assistant_with_thinking(message.content.clone(), thinking),
                ),
                ConversationMessageRole::System => {
                    unreachable!("chat view never renders system messages")
                }
            };

            export_message.model_id.clone_from(&message.model_label);
            if let Some(timestamp) = message.timestamp {
                if let Some(parsed) = chrono::Utc
                    .timestamp_millis_opt(timestamp.cast_signed())
                    .single()
                {
                    export_message.timestamp = parsed;
                }
            }
            export_message
        })
        .collect();

    render_export_content(
        &Conversation {
            id: conversation_id,
            created_at: updated_at,
            updated_at,
            title: Some(title),
            profile_id: selected_profile_id.unwrap_or_default(),
            messages,
        },
        format,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_gpui::views::chat_view::state::ChatMessage;
    use uuid::Uuid;

    #[test]
    fn build_conversation_export_content_uses_selected_format_and_transcript() {
        let conversation_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let profile_id = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
        let messages = vec![
            ChatMessage::user("What shipped?").with_timestamp(1_704_067_200_000),
            ChatMessage::assistant("Reliability fixes", "gpt-4o")
                .with_thinking("Prioritized customer pain")
                .with_timestamp(1_704_067_260_000),
        ];

        let content = build_conversation_export_content(
            conversation_id,
            "Sprint Review",
            Some("Sprint Review"),
            Some(profile_id),
            &messages,
            crate::models::ConversationExportFormat::Txt,
        )
        .expect("export content should build");

        assert!(content.contains("Conversation: Sprint Review"));
        assert!(content.contains("What shipped?"));
        assert!(content.contains("Reliability fixes"));
        assert!(content.contains("Thinking:"));
    }

    #[test]
    fn build_conversation_export_content_falls_back_to_view_title_when_selected_title_is_blank() {
        let conversation_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let content = build_conversation_export_content(
            conversation_id,
            "Fallback Title",
            Some("   "),
            None,
            &[],
            crate::models::ConversationExportFormat::Txt,
        )
        .expect("export content should build");

        assert!(content.contains("Conversation: Fallback Title"));
    }
}
