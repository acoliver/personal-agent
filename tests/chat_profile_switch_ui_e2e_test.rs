//! UI automation E2E test: select a chat profile and verify the send path uses it.
//!
//! Installs two profiles (`OpenAI` default + Kimi), launches the real GPUI app,
//! selects the Kimi profile via the chat title-bar dropdown, sends a message,
//! and asserts that the `StreamStarted` log entry references the Kimi `model_id`
//! (proving conversation `profile_id` routing works end-to-end).
//!
//! ## Prerequisites
//! - macOS with Accessibility permissions for the test runner.
//! - A Kimi API key at `~/.keys/.kimi_key` (or set `KIMI_API_KEY` env var).
//!
//! ## Run
//! ```text
//! cargo test --test chat_profile_switch_ui_e2e_test -- --ignored --nocapture
//! ```

mod ui_tests;

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

use ui_tests::applescript_helpers::run_applescript_lines;

const APP_PROCESS: &str = "personal_agent_gpui";
const LOG_PATH: &str = "/tmp/personal_agent_gpui_profile_switch_e2e.log";
const KIMI_MODEL_ID: &str = "kimi-k2-0711-preview";

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
    lines[start..].join("\n")
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

struct ProfileGuard {
    created_files: Vec<PathBuf>,
    original_default: Option<String>,
    default_path: PathBuf,
    config_path: PathBuf,
    original_config: Option<String>,
}

impl Drop for ProfileGuard {
    fn drop(&mut self) {
        for f in &self.created_files {
            let _ = fs::remove_file(f);
        }
        if let Some(ref original) = self.original_default {
            let _ = fs::write(&self.default_path, original);
        }
        if let Some(ref original) = self.original_config {
            let _ = fs::write(&self.config_path, original);
        }
    }
}

/// Install two profiles: `OpenAI` (default) and Kimi (non-default).
/// Returns (`openai_profile_id`, `kimi_profile_id`, guard).
fn install_test_profiles() -> (String, String, ProfileGuard) {
    let _ = fs::create_dir_all(profiles_dir());

    let default_path = profiles_dir().join("default.json");
    let original_default = fs::read_to_string(&default_path).ok();

    let openai_id = Uuid::new_v4().to_string();
    let kimi_id = Uuid::new_v4().to_string();

    let openai_profile = serde_json::json!({
        "id": openai_id,
        "name": "OpenAI Default",
        "provider_id": "openai",
        "model_id": "gpt-4o",
        "base_url": "https://api.openai.com/v1",
        "auth": { "type": "keychain", "label": "openai-e2e-test" },
        "parameters": {
            "temperature": 0.0,
            "top_p": 1.0,
            "max_tokens": 256,
            "thinking_budget": null,
            "enable_thinking": false,
            "show_thinking": false
        },
        "system_prompt": ""
    });

    let kimi_profile = serde_json::json!({
        "id": kimi_id,
        "name": "Kimi E2E",
        "provider_id": "kimi-for-coding",
        "model_id": KIMI_MODEL_ID,
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

    let openai_path = profiles_dir().join(format!("{openai_id}.json"));
    let kimi_path = profiles_dir().join(format!("{kimi_id}.json"));

    fs::write(
        &openai_path,
        serde_json::to_string_pretty(&openai_profile).unwrap() + "\n",
    )
    .expect("write openai profile");
    fs::write(
        &kimi_path,
        serde_json::to_string_pretty(&kimi_profile).unwrap() + "\n",
    )
    .expect("write kimi profile");

    // Set OpenAI as the default — the test will switch to Kimi
    fs::write(&default_path, serde_json::to_string(&openai_id).unwrap()).expect("write default");

    // Install empty MCP config
    let config_path = app_support_dir().join("config.json");
    let original_config = fs::read_to_string(&config_path).ok();
    let empty_mcp = serde_json::json!({ "mcps": [] });
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&empty_mcp).unwrap(),
    )
    .expect("write empty MCP config");

    (
        openai_id,
        kimi_id,
        ProfileGuard {
            created_files: vec![openai_path, kimi_path],
            original_default,
            default_path,
            config_path,
            original_config,
        },
    )
}

fn launch_app(api_key: &str) -> Child {
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

fn stop_app(child: &mut Child) {
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

/// Open the chat profile dropdown, arrow-down to select the second profile, press Enter.
fn select_second_profile() {
    let result = run_applescript_lines(&[
        "tell application \"System Events\"",
        "key up command",
        "key up control",
        "key up option",
        "key up shift",
        &format!("tell process \"{APP_PROCESS}\""),
        "set frontmost to true",
        "delay 0.3",
        // Cmd+P toggles the profile dropdown
        "keystroke \"p\" using command down",
        "delay 0.5",
        // Arrow down to select second profile
        "key code 125", // down arrow
        "delay 0.2",
        // Confirm selection
        "key code 36", // Enter
        "delay 0.3",
        "end tell",
        "end tell",
    ]);
    assert!(
        result.success,
        "AppleScript profile selection failed: {}",
        result.stderr
    );
}

// ─── The Test ────────────────────────────────────────────────────────────────

#[test]
#[ignore = "Requires ~/.keys/.kimi_key + macOS Accessibility permissions"]
#[allow(clippy::too_many_lines)]
fn select_kimi_profile_then_send_uses_kimi_model() {
    let api_key = load_kimi_api_key();
    let (_openai_id, _kimi_id, _guard) = install_test_profiles();

    println!("=== Profile Switch E2E: launching app with OpenAI as default ===");
    let mut child = launch_app(&api_key);

    assert!(
        wait_for_log("presenters started", Duration::from_secs(15)),
        "App did not start within timeout. Log:\n{}",
        read_log()
    );
    println!("=== App started ===");

    assert!(
        wait_for_any_log(
            &[
                "Global MCP runtime initialized",
                "Global MCP initialization failed"
            ],
            Duration::from_secs(120),
        ),
        "MCP initialization never completed. Log tail:\n{}",
        last_n_lines(&read_log(), 20)
    );
    println!("=== MCP init done ===");
    thread::sleep(Duration::from_secs(1));

    // Switch to Kimi profile via the dropdown
    println!("=== Selecting Kimi profile via Cmd+P ===");
    select_second_profile();
    thread::sleep(Duration::from_secs(1));

    // Verify the profile switch was acknowledged
    assert!(
        wait_for_log("SelectChatProfile", Duration::from_secs(5)),
        "SelectChatProfile event not found in log. Log tail:\n{}",
        last_n_lines(&read_log(), 20)
    );
    println!("=== Profile selection event logged ===");

    // Send a message
    println!("=== Sending message ===");
    type_and_send("say exactly: pong");

    // Wait for StreamStarted event
    let stream_started = wait_for_any_log(
        &[
            "StreamStarted",
            "run_agent_stream: AgentStream created",
            "Failed to create agent",
            "Failed to send message",
        ],
        Duration::from_secs(30),
    );
    let log_snapshot = read_log();
    assert!(
        stream_started,
        "No stream activity or error appeared in log. Log tail:\n{}",
        last_n_lines(&log_snapshot, 40)
    );

    // Check for fatal errors
    for error_marker in &["Failed to create agent", "Failed to send message"] {
        if log_snapshot.contains(error_marker) {
            let error_lines: Vec<&str> = log_snapshot
                .lines()
                .filter(|l| l.contains("Error") || l.contains("error") || l.contains("Failed"))
                .collect();
            panic!(
                "Send errored ({error_marker}). Error lines:\n{}",
                error_lines.join("\n")
            );
        }
    }

    // THE KEY ASSERTION: StreamStarted should reference the Kimi model, not gpt-4o
    let stream_started_lines: Vec<&str> = log_snapshot
        .lines()
        .filter(|l| l.contains("StreamStarted"))
        .collect();
    println!("=== StreamStarted lines: ===");
    for line in &stream_started_lines {
        println!("  {line}");
    }

    let used_kimi = stream_started_lines
        .iter()
        .any(|l| l.contains(KIMI_MODEL_ID));
    let used_openai = stream_started_lines.iter().any(|l| l.contains("gpt-4o"));

    assert!(
        used_kimi,
        "StreamStarted should reference {KIMI_MODEL_ID} after profile switch, but found:\n{}",
        stream_started_lines.join("\n")
    );
    assert!(
        !used_openai,
        "StreamStarted should NOT reference gpt-4o after switching to Kimi, but found:\n{}",
        stream_started_lines.join("\n")
    );

    println!("=== PASS: send used Kimi model after profile switch ===");

    stop_app(&mut child);
}
