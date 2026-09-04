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
