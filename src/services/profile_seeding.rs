//! First-install seeding of the "Granite (local)" profile (REQ-LM-002).
//!
//! @plan:PLAN-20260903-LOCALMODEL.P01
//! @requirement:REQ-LM-002

use super::{ProfileService, ServiceResult};
use crate::models::{AuthConfig, ModelParameters};

/// Name of the profile seeded on first install (REQ-LM-002).
pub const SEED_PROFILE_NAME: &str = "Granite (local)";
/// Provider id of the seeded profile; routes to the in-process engine.
pub const SEED_PROVIDER_ID: &str = "local";
/// Model label of the seeded profile; display-only for the local engine.
pub const SEED_MODEL_ID: &str = "granite-4.2-3b";

/// Create the "Granite (local)" profile and make it the default, unless a
/// local profile already exists.
///
/// Shared by first-install seeding (`initialize`) and the Settings → Local
/// Model one-click button, so an existing install ends up with exactly the
/// profile a fresh install gets (REQ-LM-002). Idempotent: a no-op when any
/// `provider_id == "local"` profile is present.
///
/// # Errors
///
/// Returns `ServiceError` when listing profiles, creating the seed profile,
/// or persisting the new default fails.
///
/// @plan:PLAN-20260903-LOCALMODEL.P05
/// @requirement:REQ-LM-002 REQ-LM-006
pub async fn ensure_local_seed_profile(service: &dyn ProfileService) -> ServiceResult<()> {
    if service
        .list()
        .await?
        .iter()
        .any(|profile| profile.provider_id.trim() == SEED_PROVIDER_ID)
    {
        return Ok(());
    }
    let profile = service
        .create(
            SEED_PROFILE_NAME.to_string(),
            SEED_PROVIDER_ID.to_string(),
            SEED_MODEL_ID.to_string(),
            None,
            AuthConfig::None,
            ModelParameters::default(),
            None,
        )
        .await?;
    service.set_default(profile.id).await
}

impl super::profile_impl::ProfileServiceImpl {
    /// Create the first-install "Granite (local)" profile via the shared
    /// seeding helper, so boot-time seeding and the one-click button produce
    /// the identical profile shape.
    ///
    /// @plan:PLAN-20260903-LOCALMODEL.P01
    /// @requirement:REQ-LM-002
    pub(super) async fn seed_default_local_profile(&self) -> Result<(), super::ServiceError> {
        ensure_local_seed_profile(self).await?;
        tracing::info!(
            "ProfileService: local seed profile '{}' present and default",
            SEED_PROFILE_NAME
        );
        Ok(())
    }
}
