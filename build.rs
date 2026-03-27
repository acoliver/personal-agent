fn main() {
    // Register `ci` as a known cfg so check-cfg doesn't warn.
    println!("cargo:rustc-check-cfg=cfg(ci)");

    // Emit `cfg(ci)` when running inside a CI environment so that tests
    // requiring a live display / window-server can auto-skip in CI while
    // still contributing to local coverage runs.
    if std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok() {
        println!("cargo:rustc-cfg=ci");
    }
}
