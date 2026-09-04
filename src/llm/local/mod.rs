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
