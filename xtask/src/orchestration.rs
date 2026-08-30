use crate::verify::{run_additional_gates, write_gate_suite_receipt, CommandGate};
use anyhow::{ensure, Context};
use std::fs;
use std::path::Path;
use std::process::Command;

pub(crate) fn run_access_b(root: &Path) -> anyhow::Result<()> {
    let receipt = root.join("target/fasti-receipts/access-b.json");
    match fs::remove_file(&receipt) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to remove stale receipt {}", receipt.display()));
        }
    }
    let source_before = git_status(root)?;
    let gates = access_b_gates();
    let records = run_additional_gates(root, &gates)?;
    let source_after = git_status(root)?;
    ensure!(
        source_after == source_before,
        "Access B gates changed the Git worktree; before={source_before:?}, after={source_after:?}"
    );
    write_gate_suite_receipt(
        root,
        Path::new("target/fasti-receipts/access-b.json"),
        "fasti.access-b.gates",
        "cargo xtask test milestone --body B",
        &records,
    )?;
    Ok(())
}

pub(crate) fn access_b_gates() -> [CommandGate; 8] {
    [
        CommandGate::new(
            "trailbase.release_lock",
            "python3",
            ["-B", "scripts/trailbase_runtime.py", "verify-release"],
            "repair the exact TrailBase native and OCI release lock",
        ),
        CommandGate::new(
            "trailbase.runtime_mutation_sentinels",
            "python3",
            ["-B", "scripts/trailbase_runtime.py", "self-test"],
            "restore fail-closed TrailBase release, backup, and restore validation",
        ),
        CommandGate::new(
            "trailbase.launcher_syntax",
            "bash",
            ["-n", "scripts/dev.sh"],
            "repair the sole development launcher",
        ),
        CommandGate::new(
            "trailbase.launcher_invariants",
            "bash",
            ["scripts/dev.sh", "--self-test"],
            "repair the development launcher lifecycle and safety invariants",
        ),
        CommandGate::new(
            "trailbase.oci_conformance",
            "bash",
            ["scripts/smoke-trailbase-oci.sh"],
            "prepare the exact Podman image and repair the OCI lifecycle or isolation policy",
        ),
        CommandGate::new(
            "trailbase.combined_binary",
            "cargo",
            ["build", "-p", "fastid", "--locked", "--offline"],
            "prepare the exact Fasti daemon used by the combined resource probe",
        ),
        CommandGate::new(
            "trailbase.combined_resource_envelope",
            "bash",
            [
                "scripts/bench-envelope.sh",
                "--target",
                "ceiling",
                "--",
                "bash",
                "scripts/smoke-trailbase-combined.sh",
            ],
            "repair combined Fasti and TrailBase startup without raising the 192 MiB ceiling",
        ),
        CommandGate::new(
            "trailbase.account_conformance",
            "python3",
            [
                "-B",
                "scripts/smoke-trailbase.py",
                "--root",
                ".dev-trailbase",
                "--receipt",
                "target/fasti-receipts/trailbase-conformance.json",
            ],
            "prepare the exact native TrailBase depot and repair the account-lifecycle fixture",
        ),
    ]
}

pub(crate) fn run_portable_b1(root: &Path) -> anyhow::Result<()> {
    let source_before = git_status(root)?;
    let gates = portable_b1_gates();
    let records = run_additional_gates(root, &gates)?;
    let source_after = git_status(root)?;
    ensure!(
        source_after == source_before,
        "portable B1 gates changed the Git worktree; before={source_before:?}, after={source_after:?}"
    );
    write_gate_suite_receipt(
        root,
        Path::new("target/fasti-receipts/b1-portable.json"),
        "fasti.b1.portable-gates",
        "cargo xtask test pr",
        &records,
    )?;
    Ok(())
}

pub(crate) fn portable_b1_gates() -> [CommandGate; 11] {
    [
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
    ]
}

pub(crate) fn run_deep_b1(root: &Path) -> anyhow::Result<()> {
    let source_before = git_status(root)?;
    let gates = deep_b1_gates();
    let records = run_additional_gates(root, &gates)?;
    let source_after = git_status(root)?;
    ensure!(
        source_after == source_before,
        "deep B1 gates changed the Git worktree; before={source_before:?}, after={source_after:?}"
    );
    println!(
        "NOT APPLICABLE: B2/B3 crash, persistence, corpus-load, restart, restore, and writer-saturation matrices do not exist in the B1 headless contract body"
    );
    write_gate_suite_receipt(
        root,
        Path::new("target/fasti-receipts/b1-deep.json"),
        "fasti.b1.deep-gates",
        "cargo xtask test deep",
        &records,
    )?;
    Ok(())
}

/// Defines the deep B1 validation gates for documentation and arm64 OCI packaging.
///
/// # Examples
///
/// ```
/// let gates = deep_b1_gates();
/// assert_eq!(gates.len(), 3);
/// ```
pub(crate) fn deep_b1_gates() -> [CommandGate; 3] {
    [
        CommandGate::new(
            "rust.documentation_tests",
            "cargo",
            ["test", "--workspace", "--doc", "--locked", "--offline"],
            "fix the deep documentation test failure without weakening the PR gate",
        ),
        CommandGate::new(
            "package.arm64_oci_build",
            "podman",
            [
                "build",
                "--platform",
                "linux/arm64",
                "--tag",
                "fasti:deep-arm64",
                ".",
            ],
            "install Podman with arm64 execution support and repair the locked OCI package",
        ),
        CommandGate::new(
            "package.arm64_oci_smoke",
            "bash",
            [
                "scripts/smoke-oci.sh",
                "fasti:deep-arm64",
                "arm64",
                "podman",
            ],
            "repair the arm64 daemon/CLI package or its network-denied smoke journey",
        ),
    ]
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
    fn access_b_has_one_canonical_prepared_machine_gate() {
        let gates = access_b_gates();
        assert_eq!(
            gates.iter().map(CommandGate::id).collect::<Vec<_>>(),
            [
                "trailbase.release_lock",
                "trailbase.runtime_mutation_sentinels",
                "trailbase.launcher_syntax",
                "trailbase.launcher_invariants",
                "trailbase.oci_conformance",
                "trailbase.combined_binary",
                "trailbase.combined_resource_envelope",
                "trailbase.account_conformance",
            ]
        );
        assert!(gates[6]
            .display()
            .contains("scripts/smoke-trailbase-combined.sh"));
        assert!(gates[7].display().contains("scripts/smoke-trailbase.py"));
        assert!(gates[7].display().contains(".dev-trailbase"));
    }

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

    #[test]
    fn deep_gate_executes_the_arm64_package_under_network_denial() {
        let gates = deep_b1_gates();
        assert_eq!(
            gates.iter().map(CommandGate::id).collect::<Vec<_>>(),
            [
                "rust.documentation_tests",
                "package.arm64_oci_build",
                "package.arm64_oci_smoke",
            ]
        );
        assert!(gates[1].display().contains("linux/arm64"));
        assert!(gates[1].display().starts_with("\"podman\" \"build\""));
        assert!(gates[2].display().contains("scripts/smoke-oci.sh"));
        assert!(gates[2].display().contains("arm64"));
        assert!(gates[2].display().contains("podman"));
    }

    #[test]
    fn arm64_build_gate_no_longer_uses_docker_buildx_flags() {
        let gates = deep_b1_gates();
        let build_gate_display = gates[1].display();
        assert!(!build_gate_display.contains("buildx"));
        assert!(!build_gate_display.contains("--load"));
        assert!(!build_gate_display.contains("\"docker\""));
        assert_eq!(
            build_gate_display,
            "\"podman\" \"build\" \"--platform\" \"linux/arm64\" \"--tag\" \"fasti:deep-arm64\" \".\""
        );
    }

    #[test]
    fn arm64_smoke_gate_passes_the_runtime_argument_in_the_expected_order() {
        let gates = deep_b1_gates();
        assert_eq!(
            gates[2].display(),
            "\"bash\" \"scripts/smoke-oci.sh\" \"fasti:deep-arm64\" \"arm64\" \"podman\""
        );
    }
}
