use crate::llm::{Message as LlmMessage, Role};

pub fn strip_thinking_from_previous_turns(messages: &mut [LlmMessage]) {
    let Some(last_user_index) = messages
        .iter()
        .rposition(|message| matches!(message.role, Role::User))
    else {
        return;
    };

    for message in &mut messages[..last_user_index] {
        if matches!(message.role, Role::Assistant) {
            message.thinking_content = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_thinking_before_last_user_message() {
        let mut messages = vec![
            LlmMessage::assistant("older answer").with_thinking("keep no more"),
            LlmMessage::user("follow up"),
            LlmMessage::assistant("active loop").with_thinking("preserve this"),
        ];

        strip_thinking_from_previous_turns(&mut messages);

        assert_eq!(messages[0].thinking_content, None);
        assert_eq!(
            messages[2].thinking_content.as_deref(),
            Some("preserve this")
        );
    }

    #[test]
    fn leaves_messages_unchanged_when_no_user_message_exists() {
        let mut messages = vec![LlmMessage::assistant("answer").with_thinking("reasoning")];

        strip_thinking_from_previous_turns(&mut messages);

        assert_eq!(messages[0].thinking_content.as_deref(), Some("reasoning"));
    }

    #[test]
    fn handles_single_user_message() {
        let mut messages = vec![LlmMessage::user("prompt")];

        strip_thinking_from_previous_turns(&mut messages);

        assert_eq!(messages.len(), 1);
    }
}
