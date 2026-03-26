use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

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
        Some("coverage") => coverage(),
        Some("fmt") => run_checked(command("cargo", ["fmt", "--all", "--", "--check"]), "cargo fmt"),
        Some("clippy") => run_checked(
            command("cargo", ["clippy", "--all-targets", "--", "-D", "warnings"]),
            "cargo clippy",
        ),
        Some("test") => run_checked(command("cargo", ["test", "--lib", "--tests"]), "cargo test"),
        Some(cmd) => bail!("unknown xtask command: {cmd}"),
        None => {
            eprintln!("usage: cargo xtask <qa|coverage|fmt|clippy|test>");
            Ok(())
        }
    }
}

fn qa() -> Result<()> {
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

fn coverage_ignore_regex() -> String {
    [
        // External vendored dependency
        "research/serdesAI/",
        // Binary entry points (no unit-testable logic)
        "src/main_gpui",
        "src/bin/",
        // Requires live LLM provider
        "src/llm/client_agent.rs",
        // macOS AppKit / IPC plumbing (requires window server)
        "src/ui_gpui/popup_window.rs",
        "src/ui_gpui/tray_bridge.rs",
        "src/ui_gpui/navigation_channel.rs",
        "src/ui_gpui/selection_intent_channel.rs",
        // GPUI declarative render + IME impls (require live GPUI window context)
        r"/render\.rs$",
        r"/render_bars\.rs$",
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
}
