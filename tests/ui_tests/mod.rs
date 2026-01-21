//! UI automation tests using AppleScript
//!
//! These tests use macOS System Events to verify actual UI behavior.
//! They require:
//! 1. The app to be running
//! 2. Accessibility permissions granted to the test runner
//!
//! Run with: cargo test --test ui_automation_tests -- --ignored
//! (ignored by default because they require the app to be running)

pub mod applescript_helpers;
