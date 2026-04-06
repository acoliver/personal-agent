use anyhow::{bail, Context, Result};
use proc_macro2::TokenStream;
use quote::ToTokens;
use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use syn::spanned::Spanned;
use syn::visit::Visit;

const LINE_COVERAGE_GATE: f64 = 80.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CoverageMetric {
    count: u64,
    covered: u64,
}

impl CoverageMetric {
    fn percent(self) -> f64 {
        if self.count == 0 {
            100.0
        } else {
            (self.covered as f64 / self.count as f64) * 100.0
        }
    }

    fn missed(self) -> u64 {
        self.count.saturating_sub(self.covered)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct WorkspaceCoverage {
    lines: CoverageMetric,
    regions: CoverageMetric,
    functions: CoverageMetric,
    file_count: usize,
}

impl WorkspaceCoverage {
    fn add_file_metrics(
        &mut self,
        lines: CoverageMetric,
        regions: CoverageMetric,
        functions: CoverageMetric,
    ) {
        self.lines.count += lines.count;
        self.lines.covered += lines.covered;
        self.regions.count += regions.count;
        self.regions.covered += regions.covered;
        self.functions.count += functions.count;
        self.functions.covered += functions.covered;
        self.file_count += 1;
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("xtask error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("qa") => qa(),
        Some("guard") => guard(),
        Some("coverage") => coverage(),
        Some("fmt") => run_checked(command("cargo", ["fmt", "--all", "--", "--check"]), "cargo fmt"),
        Some("clippy") => run_checked(
            command("cargo", ["clippy", "--all-targets", "--", "-D", "warnings"]),
            "cargo clippy",
        ),
        Some("test") => run_checked(command("cargo", ["test", "--lib", "--tests"]), "cargo test"),
        Some(cmd) => bail!("unknown xtask command: {cmd}"),
        None => {
            eprintln!("usage: cargo xtask <qa|guard|coverage|fmt|clippy|test>");
            Ok(())
        }
    }
}

fn guard() -> Result<()> {
    enforce_no_runtime_stubs_or_todos()?;
    enforce_theme_usage()
}

fn qa() -> Result<()> {
    enforce_no_runtime_stubs_or_todos()?;
    enforce_theme_usage()?;
    run_checked(command("cargo", ["fmt", "--all", "--", "--check"]), "cargo fmt")?;
    run_checked(
        command("cargo", ["clippy", "--all-targets", "--", "-D", "warnings"]),
        "cargo clippy",
    )?;
    run_checked(command("cargo", ["test", "--lib", "--tests"]), "cargo test")?;
    coverage()
}

fn coverage() -> Result<()> {
    ensure_tool("cargo-llvm-cov", "cargo install cargo-llvm-cov")?;
    ensure_tool("rustup", "install rustup and llvm-tools-preview")?;

    let workspace_root = workspace_root();
    let llvm_cov = find_rustup_llvm_tool("llvm-cov")?;
    let llvm_profdata = find_rustup_llvm_tool("llvm-profdata")?;
    let ignore_regex = coverage_ignore_regex();

    let target_dir = workspace_root.join("target/llvm-cov-target");
    let summary_path = target_dir.join("workspace-summary.json");
    if target_dir.exists() {
        fs::remove_dir_all(&target_dir)
            .with_context(|| format!("remove stale coverage directory {}", target_dir.display()))?;
    }

    run_checked(
        command("cargo", ["llvm-cov", "clean", "--workspace"]),
        "cargo llvm-cov clean",
    )?;

    let mut run_cmd = command("cargo", ["llvm-cov", "--no-report", "--lib", "--tests", "-q"]);
    run_cmd.env("LLVM_COV", &llvm_cov);
    run_cmd.env("LLVM_PROFDATA", &llvm_profdata);
    run_checked(run_cmd, "cargo llvm-cov --no-report")?;

    let summary_path_arg = summary_path.to_string_lossy().into_owned();
    let mut report_cmd = command(
        "cargo",
        [
            "llvm-cov",
            "report",
            "--json",
            "--summary-only",
            "--skip-functions",
            "--ignore-filename-regex",
            ignore_regex.as_str(),
            "--output-path",
            summary_path_arg.as_str(),
        ],
    );
    report_cmd.env("LLVM_COV", &llvm_cov);
    report_cmd.env("LLVM_PROFDATA", &llvm_profdata);
    run_checked(report_cmd, "cargo llvm-cov report")?;

    let coverage = load_workspace_coverage(&summary_path, &workspace_root)?;
    print_workspace_coverage(&coverage, &summary_path);

    if coverage.lines.percent() < LINE_COVERAGE_GATE {
        bail!(
            "workspace line coverage {:.2}% is below the {:.2}% gate",
            coverage.lines.percent(),
            LINE_COVERAGE_GATE
        );
    }

    Ok(())
}

fn enforce_no_runtime_stubs_or_todos() -> Result<()> {
    let workspace_root = workspace_root();
    let src_dir = workspace_root.join("src");

    // Keep the dedicated test constructor available for test modules, but ban runtime usage.
    ensure_no_pattern_in_tree(
        &src_dir,
        "new_for_tests(",
        &["services/chat_impl.rs", "services/chat_impl/tests.rs"],
    )?;

    // Prevent TODO/FIXME/todo!/unimplemented! from landing in production code.
    for pattern in ["TODO", "FIXME", "todo!(", "unimplemented!("] {
        ensure_no_pattern_in_tree(&src_dir, pattern, &[])?;
    }

    Ok(())
}

fn enforce_theme_usage() -> Result<()> {
    let ui_gpui_dir = workspace_root().join("src/ui_gpui");

    // Theme infrastructure and builder helpers legitimately construct raw colors internally.
    let theme_allowlist = [
        "theme.rs",
        "mac_native.rs",
        "theme_catalog.rs",
        "theme/builders.rs",
    ];

    for pattern in ["hsla(", "rgb(", "rgba("] {
        ensure_no_pattern_in_tree(&ui_gpui_dir, pattern, &theme_allowlist)?;
    }

    Ok(())
}

fn ensure_no_pattern_in_tree(root: &Path, pattern: &str, allowlist: &[&str]) -> Result<()> {
    let mut violations = Vec::new();
    collect_pattern_violations(root, root, pattern, allowlist, &mut violations)?;

    if violations.is_empty() {
        return Ok(());
    }

    let details = violations
        .into_iter()
        .map(|(path, line, content)| format!("{}:{}: {}", path.display(), line, content.trim()))
        .collect::<Vec<_>>()
        .join("\n");

    bail!(
        "forbidden source pattern `{pattern}` detected in src/:\n{details}\n\nmove temporary markers or test-only wiring behind #[cfg(test)] and out of runtime source"
    )
}

struct CallPatternCollector<'a> {
    function_name: &'a str,
    violations: Vec<(usize, String)>,
}

impl<'ast> Visit<'ast> for CallPatternCollector<'_> {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*node.func {
            if path.path.is_ident(self.function_name) {
                let snippet: TokenStream = node.to_token_stream();
                self.violations.push((node.span().start().line, snippet.to_string()));
            }
        }

        syn::visit::visit_expr_call(self, node);
    }
}

fn collect_call_pattern_violations(content: &str, pattern: &str) -> Result<Vec<(usize, String)>> {
    let file = syn::parse_file(content).context("parse Rust source for pattern scan")?;
    let mut collector = CallPatternCollector {
        function_name: pattern.trim_end_matches('('),
        violations: Vec::new(),
    };
    collector.visit_file(&file);
    Ok(collector.violations)
}

fn collect_pattern_violations(
    dir: &Path,
    root: &Path,
    pattern: &str,
    allowlist: &[&str],
    violations: &mut Vec<(PathBuf, usize, String)>,
) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("read dir {}", dir.display()))? {
        let entry = entry.with_context(|| format!("read dir entry under {}", dir.display()))?;
        let path = entry.path();

        if path.is_dir() {
            collect_pattern_violations(&path, root, pattern, allowlist, violations)?;
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace("\\", "/");
        if allowlist.iter().any(|allowed| *allowed == relative) {
            continue;
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("read source file {}", path.display()))?;
        for (line, snippet) in collect_call_pattern_violations(&content, pattern)
            .with_context(|| format!("scan source file {}", path.display()))?
        {
            violations.push((PathBuf::from(&relative), line, snippet));
        }
    }

    Ok(())
}

fn coverage_ignore_regex() -> String {
    [
        // External vendored dependency
        "research/serdesAI/",
        // Binary entry points (no unit-testable logic)
        "src/main_gpui",
        "src/bin/",
        // Requires live LLM provider
        "src/llm/client_agent.rs",
        // macOS AppKit / IPC global singletons (no meaningful branch logic)
        "src/ui_gpui/navigation_channel.rs",
        "src/ui_gpui/selection_intent_channel.rs",
        // GPUI declarative render + IME impls (require live GPUI window context)
        r"/render\.rs$",
        r"/render_bars\.rs$",
        r"/render_sidebar\.rs$",
        r"/render_tool_approval\.rs$",
        r"/render_appearance\.rs$",
        r"/ime\.rs$",
    ]
    .join("|")
}

fn load_workspace_coverage(summary_path: &Path, workspace_root: &Path) -> Result<WorkspaceCoverage> {
    let report = fs::read_to_string(summary_path)
        .with_context(|| format!("read coverage summary {}", summary_path.display()))?;
    let report: Value = serde_json::from_str(&report)
        .with_context(|| format!("parse coverage summary {}", summary_path.display()))?;
    aggregate_workspace_coverage(&report, workspace_root)
}

fn aggregate_workspace_coverage(report: &Value, workspace_root: &Path) -> Result<WorkspaceCoverage> {
    let files = report
        .get("data")
        .and_then(Value::as_array)
        .and_then(|data| data.first())
        .and_then(|entry| entry.get("files"))
        .and_then(Value::as_array)
        .context("coverage summary missing data[0].files")?;

    let mut coverage = WorkspaceCoverage::default();
    for file in files {
        let filename = file
            .get("filename")
            .and_then(Value::as_str)
            .context("coverage file missing filename")?;
        let path = Path::new(filename);
        if !is_workspace_file(path, workspace_root) {
            continue;
        }

        let summary = file.get("summary").context("coverage file missing summary")?;
        coverage.add_file_metrics(
            read_metric(summary, "lines")?,
            read_metric(summary, "regions")?,
            read_metric(summary, "functions")?,
        );
    }

    if coverage.file_count == 0 {
        bail!(
            "coverage summary did not include any workspace files under {}",
            workspace_root.display()
        );
    }

    Ok(coverage)
}

fn is_workspace_file(path: &Path, workspace_root: &Path) -> bool {
    if path.is_absolute() {
        path.starts_with(workspace_root)
    } else {
        true
    }
}

fn read_metric(summary: &Value, key: &str) -> Result<CoverageMetric> {
    let metric = summary
        .get(key)
        .with_context(|| format!("coverage summary missing `{key}` metric"))?;
    Ok(CoverageMetric {
        count: metric
            .get("count")
            .and_then(Value::as_u64)
            .with_context(|| format!("coverage metric `{key}.count` missing or invalid"))?,
        covered: metric
            .get("covered")
            .and_then(Value::as_u64)
            .with_context(|| format!("coverage metric `{key}.covered` missing or invalid"))?,
    })
}

fn print_workspace_coverage(coverage: &WorkspaceCoverage, summary_path: &Path) {
    eprintln!(
        "workspace coverage summary ({} files, source: {}):",
        coverage.file_count,
        summary_path.display()
    );
    print_metric("lines", coverage.lines);
    print_metric("regions", coverage.regions);
    print_metric("functions", coverage.functions);
}

fn print_metric(label: &str, metric: CoverageMetric) {
    eprintln!(
        "  {label:<9} {:>6.2}% ({}/{}, missed {})",
        metric.percent(),
        metric.covered,
        metric.count,
        metric.missed()
    );
}

fn ensure_tool(tool: &str, install_hint: &str) -> Result<()> {
    if which(tool).is_some() {
        Ok(())
    } else {
        bail!("required tool `{tool}` not found; install with `{install_hint}`")
    }
}

fn find_rustup_llvm_tool(tool: &str) -> Result<PathBuf> {
    let rustc = capture("rustup", ["which", "rustc"])?;
    let rustc = PathBuf::from(rustc.trim());
    let toolchain_root = rustc
        .parent()
        .and_then(Path::parent)
        .context("resolve rustup toolchain root")?;
    let host = capture("rustc", ["-vV"])?;
    let host = host
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .context("read rustc host triple")?;
    let candidate = toolchain_root
        .join("lib")
        .join("rustlib")
        .join(host)
        .join("bin")
        .join(tool);

    if candidate.is_file() {
        Ok(candidate)
    } else {
        bail!(
            "required rustup LLVM tool `{}` not found at {}; run `rustup component add llvm-tools-preview`",
            tool,
            candidate.display()
        )
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives under workspace root")
        .to_path_buf()
}

fn command<I, S>(program: &str, args: I) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut cmd = Command::new(program);
    cmd.current_dir(workspace_root());
    cmd.args(args);
    cmd
}

fn run_checked(mut cmd: Command, label: &str) -> Result<()> {
    eprintln!("==> {label}");
    let status = cmd.status().with_context(|| format!("spawn {label}"))?;
    ensure_success(status, label)
}

fn ensure_success(status: ExitStatus, label: &str) -> Result<()> {
    if status.success() {
        Ok(())
    } else {
        bail!("{label} failed with status {status}")
    }
}

fn which(tool: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(tool))
        .find(|candidate| candidate.is_file())
}

fn capture<I, S>(program: &str, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = command(program, args)
        .output()
        .with_context(|| format!("spawn {program}"))?;
    if output.status.success() {
        String::from_utf8(output.stdout).context("decode command output")
    } else {
        bail!(
            "{} failed: {}",
            program,
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn aggregate_workspace_coverage_filters_external_files() {
        let workspace_root = Path::new("/workspace/gpuui");
        let report = json!({
            "data": [{
                "files": [
                    {
                        "filename": "/workspace/gpuui/src/lib.rs",
                        "summary": {
                            "lines": {"count": 10, "covered": 8},
                            "regions": {"count": 12, "covered": 9},
                            "functions": {"count": 4, "covered": 3}
                        }
                    },
                    {
                        "filename": "/private/tmp/rustc/library/std/src/lib.rs",
                        "summary": {
                            "lines": {"count": 100, "covered": 0},
                            "regions": {"count": 120, "covered": 0},
                            "functions": {"count": 40, "covered": 0}
                        }
                    }
                ]
            }]
        });

        let coverage = aggregate_workspace_coverage(&report, workspace_root).expect("coverage loads");

        assert_eq!(coverage.file_count, 1);
        assert_eq!(coverage.lines, CoverageMetric { count: 10, covered: 8 });
        assert_eq!(coverage.regions, CoverageMetric { count: 12, covered: 9 });
        assert_eq!(coverage.functions, CoverageMetric { count: 4, covered: 3 });
    }

    #[test]
    fn aggregate_workspace_coverage_keeps_relative_paths() {
        let workspace_root = Path::new("/workspace/gpuui");
        let report = json!({
            "data": [{
                "files": [
                    {
                        "filename": "src/ui_gpui/views/main_panel.rs",
                        "summary": {
                            "lines": {"count": 25, "covered": 10},
                            "regions": {"count": 30, "covered": 12},
                            "functions": {"count": 5, "covered": 2}
                        }
                    }
                ]
            }]
        });

        let coverage = aggregate_workspace_coverage(&report, workspace_root).expect("coverage loads");

        assert_eq!(coverage.file_count, 1);
        assert_eq!(coverage.lines.percent(), 40.0);
        assert_eq!(coverage.regions.percent(), 40.0);
        assert_eq!(coverage.functions.percent(), 40.0);
    }

    #[test]
    fn ensure_no_pattern_in_tree_ignores_allowlisted_files() {
        let top_level_temp = tempdir().expect("create top-level tempdir");
        let top_level_root = top_level_temp.path();
        let allowlisted = top_level_root.join("theme.rs");
        fs::write(&allowlisted, "fn transparent() { let _ = hsla(0.0, 0.0, 0.0, 0.0); }\n")
            .expect("write allowlisted file");
        ensure_no_pattern_in_tree(top_level_root, "hsla(", &["theme.rs"])
            .expect("allowlisted pattern passes");

        let nested_temp = tempdir().expect("create nested tempdir");
        let nested_root = nested_temp.path();
        let nested_dir = nested_root.join("theme");
        fs::create_dir_all(&nested_dir).expect("create nested theme directory");
        let nested_allowlisted = nested_dir.join("builders.rs");
        fs::write(
            &nested_allowlisted,
            "fn transparent_builder() { let _ = hsla(0.0, 0.0, 0.0, 0.0); }\n",
        )
        .expect("write nested allowlisted file");
        ensure_no_pattern_in_tree(nested_root, "hsla(", &["theme/builders.rs"])
            .expect("allowlisted pattern passes");
    }

    #[test]
    fn ensure_no_pattern_in_tree_reports_non_allowlisted_violations() {
        let temp = tempdir().expect("create tempdir");
        let root = temp.path();
        let violating = root.join("views").join("panel.rs");
        fs::create_dir_all(violating.parent().expect("panel parent exists"))
            .expect("create source directory");
        fs::write(&violating, "fn panel() { let _ = hsla(0.0, 0.0, 0.0, 0.0); }\n")
            .expect("write violating file");

        let error = ensure_no_pattern_in_tree(root, "hsla(", &["theme.rs"])
            .expect_err("non-allowlisted pattern should fail");
        let message = format!("{error:#}");

        assert!(message.contains("views/panel.rs:1"), "unexpected error: {message}");
        assert!(message.contains("hsla("), "unexpected error: {message}");
    }

    #[test]
    fn ensure_no_pattern_in_tree_ignores_comments_and_strings_without_calls() {
        let temp = tempdir().expect("create tempdir");
        let root = temp.path();
        let source = root.join("views").join("notes.rs");
        fs::create_dir_all(source.parent().expect("notes parent exists"))
            .expect("create source directory");
        fs::write(
            &source,
            "fn notes() {\n    // hsla(0.0, 0.0, 0.0, 0.0)\n    let _ = \"rgb(0xff0000)\";\n}\n",
        )
        .expect("write notes file");

        ensure_no_pattern_in_tree(root, "hsla(", &[]).expect("comment-only pattern passes");
        ensure_no_pattern_in_tree(root, "rgb(", &[]).expect("string-only pattern passes");
    }
}
