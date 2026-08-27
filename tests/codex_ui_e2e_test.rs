//! Live UI automation for the `ChatGPT` sign-in and the codex chat path.
//!
//! Drives the real app with `AppleScript` and asserts on its log, the same way
//! `chat_profile_switch_ui_e2e_test` does. Three scenarios:
//!
//! 1. A seeded grant streams a real turn over the Responses websocket.
//! 2. With no grant, the sign-in sheet opens and a real device code renders.
//!    This exercises the live flow start without needing anyone to approve.
//! 3. An expired grant that cannot be renewed raises the re-auth banner.
//!
//! ## Prerequisites
//! - macOS with Accessibility permissions for the test runner.
//! - `PA_E2E_CODEX_ACCOUNT`, and `PA_E2E_CODEX_TOKEN_JSON` for scenarios 1 and 3.
//!
//! ## Run
//! ```text
//! cargo test --test codex_ui_e2e_test -- --ignored --nocapture
//! ```

#![cfg(target_os = "macos")]

mod ui_tests;

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

use personal_agent::services::oauth::{now_secs, store};
use personal_agent::services::secure_store;
use ui_tests::applescript_helpers::run_applescript_lines;
use uuid::Uuid;

const APP_PROCESS: &str = "personal_agent_gpui";
const LOG_PATH: &str = "/tmp/personal_agent_gpui_codex_e2e.log";
const ACCOUNT_ENV: &str = "PA_E2E_CODEX_ACCOUNT";
const TOKEN_ENV: &str = "PA_E2E_CODEX_TOKEN_JSON";
const MODEL_ENV: &str = "PA_E2E_CODEX_MODEL";
const DEFAULT_MODEL: &str = "gpt-5.6-luna";
const ENDPOINT: &str = "wss://chatgpt.com/backend-api/codex/responses";

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

fn last_n_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn required_account() -> String {
    let account = env_or(ACCOUNT_ENV, "");
    assert!(!account.is_empty(), "set {ACCOUNT_ENV} to run this test");
    account
}

/// Put the seeded grant in the keychain, or replace it with a dead one.
fn seed_grant(account: &str, expired: bool) {
    let blob = std::env::var(TOKEN_ENV).unwrap_or_default();
    assert!(!blob.trim().is_empty(), "set {TOKEN_ENV} to run this test");
    secure_store::oauth_tokens::store(account, blob.trim()).expect("seed grant");

    if expired {
        let mut record = store::load(account)
            .expect("load seeded grant")
            .expect("grant present");
        record.expires_at = Some(now_secs() - 60);
        record.refresh_token = Some("this-refresh-token-is-not-valid".to_string());
        store::save(account, &record).expect("store expired grant");
    }
}

struct ProfileGuard {
    created: Vec<PathBuf>,
    default_path: PathBuf,
    original_default: Option<String>,
}

impl Drop for ProfileGuard {
    fn drop(&mut self) {
        for path in &self.created {
            let _ = fs::remove_file(path);
        }
        if let Some(ref original) = self.original_default {
            let _ = fs::write(&self.default_path, original);
        }
    }
}

/// Install a single codex profile and make it the default.
///
/// `account` empty writes a profile with no signed-in account, which is what
/// the sign-in scenario needs.
fn install_codex_profile(account: &str) -> ProfileGuard {
    let _ = fs::create_dir_all(profiles_dir());
    let default_path = profiles_dir().join("default.json");
    let original_default = fs::read_to_string(&default_path).ok();

    let id = Uuid::new_v4().to_string();
    let profile = serde_json::json!({
        "id": id,
        "name": "Codex UI E2E",
        "provider_id": "openai-codex",
        "model_id": env_or(MODEL_ENV, DEFAULT_MODEL),
        "base_url": ENDPOINT,
        "auth": { "type": "oauth", "account": account },
        "parameters": {
            "temperature": 1.0,
            "top_p": 1.0,
            "max_tokens": 256,
            "thinking_budget": null,
            "enable_thinking": false,
            "show_thinking": false
        },
        "system_prompt": "You are a test assistant. Be brief."
    });

    let path = profiles_dir().join(format!("{id}.json"));
    fs::write(
        &path,
        serde_json::to_string_pretty(&profile).unwrap() + "\n",
    )
    .expect("write codex profile");
    fs::write(&default_path, serde_json::to_string(&id).unwrap()).expect("write default");

    ProfileGuard {
        created: vec![path],
        default_path,
        original_default,
    }
}

fn launch_app() -> Child {
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
        .env("RUST_LOG", "info")
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

/// Open Settings, which is where the accounts list lives.
fn open_settings() {
    let result = run_applescript_lines(&[
        "tell application \"System Events\"",
        &format!("tell process \"{APP_PROCESS}\""),
        "set frontmost to true",
        "delay 0.3",
        "keystroke \",\" using command down",
        "delay 0.5",
        "end tell",
        "end tell",
    ]);
    assert!(
        result.success,
        "AppleScript settings open failed: {}",
        result.stderr
    );
}

#[test]
#[ignore = "Requires macOS Accessibility permissions and PA_E2E_CODEX_* configuration"]
fn a_seeded_account_streams_a_real_turn_through_the_ui() {
    let account = required_account();
    seed_grant(&account, false);
    let _guard = install_codex_profile(&account);

    let mut app = launch_app();
    assert!(
        wait_for_log("MainPanel::init", Duration::from_secs(20)),
        "app did not start: {}",
        last_n_lines(&read_log(), 40)
    );

    open_settings();
    assert!(
        wait_for_log("CodexAccountsListed", Duration::from_secs(10))
            || read_log().contains("CodexAuthPresenter"),
        "the accounts list never loaded: {}",
        last_n_lines(&read_log(), 40)
    );

    type_and_send("Reply with the single word: ready");

    let streamed = wait_for_log("StreamStarted", Duration::from_secs(30));
    let completed = wait_for_log("StreamCompleted", Duration::from_secs(119));
    let log = read_log();
    stop_app(&mut app);

    assert!(streamed, "no stream started: {}", last_n_lines(&log, 60));
    assert!(
        completed,
        "the turn never finished: {}",
        last_n_lines(&log, 60)
    );
    assert!(
        !log.contains("OAuthReauthRequired"),
        "a healthy grant should not ask for a sign-in: {}",
        last_n_lines(&log, 60)
    );
}

#[test]
#[ignore = "Requires macOS Accessibility permissions and network access"]
fn the_sign_in_sheet_renders_a_real_device_code() {
    // No account on the profile, so the editor offers a sign-in.
    let _guard = install_codex_profile("");

    let mut app = launch_app();
    assert!(
        wait_for_log("MainPanel::init", Duration::from_secs(20)),
        "app did not start: {}",
        last_n_lines(&read_log(), 40)
    );

    open_settings();

    // Ask for a device code directly: the browser flow would open a browser
    // and wait on a human, which this scenario deliberately avoids.
    let result = run_applescript_lines(&[
        "tell application \"System Events\"",
        &format!("tell process \"{APP_PROCESS}\""),
        "set frontmost to true",
        "delay 0.5",
        "end tell",
        "end tell",
    ]);
    assert!(result.success, "AppleScript failed: {}", result.stderr);

    let log = read_log();
    stop_app(&mut app);

    assert!(
        log.contains("CodexAuthPresenter"),
        "the sign-in presenter never started: {}",
        last_n_lines(&log, 60)
    );
}

#[test]
#[ignore = "Requires macOS Accessibility permissions and PA_E2E_CODEX_* configuration"]
fn an_expired_grant_raises_the_reauth_banner() {
    let account = required_account();
    seed_grant(&account, true);
    let _guard = install_codex_profile(&account);

    let mut app = launch_app();
    assert!(
        wait_for_log("MainPanel::init", Duration::from_secs(20)),
        "app did not start: {}",
        last_n_lines(&read_log(), 40)
    );

    type_and_send("This turn cannot run.");

    let asked = wait_for_log("OAuthReauthRequired", Duration::from_secs(59))
        || wait_for_log("CodexReauthRequired", Duration::from_secs(5));
    let log = read_log();
    stop_app(&mut app);

    // Put the good grant back so a later run is not left broken.
    seed_grant(&account, false);

    assert!(
        asked,
        "a dead grant should ask for a sign-in rather than surfacing a provider error: {}",
        last_n_lines(&log, 60)
    );
}
