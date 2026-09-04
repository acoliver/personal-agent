//! The in-process llama.cpp engine: one actor thread owns every `!Send`
//! llama.cpp object for the whole process.
//!
//! `LlamaContext`, `LlamaBatch`, and `LlamaSampler` cannot cross threads, and
//! `LlamaContext` borrows the `LlamaModel` it was built from, so the loaded
//! pair cannot live in a struct. The actor therefore keeps them as plain
//! locals of a loop iteration: phase one blocks unloaded until a job needs the
//! model, phase two runs while the pair is alive, and leaving phase two drops
//! it. That drop is the idle unload; model memory returns to the OS.
//!
//! Process exit is orderly, not abrupt: llama.cpp work must never run during
//! C++ static teardown, because ggml registers destructors for function-local
//! statics lazily (backend init, model load, Metal device creation), and
//! atexit's LIFO order destroys them before any hook inside `main` can run.
//! The engine is therefore quiesced while the process is fully alive: the app
//! calls `shutdown_local` on quit, and dropping the last `EngineHandle`
//! stops the actor, which drops model, context, and the backend guard on its
//! own stack before the thread is joined.
//!
// @plan:PLAN-20260903-LOCALMODEL.P02
// @plan:PLAN-20260903-LOCALMODEL.P05
// @requirement:REQ-LM-003 REQ-LM-006

use std::collections::HashSet;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use tokio::sync::mpsc as tokio_mpsc;

use super::generator::{GenEvent, GenRequest, GenSampling, GenerateError, Generation};

/// The tool scaffold puts no piece between filters and temperature, so the
/// chain mirrors the `PoC`'s proven shape: trim with top-k, then min-p, then
/// temperature, then the seeded selector.
const TOP_K: i32 = 40;
const MIN_P: f32 = 0.05;

/// Snapshot of the engine state machine, readable without a round trip
/// through the actor.
// `last_tok_s: f64` keeps `Eq` out of reach. Serde is needed so the status
// can travel inside a `ViewCommand` to the settings panel.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EngineStatus {
    /// No model resident.
    NotLoaded,
    /// A GGUF is being mmap-loaded.
    Loading,
    /// Model resident; `last_tok_s` is the previous generation's throughput.
    Loaded {
        /// Metal-offloaded layer count: the requested GPU layers clamped to
        /// the model's total, mirroring llama.cpp's own clamp.
        layers: u32,
        /// Total layers in the model, from GGUF metadata; 0 when unknown,
        /// which older snapshots deserialize to via `serde(default)`.
        #[serde(default)]
        total_layers: u32,
        /// Context window the context was created with.
        n_ctx: u32,
        /// Decoded tokens per second of the last generation.
        last_tok_s: f64,
    },
    /// The last load or backend init failed.
    Error {
        /// What went wrong.
        message: String,
    },
}

/// Model-defining engine settings for one load.
///
/// Sampling knobs travel with each [`GenRequest`]; everything here decides
/// which GGUF is resident and how much memory it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineLoadSettings {
    /// GGUF file to load.
    pub model_path: PathBuf,
    /// Context window; must be non-zero (`LlamaContextParams` defaults to 512).
    pub n_ctx: u32,
    /// GPU layers to offload (llama.cpp clamps to the layer count).
    pub gpu_layers: u32,
    /// Whether the actor drops the model after [`Self::idle_timeout`].
    pub idle_unload: bool,
    /// How long the model may sit unused before an unload.
    pub idle_timeout: Duration,
}

impl EngineLoadSettings {
    /// Reads the persisted app-level settings, falling back to defaults.
    ///
    /// A synchronous read is fine here: the file is small, and this runs once
    /// per model build, not per token.
    #[must_use]
    pub fn from_persisted() -> Self {
        let persisted = crate::services::local_model_settings::app_settings_path()
            .and_then(|path| {
                crate::services::local_model_settings::try_load_from_disk(&path)
                    .ok()
                    .flatten()
            })
            .unwrap_or_default();
        Self::from_local(&persisted)
    }

    /// Maps the app-level settings type onto the engine's.
    #[must_use]
    pub fn from_local(
        settings: &crate::services::local_model_settings::LocalModelSettings,
    ) -> Self {
        Self {
            model_path: settings.model_path.clone(),
            n_ctx: settings.n_ctx,
            gpu_layers: settings.gpu_layers,
            idle_unload: settings.idle_unload,
            idle_timeout: Duration::from_secs(u64::from(settings.idle_timeout_minutes) * 60),
        }
    }

    /// Whether two settings describe the same resident model.
    fn same_model(&self, other: &Self) -> bool {
        self.model_path == other.model_path
            && self.n_ctx == other.n_ctx
            && self.gpu_layers == other.gpu_layers
    }
}

enum Job {
    /// Preload (or reload after a settings change) the model.
    Load {
        settings: EngineLoadSettings,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// Drop the resident model.
    Unload,
    /// Run one generation, streaming `GenEvent`s to the sender.
    Generate {
        gen_id: u64,
        request: GenRequest,
        settings: EngineLoadSettings,
        events: tokio_mpsc::UnboundedSender<GenEvent>,
    },
    /// Exit the actor loop, dropping model/context/backend first. Sent by the
    /// at-exit hook and explicit shutdown so process exit never tears
    /// llama.cpp down underneath a live engine thread.
    Shutdown,
}

/// The two stop conditions shared between the handle and the actor: a
/// generation's cancel entry, and the engine-wide shutdown flag.
struct StopFlags {
    cancelled: Arc<Mutex<HashSet<u64>>>,
    shutting_down: Arc<AtomicBool>,
}

/// One live engine. The `EngineHandle` is the only strong owner, so dropping
/// the last handle stops the actor and frees llama.cpp state while the
/// process is still fully alive.
struct EngineEntry {
    tx: mpsc::Sender<Job>,
    join: Mutex<Option<std::thread::JoinHandle<()>>>,
    stop: StopFlags,
}

/// Stops one engine: raises its shutdown flag, tells the actor to exit, and
/// joins the thread.
///
/// The join is the determinism guarantee: when it returns, the actor has
/// dropped any resident model, context, and the `LlamaBackend` guard on its
/// own stack. That must happen before process exit machinery starts — during
/// finalize, ggml's lazily-registered statics are already destroyed and any
/// llama.cpp call faults.
fn shutdown_engine(entry: &EngineEntry) {
    entry.stop.shutting_down.store(true, Ordering::SeqCst);
    let _ = entry.tx.send(Job::Shutdown);
    let join = entry
        .join
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    if let Some(join) = join {
        let _ = join.join();
    }
}

/// Process-wide handle to the engine actor.
pub struct EngineHandle {
    entry: Arc<EngineEntry>,
    status: Arc<Mutex<EngineStatus>>,
    next_gen_id: AtomicU64,
}

impl EngineHandle {
    /// Spawns the actor thread and returns its handle.
    ///
    /// Called at most once (the caller holds an `OnceLock`), because
    /// `LlamaBackend::init` is process-global. Each engine also registers an
    /// at-exit hook so its thread shuts down before llama.cpp's static
    /// destructors run.
    #[must_use]
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<Job>();
        let status = Arc::new(Mutex::new(EngineStatus::NotLoaded));
        let stop = StopFlags {
            cancelled: Arc::new(Mutex::new(HashSet::new())),
            shutting_down: Arc::new(AtomicBool::new(false)),
        };
        let thread_status = Arc::clone(&status);
        let thread_stop = StopFlags {
            cancelled: Arc::clone(&stop.cancelled),
            shutting_down: Arc::clone(&stop.shutting_down),
        };
        let spawned = std::thread::Builder::new()
            .name("local-model-engine".to_string())
            .spawn(move || actor_entry(&rx, &thread_status, &thread_stop));
        if let Err(error) = &spawned {
            // The engine cannot start without its thread; record why so the
            // first request fails with the real cause instead of a hang.
            set_status(
                &status,
                EngineStatus::Error {
                    message: format!("failed to spawn engine thread: {error}"),
                },
            );
        }
        let entry = Arc::new(EngineEntry {
            tx,
            join: Mutex::new(spawned.ok()),
            stop,
        });
        Self {
            entry,
            status,
            next_gen_id: AtomicU64::new(0),
        }
    }

    /// Starts a generation, returning its event stream.
    ///
    /// # Errors
    ///
    /// Returns [`GenerateError`] when the actor thread is gone.
    pub fn start_generation(
        &self,
        request: GenRequest,
        settings: EngineLoadSettings,
    ) -> Result<Generation, GenerateError> {
        let gen_id = self.next_gen_id.fetch_add(1, Ordering::Relaxed);
        let (events_tx, events_rx) = tokio_mpsc::unbounded_channel();
        self.entry
            .tx
            .send(Job::Generate {
                gen_id,
                request,
                settings,
                events: events_tx,
            })
            .map_err(|_| GenerateError("local model engine thread is gone".to_string()))?;
        Ok(Generation::new(
            gen_id,
            Box::pin(ReceiverStream::new(events_rx)),
            Arc::clone(&self.entry.stop.cancelled),
        ))
    }

    /// Asks the actor to drop the resident model. Best effort: a running
    /// generation finishes first.
    pub fn request_unload(&self) {
        let _ = self.entry.tx.send(Job::Unload);
    }

    /// Stops the actor thread, letting it drop any resident model, context,
    /// and the backend guard while the process is fully alive. Idempotent;
    /// later jobs fail with the usual "engine thread is gone" errors.
    pub fn shutdown(&self) {
        shutdown_engine(&self.entry);
    }

    /// The current status snapshot.
    #[must_use]
    pub fn status(&self) -> EngineStatus {
        self.status.lock().map_or_else(
            |_| EngineStatus::Error {
                message: "engine status poisoned".to_string(),
            },
            |guard| guard.clone(),
        )
    }

    /// Preloads the model and waits for the load to finish.
    ///
    /// # Errors
    ///
    /// Returns the load failure message.
    pub async fn load(&self, settings: EngineLoadSettings) -> Result<(), String> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.entry
            .tx
            .send(Job::Load {
                settings,
                reply: reply_tx,
            })
            .map_err(|_| "local model engine thread is gone".to_string())?;
        reply_rx
            .await
            .map_err(|_| "engine dropped the load reply".to_string())?
    }
}

/// The exit-time contract: llama.cpp must never be live during C++ static
/// teardown (ggml destroys its lazily-registered statics first, and any
/// later `llama_free` faults). When the last handle goes, quiesce the engine
/// while the process is still fully alive.
impl Drop for EngineHandle {
    fn drop(&mut self) {
        if Arc::strong_count(&self.entry) == 1 {
            shutdown_engine(&self.entry);
        }
    }
}

fn actor_entry(rx: &mpsc::Receiver<Job>, status: &Arc<Mutex<EngineStatus>>, stop: &StopFlags) {
    // LlamaBackend::init is process-global and refuses a second call, so the
    // guard lives on this thread for the process lifetime.
    let backend = match LlamaBackend::init() {
        Ok(backend) => backend,
        Err(error) => {
            set_status(
                status,
                EngineStatus::Error {
                    message: format!("llama.cpp backend init failed: {error}"),
                },
            );
            drain_failed(rx, status);
            return;
        }
    };
    actor_loop(&backend, rx, status, stop);
}

/// Answers every queued job with a failure after an unrecoverable init error.
fn drain_failed(rx: &mpsc::Receiver<Job>, status: &Mutex<EngineStatus>) {
    let message = match &*status
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
    {
        EngineStatus::Error { message } => message.clone(),
        _ => "local model engine failed".to_string(),
    };
    for job in rx {
        match job {
            Job::Load { reply, .. } => {
                let _ = reply.send(Err(message.clone()));
            }
            Job::Generate { events, .. } => {
                let _ = events.send(GenEvent::Failed(message.clone()));
            }
            Job::Unload => {}
            // Exiting the drain ends the thread, which is what a shutdown of
            // a failed-to-init engine means.
            Job::Shutdown => return,
        }
    }
}

/// The user-facing message for a missing GGUF. The fix travels inside the
/// error string so a chat toast, the error log, and the settings card all
/// point at the one place the file can be chosen.
#[must_use]
pub fn missing_model_file_message(path: &Path) -> String {
    format!(
        "Local model file not found: {}. Pick the GGUF file in Settings → Local Model.",
        path.display()
    )
}

// One coherent state machine: splitting the load/resident phases apart would
// scatter the `model`/`ctx` borrows and the deferred-job replay.
#[allow(clippy::too_many_lines)]
fn actor_loop(
    backend: &LlamaBackend,
    rx: &mpsc::Receiver<Job>,
    status: &Mutex<EngineStatus>,
    stop: &StopFlags,
) {
    // A job that arrived for the wrong resident model is replayed after the
    // unload; the channel cannot take it back.
    let mut deferred: Option<Job> = None;
    loop {
        // No NotLoaded reset here: a failed load must stay `Error` until the
        // next job arrives, and later iterations end NotLoaded at the unload.
        //
        // Phase 1: unloaded. Block until a job names the model to load.
        let mut load_reply = None;
        let mut pending_gen = None;
        let settings;
        loop {
            let job = match deferred.take() {
                Some(job) => job,
                None => match rx.recv() {
                    Ok(job) => job,
                    Err(_) => return,
                },
            };
            match job {
                Job::Unload => {}
                // Nothing resident: exiting the loop ends the thread.
                Job::Shutdown => return,
                Job::Load {
                    settings: next,
                    reply,
                } => {
                    settings = next;
                    load_reply = Some(reply);
                    break;
                }
                Job::Generate {
                    gen_id,
                    request,
                    settings: next,
                    events,
                } => {
                    settings = next;
                    pending_gen = Some((gen_id, request, events));
                    break;
                }
            }
        }

        set_status(status, EngineStatus::Loading);
        let mut fail_load = |message: String| {
            if let Some(reply) = load_reply.take() {
                let _ = reply.send(Err(message.clone()));
            }
            if let Some((_, _, events)) = pending_gen.take() {
                let _ = events.send(GenEvent::Failed(message.clone()));
            }
            set_status(status, EngineStatus::Error { message });
        };
        // llama-cpp-2 panics on a missing model file instead of returning an
        // error; a bad path must fail the job, not kill the actor thread.
        if !settings.model_path.exists() {
            fail_load(missing_model_file_message(&settings.model_path));
            continue;
        }
        // `ctx` borrows `model`, so both stay locals of this iteration and the
        // context is created before any value moves.
        let model = match LlamaModel::load_from_file(
            backend,
            &settings.model_path,
            &LlamaModelParams::default().with_n_gpu_layers(settings.gpu_layers),
        ) {
            Ok(model) => model,
            Err(error) => {
                fail_load(format!(
                    "failed to load {}: {error}",
                    settings.model_path.display()
                ));
                continue;
            }
        };
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(settings.n_ctx))
            .with_n_batch(settings.n_ctx);
        let mut ctx = match model.new_context(backend, ctx_params) {
            Ok(ctx) => ctx,
            Err(error) => {
                fail_load(format!("failed to create inference context: {error}"));
                continue;
            }
        };
        if let Some(reply) = load_reply.take() {
            let _ = reply.send(Ok(()));
        }

        // Phase 2: model resident. Serve jobs until Unload, Shutdown, or the
        // idle timer.
        let mut last_tok_s = if let Some((gen_id, request, events)) = pending_gen {
            run_generation(&model, &mut ctx, &request, &events, stop, gen_id)
        } else {
            0.0
        };
        update_loaded_status(status, &model, &settings, last_tok_s);
        let mut idle_deadline = settings
            .idle_unload
            .then(|| Instant::now() + settings.idle_timeout);
        let mut shutdown_requested = false;
        loop {
            let job = match idle_deadline {
                Some(deadline) => {
                    match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                        Ok(job) => Some(job),
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }
                None => match rx.recv() {
                    Ok(job) => Some(job),
                    Err(_) => return,
                },
            };
            match job {
                // Idle timer fired: fall through to the unload.
                None | Some(Job::Unload) => break,
                Some(Job::Shutdown) => {
                    shutdown_requested = true;
                    break;
                }
                Some(Job::Load {
                    settings: next,
                    reply,
                }) => {
                    if settings.same_model(&next) {
                        let _ = reply.send(Ok(()));
                    } else {
                        deferred = Some(Job::Load {
                            settings: next,
                            reply,
                        });
                        break;
                    }
                }
                Some(Job::Generate {
                    gen_id,
                    request,
                    settings: next,
                    events,
                }) => {
                    if settings.same_model(&next) {
                        last_tok_s =
                            run_generation(&model, &mut ctx, &request, &events, stop, gen_id);
                        update_loaded_status(status, &model, &settings, last_tok_s);
                    } else {
                        deferred = Some(Job::Generate {
                            gen_id,
                            request,
                            settings: next,
                            events,
                        });
                        break;
                    }
                }
            }
            if settings.idle_unload {
                idle_deadline = Some(Instant::now() + settings.idle_timeout);
            }
        }
        // Dropping `ctx` and `model` here is the unload: llama.cpp frees its
        // allocations and the mmap is released. On shutdown the backend guard
        // drops right after, when `actor_entry` returns, before the join in
        // `shutdown_engine` completes.
        drop(ctx);
        drop(model);
        set_status(status, EngineStatus::NotLoaded);
        if shutdown_requested {
            return;
        }
    }
}

fn update_loaded_status(
    status: &Mutex<EngineStatus>,
    model: &LlamaModel,
    settings: &EngineLoadSettings,
    last_tok_s: f64,
) {
    // llama.cpp clamps the requested gpu layer count to the model's total;
    // reproducing that clamp here keeps the N/M pair the status card shows
    // honest for both partial offload and the "999 = all" sentinel.
    let total_layers = model.n_layer();
    set_status(
        status,
        EngineStatus::Loaded {
            layers: settings.gpu_layers.min(total_layers),
            total_layers,
            n_ctx: settings.n_ctx,
            last_tok_s,
        },
    );
}

/// Runs one generation to completion on the actor thread, streaming deltas.
///
/// Returns the decoded tokens-per-second, or 0.0 when the generation failed
/// (the failure itself travels through the event channel).
#[allow(clippy::cast_possible_truncation)]
fn run_generation(
    model: &LlamaModel,
    ctx: &mut LlamaContext<'_>,
    request: &GenRequest,
    events: &tokio_mpsc::UnboundedSender<GenEvent>,
    stop: &StopFlags,
    gen_id: u64,
) -> f64 {
    let started = Instant::now();
    let outcome = generate_turn(model, ctx, request, events, stop, gen_id);
    let _ = stop
        .cancelled
        .lock()
        .is_ok_and(|mut set| set.remove(&gen_id));
    match outcome {
        Ok((prompt_tokens, generated_tokens)) => {
            let elapsed = started.elapsed().as_secs_f64();
            let _ = events.send(GenEvent::Complete {
                prompt_tokens,
                generated_tokens,
            });
            if elapsed > 0.0 {
                f64::from(generated_tokens as u32) / elapsed
            } else {
                0.0
            }
        }
        Err(message) => {
            let _ = events.send(GenEvent::Failed(message));
            0.0
        }
    }
}

/// The `PoC`'s verified decode loop, ported: full prefill from position 0, then
/// one token per decode with the sampler accepting internally.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
fn generate_turn(
    model: &LlamaModel,
    ctx: &mut LlamaContext<'_>,
    request: &GenRequest,
    events: &tokio_mpsc::UnboundedSender<GenEvent>,
    stop: &StopFlags,
    gen_id: u64,
) -> Result<(usize, usize), String> {
    ctx.clear_kv_cache();
    let add_bos = detect_bos_handling(model, &request.prompt)?;
    let tokens = model
        .str_to_token(&request.prompt, add_bos)
        .map_err(|error| format!("failed to tokenize the prompt: {error}"))?;
    let prompt_tokens = tokens.len();
    if prompt_tokens == 0 {
        return Ok((0, 0));
    }

    let mut batch = LlamaBatch::new(prompt_tokens, 1);
    for (position, &token) in tokens.iter().enumerate() {
        batch
            .add(token, position as i32, &[0], position + 1 == prompt_tokens)
            .map_err(|error| format!("prefill batch add failed: {error}"))?;
    }
    ctx.decode(&mut batch)
        .map_err(|error| format!("prefill decode failed: {error}"))?;

    let mut sampler = build_sampler(&request.sampling);
    // One stateful decoder reassembles tokens that split UTF-8 code points.
    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut text = String::new();
    let mut generated_tokens = 0usize;

    for next_position in (prompt_tokens as i32..).take(request.max_tokens) {
        if is_cancelled(stop, gen_id) {
            break;
        }
        let token = sampler.sample(ctx, batch.n_tokens() - 1);
        if model.is_eog_token(token) {
            break;
        }
        // `special = true` keeps tool-call markers such as `<tool_call>` intact.
        let piece = model
            .token_to_piece(token, &mut decoder, true, None)
            .map_err(|error| format!("failed to detokenize a generated token: {error}"))?;
        text.push_str(&piece);
        let _ = events.send(GenEvent::Delta(piece));
        generated_tokens += 1;
        if request
            .stop
            .iter()
            .any(|stop| !stop.is_empty() && text.contains(stop.as_str()))
        {
            break;
        }
        batch.clear();
        batch
            .add(token, next_position, &[0], true)
            .map_err(|error| format!("decode batch add failed: {error}"))?;
        ctx.decode(&mut batch)
            .map_err(|error| format!("decode failed during generation: {error}"))?;
    }

    Ok((prompt_tokens, generated_tokens))
}

/// Greedy at (or near) zero temperature; otherwise the `PoC` chain with `top_p`
/// inserted when the profile supplies one. The chain ends in a selector, and
/// `sample` accepts the token itself, so no explicit accept may follow.
#[allow(clippy::cast_possible_truncation)]
fn build_sampler(sampling: &GenSampling) -> LlamaSampler {
    let temperature = sampling.temperature as f32;
    if temperature <= 0.01 {
        return LlamaSampler::greedy();
    }
    let mut stages = vec![LlamaSampler::top_k(TOP_K)];
    if let Some(top_p) = sampling.top_p {
        stages.push(LlamaSampler::top_p(top_p as f32, 1));
    }
    stages.push(LlamaSampler::min_p(MIN_P, 1));
    stages.push(LlamaSampler::temp(temperature));
    // `u32::MAX` is llama.cpp's default seed, meaning entropy.
    stages.push(LlamaSampler::dist(
        sampling.seed.map_or(u32::MAX, |seed| seed as u32),
    ));
    LlamaSampler::chain_simple(stages)
}

/// Probes whether the rendered prompt already carries the BOS piece so the
/// tokenizer never prepends a duplicate.
fn detect_bos_handling(model: &LlamaModel, prompt: &str) -> Result<AddBos, String> {
    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let bos_piece = model
        .token_to_piece(model.token_bos(), &mut decoder, true, None)
        .map_err(|error| format!("failed to read the BOS token piece: {error}"))?;
    if !bos_piece.is_empty() && prompt.starts_with(&bos_piece) {
        Ok(AddBos::Never)
    } else {
        Ok(AddBos::Always)
    }
}

/// Shutdown shares the cancellation path: an in-flight generation stops at
/// the next token boundary, so the exit-time join is bounded by one decode.
fn is_cancelled(stop: &StopFlags, gen_id: u64) -> bool {
    stop.shutting_down.load(Ordering::SeqCst)
        || stop.cancelled.lock().is_ok_and(|set| set.contains(&gen_id))
}

fn set_status(status: &Mutex<EngineStatus>, next: EngineStatus) {
    if let Ok(mut guard) = status.lock() {
        *guard = next;
    }
}

/// Bridge from the actor's tokio channel to the `Stream` the `Model` returns.
struct ReceiverStream(tokio_mpsc::UnboundedReceiver<GenEvent>);

impl ReceiverStream {
    const fn new(receiver: tokio_mpsc::UnboundedReceiver<GenEvent>) -> Self {
        Self(receiver)
    }
}

impl futures::Stream for ReceiverStream {
    type Item = GenEvent;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<GenEvent>> {
        // The receiver is `Unpin`, so getting a plain `&mut` is sound here.
        self.get_mut().0.poll_recv(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // @plan:PLAN-20260903-LOCALMODEL.P05
    // @requirement:REQ-LM-006
    #[test]
    fn missing_file_message_carries_the_fix() {
        let message = missing_model_file_message(Path::new("/models/granite.gguf"));
        assert_eq!(
            message,
            "Local model file not found: /models/granite.gguf. Pick the GGUF file in \
             Settings → Local Model."
        );
    }
}
