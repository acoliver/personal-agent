//! Issue #171 reproduction test: top bar / toolbar must not shift horizontally
//! when the chat area's content changes (specifically: toggling thinking
//! visibility on a conversation that already has thinking deltas).
//!
//! ## What this reproduces
//!
//! From the bug report: with a conversation that contains assistant messages
//! with `thinking_content`, the user clicks the `T` toolbar button to toggle
//! thinking visibility. When thinking content becomes visible inside the chat
//! bubble, its intrinsic width propagates up the flex tree and shifts the
//! entire toolbar group (Save / Popout / Settings / Exit) horizontally.
//!
//! ## How the test works
//!
//! 1. Stage a synthetic conversation file in the user's `PersonalAgent` data
//!    directory containing a single assistant turn with a wide, multi-line
//!    `thinking_content` block. A guard restores the original conversations
//!    list on drop.
//! 2. Launch `personal_agent_gpui` with `PA_AUTO_OPEN_POPUP=1` and
//!    `PA_TEST_POPUP_ONSCREEN=1` so the popup opens at a deterministic
//!    location near the top-right of the main display. The newest
//!    conversation (ours) is auto-selected at startup.
//! 3. Capture a screenshot of the top bar (44px high, full popup width)
//!    while `show_thinking == false` (the default).
//! 4. Bring the popup to the foreground and click the `T` toolbar button at
//!    its computed screen coordinates. Verify the click registered by
//!    polling for the `ToggleThinkingVisibility` log marker.
//! 5. Capture the top bar again.
//! 6. Crop both screenshots to the *rightmost* slice (where the trailing
//!    toolbar buttons live) and run `ImageMagick` `magick compare -metric AE`.
//!    The rightmost buttons should be pinned to the right edge of the bar
//!    and therefore pixel-identical across the toggle. With the bug present
//!    the entire toolbar slides left, producing a large diff count.
//!
//! ## Run
//!
//! ```text
//! cargo test --test issue171_top_bar_stability_test -- --ignored --nocapture
//! ```
//!
//! ## Requirements
//!
//! - macOS (uses `screencapture`, `osascript`, `cliclick`).
//! - `ImageMagick`'s `magick` on `PATH` (Homebrew: `imagemagick`).
//! - Accessibility permissions granted to the test runner.

#![cfg(target_os = "macos")]
#![allow(clippy::doc_markdown, clippy::missing_const_for_fn)]

mod ui_tests;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

use uuid::Uuid;

use ui_tests::applescript_helpers::run_applescript_lines;

const APP_PROCESS: &str = "personal_agent_gpui";
const LOG_PATH: &str = "/tmp/personal_agent_gpui_issue171.log";

// Popup geometry must match `src/main_gpui/system_tray.rs` (`get_popup_position`
// for `target_os = "macos"` with `PA_TEST_POPUP_ONSCREEN=1`):
//   menu_width  = 780.0
//   menu_height = 600.0
//   origin      = (screen_w - 780 - 24, 36)
const POPUP_WIDTH: u32 = 780;
const POPUP_RIGHT_MARGIN: u32 = 24;
const POPUP_TOP_OFFSET: u32 = 36;

// Top bar height; capture a slightly taller strip so we also see the thin
// bottom border (helps catch vertical shifts).
const TOP_BAR_HEIGHT: u32 = 48;

// Width of the rightmost-toolbar crop. The trailing toolbar cluster (Save,
// Popout, Settings, Exit + a couple of icons just before them) needs ~220px
// at logical scale; 240 leaves a comfortable margin.
const RIGHT_TOOLBAR_CROP_WIDTH: u32 = 240;

// Toolbar button geometry from `render_bars.rs` (`render_toolbar_buttons` and
// the `icon_btn!` macro): each icon is 28px square, `gap(8.0)` between them,
// `pr(12.0)` on the bar.
const ICON_W: u32 = 28;
const ICON_GAP: u32 = 8;
const ICON_STRIDE: u32 = ICON_W + ICON_GAP;
const TOP_BAR_PR: u32 = 12;

// Buttons in the toolbar (left -> right) in tray-popup mode (`app_mode !=
// Popout` so `show_history_btn = true`):
//   0: T (thinking)
//   1: E (emoji filter)
//   2: Y (yolo)
//   3: R (rename)
//   4: H (history)
//   5: MD/TXT/JSON (export format)
//   6: copy
//   7: down-arrow / save
//   8: popout-arrow
//   9: settings (gear)
//  10: exit (power)
// T is at index 0; total icons = 11.
const T_BUTTON_INDEX_FROM_LEFT_OF_TOOLBAR: u32 = 0;
const TOOLBAR_BUTTON_COUNT_POPIN: u32 = 11;

fn artifacts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("artifacts/issue171")
}

fn gpui_bin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_personal_agent_gpui"))
}

fn app_data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/PersonalAgent")
}

fn conversations_dir() -> PathBuf {
    app_data_dir().join("conversations")
}

fn clear_log() {
    let _ = fs::write(LOG_PATH, "");
}

fn read_log() -> String {
    fs::read_to_string(LOG_PATH).unwrap_or_default()
}

fn wait_for_log_substring(needle: &str, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if read_log().contains(needle) {
            return true;
        }
        thread::sleep(Duration::from_millis(150));
    }
    false
}

/// Abort the test if a `personal_agent_gpui` process is already running. The
/// test launches its own instance against the real user data directory, so
/// clashing with a developer's own session would be both destructive and
/// confusing. Prefer a loud failure over silently pkill'ing the user's work.
fn assert_no_existing_gpui_instance() {
    let output = Command::new("pgrep").arg("-f").arg(APP_PROCESS).output();
    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let pids: Vec<&str> = stdout
            .split_whitespace()
            .filter(|p| !p.is_empty())
            .collect();
        assert!(
            pids.is_empty(),
            "a `{APP_PROCESS}` instance is already running (PIDs: {pids:?}). \
             Close it before running this test; this harness refuses to \
             pkill arbitrary matching processes."
        );
    }
}

fn launch_gpui() -> Child {
    assert_no_existing_gpui_instance();

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

    Command::new(bin)
        .env("PA_AUTO_OPEN_POPUP", "1")
        .env("PA_TEST_POPUP_ONSCREEN", "1")
        .stdout(log_file)
        .stderr(log_file_err)
        .spawn()
        .expect("failed to launch personal_agent_gpui")
}

fn stop_gpui(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// RAII guard that kills the launched GPUI child on drop, so a panic in any
/// assertion before the explicit `stop_gpui()` call still cleans up the
/// spawned process. Only the `Child` we started is touched — this never
/// `pkill`s arbitrary `personal_agent_gpui` instances a developer might
/// have running.
struct GpuiChildGuard {
    child: Option<Child>,
}

impl GpuiChildGuard {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn stop(mut self) {
        if let Some(mut child) = self.child.take() {
            stop_gpui(&mut child);
        }
    }
}

impl Drop for GpuiChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// RAII guard that drops a synthetic conversation file into the user's
/// PersonalAgent data directory, then deletes it on drop. Touches the file's
/// mtime to make it the newest entry so the app auto-selects it on startup.
struct ConversationGuard {
    path: PathBuf,
}

impl ConversationGuard {
    fn install_with_thinking() -> Option<Self> {
        let dir = conversations_dir();
        fs::create_dir_all(&dir).ok()?;
        let id = Uuid::new_v4();
        let path = dir.join(format!("{id}.json"));

        // Wide thinking content forces the chat area's intrinsic width to be
        // larger than the popup; this is what triggers the bug if the bar
        // layout is not isolated from chat content.
        let thinking_text =
            "Thinking: Let me start by exploring the project structure to understand what \
             this is about. I will look at the most recent branches first, then dig deeper \
             into the architecture and source code of the most recent branch, the designs \
             folder, and check the GitHub repo. There are multiple branches/directories. \
             Let me explore each one to understand the project. Now let me look at the key \
             differences between branches and dig into the design documents, and look at \
             the actual source code to understand the scope and maturity of this project.";

        let assistant_text =
            "Quick summary of the project after exploring it for a bit. The streaming \
             pipeline emits both text and thinking deltas; the chat view binds them into \
             a single bubble that respects the show_thinking toggle in the toolbar.";

        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true);

        let body = serde_json::json!({
            "id": id.to_string(),
            "created_at": now,
            "updated_at": now,
            "title": "issue171 repro",
            "profile_id": serde_json::Value::Null,
            "messages": [
                {
                    "role": "user",
                    "content": "look at ~/projects/personal-agent and analyze it. \
                                 To what degree is this useful. compare it to other \
                                 solutions. Who would want this?",
                    "thinking_content": null,
                    "timestamp": now,
                },
                {
                    "role": "assistant",
                    "content": assistant_text,
                    "thinking_content": thinking_text,
                    "timestamp": now,
                }
            ],
        });

        fs::write(&path, serde_json::to_string_pretty(&body).ok()?).ok()?;
        Some(Self { path })
    }
}

impl Drop for ConversationGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Return the main display's *logical* (points) width and height.
///
/// AppleScript's "bounds of window of desktop" reports logical points, which
/// is what `screencapture -R` and `cliclick` accept on macOS.
fn main_display_logical_size() -> Option<(u32, u32)> {
    let result = run_applescript_lines(&[
        "tell application \"Finder\"",
        "    set scn to bounds of window of desktop",
        "end tell",
        "set w to (item 3 of scn) - (item 1 of scn)",
        "set h to (item 4 of scn) - (item 2 of scn)",
        "return (w as string) & \"x\" & (h as string)",
    ]);
    if !result.success {
        return None;
    }
    let s = result.stdout.trim();
    let mut parts = s.split('x');
    let w: u32 = parts.next()?.parse().ok()?;
    let h: u32 = parts.next()?.parse().ok()?;
    Some((w, h))
}

/// Compute popup origin in logical points on the main display, matching
/// `get_popup_position` with `PA_TEST_POPUP_ONSCREEN=1` on macOS.
fn popup_origin_logical(screen_width: u32) -> (u32, u32) {
    let x = screen_width.saturating_sub(POPUP_WIDTH + POPUP_RIGHT_MARGIN);
    let y = POPUP_TOP_OFFSET;
    (x, y)
}

/// Compute the screen-space center of the `T` toolbar button.
///
/// The toolbar is right-aligned inside the top bar with `pr(12.0)`. With
/// `TOOLBAR_BUTTON_COUNT_POPIN` icons of `ICON_W` each separated by `ICON_GAP`,
/// the toolbar's total width is fixed; the leftmost icon's left edge sits at
/// `popup_right - 12 - toolbar_width`. The `T` button is at index 0.
fn t_button_center(popup_x: u32, popup_y: u32) -> (u32, u32) {
    let toolbar_w =
        TOOLBAR_BUTTON_COUNT_POPIN * ICON_W + (TOOLBAR_BUTTON_COUNT_POPIN - 1) * ICON_GAP;
    let toolbar_left = popup_x + POPUP_WIDTH - TOP_BAR_PR - toolbar_w;
    let center_x = toolbar_left + T_BUTTON_INDEX_FROM_LEFT_OF_TOOLBAR * ICON_STRIDE + ICON_W / 2;
    // Top bar is 44px tall; center vertically at popup_y + 22.
    let center_y = popup_y + 22;
    (center_x, center_y)
}

/// Capture a region of the screen to `dest` using `screencapture -R`.
fn capture_region(x: u32, y: u32, w: u32, h: u32, dest: &Path) -> bool {
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let region = format!("{x},{y},{w},{h}");
    Command::new("screencapture")
        .args(["-x", "-t", "png", "-R", &region])
        .arg(dest)
        .status()
        .is_ok_and(|s| s.success())
}

/// Bring the GPUI process to the foreground.
fn bring_app_frontmost() -> bool {
    let result = run_applescript_lines(&[
        "tell application \"System Events\"",
        &format!("set frontmost of (first process whose name is \"{APP_PROCESS}\") to true"),
        "end tell",
    ]);
    result.success
}

/// Click at logical screen coords using `cliclick`.
fn click_at(x: u32, y: u32) -> bool {
    Command::new("cliclick")
        .arg(format!("c:{x},{y}"))
        .status()
        .is_ok_and(|s| s.success())
}

/// Query the physical (pixel) dimensions of an image via `magick identify`.
/// Returns `(width_px, height_px)` or `None` on failure.
fn image_pixel_dimensions(src: &Path) -> Option<(u32, u32)> {
    let output = Command::new("magick")
        .args(["identify", "-format", "%w %h", src.to_str()?])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut parts = stdout.split_whitespace();
    let w = parts.next()?.parse::<u32>().ok()?;
    let h = parts.next()?.parse::<u32>().ok()?;
    Some((w, h))
}

/// Derive the capture scale (physical pixels per logical point) by comparing
/// the captured image's physical width to the logical width we asked for.
/// Returns `1` on standard displays, `2` on Retina, etc. Falls back to `2`
/// (macOS Retina) if dimensions can't be queried.
fn capture_scale(src: &Path, requested_logical_width: u32) -> u32 {
    image_pixel_dimensions(src)
        .and_then(|(w_px, _h_px)| {
            if requested_logical_width == 0 {
                None
            } else {
                Some((w_px / requested_logical_width).max(1))
            }
        })
        .unwrap_or(2)
}

/// Crop `src` to the rightmost `crop_w` *logical* pixels (full height) and
/// write to `dest`. Scale is derived from the actual image dimensions so
/// this works on both Retina (2×) and standard (1×) displays.
fn crop_right(src: &Path, crop_w_logical: u32, logical_src_width: u32, dest: &Path) -> bool {
    let scale = capture_scale(src, logical_src_width);
    let crop = format!("{}x+0+0", crop_w_logical * scale);
    Command::new("magick")
        .args([
            src.to_str().expect("src path utf8"),
            "-gravity",
            "East",
            "-crop",
            &crop,
            "+repage",
            dest.to_str().expect("dest path utf8"),
        ])
        .status()
        .is_ok_and(|s| s.success())
}

/// Run `magick compare -metric AE -fuzz 5%` between two images and return
/// the absolute number of differing pixels. ImageMagick prints the AE count
/// to stderr; some versions append the matched-pixel count in parens
/// (e.g. `"1234 (5)"`), so we take the first whitespace-separated token.
fn count_diff_pixels(a: &Path, b: &Path, diff_dest: &Path) -> Option<u64> {
    let output = Command::new("magick")
        .args([
            "compare",
            "-metric",
            "AE",
            "-fuzz",
            "5%",
            a.to_str()?,
            b.to_str()?,
            diff_dest.to_str()?,
        ])
        .output()
        .ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr.split_whitespace().next()?.parse::<u64>().ok()
}

/// SCN-171: top-bar pixel stability under a thinking-visibility toggle.
///
/// **Failure mode (bug present):** Clicking the `T` button while a
/// conversation contains assistant messages with `thinking_content` changes
/// the chat area's intrinsic content size. Without proper layout isolation,
/// the change propagates upward and shifts the entire trailing toolbar
/// (Save/Popout/Settings/Exit) horizontally; the rightmost-toolbar crop
/// changes wildly between captures.
///
/// **Pass condition (bug fixed):** The trailing toolbar buttons are pinned
/// to the right edge of the top bar via `justify_between` + `flex_shrink_0`,
/// so the rightmost crop is bit-identical (modulo a small fuzz tolerance)
/// across the toggle.
#[test]
#[ignore = "Requires GPUI launch + macOS Accessibility permissions + ImageMagick"]
#[allow(clippy::too_many_lines)]
fn issue_171_top_bar_does_not_shift_when_thinking_toggles() {
    clear_log();

    let _conv = ConversationGuard::install_with_thinking()
        .expect("failed to stage synthetic conversation with thinking content");

    let (screen_w, _screen_h) =
        main_display_logical_size().expect("could not query main display size");
    let (popup_x, popup_y) = popup_origin_logical(screen_w);
    let dir = artifacts_dir();
    let _ = fs::create_dir_all(&dir);
    let before_full = dir.join("topbar_before.png");
    let after_full = dir.join("topbar_after.png");
    let before_right = dir.join("topbar_right_before.png");
    let after_right = dir.join("topbar_right_after.png");
    let diff = dir.join("topbar_right_diff.png");

    let child_guard = GpuiChildGuard::new(launch_gpui());
    assert!(
        wait_for_log_substring("All 9 presenters started", Duration::from_secs(20)),
        "GPUI app did not start within timeout"
    );
    // Settle: presenters started fires before the first frame paints, and
    // the popup is positioned a moment later.
    thread::sleep(Duration::from_millis(1_500));
    assert!(
        bring_app_frontmost(),
        "AppleScript failed to bring {APP_PROCESS} frontmost"
    );
    thread::sleep(Duration::from_millis(400));

    // ── BEFORE: capture the top bar with thinking hidden (default). ──────────
    assert!(
        capture_region(popup_x, popup_y, POPUP_WIDTH, TOP_BAR_HEIGHT, &before_full),
        "screencapture failed for BEFORE state"
    );

    // ── ACT: click the T (thinking) button. ──────────────────────────────────
    let (t_x, t_y) = t_button_center(popup_x, popup_y);
    eprintln!("issue171: clicking T at ({t_x}, {t_y}); popup_origin=({popup_x},{popup_y})");
    assert!(click_at(t_x, t_y), "cliclick failed for T toggle");

    // The view-local handler logs "Toggle thinking clicked - emitting UserEvent"
    // when the T button click is dispatched into ChatView::emit.
    let toggle_logged =
        wait_for_log_substring("Toggle thinking clicked", Duration::from_millis(2_500));
    if !toggle_logged {
        eprintln!(
            "issue171: WARNING - 'Toggle thinking clicked' log not seen within 2.5s; \
             click likely missed (popup not focused or coordinates off)"
        );
    }
    // Move the mouse far away to clear any hover/tooltip state on the T button
    // (otherwise the AFTER capture shows a tooltip HUD over the toolbar).
    let _ = Command::new("cliclick").arg("m:50,50").status();
    // Allow the next frame to render and any tooltip to dismiss.
    thread::sleep(Duration::from_millis(900));

    // ── AFTER: capture the top bar with thinking visible. ────────────────────
    assert!(
        capture_region(popup_x, popup_y, POPUP_WIDTH, TOP_BAR_HEIGHT, &after_full),
        "screencapture failed for AFTER state"
    );

    child_guard.stop();

    // ── COMPARE. ─────────────────────────────────────────────────────────────
    assert!(
        crop_right(
            &before_full,
            RIGHT_TOOLBAR_CROP_WIDTH,
            POPUP_WIDTH,
            &before_right,
        ),
        "right-crop failed for BEFORE"
    );
    assert!(
        crop_right(
            &after_full,
            RIGHT_TOOLBAR_CROP_WIDTH,
            POPUP_WIDTH,
            &after_right,
        ),
        "right-crop failed for AFTER"
    );

    let diff_count = count_diff_pixels(&before_right, &after_right, &diff)
        .expect("ImageMagick `magick compare` failed (is `imagemagick` installed?)");

    // Derive scale dynamically from the captured image so the crop-area math
    // is correct on both Retina (2×) and standard (1×) displays.
    let scale = capture_scale(&before_full, POPUP_WIDTH);
    let crop_total_pixels: u64 = u64::from(RIGHT_TOOLBAR_CROP_WIDTH)
        * u64::from(scale)
        * u64::from(TOP_BAR_HEIGHT)
        * u64::from(scale);
    let diff_permille = diff_count.saturating_mul(1000) / crop_total_pixels.max(1);

    eprintln!(
        "issue171: rightmost toolbar crop differs in {diff_count} of \
         {crop_total_pixels} pixels ({diff_permille}‰)"
    );
    eprintln!("issue171: artifacts in {}", dir.display());

    // The click must have actually toggled thinking, otherwise the test is
    // useless — fail loud rather than passing for the wrong reason.
    assert!(
        toggle_logged,
        "T-button click did not register a ToggleThinkingVisibility event; the \
         test cannot draw conclusions about layout stability without the \
         toggle actually firing. Inspect {LOG_PATH} for details."
    );

    // Threshold: with the fix, only the T button itself (in this rightmost
    // crop, since T is the leftmost icon of the toolbar) flips its active
    // background. That covers ~28×28 logical px = 56×56 physical = 3136 px
    // out of ~46k total in the crop. We'd expect ≤ ~7% to be safe.
    //
    // With the bug present the entire toolbar shifts horizontally by ~28px
    // (one icon width) per content-change pass; the crop would change in
    // roughly half its area (~50%+).
    //
    // We split the difference at 15% (150‰) — well above any legitimate
    // T-button-only change but well below a full toolbar shift.
    let max_allowed_permille: u64 = 150;
    assert!(
        diff_permille <= max_allowed_permille,
        "TOP-BAR SHIFT DETECTED: {diff_count} differing pixels ({diff_permille}‰) \
         in the rightmost {RIGHT_TOOLBAR_CROP_WIDTH}px slice of the top bar after \
         toggling thinking visibility (allowed ≤ {max_allowed_permille}‰). The \
         trailing toolbar buttons should be pinned to the right edge and \
         unaffected by chat-area content changes. Inspect:\n  {}\n  {}\n  {}",
        before_right.display(),
        after_right.display(),
        diff.display(),
    );

    println!(
        "SCENARIO: SCN-171\n\
         STATUS: PASS\n\
         STEPS_RUN: 5\n\
         ASSERTIONS_PASSED: 5\n\
         ASSERTIONS_FAILED: 0\n\
         ARTIFACTS:\n  - {}\n  - {}\n  - {}\n  - {}\n  - {}\n\
         NOTES:\n\
         \x20 - rightmost toolbar crop diff: {diff_count} pixels ({diff_permille}‰)\n\
         \x20 - max allowed: {max_allowed_permille}‰\n\
         \x20 - 'Toggle thinking clicked' log marker seen: {toggle_logged}",
        before_full.display(),
        after_full.display(),
        before_right.display(),
        after_right.display(),
        diff.display(),
    );
}
