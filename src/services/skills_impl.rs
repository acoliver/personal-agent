use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::{Skill, SkillSource};

use super::skill_parser::parse_skill_file;
use super::{AppSettingsService, ServiceError, ServiceResult, SkillsService};

const DISABLED_SKILLS_SETTING_KEY: &str = "skills.disabled";

pub struct SkillsServiceImpl {
    app_settings_service: Arc<dyn AppSettingsService>,
    bundled_skills_dir: PathBuf,
    user_skills_dir: PathBuf,
    skills: RwLock<Vec<Skill>>,
}

impl SkillsServiceImpl {
    /// # Errors
    ///
    /// Returns `ServiceError::Configuration` if the user skills directory cannot
    /// be determined or `ServiceError::Io` if required directories cannot be created.
    pub fn new(app_settings_service: Arc<dyn AppSettingsService>) -> ServiceResult<Self> {
        let bundled_skills_dir = std::env::current_exe()
            .map_err(|error| {
                ServiceError::Configuration(format!(
                    "Failed to determine current executable: {error}"
                ))
            })?
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                ServiceError::Configuration(
                    "Failed to resolve executable directory for bundled skills".to_string(),
                )
            })?
            .join("resources")
            .join("skills");

        let user_base_dir = dirs::data_local_dir().ok_or_else(|| {
            ServiceError::Configuration(
                "Could not determine data_local_dir for user skills".to_string(),
            )
        })?;
        let user_skills_dir = user_base_dir.join("PersonalAgent").join("skills");
        std::fs::create_dir_all(&user_skills_dir).map_err(|error| {
            ServiceError::Io(format!(
                "Failed to create user skills directory {}: {error}",
                user_skills_dir.display()
            ))
        })?;

        Ok(Self {
            app_settings_service,
            bundled_skills_dir,
            user_skills_dir,
            skills: RwLock::new(Vec::new()),
        })
    }

    /// Create a test-scoped skills service with explicit bundled and user directories.
    ///
    /// # Errors
    ///
    /// Returns an error when the user skills directory cannot be created.
    pub fn new_for_tests(
        app_settings_service: Arc<dyn AppSettingsService>,
        bundled_skills_dir: PathBuf,
        user_skills_dir: PathBuf,
    ) -> ServiceResult<Self> {
        std::fs::create_dir_all(&user_skills_dir).map_err(|error| {
            ServiceError::Io(format!(
                "Failed to create user skills directory {}: {error}",
                user_skills_dir.display()
            ))
        })?;

        Ok(Self {
            app_settings_service,
            bundled_skills_dir,
            user_skills_dir,
            skills: RwLock::new(Vec::new()),
        })
    }

    /// # Errors
    ///
    /// Returns an error when skill discovery or persisted setting reads fail.
    pub async fn discover_skills(&self) -> ServiceResult<()> {
        let disabled_names = self.load_disabled_skill_names().await?;
        let mut discovered = Self::discover_from_directory(
            &self.bundled_skills_dir,
            SkillSource::Bundled,
            &disabled_names,
        )?;

        for (name, user_skill) in Self::discover_from_directory(
            &self.user_skills_dir,
            SkillSource::User,
            &disabled_names,
        )? {
            discovered.insert(name, user_skill);
        }

        let mut skills = discovered.into_values().collect::<Vec<_>>();
        skills.sort_by(|left, right| left.name.cmp(&right.name));
        *self.skills.write().await = skills;
        Ok(())
    }

    fn discover_from_directory(
        base_dir: &Path,
        source: SkillSource,
        disabled_names: &HashSet<String>,
    ) -> ServiceResult<HashMap<String, Skill>> {
        let mut discovered = HashMap::new();
        if !base_dir.exists() {
            return Ok(discovered);
        }

        Self::discover_from_directory_recursive(base_dir, source, disabled_names, &mut discovered)?;
        Ok(discovered)
    }

    fn discover_from_directory_recursive(
        base_dir: &Path,
        source: SkillSource,
        disabled_names: &HashSet<String>,
        discovered: &mut HashMap<String, Skill>,
    ) -> ServiceResult<()> {
        let entries = std::fs::read_dir(base_dir).map_err(|error| {
            ServiceError::Io(format!(
                "Failed to read skills directory {}: {error}",
                base_dir.display()
            ))
        })?;

        for entry_result in entries {
            let entry = entry_result.map_err(|error| {
                ServiceError::Io(format!(
                    "Failed to read skill entry in {}: {error}",
                    base_dir.display()
                ))
            })?;
            let file_type = entry.file_type().map_err(|error| {
                ServiceError::Io(format!(
                    "Failed to read skill entry type in {}: {error}",
                    base_dir.display()
                ))
            })?;
            if file_type.is_symlink() || !file_type.is_dir() {
                continue;
            }

            let path = entry.path();
            let skill_file = path.join("SKILL.md");
            if skill_file.is_file() {
                let (metadata, _body) = parse_skill_file(&skill_file)?;
                let enabled = !disabled_names.contains(&metadata.name);
                let skill = Skill::new(
                    metadata.name.clone(),
                    metadata.description,
                    skill_file,
                    source,
                    enabled,
                );
                discovered.insert(metadata.name, skill);
                continue;
            }

            Self::discover_from_directory_recursive(&path, source, disabled_names, discovered)?;
        }

        Ok(())
    }

    async fn load_disabled_skill_names(&self) -> ServiceResult<HashSet<String>> {
        let raw = self
            .app_settings_service
            .get_setting(DISABLED_SKILLS_SETTING_KEY)
            .await?;

        let Some(raw) = raw else {
            return Ok(HashSet::new());
        };

        serde_json::from_str::<Vec<String>>(&raw)
            .map(|items| items.into_iter().collect())
            .map_err(|error| {
                ServiceError::Validation(format!(
                    "Failed to parse disabled skills setting {DISABLED_SKILLS_SETTING_KEY}: {error}"
                ))
            })
    }

    fn serialize_disabled_skill_names(disabled_names: &[String]) -> ServiceResult<String> {
        serde_json::to_string(disabled_names)
            .map_err(|error| ServiceError::Serialization(error.to_string()))
    }
}

#[async_trait]
impl SkillsService for SkillsServiceImpl {
    async fn list_skills(&self) -> ServiceResult<Vec<Skill>> {
        Ok(self.skills.read().await.clone())
    }

    async fn get_skill(&self, name: &str) -> ServiceResult<Option<Skill>> {
        Ok(self
            .skills
            .read()
            .await
            .iter()
            .find(|skill| skill.name == name)
            .cloned())
    }

    async fn get_skill_body(&self, name: &str) -> ServiceResult<Option<String>> {
        let skill = self.get_skill(name).await?;
        let Some(skill) = skill else {
            return Ok(None);
        };

        let (_metadata, body) = parse_skill_file(&skill.path)?;
        Ok(Some(body))
    }

    async fn set_skill_enabled(&self, name: &str, enabled: bool) -> ServiceResult<()> {
        let previous_enabled = {
            let mut skills = self.skills.write().await;

            let Some(skill) = skills.iter_mut().find(|skill| skill.name == name) else {
                return Err(ServiceError::NotFound(format!("Skill not found: {name}")));
            };

            let previous_enabled = skill.enabled;
            skill.enabled = enabled;

            let mut disabled_names = skills
                .iter()
                .filter(|skill| !skill.enabled)
                .map(|skill| skill.name.clone())
                .collect::<Vec<_>>();
            disabled_names.sort();
            disabled_names.dedup();

            let serialized = Self::serialize_disabled_skill_names(&disabled_names)?;
            drop(skills);

            (previous_enabled, serialized)
        };

        let (previous_enabled, serialized) = previous_enabled;
        if let Err(error) = self
            .app_settings_service
            .set_setting(DISABLED_SKILLS_SETTING_KEY, serialized)
            .await
        {
            if let Some(skill) = self
                .skills
                .write()
                .await
                .iter_mut()
                .find(|skill| skill.name == name)
            {
                skill.enabled = previous_enabled;
            }
            return Err(error);
        }

        Ok(())
    }

    async fn get_enabled_skills(&self) -> ServiceResult<Vec<Skill>> {
        Ok(self
            .skills
            .read()
            .await
            .iter()
            .filter(|skill| skill.enabled)
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{SkillsServiceImpl, DISABLED_SKILLS_SETTING_KEY};
    use crate::services::{
        app_settings_impl::AppSettingsServiceImpl, AppSettingsService, ServiceError, SkillsService,
    };
    use async_trait::async_trait;
    use tempfile::TempDir;
    use uuid::Uuid;

    struct FailingSetSettingAppSettingsService;

    #[async_trait]
    impl AppSettingsService for FailingSetSettingAppSettingsService {
        async fn get_default_profile_id(&self) -> crate::services::ServiceResult<Option<Uuid>> {
            Ok(None)
        }

        async fn set_default_profile_id(&self, _id: Uuid) -> crate::services::ServiceResult<()> {
            Ok(())
        }

        async fn clear_default_profile_id(&self) -> crate::services::ServiceResult<()> {
            Ok(())
        }

        async fn get_current_conversation_id(
            &self,
        ) -> crate::services::ServiceResult<Option<Uuid>> {
            Ok(None)
        }

        async fn set_current_conversation_id(
            &self,
            _id: Uuid,
        ) -> crate::services::ServiceResult<()> {
            Ok(())
        }

        async fn get_hotkey(&self) -> crate::services::ServiceResult<Option<String>> {
            Ok(None)
        }

        async fn set_hotkey(&self, _hotkey: String) -> crate::services::ServiceResult<()> {
            Ok(())
        }

        async fn get_theme(&self) -> crate::services::ServiceResult<Option<String>> {
            Ok(None)
        }

        async fn set_theme(&self, _theme: String) -> crate::services::ServiceResult<()> {
            Ok(())
        }

        async fn get_setting(&self, _key: &str) -> crate::services::ServiceResult<Option<String>> {
            Ok(None)
        }

        async fn set_setting(
            &self,
            _key: &str,
            _value: String,
        ) -> crate::services::ServiceResult<()> {
            Err(ServiceError::Io(
                "simulated persistence failure".to_string(),
            ))
        }

        async fn reset_to_defaults(&self) -> crate::services::ServiceResult<()> {
            Ok(())
        }
    }

    fn write_skill(
        root: &std::path::Path,
        dir_name: &str,
        name: &str,
        description: &str,
        body: &str,
    ) {
        let skill_dir = root.join(dir_name);
        std::fs::create_dir_all(&skill_dir).expect("skill dir should exist");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {description}\n---\n{body}"),
        )
        .expect("skill file should write");
    }

    fn create_service(temp_dir: &TempDir) -> SkillsServiceImpl {
        let settings = std::sync::Arc::new(
            AppSettingsServiceImpl::new(temp_dir.path().join("settings.json"))
                .expect("settings should initialize"),
        );
        SkillsServiceImpl::new_for_tests(
            settings,
            temp_dir.path().join("bundled"),
            temp_dir.path().join("user"),
        )
        .expect("skills service should initialize")
    }

    #[tokio::test]
    async fn discover_skills_prefers_user_skill_on_name_collision() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        write_skill(
            &temp_dir.path().join("bundled"),
            "shared",
            "shared-skill",
            "Bundled version",
            "Bundled body\n",
        );
        write_skill(
            &temp_dir.path().join("user"),
            "shared",
            "shared-skill",
            "User version",
            "User body\n",
        );

        let service = create_service(&temp_dir);
        service
            .discover_skills()
            .await
            .expect("discovery should succeed");

        let skill = service
            .get_skill("shared-skill")
            .await
            .expect("lookup should succeed")
            .expect("skill should exist");
        assert_eq!(skill.source, crate::models::SkillSource::User);
        assert_eq!(skill.description, "User version");
    }

    #[tokio::test]
    async fn set_skill_enabled_persists_disabled_list() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        write_skill(
            &temp_dir.path().join("bundled"),
            "writer",
            "docs-writer",
            "Write docs",
            "Body\n",
        );
        let service = create_service(&temp_dir);
        service
            .discover_skills()
            .await
            .expect("discovery should succeed");

        service
            .set_skill_enabled("docs-writer", false)
            .await
            .expect("disable should succeed");

        let disabled = service
            .app_settings_service
            .get_setting(DISABLED_SKILLS_SETTING_KEY)
            .await
            .expect("settings read should succeed")
            .expect("disabled list should exist");
        assert!(disabled.contains("docs-writer"));
    }

    #[tokio::test]
    async fn set_skill_enabled_rolls_back_in_memory_state_on_persistence_error() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        write_skill(
            &temp_dir.path().join("bundled"),
            "writer",
            "docs-writer",
            "Write docs",
            "Body\n",
        );
        let service = SkillsServiceImpl::new_for_tests(
            std::sync::Arc::new(FailingSetSettingAppSettingsService),
            temp_dir.path().join("bundled"),
            temp_dir.path().join("user"),
        )
        .expect("skills service should initialize");
        service
            .discover_skills()
            .await
            .expect("discovery should succeed");

        let error = service
            .set_skill_enabled("docs-writer", false)
            .await
            .expect_err("disable should surface persistence failure");
        assert!(error.to_string().contains("simulated persistence failure"));

        let skill = service
            .get_skill("docs-writer")
            .await
            .expect("lookup should succeed")
            .expect("skill should exist");
        assert!(
            skill.enabled,
            "failed persistence should restore in-memory state"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn discover_skills_skips_symlinked_directories() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().expect("temp dir should exist");
        let bundled_dir = temp_dir.path().join("bundled");
        write_skill(
            &bundled_dir,
            "writer",
            "docs-writer",
            "Write docs",
            "Body\n",
        );
        symlink(&bundled_dir, bundled_dir.join("loop"))
            .expect("symlinked directory should be created");

        let service = create_service(&temp_dir);
        service
            .discover_skills()
            .await
            .expect("discovery should skip symlink loops");

        let skills = service
            .list_skills()
            .await
            .expect("listing skills should succeed");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "docs-writer");
    }

    #[tokio::test]
    async fn get_skill_body_returns_markdown_body_for_discovered_skill() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        write_skill(
            &temp_dir.path().join("bundled"),
            "writer",
            "docs-writer",
            "Write docs",
            "Body line one\nBody line two\n",
        );
        let service = create_service(&temp_dir);
        service
            .discover_skills()
            .await
            .expect("discovery should succeed");

        let body = service
            .get_skill_body("docs-writer")
            .await
            .expect("body lookup should succeed")
            .expect("skill body should exist");
        assert_eq!(body, "Body line one\nBody line two\n");
    }

    #[tokio::test]
    async fn discover_skills_rejects_invalid_disabled_skills_setting() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        let service = create_service(&temp_dir);
        service
            .app_settings_service
            .set_setting(DISABLED_SKILLS_SETTING_KEY, "not-json".to_string())
            .await
            .expect("settings write should succeed");

        let error = service
            .discover_skills()
            .await
            .expect_err("invalid disabled skills setting should fail discovery");
        assert!(error
            .to_string()
            .contains("Failed to parse disabled skills setting"));
    }
}
