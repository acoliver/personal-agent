use crate::models::{Message, MessageRole};

#[must_use]
pub fn find_tool_pairs(messages: &[Message]) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();

    for (index, message) in messages.iter().enumerate() {
        if message.role != MessageRole::Assistant || message.tool_calls.is_none() {
            continue;
        }

        let Some(next_message) = messages.get(index + 1) else {
            continue;
        };

        if next_message.role == MessageRole::User && next_message.tool_results.is_some() {
            pairs.push((index, index + 1));
        }
    }

    pairs
}

#[must_use]
pub fn is_safe_split_point(
    _messages: &[Message],
    index: usize,
    tool_pairs: &[(usize, usize)],
) -> bool {
    !tool_pairs
        .iter()
        .any(|(assistant_index, user_index)| *assistant_index < index && index <= *user_index)
}

#[must_use]
pub fn find_nearest_safe_split(
    messages: &[Message],
    target_index: usize,
    tool_pairs: &[(usize, usize)],
) -> usize {
    if is_safe_split_point(messages, target_index, tool_pairs) {
        return target_index;
    }

    for distance in 1..=messages.len() {
        if let Some(left_index) = target_index.checked_sub(distance) {
            if is_safe_split_point(messages, left_index, tool_pairs) {
                return left_index;
            }
        }

        let right_index = target_index + distance;
        if right_index <= messages.len() && is_safe_split_point(messages, right_index, tool_pairs) {
            return right_index;
        }
    }

    target_index.min(messages.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Message;

    #[test]
    fn finds_adjacent_tool_pairs() {
        let mut assistant = Message::assistant("tool call".to_string());
        assistant.tool_calls = Some("[{\"id\":\"tool-1\"}]".to_string());

        let mut user = Message::user("tool result".to_string());
        user.tool_results = Some("[{\"tool_use_id\":\"tool-1\",\"content\":\"ok\"}]".to_string());

        let messages = vec![assistant, user];

        assert_eq!(find_tool_pairs(&messages), vec![(0, 1)]);
    }

    #[test]
    fn split_point_inside_pair_is_not_safe() {
        let mut assistant = Message::assistant("tool call".to_string());
        assistant.tool_calls = Some("[]".to_string());
        let mut user = Message::user("tool result".to_string());
        user.tool_results = Some("[]".to_string());
        let messages = vec![assistant, user];
        let pairs = find_tool_pairs(&messages);

        assert!(!is_safe_split_point(&messages, 1, &pairs));
        assert_eq!(find_nearest_safe_split(&messages, 1, &pairs), 0);
    }
}
