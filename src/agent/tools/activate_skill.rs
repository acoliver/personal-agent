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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tool_approval_policy::ToolApprovalPolicy;
    use crate::llm::client_agent::{ApprovalGate, McpToolContext};
    use crate::services::{AppSettingsServiceImpl, SkillsService, SkillsServiceImpl};
    use serdes_ai_agent::prelude::RunContext;
    use std::sync::Arc;
    use tokio::sync::Mutex as AsyncMutex;

    /// Write a skill file to disk with YAML frontmatter and body.
    fn write_skill(root: &std::path::Path, dir_name: &str, name: &str, body: &str) {
        let skill_dir = root.join(dir_name);
        std::fs::create_dir_all(&skill_dir).expect("skill dir should exist");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Test skill\n---\n{body}"),
        )
        .expect("skill file should write");
    }

    /// Create a test context with real services and a temp directory.
    fn create_test_context() -> (
        McpToolContext,
        tempfile::TempDir,
        Arc<SkillsServiceImpl>,
        tokio::sync::mpsc::Receiver<ViewCommand>,
    ) {
        let temp_dir = tempfile::TempDir::new().expect("temp dir should exist");

        let settings = Arc::new(
            AppSettingsServiceImpl::new(temp_dir.path().join("settings.json"))
                .expect("settings should initialize"),
        );

        let skills_service = Arc::new(
            SkillsServiceImpl::new_for_tests(
                settings,
                temp_dir.path().join("bundled"),
                temp_dir.path().join("user"),
            )
            .expect("skills service should initialize"),
        );

        let (view_tx, view_rx) = tokio::sync::mpsc::channel(16);
        let approval_gate = Arc::new(ApprovalGate::new());
        let policy = Arc::new(AsyncMutex::new(ToolApprovalPolicy {
            skills_auto_approve: true,
            ..ToolApprovalPolicy::default()
        }));

        let ctx = McpToolContext {
            view_tx,
            approval_gate,
            policy,
            skills_service: skills_service.clone(),
        };

        (ctx, temp_dir, skills_service, view_rx)
    }

    /// Activating an existing enabled skill returns its body.
    #[tokio::test]
    async fn execute_returns_skill_body_for_existing_enabled_skill() {
        let (ctx, temp_dir, skills_service, _view_rx) = create_test_context();

        write_skill(
            &temp_dir.path().join("bundled"),
            "writer",
            "docs-writer",
            "Write comprehensive documentation.\n",
        );

        skills_service
            .discover_skills()
            .await
            .expect("discovery should succeed");

        let run_context = RunContext::new(ctx, "test-model");
        let args = serde_json::json!({"skill_name": "docs-writer"});

        let result = ActivateSkillExecutor
            .execute(args, &run_context)
            .await
            .expect("execute should succeed");

        let text = result.as_text().expect("should have text content");
        assert!(text.contains("# Skill: docs-writer"));
        assert!(text.contains("Write comprehensive documentation"));
    }

    /// Activating a nonexistent skill returns an error.
    #[tokio::test]
    async fn execute_returns_error_for_nonexistent_skill() {
        let (ctx, _temp_dir, skills_service, _view_rx) = create_test_context();

        skills_service
            .discover_skills()
            .await
            .expect("discovery should succeed");

        let run_context = RunContext::new(ctx, "test-model");
        let args = serde_json::json!({"skill_name": "no-such-skill"});

        let error = ActivateSkillExecutor
            .execute(args, &run_context)
            .await
            .expect_err("should fail for nonexistent skill");

        assert!(error.to_string().contains("Skill not found"));
    }

    /// Activating a disabled skill returns an error.
    #[tokio::test]
    async fn execute_returns_error_for_disabled_skill() {
        let (ctx, temp_dir, skills_service, _view_rx) = create_test_context();

        write_skill(
            &temp_dir.path().join("bundled"),
            "writer",
            "docs-writer",
            "Write documentation.\n",
        );

        skills_service
            .discover_skills()
            .await
            .expect("discovery should succeed");

        skills_service
            .set_skill_enabled("docs-writer", false)
            .await
            .expect("disable should succeed");

        let run_context = RunContext::new(ctx, "test-model");
        let args = serde_json::json!({"skill_name": "docs-writer"});

        let error = ActivateSkillExecutor
            .execute(args, &run_context)
            .await
            .expect_err("should fail for disabled skill");

        assert!(error.to_string().contains("Skill is disabled"));
    }

    /// Missing `skill_name` argument returns an error.
    #[tokio::test]
    async fn execute_returns_error_for_missing_skill_name_argument() {
        let (ctx, _temp_dir, skills_service, _view_rx) = create_test_context();

        skills_service
            .discover_skills()
            .await
            .expect("discovery should succeed");

        let run_context = RunContext::new(ctx, "test-model");
        let args = serde_json::json!({});

        let error = ActivateSkillExecutor
            .execute(args, &run_context)
            .await
            .expect_err("should fail for missing argument");

        assert!(error
            .to_string()
            .contains("Missing required 'skill_name' argument"));
    }
}
