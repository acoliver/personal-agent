//! Stabilize branch: long-running conversation loop driven through the real
//! GPUI UI.
//!
//! Step 1 (this file): prove we can launch `personal_agent_gpui` with the
//! existing `fireworks kimi` profile as the default, deliberately open a new
//! conversation via the `Ctrl-N` shortcut so the conversation is bound to the
//! fireworks kimi profile id, send a single prompt via AppleScript keystrokes,
//! wait for the stream to complete, and read the persisted assistant response
//! from the SQLite store.
//!
//! No keychain prompt: the test reads the raw key from
//! `~/.keys/.fireworks_key` and passes it to the launched binary as
//! `PA_E2E_API_KEY`, which short-circuits `LlmClient::resolve_api_key` before
//! any keychain lookup.
//!
//! Run with:
//!   cargo test --test stabilize_long_conversation -- --ignored --nocapture

mod ui_tests;

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

use rusqlite::Connection;
use ui_tests::applescript_helpers::{run_applescript_lines, AppleScriptResult};

const APP_PROCESS: &str = "personal_agent_gpui";
const LOG_PATH: &str = "/tmp/personal_agent_gpui.log";

/// UUID of the existing "fireworks kimi" profile on this machine.
#[allow(dead_code)]
const FIREWORKS_KIMI_PROFILE_ID: &str = "2c362adf-d506-43e2-a955-c720e32e4bd2";

/// UUID of the existing "localqwen" profile (LM Studio @ 127.0.0.1:1234,
/// auth.type = "none"; no keychain prompt).
const LOCALQWEN_PROFILE_ID: &str = "a9fde715-36bb-488f-b304-e9c9e2fe46b8";

// ── path helpers ─────────────────────────────────────────────────────────────

fn gpui_bin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_personal_agent_gpui"))
}

fn app_support_dir() -> PathBuf {
    dirs::config_dir()
        .expect("platform config dir unavailable")
        .join("PersonalAgent")
}

fn profiles_dir() -> PathBuf {
    app_support_dir().join("profiles")
}

fn default_profile_path() -> PathBuf {
    profiles_dir().join("default.json")
}

#[allow(dead_code)]
fn fireworks_profile_path() -> PathBuf {
    profiles_dir().join(format!("{FIREWORKS_KIMI_PROFILE_ID}.json"))
}

fn localqwen_profile_path() -> PathBuf {
    profiles_dir().join(format!("{LOCALQWEN_PROFILE_ID}.json"))
}

fn db_path() -> PathBuf {
    app_support_dir().join("personalagent.db")
}

// ── log helpers ──────────────────────────────────────────────────────────────

fn clear_log() {
    let _ = fs::write(LOG_PATH, "");
}

fn read_log() -> String {
    fs::read_to_string(LOG_PATH).unwrap_or_default()
}

fn log_contains(needle: &str) -> bool {
    read_log().contains(needle)
}

fn wait_for_log_substring(needle: &str, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if log_contains(needle) {
            return true;
        }
        thread::sleep(Duration::from_millis(150));
    }
    false
}

fn count_occurrences(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

fn wait_for_stream_started(baseline: usize, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if count_occurrences(&read_log(), "StreamStarted") > baseline {
            return true;
        }
        thread::sleep(Duration::from_millis(200));
    }
    false
}

fn wait_for_stream_to_finish(
    completed_before: usize,
    error_before: usize,
    timeout: Duration,
) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        let log = read_log();
        let completed = count_occurrences(&log, "StreamCompleted");
        let errored = count_occurrences(&log, "StreamError");
        if completed + errored > completed_before + error_before {
            return true;
        }
        thread::sleep(Duration::from_millis(250));
    }
    false
}

fn tail_log(n_lines: usize) -> String {
    let log = read_log();
    let lines: Vec<&str> = log.lines().collect();
    let start = lines.len().saturating_sub(n_lines);
    lines[start..].join("\n")
}

// ── AppleScript helpers ──────────────────────────────────────────────────────

fn frontmost_and_type(message: &str, press_enter: bool) -> AppleScriptResult {
    let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
    let mut lines: Vec<String> = vec![
        "tell application \"System Events\"".to_string(),
        "key up command".to_string(),
        "key up control".to_string(),
        "key up option".to_string(),
        "key up shift".to_string(),
        format!("tell process \"{APP_PROCESS}\""),
        "set frontmost to true".to_string(),
        "delay 0.1".to_string(),
        format!("keystroke \"{escaped}\""),
    ];
    if press_enter {
        lines.push("key code 36".to_string());
    }
    lines.push("end tell".to_string());
    lines.push("end tell".to_string());
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    run_applescript_lines(&refs)
}

/// Fire the `Ctrl-N` shortcut so the app calls `UserEvent::NewConversation`.
fn press_new_conversation_shortcut() -> AppleScriptResult {
    run_applescript_lines(&[
        "tell application \"System Events\"",
        "key up command",
        "key up control",
        "key up option",
        "key up shift",
        &format!("tell process \"{APP_PROCESS}\""),
        "set frontmost to true",
        "delay 0.1",
        "keystroke \"n\" using control down",
        "end tell",
        "end tell",
    ])
}

// ── profile default guard (restore on Drop) ──────────────────────────────────

/// Overwrites `default.json` with `profile_id` and restores the original
/// contents on Drop. The Drop guard only runs on normal test exit; if the
/// test is killed with ctrl-c, restore by hand.
struct DefaultProfileGuard {
    path: PathBuf,
    original_content: Option<String>,
}

impl DefaultProfileGuard {
    fn point_default_at(profile_id: &str) -> Self {
        let path = default_profile_path();
        let original_content = fs::read_to_string(&path).ok();
        let new_value =
            serde_json::to_string(profile_id).expect("serializing profile id string cannot fail");
        fs::write(&path, new_value).expect("failed to write default.json");
        Self {
            path,
            original_content,
        }
    }
}

impl Drop for DefaultProfileGuard {
    fn drop(&mut self) {
        if let Some(ref original) = self.original_content {
            let _ = fs::write(&self.path, original);
        } else {
            let _ = fs::remove_file(&self.path);
        }
    }
}

// ── launch / stop ────────────────────────────────────────────────────────────

fn launch_gpui(extra_envs: &[(&str, &str)]) -> Child {
    let _ = Command::new("pkill").arg("-f").arg(APP_PROCESS).status();
    thread::sleep(Duration::from_millis(400));

    let bin = gpui_bin_path();
    let log_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(LOG_PATH)
        .expect("failed to open GPUI log path");
    let log_file_err = log_file
        .try_clone()
        .expect("failed to clone GPUI log file handle");

    let mut cmd = Command::new(bin);
    cmd.env("PA_AUTO_OPEN_POPUP", "1")
        .env("PA_TEST_POPUP_ONSCREEN", "1")
        .env("PA_LOG_LEVEL", "debug")
        .stdout(log_file)
        .stderr(log_file_err);
    for (k, v) in extra_envs {
        cmd.env(k, v);
    }
    cmd.spawn().expect("failed to launch personal_agent_gpui")
}

fn stop_gpui(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
    let _ = Command::new("pkill").arg("-f").arg(APP_PROCESS).status();
}

// ── SQLite helpers ───────────────────────────────────────────────────────────

#[derive(Debug)]
struct ConversationRow {
    id: String,
    title: Option<String>,
    profile_id: Option<String>,
    updated_at: String,
}

fn newest_conversation_after(iso_cutoff: &str) -> Option<ConversationRow> {
    let conn = Connection::open_with_flags(
        db_path(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .ok()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, title, profile_id, updated_at FROM conversations \
             WHERE created_at >= ?1 ORDER BY updated_at DESC LIMIT 1",
        )
        .ok()?;
    stmt.query_row([iso_cutoff], |row| {
        Ok(ConversationRow {
            id: row.get(0)?,
            title: row.get(1)?,
            profile_id: row.get(2)?,
            updated_at: row.get(3)?,
        })
    })
    .ok()
}

#[derive(Debug)]
struct MessageRow {
    role: String,
    content: String,
    #[allow(dead_code)]
    seq: i64,
}

fn messages_for(conversation_id: &str) -> Vec<MessageRow> {
    let Ok(conn) = Connection::open_with_flags(
        db_path(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    ) else {
        return Vec::new();
    };
    let Ok(mut stmt) = conn.prepare(
        "SELECT role, content, seq FROM messages WHERE conversation_id = ?1 ORDER BY seq ASC",
    ) else {
        return Vec::new();
    };
    stmt.query_map([conversation_id], |row| {
        Ok(MessageRow {
            role: row.get(0)?,
            content: row.get(1)?,
            seq: row.get(2)?,
        })
    })
    .map(|rows| rows.flatten().collect())
    .unwrap_or_default()
}

/// Current UTC time as an ISO-8601 string comparable to `created_at` column.
fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

// ── test ─────────────────────────────────────────────────────────────────────

/// STAB-001: launch the GPUI app with the `localqwen` profile (local LM
/// Studio, auth = none), create a fresh conversation via `Ctrl-N`, send one
/// message, and verify the assistant streamed a non-empty reply that was
/// persisted to the SQLite store.
///
/// Requires:
///   - macOS Accessibility permission for the test runner (same as SCN-*).
///   - LM Studio listening on `http://127.0.0.1:1234/v1` with a model that
///     matches the localqwen profile's `model_id`.
///   - The `localqwen` profile JSON present in the profiles directory.
#[test]
#[ignore = "Requires local GPUI launch + accessibility permission + LM Studio on :1234"]
fn stab_001_single_message_round_trip() {
    // Preconditions --------------------------------------------------------
    let profile_path = localqwen_profile_path();
    assert!(
        profile_path.exists(),
        "expected localqwen profile JSON at {}",
        profile_path.display()
    );

    assert!(
        db_path().exists(),
        "expected SQLite store at {} (has the app ever been launched?)",
        db_path().display()
    );

    // Point default.json at the localqwen profile; auto-restore on Drop.
    let _default_guard = DefaultProfileGuard::point_default_at(LOCALQWEN_PROFILE_ID);

    // Launch ---------------------------------------------------------------
    clear_log();
    let run_started_iso = now_iso();
    let mut child = launch_gpui(&[]);

    if !wait_for_log_substring("All 9 presenters started", Duration::from_secs(20)) {
        let tail = tail_log(80);
        stop_gpui(&mut child);
        panic!("app did not emit startup marker within 20s; log tail:\n{tail}");
    }

    // Let the popup settle.
    thread::sleep(Duration::from_millis(800));

    // Force a fresh conversation so the new row is bound to fireworks kimi.
    let new_conv = press_new_conversation_shortcut();
    assert!(
        new_conv.success,
        "applescript ctrl-n failed: stderr={}",
        new_conv.stderr
    );
    thread::sleep(Duration::from_millis(500));

    // Send one prompt ------------------------------------------------------
    let prompt = "respond with exactly the word: banana.";
    let send = frontmost_and_type(prompt, true);
    assert!(
        send.success,
        "applescript send failed: stderr={}",
        send.stderr
    );

    // Wait for stream progression.
    if !wait_for_stream_started(0, Duration::from_secs(30)) {
        let tail = tail_log(80);
        stop_gpui(&mut child);
        panic!("no StreamStarted within 30s after prompt; log tail:\n{tail}");
    }

    if !wait_for_stream_to_finish(0, 0, Duration::from_secs(180)) {
        let tail = tail_log(120);
        stop_gpui(&mut child);
        panic!("stream did not complete within 180s; log tail:\n{tail}");
    }

    // Give the persistence layer a tick to flush its final writes.
    thread::sleep(Duration::from_secs(1));

    // Verify conversation row in SQLite -----------------------------------
    let conv = newest_conversation_after(&run_started_iso).unwrap_or_else(|| {
        let tail = tail_log(80);
        stop_gpui(&mut child);
        panic!("no conversation created with created_at >= {run_started_iso}; log tail:\n{tail}");
    });
    let messages = messages_for(&conv.id);

    stop_gpui(&mut child);

    let user_count = messages.iter().filter(|m| m.role == "user").count();
    let assistant_count = messages.iter().filter(|m| m.role == "assistant").count();

    assert_eq!(
        user_count, 1,
        "expected exactly 1 user message, got {user_count} (conv={}, messages={:?})",
        conv.id, messages
    );
    assert!(
        assistant_count >= 1,
        "expected >= 1 assistant message, got {assistant_count} (conv={}, messages={:?})",
        conv.id,
        messages
    );

    let assistant_text = messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .map(|m| m.content.clone())
        .expect("no assistant message present despite assistant_count >= 1");
    assert!(
        !assistant_text.trim().is_empty(),
        "last assistant message is empty (conv={})",
        conv.id
    );

    assert_eq!(
        conv.profile_id.as_deref(),
        Some(LOCALQWEN_PROFILE_ID),
        "conversation profile_id != localqwen id (conv={}, title={:?}, updated_at={})",
        conv.id,
        conv.title,
        conv.updated_at,
    );

    println!(
        "STAB-001 PASS\n  conversation id: {}\n  title: {:?}\n  profile_id: {:?}\n  user msgs: {user_count}\n  assistant msgs: {assistant_count}\n  assistant text: {}",
        conv.id,
        conv.title,
        conv.profile_id,
        assistant_text.chars().take(200).collect::<String>()
    );
}
