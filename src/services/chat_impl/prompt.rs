//! System-prompt assembly for a chat send.
//!
//! The prompt the model sees is the conversation's own system message (falling back to
//! the profile's), with template variables expanded, followed by the enabled-skills
//! block and the emoji instruction.

use std::sync::Arc;

use crate::models::{Conversation, MessageRole, ModelProfile};
use crate::services::template::{build_skills_prompt_block, expand_system_prompt, TemplateContext};
use crate::services::{AppSettingsService, SkillsService};

/// Build the system prompt for a send.
pub(super) async fn build_system_prompt(
    skills_service: &Arc<dyn SkillsService>,
    conversation: &Conversation,
    profile: &ModelProfile,
    filter_emoji: bool,
) -> String {
    let raw_system_prompt = system_prompt_for_conversation(conversation, profile).to_string();
    let template_ctx =
        TemplateContext::new(conversation.created_at, &profile.name, &profile.model_id);
    let mut system_prompt = expand_system_prompt(&raw_system_prompt, &template_ctx);

    let enabled_skills = match skills_service.get_enabled_skills().await {
        Ok(skills) => skills,
        Err(error) => {
            tracing::warn!(
                error = %error,
                "Failed to fetch enabled skills; continuing without skills prompt block"
            );
            Vec::new()
        }
    };
    append_prompt_section(
        &mut system_prompt,
        &build_skills_prompt_block(&enabled_skills),
    );

    if filter_emoji {
        append_prompt_section(
            &mut system_prompt,
            "Please avoid using emojis in your responses.",
        );
    }

    system_prompt
}

/// The conversation's own system message, or the profile's prompt when it has none.
pub(super) fn system_prompt_for_conversation<'a>(
    conversation: &'a Conversation,
    profile: &'a ModelProfile,
) -> &'a str {
    conversation
        .messages
        .iter()
        .find(|message| message.role == MessageRole::System && !message.content.trim().is_empty())
        .map(|message| message.content.as_str())
        .filter(|prompt| !prompt.trim().is_empty())
        .unwrap_or(profile.system_prompt.as_str())
}

/// Whether emoji filtering is enabled, defaulting to disabled when unreadable.
///
/// Read once per send and shared, so the system prompt and tool-output filtering cannot
/// disagree about the setting.
pub(super) async fn filter_emoji_setting(
    app_settings_service: &Arc<dyn AppSettingsService>,
) -> bool {
    match app_settings_service.get_filter_emoji().await {
        Ok(Some(enabled)) => enabled,
        Ok(None) => false,
        Err(error) => {
            tracing::warn!(
                error = %error,
                "Failed to read emoji filter setting; defaulting to disabled"
            );
            false
        }
    }
}

/// Append `section` to `system_prompt`, separated by a blank line when both are present.
fn append_prompt_section(system_prompt: &mut String, section: &str) {
    if section.is_empty() {
        return;
    }
    if !system_prompt.trim().is_empty() {
        system_prompt.push_str("\n\n");
    }
    system_prompt.push_str(section);
}
