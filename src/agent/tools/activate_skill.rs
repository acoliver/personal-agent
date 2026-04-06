use crate::agent::tool_approval_policy::ToolApprovalDecision;
use crate::llm::client_agent::McpToolContext;
use crate::presentation::view_command::ViewCommand;
use serdes_ai_agent::prelude::*;
use serdes_ai_agent::ToolExecutor;
use serdes_ai_tools::{ToolDefinition, ToolError, ToolReturn};

#[derive(Debug, Clone, Copy)]
pub struct ActivateSkillExecutor;

#[async_trait::async_trait]
impl ToolExecutor<McpToolContext> for ActivateSkillExecutor {
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &RunContext<McpToolContext>,
    ) -> Result<ToolReturn, ToolError> {
        let skill_name = args
            .get("skill_name")
            .and_then(|value| value.as_str())
            .ok_or_else(|| ToolError::execution_failed("Missing required 'skill_name' argument"))?
            .to_string();

        let skill = ctx
            .deps()
            .skills_service
            .get_skill(&skill_name)
            .await
            .map_err(|error| ToolError::execution_failed(error.to_string()))?
            .ok_or_else(|| ToolError::execution_failed(format!("Skill not found: {skill_name}")))?;

        if !skill.enabled {
            return Err(ToolError::execution_failed(format!(
                "Skill is disabled: {}",
                skill.name
            )));
        }

        check_approval(ctx.deps(), &skill).await?;

        let body = ctx
            .deps()
            .skills_service
            .get_skill_body(&skill.name)
            .await
            .map_err(|error| ToolError::execution_failed(error.to_string()))?
            .ok_or_else(|| {
                ToolError::execution_failed(format!("Skill body not found: {skill_name}"))
            })?;

        Ok(ToolReturn::text(format!(
            "# Skill: {}\n\n{}",
            skill.name, body
        )))
    }
}

async fn check_approval(
    tool_context: &McpToolContext,
    skill: &crate::models::Skill,
) -> Result<(), ToolError> {
    let decision = {
        let policy = tool_context.policy.lock().await;
        policy.evaluate("activate_skill")
    };

    match decision {
        ToolApprovalDecision::Allow => Ok(()),
        ToolApprovalDecision::Deny => Err(ToolError::execution_failed(
            "Tool execution denied by policy",
        )),
        ToolApprovalDecision::AskUser => {
            let request_id = uuid::Uuid::new_v4().to_string();
            let waiter = tool_context
                .approval_gate
                .wait_for_approval(request_id.clone(), "activate_skill".to_string());

            if tool_context
                .view_tx
                .try_send(ViewCommand::ToolApprovalRequest {
                    request_id: request_id.clone(),
                    tool_name: "activate_skill".to_string(),
                    tool_argument: format!("{} — {}", skill.name, skill.description),
                })
                .is_err()
            {
                let _ = tool_context.approval_gate.resolve(&request_id, false);
                return Err(ToolError::execution_failed(
                    "Failed to send approval request to UI (channel full or closed)",
                ));
            }

            if waiter.wait().await.unwrap_or(false) {
                Ok(())
            } else {
                Err(ToolError::execution_failed("Tool execution denied by user"))
            }
        }
    }
}

#[must_use]
pub fn get_activate_skill_tool_definition() -> ToolDefinition {
    let input_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "skill_name": {
                "type": "string",
                "description": "The name of the skill to activate"
            }
        },
        "required": ["skill_name"]
    });

    ToolDefinition::new(
        "activate_skill",
        "Load the full instruction body for a discovered skill by name.",
    )
    .with_parameters(input_schema)
}
