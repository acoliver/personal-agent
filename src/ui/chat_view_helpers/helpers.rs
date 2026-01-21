use std::sync::{Arc, Mutex};

use objc2::{DefinedClass, MainThreadMarker};
use objc2_app_kit::NSStackView;
use objc2_foundation::NSString;

use personal_agent::config::Config;
use personal_agent::mcp::McpService;
use personal_agent::models::Conversation;
use personal_agent::models::ModelProfile;
use personal_agent::storage::ConversationStorage;
use personal_agent::LlmMessage;

use crate::ui::chat_view::log_to_file;
use crate::ui::chat_view::Message;
use crate::ui::ChatViewController;

use super::layout::load_initial_messages;

pub struct StreamingState {
    pub final_text: String,
    pub thinking_text: Option<String>,
    pub tool_uses: Vec<personal_agent::llm::tools::ToolUse>,
}

pub fn collect_profile(config: &Config) -> Option<ModelProfile> {
    config
        .default_profile
        .and_then(|id| config.profiles.iter().find(|p| p.id == id).cloned())
        .or_else(|| config.profiles.first().cloned())
}

pub fn fetch_mcp_tools() -> Vec<personal_agent::llm::tools::Tool> {
    log_to_file("Fetching MCP tools...");
    let service_arc = McpService::global();
    let mut attempts = 0;
    loop {
        if let Ok(svc) = service_arc.try_lock() {
            let tools = svc.get_llm_tools();
            log_to_file(&format!("Got {} MCP tools", tools.len()));
            return tools;
        }

        attempts += 1;
        if attempts > 50 {
            log_to_file("MCP service still busy after 5s, proceeding without tools");
            return vec![];
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

pub fn build_llm_messages(
    profile: &ModelProfile,
    conversation: Option<&Conversation>,
) -> Vec<LlmMessage> {
    let mut llm_messages = Vec::new();
    if !profile.system_prompt.is_empty() {
        llm_messages.push(LlmMessage::system(&profile.system_prompt));
    }

    if let Some(conv) = conversation {
        for m in &conv.messages {
            let msg = match m.role {
                personal_agent::models::MessageRole::User => LlmMessage::user(&m.content),
                personal_agent::models::MessageRole::Assistant => LlmMessage::assistant(&m.content),
                personal_agent::models::MessageRole::System => LlmMessage::system(&m.content),
            };
            llm_messages.push(msg);
        }
    }

    llm_messages
}

pub fn update_last_message(messages: &mut [Message], text: String) {
    if let Some(last_msg) = messages.last_mut() {
        if !last_msg.is_user {
            last_msg.text = text;
        }
    }
}

pub fn streaming_state_from_buffers(
    response: &Arc<Mutex<String>>,
    thinking: &Arc<Mutex<String>>,
    tool_uses: &Arc<Mutex<Vec<personal_agent::llm::tools::ToolUse>>>,
) -> StreamingState {
    let final_text = response.lock().map_or_else(
        |_| "[Error: Failed to get response]".to_string(),
        |buf| buf.trim_end_matches('␄').to_string(),
    );

    let thinking_text = thinking.lock().ok().and_then(|buf| {
        if buf.is_empty() {
            None
        } else {
            Some(buf.clone())
        }
    });

    let tool_uses = tool_uses
        .lock()
        .map(|mut buf| {
            let uses = buf.clone();
            buf.clear();
            uses
        })
        .unwrap_or_default();

    StreamingState {
        final_text,
        thinking_text,
        tool_uses,
    }
}

pub fn update_thinking_button_state(controller: &ChatViewController) {
    let config = Config::load(Config::default_path().unwrap_or_default()).unwrap_or_default();

    if let Some(conversation) = &*controller.ivars().conversation.borrow() {
        if let Ok(profile) = config.get_profile(&conversation.profile_id) {
            if let Some(btn) = &*controller.ivars().thinking_button.borrow() {
                let title = if profile.parameters.show_thinking {
                    "T*"
                } else {
                    "T"
                };
                btn.setTitle(&NSString::from_str(title));
            }
        }
    }
}

pub fn should_show_thinking(controller: &ChatViewController) -> bool {
    let config = Config::load(Config::default_path().unwrap_or_default()).unwrap_or_default();

    if let Some(conversation) = &*controller.ivars().conversation.borrow() {
        if let Ok(profile) = config.get_profile(&conversation.profile_id) {
            return profile.parameters.show_thinking;
        }
    }

    false
}

pub fn update_title_and_model(controller: &ChatViewController) {
    let conv_title = controller
        .ivars()
        .conversation
        .borrow()
        .as_ref()
        .and_then(|c| c.title.clone())
        .unwrap_or_else(|| "New Conversation".to_string());

    // Repopulate the popup with fresh data from storage, then select current title
    if let Some(popup) = &*controller.ivars().title_popup.borrow() {
        populate_title_popup(popup, &conv_title);
    }

    if let Some(field) = &*controller.ivars().title_edit_field.borrow() {
        field.setStringValue(&NSString::from_str(&conv_title));
    }
}

pub fn populate_title_popup(popup: &objc2_app_kit::NSPopUpButton, current_title: &str) {
    popup.removeAllItems();

    // Load all conversations from storage and add their titles to the popup
    if let Ok(storage) = ConversationStorage::with_default_path() {
        if let Ok(conversations) = storage.load_all() {
            for conv in &conversations {
                if let Some(title) = &conv.title {
                    popup.addItemWithTitle(&NSString::from_str(title));
                }
            }

            // Select the current title if it exists in the list
            if let Some(item) = popup.itemWithTitle(&NSString::from_str(current_title)) {
                popup.selectItem(Some(&item));
            }

            // If we got conversations, we're done
            if !conversations.is_empty() {
                return;
            }
        }
    }

    // Fallback: just add the current title if storage failed or was empty
    popup.addItemWithTitle(&NSString::from_str(current_title));
}

pub fn load_conversation_by_title(controller: &ChatViewController, title: &str) {
    log_to_file(&format!("Loading conversation by title: {title}"));

    if let Ok(storage) = ConversationStorage::with_default_path() {
        if let Ok(conversations) = storage.load_all() {
            if let Some(conv) = conversations
                .into_iter()
                .find(|c| c.title.as_deref() == Some(title))
            {
                log_to_file(&format!(
                    "Found conversation: {} ({:?})",
                    conv.id, conv.title
                ));
                *controller.ivars().conversation.borrow_mut() = Some(conv);
                controller.ivars().messages.borrow_mut().clear();
                load_initial_messages(controller);
            } else {
                log_to_file(&format!("No conversation found with title: {title}"));
            }
        }
    }
}

pub fn rebuild_messages_with_thinking(
    controller: &ChatViewController,
    thinking_text: Option<&str>,
    show_thinking: bool,
) {
    let mtm = MainThreadMarker::new().unwrap();

    let message_count = controller.ivars().messages.borrow().len();
    log_to_file(&format!(
        "rebuild_messages called, {message_count} messages in store"
    ));

    if let Some(container) = &*controller.ivars().messages_container.borrow() {
        log_to_file("Container found, clearing old views");

        let subviews = container.subviews();
        log_to_file(&format!("Removing {} existing subviews", subviews.len()));
        for view in &subviews {
            if let Some(stack) = container.downcast_ref::<NSStackView>() {
                unsafe {
                    stack.removeArrangedSubview(&view);
                }
            }
            view.removeFromSuperview();
        }

        if let Some(stack) = container.downcast_ref::<NSStackView>() {
            if show_thinking {
                if let Some(thinking) = thinking_text {
                    if !thinking.is_empty() {
                        let thinking_view = controller.create_thinking_bubble(thinking, mtm);
                        unsafe {
                            stack.addArrangedSubview(&thinking_view);
                        }
                    }
                }
            }

            for msg in controller.ivars().messages.borrow().iter() {
                let bubble = controller.create_message_bubble(&msg.text, msg.is_user, mtm);
                unsafe {
                    stack.addArrangedSubview(&bubble);
                }
            }

            log_to_file("All message bubbles added");
        } else {
            log_to_file("ERROR: Container is not an NSStackView!");
        }
    } else {
        log_to_file("ERROR: No messages_container reference!");
    }
}

pub fn rebuild_messages(controller: &ChatViewController) {
    rebuild_messages_with_thinking(controller, None, false);
}
