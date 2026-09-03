//! CLI proof-of-concept: an agent loop that runs Granite 4.2 fully in process
//! through llama.cpp with Metal offload.
//!
//! The binary renders the chat template by hand ([`render`]), generates with
//! `llama-cpp-2`, parses Granite's XML tool-call dialect ([`toolcall`]), runs
//! the tools ([`tools`]), and feeds results back until the model answers in
//! plain prose. Everything stays on the main thread because `LlamaContext`,
//! `LlamaBatch`, and `LlamaSampler` are `!Send`.

use std::io::Write as _;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{bail, Context as _, Result};
use clap::Parser;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

mod primes;
mod render;
mod toolcall;
mod tools;

use render::ChatTurn;
use toolcall::RawToolCall;

/// How many tool-call rounds the agent may take before giving up.
const MAX_TOOL_ITERATIONS: usize = 6;

/// System instruction paired with the tool scaffold the template adds.
const SYSTEM_PROMPT: &str = "You are a helpful assistant with access to tools. \
Use a tool when the task needs a computation or a lookup. \
Tool results are the authoritative data for your answer. Some results contain \
verification markers that the caller checks for, such as a word to repeat; copy \
any such marker into your answer exactly as written so the caller can confirm \
the result was delivered to you. \
Once you have everything you need, answer the user in plain prose.";

/// Prompt used when `--prompt` is absent: it needs both tools to answer, and
/// asking for a verbatim quote proves the model consumed the tool result.
const DEFAULT_PROMPT: &str = "Find me a secure prime greater than 1000000, and also \
search the web for what a secure prime is. Report both results, quoting the \
search output exactly as returned.";

/// Command line surface of the toy.
#[derive(Debug, Parser)]
#[command(
    name = "localmodel-toy",
    about = "In-process llama.cpp agent loop proof-of-concept with Metal offload"
)]
struct Cli {
    /// Path to the Granite 4.2 GGUF file. Falls back to `GRANITE_GGUF`, then
    /// to `<repo>/tmp/models/granite-4.2-3b-Q8_0.gguf`.
    #[arg(long)]
    model: Option<PathBuf>,

    /// User prompt for the agent.
    #[arg(long)]
    prompt: Option<String>,

    /// Context window size in tokens.
    #[arg(long, default_value_t = 8192)]
    n_ctx: u32,

    /// Sampling temperature.
    #[arg(long, default_value_t = 0.1)]
    temp: f32,

    /// Sampling seed for the `dist` selector.
    #[arg(long, default_value_t = 1234)]
    seed: u32,

    /// Maximum tokens generated per assistant turn.
    #[arg(long, default_value_t = 1024)]
    max_tokens: usize,

    /// Print the full rendered prompt for every turn.
    #[arg(long)]
    verbose: bool,
}

/// Per-turn sampling and length limits.
struct GenerationConfig {
    max_tokens: usize,
    temp: f32,
    seed: u32,
}

/// What one generate call produced.
struct TurnStats {
    text: String,
    prompt_tokens: usize,
    generated_tokens: usize,
    elapsed: Duration,
}

/// End state of a complete agent run.
struct AgentOutcome {
    tool_rounds: usize,
    final_answer: String,
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<()> {
    let model_path = resolve_model_path(cli)?;
    let prompt = cli
        .prompt
        .clone()
        .unwrap_or_else(|| DEFAULT_PROMPT.to_string());
    let config = GenerationConfig {
        max_tokens: cli.max_tokens,
        temp: cli.temp,
        seed: cli.seed,
    };

    println!("loading model: {}", model_path.display());
    println!("llama.cpp logs Metal offload details on stderr");
    let backend = LlamaBackend::init().context("failed to initialise the llama.cpp backend")?;
    let model_params = LlamaModelParams::default().with_n_gpu_layers(999);
    let model = LlamaModel::load_from_file(&backend, &model_path, &model_params)
        .context("failed to load the model")?;
    println!(
        "model loaded: n_layer={} n_params={}",
        model.n_layer(),
        model.n_params()
    );
    println!("n_gpu_layers=999 (llama.cpp clamps to the model's layer count)");

    let n_ctx = NonZeroU32::new(cli.n_ctx).context("--n-ctx must be greater than zero")?;
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(Some(n_ctx))
        // The toy prefills the whole prompt in one batch, so n_batch tracks n_ctx.
        .with_n_batch(cli.n_ctx);
    let mut ctx = model
        .new_context(&backend, ctx_params)
        .context("failed to create the inference context")?;

    let outcome = run_agent_loop(&model, &mut ctx, &prompt, &config, cli.verbose)?;
    println!("\n=== final answer ===\n{}", outcome.final_answer);
    println!("\ndone: {} tool round(s)", outcome.tool_rounds);
    Ok(())
}

/// Resolves the model file from `--model`, then `GRANITE_GGUF`, then the
/// repo-relative default, failing fast when nothing exists on disk.
fn resolve_model_path(cli: &Cli) -> Result<PathBuf> {
    let path = cli
        .model
        .clone()
        .or_else(|| std::env::var_os("GRANITE_GGUF").map(PathBuf::from))
        .unwrap_or_else(default_model_path);
    if !path.is_file() {
        bail!(
            "model file not found: {} (pass --model <PATH> or set GRANITE_GGUF)",
            path.display()
        );
    }
    Ok(path)
}

/// Default location of the downloaded GGUF, relative to this crate.
fn default_model_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/models/granite-4.2-3b-Q8_0.gguf")
}

/// Renders, generates, parses, executes, and feeds back, until the model
/// answers without calling a tool.
fn run_agent_loop(
    model: &LlamaModel,
    ctx: &mut LlamaContext<'_>,
    prompt: &str,
    config: &GenerationConfig,
    verbose: bool,
) -> Result<AgentOutcome> {
    let tools = tools::default_tools();
    let mut turns = vec![
        ChatTurn::System {
            content: SYSTEM_PROMPT.to_string(),
        },
        ChatTurn::User {
            content: prompt.to_string(),
        },
    ];

    for round in 1..=MAX_TOOL_ITERATIONS {
        println!("\n=== round {round}: generating ===");
        let prompt_text = render::render(&turns, &tools, true);
        println!("prompt: {} characters", prompt_text.len());
        if verbose {
            println!("--- rendered prompt ---\n{prompt_text}\n--- end of prompt ---");
        }
        ctx.clear_kv_cache();
        let stats = generate_turn(model, ctx, &prompt_text, config)?;
        println!(
            "generated {} token(s) from {} prompt token(s) in {:.2}s",
            stats.generated_tokens,
            stats.prompt_tokens,
            stats.elapsed.as_secs_f64()
        );
        println!("throughput: {:.1} tok/s", tokens_per_second(&stats));

        let parsed = toolcall::parse_response(&stats.text)
            .context("model emitted a malformed tool-call block")?;
        if parsed.calls.is_empty() {
            return Ok(AgentOutcome {
                tool_rounds: round - 1,
                final_answer: parsed.rationale,
            });
        }

        println!("=== round {round}: {} tool call(s) ===", parsed.calls.len());
        let mut results = Vec::with_capacity(parsed.calls.len());
        for call in &parsed.calls {
            println!("-> {}({})", call.name, format_arguments(call));
            let output =
                tools::execute(&tools, call).unwrap_or_else(|error| format!("tool error: {error}"));
            println!("<- {output}");
            results.push(output);
        }
        turns.push(ChatTurn::AssistantToolCalls {
            rationale: (!parsed.rationale.is_empty()).then(|| parsed.rationale.clone()),
            calls: parsed.calls.clone(),
        });
        turns.push(ChatTurn::ToolResponses { results });
    }

    bail!("model made no final answer within {MAX_TOOL_ITERATIONS} tool rounds")
}

/// Generates one assistant turn for `prompt` and returns its text.
///
/// The caller owns the context and is responsible for clearing the KV cache
/// before each call; this function always decodes from position 0.
fn generate_turn(
    model: &LlamaModel,
    ctx: &mut LlamaContext<'_>,
    prompt: &str,
    config: &GenerationConfig,
) -> Result<TurnStats> {
    let add_bos = detect_bos_handling(model, prompt)?;
    let tokens = model
        .str_to_token(prompt, add_bos)
        .context("failed to tokenize the prompt")?;
    let prompt_tokens = tokens.len();

    let mut batch = LlamaBatch::new(prompt_tokens, 1);
    for (position, &token) in tokens.iter().enumerate() {
        batch.add(token, position as i32, &[0], position + 1 == prompt_tokens)?;
    }
    ctx.decode(&mut batch).context("prefill decode failed")?;

    // The chain must end in a selector; `sample` accepts the token itself, so
    // no explicit `accept` call may follow.
    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::temp(config.temp),
        LlamaSampler::dist(config.seed),
    ]);
    // One stateful decoder reassembles tokens that split UTF-8 code points.
    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut text = String::new();
    let mut generated_tokens = 0usize;
    let started = Instant::now();

    for next_position in (prompt_tokens as i32..).take(config.max_tokens) {
        let token = sampler.sample(ctx, batch.n_tokens() - 1);
        if model.is_eog_token(token) {
            break;
        }
        // `special = true` keeps tool-call markers such as `<tool_call>` intact.
        let piece = model
            .token_to_piece(token, &mut decoder, true, None)
            .context("failed to detokenize a generated token")?;
        text.push_str(&piece);
        print!("{piece}");
        let _ = std::io::stdout().flush();
        generated_tokens += 1;

        batch.clear();
        batch.add(token, next_position, &[0], true)?;
        ctx.decode(&mut batch)
            .context("decode failed during generation")?;
    }

    Ok(TurnStats {
        text,
        prompt_tokens,
        generated_tokens,
        elapsed: started.elapsed(),
    })
}

/// Probes whether the rendered prompt already carries the BOS piece, so the
/// tokenizer never prepends a duplicate.
fn detect_bos_handling(model: &LlamaModel, prompt: &str) -> Result<AddBos> {
    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let bos_piece = model
        .token_to_piece(model.token_bos(), &mut decoder, true, None)
        .context("failed to read the BOS token piece")?;
    if !bos_piece.is_empty() && prompt.starts_with(&bos_piece) {
        Ok(AddBos::Never)
    } else {
        Ok(AddBos::Always)
    }
}

/// Average generation speed for one turn.
fn tokens_per_second(stats: &TurnStats) -> f64 {
    let seconds = stats.elapsed.as_secs_f64();
    if seconds > 0.0 {
        stats.generated_tokens as f64 / seconds
    } else {
        0.0
    }
}

/// Formats call arguments for the human-readable transcript.
fn format_arguments(call: &RawToolCall) -> String {
    let mut parts = Vec::with_capacity(call.arguments.len());
    for (name, value) in &call.arguments {
        parts.push(format!("{name}: {value}"));
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_path() -> PathBuf {
        std::env::var_os("GRANITE_GGUF")
            .map(PathBuf::from)
            .unwrap_or_else(default_model_path)
    }

    #[test]
    #[ignore = "loads the 3.6 GB GGUF and runs Metal inference; run with: cargo test -- --ignored"]
    fn agent_loop_answers_a_prime_question_end_to_end() {
        let path = model_path();
        if !path.is_file() {
            panic!(
                "model file missing at {}; set GRANITE_GGUF or download it first",
                path.display()
            );
        }
        let backend = LlamaBackend::init().expect("backend initialises");
        let model = LlamaModel::load_from_file(
            &backend,
            &path,
            &LlamaModelParams::default().with_n_gpu_layers(999),
        )
        .expect("model loads");
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(8192))
            .with_n_batch(8192);
        let mut ctx = model.new_context(&backend, ctx_params).expect("context");
        let config = GenerationConfig {
            max_tokens: 1024,
            temp: 0.1,
            seed: 1234,
        };
        let outcome = run_agent_loop(
            &model,
            &mut ctx,
            "Use the next_secure_prime tool to find a secure prime greater than 1000000.",
            &config,
            false,
        )
        .expect("agent loop completes");
        assert!(outcome.tool_rounds >= 1, "expected at least one tool round");
        assert!(!outcome.final_answer.trim().is_empty());
        // The crate's own checker confirms whatever prime the model reported
        // through the tool actually is a safe prime above the bound.
        let tools = tools::default_tools();
        let call = RawToolCall {
            name: tools::NEXT_SECURE_PRIME.to_string(),
            arguments: vec![("after".to_string(), "1000000".to_string())],
        };
        let output = tools::execute(&tools, &call).expect("tool runs");
        assert!(output.contains("\"verified\":true"));
    }

    #[test]
    fn default_prompt_names_both_tools_implicit_tasks() {
        assert!(DEFAULT_PROMPT.contains("secure prime"));
        assert!(DEFAULT_PROMPT.contains("search the web"));
        // The verbatim-quote request is what makes the model reproduce the
        // april-fools marker in its final answer.
        assert!(DEFAULT_PROMPT.contains("exactly as returned"));
    }

    #[test]
    fn system_prompt_tells_the_model_to_answer_in_prose() {
        assert!(SYSTEM_PROMPT.contains("answer the user in plain prose"));
    }
}
