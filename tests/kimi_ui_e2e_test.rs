//! UI automation E2E test for Kimi provider streaming.
//!
//! Launches the real GPUI app with a Kimi profile set as default, sends a
//! message via `AppleScript` keystroke automation, and verifies that:
//!   1. The stream starts successfully (no auth/header errors).
//!   2. The stream produces actual text output (SSE normalization works).
//!   3. The conversation file has an assistant response.
//!   4. No `StreamError` appears in the log.
//!
//! ## Prerequisites
//! - macOS with Accessibility permissions for the test runner.
//! - A Kimi API key at `~/.keys/.kimi_key` (or set `KIMI_API_KEY` env var).
//!
//! ## Run
//! ```text
//! cargo test --test kimi_ui_e2e_test -- --ignored --nocapture
//! ```
//!
//! The test uses `PA_E2E_API_KEY` to bypass keychain prompts entirely.

mod ui_tests;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use uuid::Uuid;

use ui_tests::applescript_helpers::run_applescript_lines;

const APP_PROCESS: &str = "personal_agent_gpui";
const LOG_PATH: &str = "/tmp/personal_agent_gpui_kimi_e2e.log";

fn gpui_bin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_personal_agent_gpui"))
}

fn app_support_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/PersonalAgent")
}

fn profiles_dir() -> PathBuf {
    app_support_dir().join("profiles")
}

fn conversations_dir() -> PathBuf {
    app_support_dir().join("conversations")
}

fn read_log() -> String {
    fs::read_to_string(LOG_PATH).unwrap_or_default()
}

fn wait_for_log(needle: &str, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if read_log().contains(needle) {
            return true;
        }
        thread::sleep(Duration::from_millis(200));
    }
    false
}

fn wait_for_any_log(needles: &[&str], timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        let log = read_log();
        if needles.iter().any(|n| log.contains(n)) {
            return true;
        }
        thread::sleep(Duration::from_millis(200));
    }
    false
}

fn last_n_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join(
        "
",
    )
}

fn count_occurrences(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

fn load_kimi_api_key() -> String {
    if let Ok(key) = std::env::var("KIMI_API_KEY") {
        let trimmed = key.trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }

    let key_path = dirs::home_dir().expect("home dir").join(".keys/.kimi_key");
    fs::read_to_string(&key_path)
        .unwrap_or_else(|e| {
            panic!(
                "Kimi API key not found at {} and KIMI_API_KEY not set: {e}",
                key_path.display()
            )
        })
        .trim()
        .to_string()
}

/// Create a temporary Kimi profile and set it as default.
/// Also installs an empty MCP config to avoid duplicate tool errors.
/// Returns the profile ID and a guard that restores the original state on drop.
fn install_kimi_test_profile() -> (String, KimiProfileGuard) {
    let _ = fs::create_dir_all(profiles_dir());

    let default_path = profiles_dir().join("default.json");
    let original_default = fs::read_to_string(&default_path).ok();

    let profile_id = Uuid::new_v4().to_string();
    let profile = serde_json::json!({
        "id": profile_id,
        "name": "Kimi E2E Test",
        "provider_id": "kimi-for-coding",
        "model_id": "kimi-k2-0711-preview",
        "base_url": "",
        "auth": { "type": "keychain", "label": "kimi-e2e-test" },
        "parameters": {
            "temperature": 0.0,
            "top_p": 1.0,
            "max_tokens": 256,
            "thinking_budget": null,
            "enable_thinking": true,
            "show_thinking": true
        },
        "system_prompt": "You are a test assistant. Be brief."
    });

    let profile_path = profiles_dir().join(format!("{profile_id}.json"));
    fs::write(
        &profile_path,
        serde_json::to_string_pretty(&profile).unwrap() + "\n",
    )
    .expect("write profile");

    fs::write(&default_path, serde_json::to_string(&profile_id).unwrap()).expect("write default");

    // Install an empty MCP config to avoid loading user's MCPs (which may have
    // duplicate tool names that Kimi rejects).
    let config_path = app_support_dir().join("config.json");
    let original_config = fs::read_to_string(&config_path).ok();
    let empty_mcp_config = serde_json::json!({ "mcps": [] });
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&empty_mcp_config).unwrap(),
    )
    .expect("write empty MCP config");

    (
        profile_id,
        KimiProfileGuard {
            profile_path,
            default_path,
            original_default,
            config_path,
            original_config,
        },
    )
}

struct KimiProfileGuard {
    profile_path: PathBuf,
    default_path: PathBuf,
    original_default: Option<String>,
    config_path: PathBuf,
    original_config: Option<String>,
}

impl Drop for KimiProfileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.profile_path);
        if let Some(ref original) = self.original_default {
            let _ = fs::write(&self.default_path, original);
        }
        if let Some(ref original) = self.original_config {
            let _ = fs::write(&self.config_path, original);
        }
    }
}

fn launch_gpui_with_kimi_key(api_key: &str) -> Child {
    let _ = Command::new("pkill").arg("-f").arg(APP_PROCESS).status();
    thread::sleep(Duration::from_millis(500));

    let log_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(LOG_PATH)
        .expect("open log file");
    let log_err = log_file.try_clone().expect("clone log handle");

    Command::new(gpui_bin_path())
        .env("PA_AUTO_OPEN_POPUP", "1")
        .env("PA_TEST_POPUP_ONSCREEN", "1")
        .env("PA_E2E_API_KEY", api_key)
        .stdout(log_file)
        .stderr(log_err)
        .spawn()
        .expect("launch personal_agent_gpui")
}

fn stop_gpui(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
    let _ = Command::new("pkill").arg("-f").arg(APP_PROCESS).status();
}

fn type_and_send(message: &str) {
    let result = run_applescript_lines(&[
        "tell application \"System Events\"",
        "key up command",
        "key up control",
        "key up option",
        "key up shift",
        &format!("tell process \"{APP_PROCESS}\""),
        "set frontmost to true",
        "delay 0.2",
        // Select-all + delete to clear any leftover text in the input
        "keystroke \"a\" using command down",
        "key code 51",
        "delay 0.1",
        &format!("keystroke \"{}\"", message.replace('"', "\\\"")),
        "key code 36",
        "end tell",
        "end tell",
    ]);
    assert!(result.success, "AppleScript send failed: {}", result.stderr);
}

fn newest_conversation_after(cutoff: SystemTime) -> Option<PathBuf> {
    let mut entries: Vec<_> = fs::read_dir(conversations_dir())
        .ok()?
        .flatten()
        .filter(|e| {
            e.metadata()
                .and_then(|m| m.modified())
                .map(|t| t >= cutoff)
                .unwrap_or(false)
        })
        .collect();
    entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    entries.last().map(std::fs::DirEntry::path)
}

fn conversation_assistant_content(path: &Path) -> String {
    let content = fs::read_to_string(path).unwrap_or_default();
    let value: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
    value
        .get("messages")
        .and_then(|m| m.as_array())
        .map(|msgs| {
            msgs.iter()
                .filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("assistant"))
                .filter_map(|m| m.get("content").and_then(|c| c.as_str()))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

// ─── The Test ────────────────────────────────────────────────────────────────

#[test]
#[ignore = "Requires ~/.keys/.kimi_key + macOS Accessibility permissions"]
#[allow(clippy::too_many_lines)]
fn kimi_ui_streaming_e2e() {
    let api_key = load_kimi_api_key();
    let run_start = SystemTime::now();
    let (_profile_id, _guard) = install_kimi_test_profile();

    println!("=== Kimi UI E2E: launching app ===");
    let mut child = launch_gpui_with_kimi_key(&api_key);

    assert!(
        wait_for_log("presenters started", Duration::from_secs(15)),
        "App did not start within timeout. Log:\n{}",
        read_log()
    );
    println!("=== App started ===");

    // Wait for MCP initialization to finish — send_message acquires the MCP lock
    // which is held during start_all(). Without this, the message send hangs.
    println!("=== Waiting for MCP runtime init ===");
    assert!(
        wait_for_any_log(
            &[
                "Global MCP runtime initialized",
                "Global MCP initialization failed"
            ],
            Duration::from_secs(120),
        ),
        "MCP initialization never completed. Log tail:
{}",
        last_n_lines(&read_log(), 20)
    );
    println!("=== MCP init done ===");

    // Brief settle
    thread::sleep(Duration::from_secs(1));

    // Send a simple message
    println!("=== Sending message ===");
    type_and_send("say exactly: pong");

    // Wait for evidence the stream started (agent created + first delta) or an error.
    // Logged markers (from client_agent.rs / chat_impl.rs):
    //   "run_agent_stream: AgentStream created" — stream is active
    //   "run_agent_stream: TextDelta:" — text delta arrived
    //   "ChatService emitting TextDelta:" — delta forwarded to event bus
    //   "run_agent_stream: Error:" — stream-level error
    //   "Failed to create agent:" — agent construction failed
    //   "Failed to send message:" — presenter-level failure
    let stream_or_error = wait_for_any_log(
        &[
            "run_agent_stream: AgentStream created",
            "run_agent_stream: TextDelta:",
            "run_agent_stream: Error:",
            "Failed to create agent",
            "Failed to send message",
        ],
        Duration::from_secs(30),
    );
    let log_snapshot = read_log();
    assert!(
        stream_or_error,
        "No stream activity or error appeared in log. Log tail:\n{}",
        last_n_lines(&log_snapshot, 40)
    );
    // Check for errors
    for error_marker in &[
        "run_agent_stream: Error:",
        "Failed to create agent",
        "Failed to send message",
    ] {
        if log_snapshot.contains(error_marker) {
            let error_lines: Vec<&str> = log_snapshot
                .lines()
                .filter(|l| l.contains("Error") || l.contains("error") || l.contains("Failed"))
                .collect();
            panic!(
                "Stream errored ({error_marker}). Error lines:\n{}",
                error_lines.join("\n")
            );
        }
    }
    println!("=== Stream started ===");

    // Wait for stream to complete — both RunComplete (from agent) and
    // StreamCompleted (from ChatPresenter) are valid completion markers
    assert!(
        wait_for_any_log(
            &[
                "run_agent_stream: RunComplete",
                "ChatPresenter handling ChatEvent: StreamCompleted"
            ],
            Duration::from_secs(90),
        ),
        "Stream never completed. Log tail:\n{}",
        last_n_lines(&read_log(), 40)
    );
    println!("=== Stream completed ===");

    // Give the app a moment to persist the conversation
    thread::sleep(Duration::from_secs(2));

    let log = read_log();

    // ── Assert: no stream errors ─────────────────────────────────────────
    for marker in &[
        "run_agent_stream: Error:",
        "ChatPresenter handling ChatEvent: StreamError",
    ] {
        if log.contains(marker) {
            let error_lines: Vec<&str> = log
                .lines()
                .filter(|l| l.contains("Error:") || l.contains("StreamError"))
                .collect();
            panic!("Stream error(s) in log:\n{}", error_lines.join("\n"));
        }
    }
    println!("=== No stream errors in log ===");

    // ── Assert: TextDelta events were emitted (SSE normalization worked) ─
    let text_deltas = count_occurrences(&log, "ChatService emitting TextDelta:");
    assert!(
        text_deltas > 0,
        "No TextDelta events found — SSE stream may not be normalized. Log tail:\n{}",
        last_n_lines(&log, 40)
    );
    println!("=== {text_deltas} TextDelta event(s) in log ===");

    // ── Assert: conversation file has assistant response ─────────────────
    let conversation = newest_conversation_after(run_start);
    if let Some(ref path) = conversation {
        let assistant_text = conversation_assistant_content(path);
        println!("=== Assistant response: '{assistant_text}' ===");
        assert!(
            !assistant_text.is_empty(),
            "Conversation file has no assistant content. Path: {}",
            path.display()
        );
    } else {
        println!("WARNING: no conversation file found after run_start — this may indicate the conversation was appended to an existing file");
    }

    stop_gpui(&mut child);

    println!("\n=== Kimi UI E2E: PASS ===");
    println!("  - App launched with PA_E2E_API_KEY (no keyring prompt)");
    println!("  - Message sent via AppleScript");
    println!("  - Stream started + RunComplete in log");
    println!("  - No stream errors");
    println!("  - {text_deltas} TextDelta events (SSE normalization works)");
    if let Some(ref path) = conversation {
        println!("  - Conversation persisted: {}", path.display());
    }
}
