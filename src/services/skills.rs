use async_trait::async_trait;

use crate::models::Skill;

use super::ServiceResult;

#[async_trait]
pub trait SkillsService: Send + Sync {
    async fn list_skills(&self) -> ServiceResult<Vec<Skill>>;
    async fn get_skill(&self, name: &str) -> ServiceResult<Option<Skill>>;
    async fn get_skill_body(&self, name: &str) -> ServiceResult<Option<String>>;
    async fn set_skill_enabled(&self, name: &str, enabled: bool) -> ServiceResult<()>;
    async fn get_enabled_skills(&self) -> ServiceResult<Vec<Skill>>;
}
