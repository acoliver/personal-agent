//! Parser for the XML-flavoured tool-call dialect Granite emits.
//!
//! Granite does not emit JSON tool calls. It emits blocks shaped like this:
//!
//! ```text
//! <tool_call>
//! <function=next_secure_prime>
//! <parameter=after>
//! 1000000
//! </parameter>
//! </function>
//! </tool_call>
//! ```
//!
//! The chat template explicitly allows natural-language reasoning before the
//! first `<tool_call>` block, and allows more than one block per turn. Parameter
//! values may span multiple lines.

/// Opening marker of a tool-call block.
pub const TOOL_CALL_OPEN: &str = "<tool_call>";
/// Closing marker of a tool-call block.
pub const TOOL_CALL_CLOSE: &str = "</tool_call>";

const FUNCTION_OPEN: &str = "<function=";
const FUNCTION_CLOSE: &str = "</function>";
const PARAMETER_OPEN: &str = "<parameter=";
const PARAMETER_CLOSE: &str = "</parameter>";
const THINK_OPEN: &str = "<think>";
const THINK_CLOSE: &str = "</think>";

/// Control tokens that must never reach the user-visible transcript.
const CONTROL_TOKENS: [&str; 5] = [
    "<|im_end|>",
    "<|im_start|>",
    "<|end_of_text|>",
    "<|start_of_role|>",
    "<|end_of_role|>",
];

/// A tool call exactly as the model wrote it, with argument values still raw
/// strings. Type coercion happens later against the tool schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawToolCall {
    /// Function name from `<function=NAME>`.
    pub name: String,
    /// Ordered `(parameter name, raw value)` pairs.
    pub arguments: Vec<(String, String)>,
}

impl RawToolCall {
    /// Looks up a raw argument value by parameter name.
    ///
    /// Part of the parser's public surface for callers that need single-value
    /// lookup; the agent loop itself iterates all arguments, so this is
    /// exercised by tests only.
    #[allow(dead_code)]
    #[must_use]
    pub fn argument(&self, name: &str) -> Option<&str> {
        self.arguments
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// One decoded assistant turn: the prose the model wrote before its first tool
/// call, plus every tool call it made.
///
/// When `calls` is empty the model answered directly and `rationale` holds that
/// answer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedResponse {
    /// Text the model produced before the first `<tool_call>` block, trimmed.
    pub rationale: String,
    /// Every tool call in the turn, in the order they appeared.
    pub calls: Vec<RawToolCall>,
}

/// Failures that mean the model produced something the parser cannot read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ToolCallParseError {
    /// A `<tool_call>` block was opened but never closed.
    #[error("`<tool_call>` block was opened but never closed")]
    UnterminatedToolCall,
    /// A tool-call block contained no `<function=...>` element.
    #[error("tool call block contains no `<function=...>` element")]
    MissingFunction,
    /// A `<function=` element is missing its closing `>`.
    #[error("`<function=` element is missing its closing `>`")]
    MalformedFunctionHeader,
    /// A `<function=NAME>` element was never closed.
    #[error("`<function={0}>` was never closed")]
    UnterminatedFunction(String),
    /// A `<parameter=` element is missing its closing `>`.
    #[error("`<parameter=` element is missing its closing `>`")]
    MalformedParameterHeader,
    /// A `<parameter=NAME>` element was never closed.
    #[error("parameter `{0}` was never closed")]
    UnterminatedParameter(String),
}

/// Removes the control tokens the sampler emitted with `special = true` and any
/// `<think>...</think>` span, leaving text that is safe to show a human and to
/// feed to [`parse_response`].
#[must_use]
pub fn strip_control_tokens(text: &str) -> String {
    let mut out = strip_think_blocks(text);
    for token in CONTROL_TOKENS {
        out = out.replace(token, "");
    }
    out
}

/// Removes every `<think>...</think>` span, including an unterminated trailing one.
fn strip_think_blocks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find(THINK_OPEN) {
        out.push_str(&rest[..open]);
        let after_open = &rest[open + THINK_OPEN.len()..];
        match after_open.find(THINK_CLOSE) {
            Some(close) => rest = &after_open[close + THINK_CLOSE.len()..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Parses one assistant turn into its rationale and tool calls.
///
/// # Errors
///
/// Returns [`ToolCallParseError`] when a tool-call block is present but
/// structurally broken. Text with no tool-call block at all is not an error; it
/// is returned as the rationale with an empty call list.
pub fn parse_response(text: &str) -> Result<ParsedResponse, ToolCallParseError> {
    let cleaned = strip_control_tokens(text);
    let mut calls = Vec::new();
    let mut rationale: Option<String> = None;
    let mut cursor = 0usize;

    while let Some(offset) = cleaned[cursor..].find(TOOL_CALL_OPEN) {
        let start = cursor + offset;
        if rationale.is_none() {
            rationale = Some(cleaned[..start].trim().to_string());
        }
        let body_start = start + TOOL_CALL_OPEN.len();
        let body_len = cleaned[body_start..]
            .find(TOOL_CALL_CLOSE)
            .ok_or(ToolCallParseError::UnterminatedToolCall)?;
        let body_end = body_start + body_len;
        calls.push(parse_function(&cleaned[body_start..body_end])?);
        cursor = body_end + TOOL_CALL_CLOSE.len();
    }

    Ok(ParsedResponse {
        rationale: rationale.unwrap_or_else(|| cleaned.trim().to_string()),
        calls,
    })
}

/// Parses the inside of one `<tool_call>` block.
fn parse_function(body: &str) -> Result<RawToolCall, ToolCallParseError> {
    let open = body
        .find(FUNCTION_OPEN)
        .ok_or(ToolCallParseError::MissingFunction)?;
    let name_start = open + FUNCTION_OPEN.len();
    let name_len = body[name_start..]
        .find('>')
        .ok_or(ToolCallParseError::MalformedFunctionHeader)?;
    let header = &body[name_start..name_start + name_len];
    // A `<function=NAME>` header is a single line. If a newline appears before
    // the `>`, the closing bracket is missing and the `>` found here belongs to
    // some later element such as `</function>`, so treat it as malformed.
    if header.contains('\n') {
        return Err(ToolCallParseError::MalformedFunctionHeader);
    }
    let name = header.trim().to_string();

    let inner_start = name_start + name_len + 1;
    let inner_len = body[inner_start..]
        .find(FUNCTION_CLOSE)
        .ok_or_else(|| ToolCallParseError::UnterminatedFunction(name.clone()))?;
    let arguments = parse_parameters(&body[inner_start..inner_start + inner_len])?;

    Ok(RawToolCall { name, arguments })
}

/// Parses every `<parameter=KEY>VALUE</parameter>` pair inside a function block.
fn parse_parameters(inner: &str) -> Result<Vec<(String, String)>, ToolCallParseError> {
    let mut arguments = Vec::new();
    let mut cursor = 0usize;

    while let Some(offset) = inner[cursor..].find(PARAMETER_OPEN) {
        let key_start = cursor + offset + PARAMETER_OPEN.len();
        let key_len = inner[key_start..]
            .find('>')
            .ok_or(ToolCallParseError::MalformedParameterHeader)?;
        let key = inner[key_start..key_start + key_len].trim().to_string();

        let value_start = key_start + key_len + 1;
        let value_len = inner[value_start..]
            .find(PARAMETER_CLOSE)
            .ok_or_else(|| ToolCallParseError::UnterminatedParameter(key.clone()))?;
        let value = trim_parameter_value(&inner[value_start..value_start + value_len]);

        arguments.push((key, value));
        cursor = value_start + value_len + PARAMETER_CLOSE.len();
    }

    Ok(arguments)
}

/// Strips the single newline the format puts after `>` and before `</parameter>`
/// while leaving newlines inside a multi-line value untouched.
fn trim_parameter_value(raw: &str) -> String {
    let head_trimmed = raw
        .strip_prefix("\r\n")
        .or_else(|| raw.strip_prefix('\n'))
        .unwrap_or(raw);
    let trimmed = head_trimmed
        .strip_suffix("\r\n")
        .or_else(|| head_trimmed.strip_suffix('\n'))
        .unwrap_or(head_trimmed);
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, arguments: &[(&str, &str)]) -> RawToolCall {
        RawToolCall {
            name: name.to_string(),
            arguments: arguments
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
        }
    }

    #[test]
    fn parses_a_single_call() {
        let raw = "<tool_call>\n<function=next_secure_prime>\n<parameter=after>\n1000000\n</parameter>\n</function>\n</tool_call>";
        let parsed = parse_response(raw).expect("well formed call parses");
        assert_eq!(parsed.rationale, "");
        assert_eq!(
            parsed.calls,
            vec![call("next_secure_prime", &[("after", "1000000")])]
        );
    }

    #[test]
    fn parses_two_calls_in_one_turn() {
        let raw = concat!(
            "<tool_call>\n<function=next_secure_prime>\n<parameter=after>\n1000000\n</parameter>\n</function>\n</tool_call>\n",
            "<tool_call>\n<function=web_search>\n<parameter=query>\nwhat is a secure prime\n</parameter>\n</function>\n</tool_call>",
        );
        let parsed = parse_response(raw).expect("two calls parse");
        assert_eq!(
            parsed.calls,
            vec![
                call("next_secure_prime", &[("after", "1000000")]),
                call("web_search", &[("query", "what is a secure prime")]),
            ]
        );
    }

    #[test]
    fn keeps_multi_line_parameter_values_intact() {
        let raw = "<tool_call>\n<function=web_search>\n<parameter=query>\nline one\nline two\n\nline four\n</parameter>\n</function>\n</tool_call>";
        let parsed = parse_response(raw).expect("multi-line value parses");
        assert_eq!(
            parsed.calls[0].argument("query"),
            Some("line one\nline two\n\nline four")
        );
    }

    #[test]
    fn captures_rationale_written_before_the_first_call() {
        let raw = "I need a safe prime above one million, so I will use the tool.\n<tool_call>\n<function=next_secure_prime>\n<parameter=after>\n1000000\n</parameter>\n</function>\n</tool_call>";
        let parsed = parse_response(raw).expect("rationale plus call parses");
        assert_eq!(
            parsed.rationale,
            "I need a safe prime above one million, so I will use the tool."
        );
        assert_eq!(parsed.calls.len(), 1);
    }

    #[test]
    fn plain_answer_becomes_the_rationale_with_no_calls() {
        let parsed = parse_response("  The answer is 1000151.  ").expect("plain text parses");
        assert_eq!(parsed.rationale, "The answer is 1000151.");
        assert!(parsed.calls.is_empty());
    }

    #[test]
    fn parses_a_call_with_several_parameters() {
        let raw = "<tool_call>\n<function=demo>\n<parameter=alpha>\n1\n</parameter>\n<parameter=beta>\ntwo\n</parameter>\n</function>\n</tool_call>";
        let parsed = parse_response(raw).expect("multiple parameters parse");
        assert_eq!(
            parsed.calls,
            vec![call("demo", &[("alpha", "1"), ("beta", "two")])]
        );
    }

    #[test]
    fn parses_a_call_with_no_parameters() {
        let raw = "<tool_call>\n<function=ping>\n</function>\n</tool_call>";
        let parsed = parse_response(raw).expect("parameterless call parses");
        assert_eq!(parsed.calls, vec![call("ping", &[])]);
    }

    #[test]
    fn unterminated_tool_call_is_an_error() {
        let raw =
            "<tool_call>\n<function=web_search>\n<parameter=query>\nhi\n</parameter>\n</function>";
        assert_eq!(
            parse_response(raw),
            Err(ToolCallParseError::UnterminatedToolCall)
        );
    }

    #[test]
    fn tool_call_without_a_function_is_an_error() {
        let raw = "<tool_call>\njust some words\n</tool_call>";
        assert_eq!(
            parse_response(raw),
            Err(ToolCallParseError::MissingFunction)
        );
    }

    #[test]
    fn unterminated_function_is_an_error() {
        let raw =
            "<tool_call>\n<function=web_search>\n<parameter=query>\nhi\n</parameter>\n</tool_call>";
        assert_eq!(
            parse_response(raw),
            Err(ToolCallParseError::UnterminatedFunction(
                "web_search".to_string()
            ))
        );
    }

    #[test]
    fn unterminated_parameter_is_an_error() {
        let raw =
            "<tool_call>\n<function=web_search>\n<parameter=query>\nhi\n</function>\n</tool_call>";
        assert_eq!(
            parse_response(raw),
            Err(ToolCallParseError::UnterminatedParameter(
                "query".to_string()
            ))
        );
    }

    #[test]
    fn malformed_function_header_is_an_error() {
        let raw = "<tool_call>\n<function=web_search\n</function>\n</tool_call>";
        assert_eq!(
            parse_response(raw),
            Err(ToolCallParseError::MalformedFunctionHeader)
        );
    }

    #[test]
    fn malformed_parameter_header_is_an_error() {
        let raw = "<tool_call>\n<function=web_search>\n<parameter=query\n</function>\n</tool_call>";
        assert_eq!(
            parse_response(raw),
            Err(ToolCallParseError::MalformedParameterHeader)
        );
    }

    #[test]
    fn control_tokens_never_reach_the_rationale() {
        let parsed = parse_response("<|im_start|>assistant\nHello<|im_end|>").expect("parses");
        assert_eq!(parsed.rationale, "assistant\nHello");
    }

    #[test]
    fn think_blocks_are_removed() {
        let parsed =
            parse_response("<think>hidden reasoning</think>Visible answer").expect("parses");
        assert_eq!(parsed.rationale, "Visible answer");
    }

    #[test]
    fn empty_prefilled_think_block_is_removed_before_a_call() {
        let raw = "<think></think><tool_call>\n<function=web_search>\n<parameter=query>\nx\n</parameter>\n</function>\n</tool_call>";
        let parsed = parse_response(raw).expect("parses");
        assert_eq!(parsed.rationale, "");
        assert_eq!(parsed.calls.len(), 1);
    }

    #[test]
    fn unterminated_think_block_drops_the_remainder() {
        let parsed = parse_response("visible<think>never closed").expect("parses");
        assert_eq!(parsed.rationale, "visible");
    }
}
