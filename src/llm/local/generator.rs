//! Generator seam between the serdes `Model` impl and the engine actor.
//!
//! `LocalLlamaModel` depends on this trait, never on the actor directly, so
//! every stream-mapping behavior is testable with a scripted generator and no
//! GGUF. Tool schemas are deliberately absent here: they only affect how the
//! prompt is *rendered*, which happens above the seam, so a generator that
//! accepted them would carry dead weight.
//!
// @plan:PLAN-20260903-LOCALMODEL.P02
// @requirement:REQ-LM-004

use std::collections::HashSet;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::Stream;

use super::engine::EngineStatus;

/// Sampling controls for one generation.
///
/// Temperature at or below 0.01 means greedy decoding; `seed` of `None` means
/// the sampler draws from entropy.
#[derive(Debug, Clone, PartialEq)]
pub struct GenSampling {
    pub temperature: f64,
    pub top_p: Option<f64>,
    pub seed: Option<u64>,
}

/// One decoded generation request: a fully rendered prompt plus its knobs.
#[derive(Debug, Clone)]
pub struct GenRequest {
    pub prompt: String,
    pub sampling: GenSampling,
    /// Upper bound on generated tokens.
    pub max_tokens: usize,
    /// Extra stop strings; generation halts when the decoded text contains one.
    pub stop: Vec<String>,
}

/// One item pushed by a running generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenEvent {
    /// A raw decoded piece. Control tokens and tool-call blocks are the
    /// consumer's problem; the engine emits exactly what the tokenizer wrote.
    Delta(String),
    /// Terminal success with token counts.
    Complete {
        prompt_tokens: usize,
        generated_tokens: usize,
    },
    /// Terminal failure (load error, decode error).
    Failed(String),
}

/// Failure to start a generation at all (as opposed to a mid-stream
/// [`GenEvent::Failed`], which travels through the event channel).
#[derive(Debug, Clone)]
pub struct GenerateError(pub String);

/// Marks a generation as cancelled the moment its consumer drops it.
///
/// The actor checks this set at token boundaries; a finished generation is
/// pruned from the set by the actor, so it stays small.
pub struct AbortGuard {
    gen_id: u64,
    cancelled: Arc<Mutex<HashSet<u64>>>,
}

impl AbortGuard {
    pub const fn new(gen_id: u64, cancelled: Arc<Mutex<HashSet<u64>>>) -> Self {
        Self { gen_id, cancelled }
    }

    /// Whether this generation has been aborted by its consumer.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
            .lock()
            .is_ok_and(|set| set.contains(&self.gen_id))
    }
}

impl Drop for AbortGuard {
    fn drop(&mut self) {
        if let Ok(mut set) = self.cancelled.lock() {
            set.insert(self.gen_id);
        }
    }
}

/// One started generation: an id for cancellation plus its event stream.
pub struct Generation {
    pub gen_id: u64,
    pub events: Pin<Box<dyn Stream<Item = GenEvent> + Send>>,
    abort: AbortGuard,
}

impl Generation {
    pub fn new(
        gen_id: u64,
        events: Pin<Box<dyn Stream<Item = GenEvent> + Send>>,
        cancelled: Arc<Mutex<HashSet<u64>>>,
    ) -> Self {
        Self {
            gen_id,
            events,
            abort: AbortGuard::new(gen_id, cancelled),
        }
    }

    /// Take the abort guard out, transferring drop responsibility to the
    /// returned stream wrapper.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        u64,
        Pin<Box<dyn Stream<Item = GenEvent> + Send>>,
        AbortGuard,
    ) {
        (self.gen_id, self.events, self.abort)
    }
}

/// The engine seam `LocalLlamaModel` programs against.
#[async_trait]
pub trait Generator: Send + Sync {
    /// Start a generation. Loading the model, if needed, happens before the
    /// returned `Generation` exists, so load errors surface here.
    ///
    /// # Errors
    ///
    /// Returns [`GenerateError`] when the engine is unreachable or the model
    /// cannot be loaded.
    async fn generate(&self, request: GenRequest) -> Result<Generation, GenerateError>;

    /// A cheap snapshot of the engine state, readable without round-tripping
    /// a job through the actor.
    fn status(&self) -> EngineStatus;

    /// Ask the engine to drop the model from memory. Best effort: the actor
    /// may be busy finishing a generation, which then completes normally.
    async fn unload(&self);
}
