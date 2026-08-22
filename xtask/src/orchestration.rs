use crate::verify::{run_additional_gates, CommandGate};
use anyhow::{ensure, Context};
use std::path::Path;
use std::process::Command;

pub(crate) fn run_portable_b1(root: &Path) -> anyhow::Result<()> {
    let source_before = git_status(root)?;
    let gates = [
        CommandGate::new(
            "performance.static",
            "node",
            ["benchmarks/b1/validate-evidence.mjs", "--static"],
            "repair the B1 budgets, evidence schema, or device ledger",
        ),
        CommandGate::new(
            "performance.mutation_sentinels",
            "node",
            ["benchmarks/b1/validate-evidence.mjs", "--self-test"],
            "restore fail-closed performance receipt validation",
        ),
        CommandGate::new(
            "performance.capture_self_test",
            "python3",
            ["-B", "scripts/benchmark-b1.py", "self-test"],
            "repair the portable benchmark capture self-test",
        ),
        CommandGate::new(
            "performance.capture_unit_tests",
            "python3",
            ["-B", "-m", "unittest", "benchmarks/b1/test_benchmark_b1.py"],
            "repair the benchmark trust-boundary Python tests",
        ),
        CommandGate::new(
            "performance.tauri_mutation_sentinels",
            "node",
            [
                "benchmarks/b1/tauri-shell/validate-evidence.mjs",
                "--self-test",
            ],
            "repair the benchmark-only hidden Tauri receipt validator",
        ),
        CommandGate::new(
            "performance.tauri_capture_self_test",
            "python3",
            ["-B", "scripts/benchmark-tauri-b1.py", "self-test"],
            "repair the portable Tauri full-process-tree capture sentinels",
        ),
        CommandGate::new(
            "performance.tauri_capture_unit_tests",
            "python3",
            [
                "-B",
                "-m",
                "unittest",
                "benchmarks/b1/tauri-shell/test_benchmark_tauri.py",
            ],
            "repair the portable Tauri process-tree and receipt derivation tests",
        ),
        CommandGate::new(
            "performance.tauri_fixture_policy",
            "python3",
            ["-B", "scripts/benchmark-tauri-b1.py", "policy-check"],
            "restore the canonical tracked hidden-fixture policy",
        ),
        CommandGate::new(
            "performance.tauri_locked_release_build",
            "cargo",
            [
                "build",
                "--manifest-path",
                "benchmarks/b1/tauri-shell/src-tauri/Cargo.toml",
                "--release",
                "--locked",
                "--offline",
            ],
            "repair the isolated Tauri fixture or its locked dependency boundary",
        ),
        CommandGate::new(
            "performance.runner_bundle_self_test",
            "python3",
            ["-B", "scripts/package-b1-runner.py", "self-test"],
            "repair the exact-commit private runner bundle sentinels",
        ),
        CommandGate::new(
            "performance.runner_bundle_unit_tests",
            "python3",
            [
                "-B",
                "-m",
                "unittest",
                "benchmarks/b1/test_runner_bundle.py",
            ],
            "repair the private runner bundle schema and digest tests",
        ),
    ];
    run_additional_gates(root, &gates)?;
    let source_after = git_status(root)?;
    ensure!(
        source_after == source_before,
        "portable B1 gates changed the Git worktree; before={source_before:?}, after={source_after:?}"
    );
    Ok(())
}

pub(crate) fn run_deep_b1(root: &Path) -> anyhow::Result<()> {
    let gates = [CommandGate::new(
        "rust.documentation_tests",
        "cargo",
        ["test", "--workspace", "--doc", "--locked", "--offline"],
        "fix the deep documentation test failure without weakening the PR gate",
    )];
    run_additional_gates(root, &gates)?;
    println!(
        "NOT APPLICABLE: B2/B3 crash, persistence, corpus-load, restart, and restore matrices do not exist in the B1 headless contract body"
    );
    Ok(())
}

fn git_status(root: &Path) -> anyhow::Result<String> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .current_dir(root)
        .output()
        .context("failed to start git while checking portable-gate cleanliness")?;
    ensure!(
        output.status.success(),
        "git status failed while checking portable-gate cleanliness: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    String::from_utf8(output.stdout).context("git status emitted non-UTF-8 output")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worktree_snapshot_detects_new_untracked_files() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let status = Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(root.path())
            .status()
            .expect("initialize Git repository");
        assert!(status.success());
        let before = git_status(root.path()).expect("initial status");
        std::fs::write(root.path().join("generated.pyc"), b"cache").expect("write generated cache");
        let after = git_status(root.path()).expect("changed status");
        assert_ne!(before, after);
        assert!(after.contains("generated.pyc"));
    }
}
