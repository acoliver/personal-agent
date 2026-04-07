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
    async fn refresh(&self) -> ServiceResult<()>;
    async fn watched_directories(&self) -> ServiceResult<Vec<std::path::PathBuf>>;
    async fn add_watched_directory(&self, path: std::path::PathBuf) -> ServiceResult<()>;
    async fn remove_watched_directory(&self, path: &std::path::Path) -> ServiceResult<()>;
    fn default_user_skills_dir(&self) -> std::path::PathBuf;
    async fn install_skill_from_url(&self, url: &str) -> ServiceResult<Skill>;
}
