//! In-process local model support: a llama.cpp engine behind a serdes
//! `Model`, with no sockets and no subprocess.
//!
//! One engine serves every `provider_id == "local"` profile. The actor thread
//! is spawned lazily on first use (two boot service stacks exist, so a
//! `OnceLock` singleton is the only sane owner) and lives for the process;
//! the model itself loads on demand and unloads on idle, on profile
//! invalidation, or on request.
//!
//! The submodules are `pub` only so integration tests can drive the
//! [`generator::Generator`] seam with a scripted implementation; the surface
//! is otherwise internal to the app.
//!
// @plan:PLAN-20260903-LOCALMODEL.P02
// @requirement:REQ-LM-003 REQ-LM-004 REQ-LM-005

use std::sync::Arc;

use serdes_ai::models::Model;

pub mod engine;
pub mod generator;
pub mod llama_model;
pub mod render;
pub mod toolcall;

/// Provider id that routes to the in-process engine instead of an HTTP
/// endpoint.
pub const LOCAL_PROVIDER_ID: &str = "local";

/// The context budget the shared compression pipeline may assume for a
/// profile.
///
/// Remote profiles use their own configured window; local profiles budget
/// against the engine's real window minus the room the answer needs, because
/// a local profile's `context_window_size` (128k default) is disconnected
/// from the engine's `n_ctx` and trusting it overflows the context the
/// engine actually decodes with.
///
/// The output reserve is the profile's `max_tokens` when it exceeds the
/// engine default, else the engine default itself, so a full-length answer
/// always fits beside the compressed history.
///
/// @requirement:REQ-LM-001
#[must_use]
pub fn effective_context_window_for(
    profile: &crate::models::ModelProfile,
    engine: &crate::services::local_model_settings::LocalModelSettings,
) -> usize {
    if profile.provider_id != LOCAL_PROVIDER_ID {
        return profile.context_window_size;
    }
    let n_ctx = usize::try_from(engine.n_ctx).unwrap_or(0);
    let reserve = profile
        .parameters
        .max_tokens
        .map_or(llama_model::DEFAULT_MAX_TOKENS, |tokens| {
            usize::try_from(tokens)
                .unwrap_or(llama_model::DEFAULT_MAX_TOKENS)
                .max(llama_model::DEFAULT_MAX_TOKENS)
        });
    n_ctx.saturating_sub(reserve)
}

/// [`effective_context_window_for`] with the engine settings read through the
/// same disk load the generation path uses.
///
/// `EngineLoadSettings::from_persisted` is that loader, so the chat-side
/// budget and the engine can never disagree. Missing settings fall back to
/// defaults via that existing loader; a corrupt blob fails the load there
/// too.
///
/// @requirement:REQ-LM-001
#[must_use]
pub fn effective_context_window(profile: &crate::models::ModelProfile) -> usize {
    if profile.provider_id != LOCAL_PROVIDER_ID {
        return profile.context_window_size;
    }
    let n_ctx = EngineLoadSettings::from_persisted().n_ctx;
    let engine = crate::services::local_model_settings::LocalModelSettings {
        n_ctx,
        ..crate::services::local_model_settings::LocalModelSettings::default()
    };
    effective_context_window_for(profile, &engine)
}

use engine::{EngineHandle, EngineLoadSettings, EngineStatus};
use llama_model::LocalLlamaModel;

static ENGINE: std::sync::OnceLock<EngineHandle> = std::sync::OnceLock::new();

/// The process-wide engine handle, spawning the actor on first touch.
fn engine() -> &'static EngineHandle {
    ENGINE.get_or_init(EngineHandle::spawn)
}

/// Builds the serdes model for a local profile.
///
/// The returned wrapper is stateless over the shared engine, so one per
/// conversation is fine; the expensive resources (backend, model, context)
/// live in the engine singleton, not here.
#[must_use]
pub fn local_model_for(profile: &crate::models::ModelProfile) -> Arc<dyn Model> {
    let generator: Arc<dyn Generator> = Arc::new(EngineGenerator);
    Arc::new(LocalLlamaModel::new(generator, profile.model_id.clone()))
}

use generator::{GenRequest, GenerateError, Generation, Generator};

/// The engine-side [`Generator`]: reads the persisted app-level settings at
/// each request so settings edits take effect on the next load.
struct EngineGenerator;

#[async_trait::async_trait]
impl Generator for EngineGenerator {
    async fn generate(&self, request: GenRequest) -> Result<Generation, GenerateError> {
        engine().start_generation(request, EngineLoadSettings::from_persisted())
    }

    fn status(&self) -> EngineStatus {
        engine().status()
    }

    async fn unload(&self) {
        engine().request_unload();
    }
}

/// Drops the resident model, if any. Called wherever a profile or the engine
/// settings change under the engine's feet.
pub fn invalidate_local() {
    engine().request_unload();
}

/// Explicit unload (settings UI Unload button); same effect as
/// [`invalidate_local`].
pub fn unload_local() {
    engine().request_unload();
}

/// Stops the engine actor and frees llama.cpp state now: drops any resident
/// model/context, releases the backend guard, and joins the thread.
///
/// This is the app quit path's quiesce point; it must run before process
/// exit machinery starts, because llama.cpp may not be live during C++
/// static teardown. Afterwards new jobs fail with the usual "engine thread
/// is gone" errors.
pub fn shutdown_local() {
    engine().shutdown();
}

/// Preloads the model with the persisted settings and reports the outcome.
///
/// # Errors
///
/// Returns the engine's failure message when the model cannot be loaded.
pub async fn load_local() -> Result<(), String> {
    engine().load(EngineLoadSettings::from_persisted()).await
}

/// A cheap snapshot of the engine state for UI status cards.
#[must_use]
pub fn status() -> EngineStatus {
    engine().status()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::profile::AuthConfig;
    use crate::services::local_model_settings::ENV_LOCK;

    /// The singleton status is readable at any time. A test process never
    /// loads weights, so the only reachable snapshots are the initial one and
    /// the backend-init failure a lost init race produces.
    #[tokio::test]
    async fn status_snapshot_is_always_readable() {
        assert!(matches!(
            status(),
            EngineStatus::NotLoaded | EngineStatus::Error { .. }
        ));
    }

    /// Unload helpers are best-effort pokes at the actor; they must succeed
    /// (or no-op) without disturbing the status cell.
    #[tokio::test]
    async fn unload_helpers_leave_a_readable_status() {
        invalidate_local();
        unload_local();
        assert!(matches!(
            status(),
            EngineStatus::NotLoaded | EngineStatus::Error { .. }
        ));
    }

    /// The wrapper handed to conversations reports the local provider and the
    /// profile's model id, with the engine's capability surface.
    #[test]
    fn local_model_for_reports_the_local_capability_surface() {
        let profile = crate::models::ModelProfile::new(
            "Granite (local)".to_string(),
            LOCAL_PROVIDER_ID.to_string(),
            "granite-4.2-3b".to_string(),
            String::new(),
            AuthConfig::None,
        );
        let model = local_model_for(&profile);
        assert_eq!(model.name(), "granite-4.2-3b");
        assert_eq!(model.system(), "local");
        let capabilities = model.profile();
        assert!(capabilities.supports_tools);
        assert!(capabilities.supports_system_messages);
        assert!(capabilities.supports_streaming);
        assert!(!capabilities.supports_images);
        assert!(!capabilities.supports_parallel_tools);
        assert!(!capabilities.supports_native_structured_output);
    }

    /// The generator-side status/unload seams read the same shared engine.
    #[tokio::test]
    async fn engine_generator_seams_read_the_shared_engine() {
        let generator = EngineGenerator;
        assert!(matches!(
            generator.status(),
            EngineStatus::NotLoaded | EngineStatus::Error { .. }
        ));
        generator.unload().await;
    }

    /// `load_local` with no resident-capable configuration fails instead of
    /// hanging. Skipped on machines where the persisted settings point at a
    /// real GGUF, so the normal test path never loads model weights.
    #[tokio::test]
    async fn load_local_errors_when_the_configured_gguf_is_absent() {
        let _env = ENV_LOCK.lock().await;
        if EngineLoadSettings::from_persisted().model_path.exists() {
            return;
        }
        let message = load_local()
            .await
            .expect_err("a missing GGUF must fail the preload");
        assert!(
            message.contains("Local model file not found")
                || message.contains("backend init failed")
                || message.contains("engine thread is gone"),
            "unexpected preload failure: {message}"
        );
    }

    /// The chat-side budget must come from the persisted engine window, not
    /// from the profile's (disconnected) 128k default: a profile window of 1
    /// must not drive the budget, while the engine window minus the output
    /// reserve must.
    #[tokio::test]
    async fn local_context_window_is_derived_from_the_persisted_engine_settings() {
        let _env = ENV_LOCK.lock().await;
        let mut profile = crate::models::ModelProfile::new(
            "Granite (local)".to_string(),
            LOCAL_PROVIDER_ID.to_string(),
            "granite-4.2-3b".to_string(),
            String::new(),
            AuthConfig::None,
        );
        profile.parameters.max_tokens = Some(4096);
        profile.context_window_size = 1;

        let n_ctx = usize::try_from(EngineLoadSettings::from_persisted().n_ctx).unwrap_or(0);
        let reserve = 4096usize.max(llama_model::DEFAULT_MAX_TOKENS);
        assert_eq!(
            effective_context_window(&profile),
            n_ctx.saturating_sub(reserve),
            "the budget must use the persisted engine window, not the profile's"
        );
    }

    /// Shutdown quiesces the singleton; later helper calls must not hang or
    /// panic even though the actor is gone.
    #[tokio::test]
    async fn shutdown_local_quiesces_and_later_calls_fail_softly() {
        shutdown_local();
        assert!(matches!(
            status(),
            EngineStatus::NotLoaded | EngineStatus::Error { .. }
        ));
        unload_local();
        let _env = ENV_LOCK.lock().await;
        if EngineLoadSettings::from_persisted().model_path.exists() {
            return;
        }
        // The singleton handle survives shutdown; jobs are rejected, not
        // queued forever.
        let message = load_local()
            .await
            .expect_err("jobs after shutdown must fail");
        assert!(
            message.contains("engine thread is gone")
                || message.contains("backend init failed")
                || message.contains("Local model file not found"),
            "unexpected post-shutdown failure: {message}"
        );
    }
}
