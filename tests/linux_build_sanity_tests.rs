#![cfg(target_os = "linux")]

//! Linux-only compile/link sanity checks for issue #43.

#[test]
fn linux_build_sanity() {
    // This intentionally exercises no runtime behavior.
    // The value is in guaranteeing this target-specific test binary
    // compiles and links on Linux in CI.
}
