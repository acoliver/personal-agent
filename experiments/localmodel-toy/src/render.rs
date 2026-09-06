//! Hand-rendered Granite 4.2 chat format.
//!
//! Granite ships its chat template inside the GGUF, but the toy renders it by
//! hand so the exact byte-level prompt is visible, diffable, and free of any
//! template-engine dependency. The shapes below mirror the embedded template:
//!
//! ```text
//! <|im_start|>system
//! ...tools block...<|im_end|>
//! <|im_start|>user
//! ...<|im_end|>
//! <|im_start|>assistant
//! <think></think>...<|im_end|>
//! ```
//!
//! Thinking is always disabled by emitting the empty `<think></think>` block,
//! and tool results are fed back inside a `user` turn wrapped in
//! `<tool_response>` elements, exactly as the embedded template normalises
//! conversation history.

use crate::toolcall::RawToolCall;
use crate::tools::ToolSpec;

/// Marker that opens every ChatML turn.
pub const IM_START: &str = "<|im_start|>";
/// Marker that closes every ChatML turn.
pub const IM_END: &str = "<|im_end|>";
/// Empty thinking block that turns Granite's extended thinking off.
pub const THINK_DISABLED: &str = "<think></think>";

/// Header and instructions the system turn carries whenever tools are present.
///
/// This is the tool-scaffold text embedded in the Granite 4.2 GGUF chat
/// template, reproduced verbatim: the `# Tools` header, one compact JSON line
/// per tool, and the `<tool_call>` format contract. It begins with the blank
/// line that separates it from the system prose and ends with `</IMPORTANT>`,
/// with no trailing newline so `<|im_end|>` attaches directly.
const TOOLS_SCAFFOLD_HEAD: &str =
    "\n\n# Tools\n\nYou have access to the following functions:\n\n<tools>\n";
const TOOLS_SCAFFOLD_TAIL: &str = "\n</tools>\n\nIf you choose to call a function ONLY reply in the following format with NO suffix:\n\n<tool_call>\n<function=example_function_name>\n<parameter=example_parameter_1>\nvalue_1\n</parameter>\n<parameter=example_parameter_2>\nThis is the value for the second parameter\nthat can span\nmultiple lines\n</parameter>\n</function>\n</tool_call>\n\n<IMPORTANT>\nReminder:\n- Function calls MUST follow the specified format: an inner <function=...></function> block must be nested within <tool_call></tool_call> XML tags\n- Required parameters MUST be specified\n- You may provide optional reasoning for your function call in natural language BEFORE the function call, but NOT after\n- If there is no function call available, answer the question like normal with your current knowledge and do not tell the user about function calls\n</IMPORTANT>";

/// One turn of a rendered conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatTurn {
    /// A system-level instruction.
    System {
        /// Instruction text.
        content: String,
    },
    /// A user message.
    User {
        /// Message text.
        content: String,
    },
    /// An assistant turn that answered without calling tools.
    ///
    /// The current agent loop always stops once the model answers, so this
    /// shape is only constructed by tests; it is kept because the renderer has
    /// to cover every turn shape the template defines.
    #[allow(dead_code)]
    Assistant {
        /// Answer text. Rendered after the empty thinking block.
        content: String,
    },
    /// An assistant turn that issued one or more tool calls.
    AssistantToolCalls {
        /// Optional prose the model wrote before its first call.
        rationale: Option<String>,
        /// The calls, in emission order.
        calls: Vec<RawToolCall>,
    },
    /// Results of a previous assistant tool-call turn, wrapped in a user turn.
    ToolResponses {
        /// One serialised result per tool call, in call order.
        results: Vec<String>,
    },
}

/// Renders a whole conversation into the final prompt string.
///
/// With `add_generation_prompt` the output ends on the open assistant header
/// (`<|im_start|>assistant\n<think></think>`) so the next sampled token is the
/// first token of the assistant reply. Without it the output is a closed
/// transcript suitable for logging or tests.
#[must_use]
pub fn render(turns: &[ChatTurn], tools: &[ToolSpec], add_generation_prompt: bool) -> String {
    let mut out = String::new();
    for turn in turns {
        out.push_str(&render_turn(turn, tools));
    }
    if add_generation_prompt {
        out.push_str(&render_generation_prompt());
    }
    out
}

/// Renders the open assistant header that makes the model start a reply.
#[must_use]
pub fn render_generation_prompt() -> String {
    format!("{IM_START}assistant\n{THINK_DISABLED}")
}

/// Renders one turn including its `<|im_end|>` terminator.
fn render_turn(turn: &ChatTurn, tools: &[ToolSpec]) -> String {
    match turn {
        ChatTurn::System { content } => render_system(content, tools),
        ChatTurn::User { content } => format!("{IM_START}user\n{content}{IM_END}\n"),
        ChatTurn::Assistant { content } => {
            format!("{IM_START}assistant\n{THINK_DISABLED}{content}{IM_END}\n")
        }
        ChatTurn::AssistantToolCalls { rationale, calls } => {
            render_assistant_tool_calls(rationale.as_deref(), calls)
        }
        ChatTurn::ToolResponses { results } => render_tool_responses(results),
    }
}

/// Renders the system turn, appending the tool scaffold when tools exist.
fn render_system(content: &str, tools: &[ToolSpec]) -> String {
    let mut out = format!("{IM_START}system\n{content}");
    if !tools.is_empty() {
        out.push_str(TOOLS_SCAFFOLD_HEAD);
        for (index, tool) in tools.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            out.push_str(&tool.to_compact_json());
        }
        out.push_str(TOOLS_SCAFFOLD_TAIL);
    }
    out.push_str(IM_END);
    out.push('\n');
    out
}

/// Renders an assistant turn that made tool calls.
///
/// The shape matches what [`crate::toolcall::parse_response`] reads back:
/// optional rationale prose, then one `<tool_call>` block per call, each
/// closed with a newline.
fn render_assistant_tool_calls(rationale: Option<&str>, calls: &[RawToolCall]) -> String {
    let mut out = format!("{IM_START}assistant\n{THINK_DISABLED}");
    if let Some(prose) = rationale {
        if !prose.is_empty() {
            out.push_str(prose);
            out.push('\n');
        }
    }
    for call in calls {
        out.push_str(&render_tool_call(call));
    }
    out.push_str(IM_END);
    out.push('\n');
    out
}

/// Renders a single `<tool_call>` block for the conversation history.
fn render_tool_call(call: &RawToolCall) -> String {
    let mut out = format!("<tool_call>\n<function={}>", call.name);
    for (name, value) in &call.arguments {
        out.push_str(&format!("\n<parameter={name}>\n{value}\n</parameter>"));
    }
    out.push_str("\n</function>\n</tool_call>\n");
    out
}

/// Renders tool results as a user turn with one `<tool_response>` per result.
fn render_tool_responses(results: &[String]) -> String {
    let mut out = format!("{IM_START}user\n");
    for result in results {
        out.push_str(&format!("<tool_response>\n{result}\n</tool_response>\n"));
    }
    out.push_str(IM_END);
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{default_tools, WEB_SEARCH_RESULT};

    /// The compact JSON the two default tools render as, spelled out so the
    /// golden tests fail loudly if `ToolSpec` serialisation ever changes.
    const PRIME_TOOL_JSON: &str = r#"{"name":"next_secure_prime","description":"Return the smallest secure prime strictly greater than a lower bound. A secure prime, also called a safe prime, is a prime p for which (p-1)/2 is also prime.","parameters":{"type":"object","properties":{"after":{"type":"integer","description":"Exclusive lower bound. The result is strictly greater than this value."}},"required":["after"]}}"#;
    const SEARCH_TOOL_JSON: &str = r#"{"name":"web_search","description":"Search the web for information.","parameters":{"type":"object","properties":{"query":{"type":"string","description":"The search query."}},"required":["query"]}}"#;

    /// The exact `<tools>` scaffold the golden tests expect between the system
    /// prose and `<|im_end|>`, built from the same literals the renderer uses.
    fn expected_tools_section() -> String {
        format!(
            "{}{}{}{}{}",
            "\n\n# Tools\n\nYou have access to the following functions:\n\n<tools>\n",
            PRIME_TOOL_JSON,
            "\n",
            SEARCH_TOOL_JSON,
            "\n</tools>\n\nIf you choose to call a function ONLY reply in the following format with NO suffix:\n\n<tool_call>\n<function=example_function_name>\n<parameter=example_parameter_1>\nvalue_1\n</parameter>\n<parameter=example_parameter_2>\nThis is the value for the second parameter\nthat can span\nmultiple lines\n</parameter>\n</function>\n</tool_call>\n\n<IMPORTANT>\nReminder:\n- Function calls MUST follow the specified format: an inner <function=...></function> block must be nested within <tool_call></tool_call> XML tags\n- Required parameters MUST be specified\n- You may provide optional reasoning for your function call in natural language BEFORE the function call, but NOT after\n- If there is no function call available, answer the question like normal with your current knowledge and do not tell the user about function calls\n</IMPORTANT>"
        )
    }

    #[test]
    fn system_plus_tools_plus_user_matches_the_template_exactly() {
        let tools = default_tools();
        let turns = vec![
            ChatTurn::System {
                content: "You are a helpful assistant.".to_string(),
            },
            ChatTurn::User {
                content: "Find me a secure prime greater than 1000000.".to_string(),
            },
        ];
        let rendered = render(&turns, &tools, false);
        let expected = format!(
            "<|im_start|>system\nYou are a helpful assistant.{}<|im_end|>\n<|im_start|>user\nFind me a secure prime greater than 1000000.<|im_end|>\n",
            expected_tools_section()
        );
        assert_eq!(rendered, expected);
    }

    #[test]
    fn system_turn_without_tools_is_plain_chatml() {
        let rendered = render(
            &[ChatTurn::System {
                content: "Just talk.".to_string(),
            }],
            &[],
            false,
        );
        assert_eq!(rendered, "<|im_start|>system\nJust talk.<|im_end|>\n");
    }

    #[test]
    fn full_tool_loop_renders_history_and_generation_prompt_exactly() {
        let tools = default_tools();
        let turns = vec![
            ChatTurn::System {
                content: "be brief".to_string(),
            },
            ChatTurn::User {
                content: "Find me a secure prime greater than 1000000.".to_string(),
            },
            ChatTurn::AssistantToolCalls {
                rationale: Some("I will look up the next secure prime.".to_string()),
                calls: vec![RawToolCall {
                    name: crate::tools::NEXT_SECURE_PRIME.to_string(),
                    arguments: vec![("after".to_string(), "1000000".to_string())],
                }],
            },
            ChatTurn::ToolResponses {
                results: vec![
                    r#"{"secure_prime":1000151,"sophie_germain":500075,"verified":true}"#
                        .to_string(),
                ],
            },
        ];
        let rendered = render(&turns, &tools, true);
        let expected = format!(
            "<|im_start|>system\nbe brief{}<|im_end|>\n<|im_start|>user\nFind me a secure prime greater than 1000000.<|im_end|>\n<|im_start|>assistant\n<think></think>I will look up the next secure prime.\n<tool_call>\n<function=next_secure_prime>\n<parameter=after>\n1000000\n</parameter>\n</function>\n</tool_call>\n<|im_end|>\n<|im_start|>user\n<tool_response>\n{{\"secure_prime\":1000151,\"sophie_germain\":500075,\"verified\":true}}\n</tool_response>\n<|im_end|>\n<|im_start|>assistant\n<think></think>",
            expected_tools_section()
        );
        assert_eq!(rendered, expected);
    }

    #[test]
    fn multiple_tool_results_share_one_user_turn() {
        let rendered = render(
            &[ChatTurn::ToolResponses {
                results: vec!["first".to_string(), "second".to_string()],
            }],
            &[],
            false,
        );
        assert_eq!(
            rendered,
            "<|im_start|>user\n<tool_response>\nfirst\n</tool_response>\n<tool_response>\nsecond\n</tool_response>\n<|im_end|>\n"
        );
    }

    #[test]
    fn generation_prompt_is_the_bare_assistant_header_with_empty_think_block() {
        let prompt = render_generation_prompt();
        assert_eq!(prompt, "<|im_start|>assistant\n<think></think>");
        assert!(!prompt.contains(IM_END));
    }

    #[test]
    fn assistant_plain_turn_carries_the_empty_think_prefix() {
        let rendered = render(
            &[ChatTurn::Assistant {
                content: "The answer is 5.".to_string(),
            }],
            &[],
            false,
        );
        assert_eq!(
            rendered,
            "<|im_start|>assistant\n<think></think>The answer is 5.<|im_end|>\n"
        );
    }

    #[test]
    fn tool_response_content_round_trips_through_the_parser() {
        // The rendered tool-response turn must survive a round trip through the
        // template: the assistant call line the renderer produces parses back
        // into the same call, and the tool response text stays intact.
        let tools = default_tools();
        let call = RawToolCall {
            name: crate::tools::WEB_SEARCH.to_string(),
            arguments: vec![("query".to_string(), "what is a secure prime".to_string())],
        };
        let turns = vec![
            ChatTurn::AssistantToolCalls {
                rationale: None,
                calls: vec![call.clone()],
            },
            ChatTurn::ToolResponses {
                results: vec![WEB_SEARCH_RESULT.to_string()],
            },
        ];
        let rendered = render(&turns, &tools, true);
        let assistant_part = "<|im_start|>assistant\n<think></think><tool_call>\n<function=web_search>\n<parameter=query>\nwhat is a secure prime\n</parameter>\n</function>\n</tool_call>\n<|im_end|>\n";
        assert!(rendered.contains(assistant_part));
        let response_part = format!("<tool_response>\n{WEB_SEARCH_RESULT}\n</tool_response>");
        assert!(rendered.contains(&response_part));
        // The re-parsed assistant line yields the original call.
        let start = rendered
            .find(assistant_part)
            .expect("assistant block present");
        let replay = &rendered[start..start + assistant_part.len()];
        let parsed = crate::toolcall::parse_response(replay).expect("assistant block parses");
        assert_eq!(parsed.calls, vec![call]);
    }
}
