//! Live UI automation for the `ChatGPT` sign-in and the codex chat path.
//!
//! Drives the real app with `AppleScript` and asserts on its log, the same way
//! `chat_profile_switch_ui_e2e_test` does. Three scenarios:
//!
//! 1. A seeded grant streams a real turn over the Responses websocket.
//! 2. With no grant, the app starts the sign-in presenter and the editor
//!    offers a sign-in. Rendering of a live device code is not asserted here:
//!    it needs a real request to auth.openai.com and a rendered surface to
//!    read back, which `e2e_codex_signin_device_code` covers.
//! 3. An expired grant that cannot be renewed raises the re-auth banner.
//!
//! ## Prerequisites
//! - macOS with Accessibility permissions for the test runner.
//! - `PA_E2E_CODEX_ACCOUNT` and `PA_E2E_CODEX_TOKEN_JSON` for scenarios 1 and 3.
//!
//! The grant reaches the app through `PA_E2E_CODEX_TOKEN_JSON` rather than the
//! keychain. A keychain item written by this test binary is not readable by the
//! app binary it launches: macOS scopes access per binary, and the app's read
//! blocks on a system prompt nobody answers. This mirrors how `PA_E2E_API_KEY`
//! supplies an API key for the same reason.
//!
//! These scenarios wait for the MCP runtime before typing. A machine with
//! unreachable MCP servers spends a minute timing them out, and a turn sent
//! before that never runs.
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
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use personal_agent::services::oauth::{now_secs, store, StoredOAuthToken};
use personal_agent::services::secure_store;
use ui_tests::applescript_helpers::run_applescript_lines;
use uuid::Uuid;

const APP_PROCESS: &str = "personal_agent_gpui";
/// Serializes the whole live-app lifecycle.
///
/// These scenarios each launch the app, `pkill` it by name, share one profile
/// directory and one default profile, and log to a path keyed by this
/// binary's PID, which they all share. Run in parallel they would truncate
/// each other's logs, kill each other's app, and restore each other's default
/// profile. Held from profile setup until the app is stopped.
static SCENARIO: Mutex<()> = Mutex::new(());

/// Take exclusive use of the app, the profile directory, and the log.
fn scenario_lock() -> MutexGuard<'static, ()> {
    SCENARIO
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Where the launched app writes its log.
///
/// Unique per process: sibling checkouts of this repo run their suites at the
/// same time and share /tmp, and two runs opening one path with truncation
/// interleave into a log neither can assert on.
fn log_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "personal_agent_gpui_codex_e2e_{}.log",
        std::process::id()
    ))
}
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
    fs::read_to_string(log_path()).unwrap_or_default()
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

/// Wait until the app can actually run a turn.
///
/// `MainPanel::init` only means the window exists. Chat turns queue behind the
/// MCP runtime, and a machine with unreachable MCP servers spends a minute
/// timing them out. Typing before that burns the whole assertion budget on
/// startup and the turn never runs.
fn wait_until_ready_to_send() {
    assert!(
        wait_for_log("MainPanel::init", Duration::from_secs(30)),
        "app did not start: {}",
        last_n_lines(&read_log(), 40)
    );
    assert!(
        wait_for_log("Global MCP runtime initialized", Duration::from_secs(180)),
        "app never became ready to send: {}",
        last_n_lines(&read_log(), 40)
    );
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
/// Build the grant the app will be handed, and register the account so the
/// accounts list can enumerate it.
///
/// The value itself travels by environment rather than the keychain: on macOS a
/// keychain item written by this test binary is not readable by the app binary
/// it launches, and the attempt blocks on a system prompt nobody answers.
fn seed_grant(account: &str, expired: bool) -> String {
    let blob = std::env::var(TOKEN_ENV).unwrap_or_default();
    assert!(!blob.trim().is_empty(), "set {TOKEN_ENV} to run this test");

    let mut record: StoredOAuthToken =
        serde_json::from_str(blob.trim()).expect("{TOKEN_ENV} should hold a stored grant");
    if expired {
        record.expires_at = Some(now_secs() - 60);
        record.refresh_token = Some("this-refresh-token-is-not-valid".to_string());
    }

    // Records the account in the index, which is a plain file the app can read.
    let serialized = serde_json::to_string(&record).expect("serialize grant");
    let _ = secure_store::oauth_tokens::store(account, &serialized);
    serialized
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
        match self.original_default {
            // Put back what was there.
            Some(ref original) => {
                let _ = fs::write(&self.default_path, original);
            }
            // There was no default before this test wrote one; leaving it
            // would point the app at a profile that no longer exists.
            None => {
                let _ = fs::remove_file(&self.default_path);
            }
        }
    }
}

/// Name given to every profile this file installs.
///
/// Used to find and remove leftovers, so it has to be distinctive.
const TEST_PROFILE_NAME: &str = "Codex UI E2E";

/// Delete profiles left by an earlier run of this file.
///
/// These tests write into the developer's real profile directory, so a run
/// that is killed rather than finished leaves its profile behind. One was
/// found still installed and pointing at an account that no longer existed,
/// which made an unrelated chat fail with an authentication error. `Drop`
/// cannot help when the process never unwinds, so each run clears the last
/// one's debris before starting.
fn sweep_leftover_profiles() {
    let Ok(entries) = fs::read_dir(profiles_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        if value.get("name").and_then(serde_json::Value::as_str) == Some(TEST_PROFILE_NAME) {
            let _ = fs::remove_file(&path);
        }
    }
}

/// Install a single codex profile and make it the default.
///
/// `account` empty writes a profile with no signed-in account, which is what
/// the sign-in scenario needs.
fn install_codex_profile(account: &str) -> ProfileGuard {
    let _ = fs::create_dir_all(profiles_dir());
    sweep_leftover_profiles();
    let default_path = profiles_dir().join("default.json");
    let original_default = fs::read_to_string(&default_path).ok();

    let id = Uuid::new_v4().to_string();
    let profile = serde_json::json!({
        "id": id,
        "name": TEST_PROFILE_NAME,
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
    launch_app_with_grant(None)
}

/// Launch the app, optionally handing it a grant through the environment.
fn launch_app_with_grant(grant: Option<&str>) -> Child {
    let _ = Command::new("pkill").arg("-f").arg(APP_PROCESS).status();
    thread::sleep(Duration::from_millis(500));

    let log_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(log_path())
        .expect("open log file");
    let log_err = log_file.try_clone().expect("clone log handle");

    let mut command = Command::new(gpui_bin_path());
    command
        .env("PA_AUTO_OPEN_POPUP", "1")
        .env("PA_TEST_POPUP_ONSCREEN", "1")
        .env("RUST_LOG", "info");
    if let Some(grant) = grant {
        command.env(store::E2E_GRANT_ENV, grant);
    }
    command
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
    let _scenario = scenario_lock();
    let account = required_account();
    let grant = seed_grant(&account, false);
    let _guard = install_codex_profile(&account);

    let mut app = launch_app_with_grant(Some(&grant));
    wait_until_ready_to_send();

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
fn the_auth_presenter_starts_when_no_grant_is_stored() {
    let _scenario = scenario_lock();
    // No account on the profile, so the editor offers a sign-in.
    let _guard = install_codex_profile("");

    let mut app = launch_app();
    wait_until_ready_to_send();

    open_settings();

    // Bring the app forward and let it settle. Driving the sign-in itself
    // would open a browser and wait on a human, which this scenario avoids.
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
    let _scenario = scenario_lock();
    let account = required_account();
    let grant = seed_grant(&account, true);
    let _guard = install_codex_profile(&account);

    let mut app = launch_app_with_grant(Some(&grant));
    wait_until_ready_to_send();

    type_and_send("This turn cannot run.");

    let asked = wait_for_log("OAuthReauthRequired", Duration::from_secs(59))
        || wait_for_log("CodexReauthRequired", Duration::from_secs(5));
    let log = read_log();
    stop_app(&mut app);

    // Put the good grant back so a later run is not left broken.
    let _restored = seed_grant(&account, false);

    assert!(
        asked,
        "a dead grant should ask for a sign-in rather than surfacing a provider error: {}",
        last_n_lines(&log, 60)
    );
}
