use std::sync::{atomic::Ordering, Arc, Mutex};

use objc2::DefinedClass;

use personal_agent::agent::runtime::spawn_in_agent_runtime;
use personal_agent::config::Config;
use personal_agent::models::ModelProfile;
use personal_agent::{LlmClient, LlmMessage, StreamEvent};

use crate::ui::chat_view::Message as UiMessage;

use super::helpers::{
    build_llm_messages, collect_profile, fetch_mcp_tools, streaming_state_from_buffers,
    update_last_message,
};
use crate::ui::chat_view::log_to_file;
use crate::ui::ChatViewController;

pub fn reset_streaming_buffers(
    response: &Arc<Mutex<String>>,
    thinking: &Arc<Mutex<String>>,
    tool_uses: &Arc<Mutex<Vec<personal_agent::llm::tools::ToolUse>>>,
) {
    if let Ok(mut buf) = response.lock() {
        buf.clear();
    }
    if let Ok(mut buf) = thinking.lock() {
        buf.clear();
    }
    if let Ok(mut buf) = tool_uses.lock() {
        buf.clear();
    }
}

pub fn start_streaming_request(
    profile: ModelProfile,
    llm_messages: Vec<LlmMessage>,
    tools: Vec<personal_agent::llm::tools::Tool>,
    streaming_response: Arc<Mutex<String>>,
    streaming_thinking: Arc<Mutex<String>>,
    streaming_tool_uses: Arc<Mutex<Vec<personal_agent::llm::tools::ToolUse>>>,
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
) {
    log_to_file("Starting streaming request in background...");
    spawn_in_agent_runtime(async move {
        match LlmClient::from_profile(&profile) {
            Ok(client) => {
                let streaming_response_clone = Arc::clone(&streaming_response);
                let streaming_thinking_clone = Arc::clone(&streaming_thinking);
                let streaming_tool_uses_clone = Arc::clone(&streaming_tool_uses);
                let cancel_flag_clone = Arc::clone(&cancel_flag);

                let result = client
                    .request_stream_with_tools(&llm_messages, &tools, |event| {
                        let is_complete = matches!(event, StreamEvent::Complete);
                        handle_stream_event(
                            event,
                            &streaming_response_clone,
                            &streaming_thinking_clone,
                            &streaming_tool_uses_clone,
                            &cancel_flag_clone,
                            false,
                        );
                        if is_complete {
                            log_to_file("Streaming complete");
                        }
                    })
                    .await;

                if let Err(e) = result {
                    log_to_file(&format!("Stream request failed: {e}"));
                    if let Ok(mut buf) = streaming_response.lock() {
                        use std::fmt::Write as _;
                        let _ = write!(buf, "[Error: {e}]");
                    }
                }
            }
            Err(e) => {
                log_to_file(&format!("Failed to create client: {e}"));
                if let Ok(mut buf) = streaming_response.lock() {
                    use std::fmt::Write as _;
                    let _ = write!(buf, "[Error: {e}]");
                }
            }
        }
    });
}

pub fn handle_stream_event(
    event: StreamEvent,
    streaming_response: &Arc<Mutex<String>>,
    streaming_thinking: &Arc<Mutex<String>>,
    streaming_tool_uses: &Arc<Mutex<Vec<personal_agent::llm::tools::ToolUse>>>,
    cancel_flag: &Arc<std::sync::atomic::AtomicBool>,
    is_followup: bool,
) {
    if cancel_flag.load(Ordering::SeqCst) {
        log_to_file("Streaming cancelled by user");
        return;
    }

    match event {
        StreamEvent::TextDelta(delta) => {
            if let Ok(mut buf) = streaming_response.lock() {
                buf.push_str(&delta);
            }
        }
        StreamEvent::ThinkingDelta(delta) => {
            if let Ok(mut buf) = streaming_thinking.lock() {
                buf.push_str(&delta);
            }
        }
        StreamEvent::ToolUse(tool_use) => {
            log_to_file(&format!(
                "Tool use requested: {} ({})",
                tool_use.name, tool_use.id
            ));
            if let Ok(mut buf) = streaming_tool_uses.lock() {
                buf.push(tool_use);
            }
        }
        StreamEvent::Complete => {
            if is_followup {
                log_to_file("Streaming complete (after tool execution)");
            } else {
                log_to_file("Streaming complete");
            }
        }
        StreamEvent::Error(e) => {
            log_to_file(&format!("Stream error: {e}"));
        }
    }
}

pub fn mark_streaming_complete(controller: &ChatViewController) {
    *controller.ivars().is_streaming.borrow_mut() = false;
    if let Some(btn) = &*controller.ivars().stop_button.borrow() {
        btn.setHidden(true);
    }
}

fn log_tool_uses(tool_uses: &[personal_agent::llm::tools::ToolUse]) {
    log_to_file(&format!("=== TOOL USES DETECTED: {} ===", tool_uses.len()));
    for tool_use in tool_uses {
        log_to_file(&format!("  Tool: {} ({})", tool_use.name, tool_use.id));
        log_to_file(&format!("  Args: {}", tool_use.input));
    }
    log_to_file("=== END TOOL USES ===");
}

fn add_tool_execution_status(messages: &mut [UiMessage], final_text: &str, tool_count: usize) {
    let tool_text = format_tool_execution_text(final_text, tool_count);
    update_last_message(messages, tool_text);
}

fn format_tool_execution_text(final_text: &str, tool_count: usize) -> String {
    if tool_count == 0 {
        final_text.to_string()
    } else {
        format!("{final_text}\n\n[Executed {tool_count} tool(s)]")
    }
}

fn create_followup_messages(
    profile: &ModelProfile,
    conversation: Option<&personal_agent::models::Conversation>,
) -> Vec<LlmMessage> {
    let mut llm_messages = build_llm_messages(profile, conversation);
    if llm_messages.is_empty() {
        llm_messages.push(LlmMessage::user("Continue."));
    }
    llm_messages
}

pub async fn collect_tool_results(
    tool_uses: &[personal_agent::llm::tools::ToolUse],
) -> Vec<personal_agent::llm::tools::ToolResult> {
    let mut tool_results = Vec::new();

    for tool_use in tool_uses {
        let result = {
            let service_arc = personal_agent::mcp::McpService::global();
            let mut svc = service_arc.lock().await;
            svc.call_tool(&tool_use.name, tool_use.input.clone()).await
        };

        let (content, is_error) = match result {
            Ok(output) => match serde_json::to_string(&output) {
                Ok(serialized) => (serialized, false),
                Err(e) => {
                    log_to_file(&format!("Tool {} failed to serialize: {e}", tool_use.name));
                    (e.to_string(), true)
                }
            },
            Err(e) => {
                log_to_file(&format!("Tool {} failed: {e}", tool_use.name));
                (e, true)
            }
        };

        tool_results.push(personal_agent::llm::tools::ToolResult {
            tool_use_id: tool_use.id.clone(),
            content,
            is_error,
        });
    }

    tool_results
}

pub struct FollowupStreamContext {
    pub(super) streaming_response: Arc<Mutex<String>>,
    pub(super) streaming_thinking: Arc<Mutex<String>>,
    pub(super) streaming_tool_uses: Arc<Mutex<Vec<personal_agent::llm::tools::ToolUse>>>,
    pub(super) cancel_flag: Arc<std::sync::atomic::AtomicBool>,
    pub(super) _executing_tools: Arc<std::sync::atomic::AtomicBool>,
}

pub fn run_followup_stream(
    profile: ModelProfile,
    llm_messages: Vec<LlmMessage>,
    tools: Vec<personal_agent::llm::tools::Tool>,
    ctx: FollowupStreamContext,
) {
    log_to_file("Starting follow-up streaming request with tool results...");

    let FollowupStreamContext {
        streaming_response,
        streaming_thinking,
        streaming_tool_uses,
        cancel_flag,
        _executing_tools: _,
    } = ctx;

    spawn_in_agent_runtime(async move {
        match LlmClient::from_profile(&profile) {
            Ok(client) => {
                let streaming_response_clone = Arc::clone(&streaming_response);
                let streaming_thinking_clone = Arc::clone(&streaming_thinking);
                let streaming_tool_uses_clone = Arc::clone(&streaming_tool_uses);
                let cancel_flag_clone = Arc::clone(&cancel_flag);

                log_to_file("About to start follow-up stream...");
                let result = client
                    .request_stream_with_tools(&llm_messages, &tools, |event| {
                        let is_complete = matches!(event, StreamEvent::Complete);
                        handle_stream_event(
                            event,
                            &streaming_response_clone,
                            &streaming_thinking_clone,
                            &streaming_tool_uses_clone,
                            &cancel_flag_clone,
                            false,
                        );
                        if is_complete {
                            log_to_file("Streaming complete");
                        }
                    })
                    .await;

                if let Err(e) = result {
                    log_to_file(&format!("Follow-up stream request failed: {e}"));
                    if let Ok(mut buf) = streaming_response.lock() {
                        use std::fmt::Write as _;
                        let _ = write!(buf, "[Error: {e}]");
                    }
                }
            }
            Err(e) => {
                log_to_file(&format!("Failed to create client for follow-up: {e}"));
            }
        }
    });
}

pub fn finalize_streaming(controller: &ChatViewController) {
    let state = streaming_state_from_buffers(
        &controller.ivars().streaming_response,
        &controller.ivars().streaming_thinking,
        &controller.ivars().streaming_tool_uses,
    );

    update_last_message(
        &mut controller.ivars().messages.borrow_mut(),
        state.final_text.clone(),
    );
    let show_thinking = super::helpers::should_show_thinking(controller);
    super::helpers::rebuild_messages_with_thinking(
        controller,
        state.thinking_text.as_deref(),
        show_thinking,
    );

    mark_streaming_complete(controller);

    log_tool_uses(&state.tool_uses);
    add_tool_execution_status(
        &mut controller.ivars().messages.borrow_mut(),
        &state.final_text,
        state.tool_uses.len(),
    );
}

pub fn execute_tools_and_continue(controller: &ChatViewController) {
    let tool_uses = match controller.ivars().streaming_tool_uses.lock() {
        Ok(buf) => buf.clone(),
        Err(_) => return,
    };

    if tool_uses.is_empty() {
        return;
    }

    controller
        .ivars()
        .executing_tools
        .store(true, Ordering::SeqCst);

    let tool_results = personal_agent::agent::runtime::run_in_agent_runtime(async move {
        collect_tool_results(&tool_uses).await
    });

    let config = Config::load(Config::default_path().unwrap_or_default()).ok();
    let profile = config.as_ref().and_then(collect_profile);

    if let Some(profile) = profile {
        let tools = fetch_mcp_tools();
        let mut llm_messages =
            create_followup_messages(&profile, controller.ivars().conversation.borrow().as_ref());

        if !tool_results.is_empty() {
            let mut results_msg = LlmMessage::user("");
            results_msg.tool_results = tool_results;
            llm_messages.push(results_msg);
        }

        reset_streaming_buffers(
            &controller.ivars().streaming_response,
            &controller.ivars().streaming_thinking,
            &controller.ivars().streaming_tool_uses,
        );

        let ctx = FollowupStreamContext {
            streaming_response: Arc::clone(&controller.ivars().streaming_response),
            streaming_thinking: Arc::clone(&controller.ivars().streaming_thinking),
            streaming_tool_uses: Arc::clone(&controller.ivars().streaming_tool_uses),
            cancel_flag: Arc::clone(&controller.ivars().cancel_streaming),
            _executing_tools: Arc::clone(&controller.ivars().executing_tools),
        };

        run_followup_stream(profile, llm_messages, tools, ctx);
    } else {
        log_to_file("No profile configured for tool continuation");
    }
}

pub fn schedule_follow_up_request(controller: &ChatViewController) {
    if controller
        .ivars()
        .executing_tools
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return;
    }

    if controller
        .ivars()
        .streaming_tool_uses
        .lock()
        .is_ok_and(|buf| !buf.is_empty())
    {
        execute_tools_and_continue(controller);
    } else {
        finalize_streaming(controller);
    }
}
