use anyhow::{bail, ensure, Context};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const RECEIPT_PATH: &str = "target/fasti-receipts/b1-contract-verification.json";
const GENERATED_REGISTRY_PATH: &str = "contracts/generated/v1/capabilities.json";
const EXAMPLES_DIRECTORY: &str = "contracts/examples/v1";
const PREFLIGHT_GATES: [&str; 6] = [
    "registry.validate",
    "generation.first",
    "generation.second",
    "generation.deterministic",
    "generation.checked_in",
    "examples.inventory",
];

pub(crate) struct VerificationFacts {
    pub contract_version: String,
    pub capability_count: usize,
    pub surface_profile_count: usize,
    pub generated_artifact_count: usize,
    pub example_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommandGate {
    id: &'static str,
    program: &'static str,
    args: Vec<OsString>,
    remediation: &'static str,
}

impl CommandGate {
    fn new(
        id: &'static str,
        program: &'static str,
        args: impl IntoIterator<Item = impl Into<OsString>>,
        remediation: &'static str,
    ) -> Self {
        Self {
            id,
            program,
            args: args.into_iter().map(Into::into).collect(),
            remediation,
        }
    }

    fn display(&self) -> String {
        std::iter::once(OsStr::new(self.program))
            .chain(self.args.iter().map(OsString::as_os_str))
            .map(|part| format!("{:?}", part.to_string_lossy()))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, Eq, PartialEq)]
struct GateOutcome {
    success: bool,
    status: String,
}

trait GateExecutor {
    fn execute(&mut self, root: &Path, gate: &CommandGate) -> anyhow::Result<GateOutcome>;
}

struct ProcessGateExecutor;

impl GateExecutor for ProcessGateExecutor {
    fn execute(&mut self, root: &Path, gate: &CommandGate) -> anyhow::Result<GateOutcome> {
        let status = Command::new(gate.program)
            .args(&gate.args)
            .current_dir(root)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .with_context(|| {
                format!(
                    "gate {} could not start `{}`; ensure `{}` is installed and rerun the verifier",
                    gate.id,
                    gate.display(),
                    gate.program
                )
            })?;
        Ok(GateOutcome {
            success: status.success(),
            status: status.to_string(),
        })
    }
}

pub(crate) fn clear_receipt(root: &Path) -> anyhow::Result<()> {
    let path = root.join(RECEIPT_PATH);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("failed to remove stale receipt {}", path.display()))
        }
    }
}

pub(crate) fn verify_examples(root: &Path) -> anyhow::Result<usize> {
    let registry_path = root.join(GENERATED_REGISTRY_PATH);
    let registry: Value = serde_json::from_reader(
        File::open(&registry_path)
            .with_context(|| format!("failed to open {}", registry_path.display()))?,
    )
    .with_context(|| format!("{} is not valid JSON", registry_path.display()))?;
    let capabilities = registry
        .get("capabilities")
        .and_then(Value::as_array)
        .context("generated capability registry must contain a capabilities array")?;

    let mut owners = BTreeMap::new();
    for capability in capabilities {
        let capability_id = capability
            .get("id")
            .and_then(Value::as_str)
            .context("generated capability registry entry omits id")?;
        let examples = capability
            .get("examples")
            .and_then(Value::as_array)
            .with_context(|| format!("capability {capability_id} omits its examples array"))?;
        for example in examples {
            let example_id = example
                .as_str()
                .with_context(|| format!("capability {capability_id} has a non-string example"))?;
            ensure!(
                owners
                    .insert(example_id.to_owned(), capability_id.to_owned())
                    .is_none(),
                "example {example_id} is owned by more than one capability"
            );
        }
    }
    ensure!(
        !owners.is_empty(),
        "the B1 capability registry declares no governed examples"
    );

    let examples_directory = root.join(EXAMPLES_DIRECTORY);
    let mut present = BTreeSet::new();
    for entry in fs::read_dir(&examples_directory)
        .with_context(|| format!("failed to inspect {}", examples_directory.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        ensure!(
            file_type.is_file(),
            "example inventory contains a non-file entry: {}",
            entry.path().display()
        );
        let path = entry.path();
        let extension = path.extension().and_then(OsStr::to_str).unwrap_or_default();
        if !matches!(extension, "json" | "jsonld" | "yaml" | "yml") {
            continue;
        }
        let example_id = path
            .file_stem()
            .and_then(OsStr::to_str)
            .with_context(|| format!("example filename is not UTF-8: {}", path.display()))?;
        ensure!(
            present.insert(example_id.to_owned()),
            "duplicate example file stem {example_id}"
        );
        parse_example(&path, extension)?;
    }

    let expected: BTreeSet<_> = owners.keys().cloned().collect();
    let missing: Vec<_> = expected.difference(&present).cloned().collect();
    let unregistered: Vec<_> = present.difference(&expected).cloned().collect();
    ensure!(
        missing.is_empty() && unregistered.is_empty(),
        "example inventory disagrees with the registry: missing={missing:?}, unregistered={unregistered:?}; add one .json, .jsonld, .yaml, or .yml file named exactly after each registry example ID"
    );
    Ok(expected.len())
}

fn parse_example(path: &Path, extension: &str) -> anyhow::Result<()> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read example {}", path.display()))?;
    let value: Value = match extension {
        "json" | "jsonld" => serde_json::from_str(&source)
            .with_context(|| format!("example {} is not valid JSON", path.display()))?,
        "yaml" | "yml" => serde_saphyr::from_str(&source)
            .with_context(|| format!("example {} is not valid YAML", path.display()))?,
        _ => bail!("unsupported example extension for {}", path.display()),
    };
    ensure!(
        value.is_object(),
        "example {} must contain one object",
        path.display()
    );
    Ok(())
}

pub(crate) fn run_and_write_receipt(
    root: &Path,
    locked: bool,
    facts: &VerificationFacts,
) -> anyhow::Result<PathBuf> {
    let gates = command_gates(locked);
    let mut executor = ProcessGateExecutor;
    let passed = run_command_gates(root, &gates, &mut executor)?;
    let source = read_source_state(root)?;
    write_receipt(root, locked, facts, &passed, &source)
}

fn run_command_gates(
    root: &Path,
    gates: &[CommandGate],
    executor: &mut impl GateExecutor,
) -> anyhow::Result<Vec<String>> {
    let mut passed = Vec::with_capacity(PREFLIGHT_GATES.len() + gates.len());
    passed.extend(PREFLIGHT_GATES.iter().map(|gate| (*gate).to_owned()));
    for gate in gates {
        println!("RUN [{}]: {}", gate.id, gate.display());
        std::io::stdout()
            .flush()
            .context("failed to flush the gate label before inherited command output")?;
        let outcome = executor.execute(root, gate)?;
        ensure!(
            outcome.success,
            "gate {} failed with {}; {}; command={}",
            gate.id,
            outcome.status,
            gate.remediation,
            gate.display()
        );
        println!("PASS [{}]", gate.id);
        passed.push(gate.id.to_owned());
    }
    Ok(passed)
}

fn command_gates(locked: bool) -> Vec<CommandGate> {
    let mut gates = Vec::new();
    if locked {
        gates.push(CommandGate::new(
            "lockfiles.pnpm",
            "pnpm",
            [
                "install",
                "--offline",
                "--frozen-lockfile",
                "--ignore-scripts",
            ],
            "restore pnpm-lock.yaml and package manifests to agreement, provision the local pnpm store, then rerun with --locked",
        ));
    }

    gates.extend([
        CommandGate::new(
            "examples.semantic",
            "node",
            ["scripts/validate-examples.mjs"],
            "add or repair scripts/validate-examples.mjs so every governed example is validated against its contract",
        ),
        CommandGate::new(
            "contracts.authored",
            "node",
            ["scripts/validate-authored-contracts.mjs"],
            "fix the authored AsyncAPI or JSON-LD source and its mutation sentinel",
        ),
        CommandGate::new(
            "contracts.generated",
            "node",
            ["scripts/validate-generated-contracts.mjs"],
            "regenerate and fix OpenAPI or JSON Schema parity before rerunning",
        ),
        CommandGate::new(
            "contracts.okf_uat",
            "node",
            ["scripts/validate-okf-uat.mjs"],
            "repair OKF links or UAT ownership so every governed ID resolves exactly once",
        ),
        CommandGate::new(
            "javascript.format",
            "pnpm",
            ["run", "format:check"],
            "run pnpm format and review only the intended formatting changes",
        ),
        CommandGate::new(
            "javascript.typecheck",
            "pnpm",
            ["run", "typecheck"],
            "fix TypeScript contract or SDK type errors before rerunning",
        ),
        CommandGate::new(
            "javascript.mutation_sdk_tests",
            "node",
            [
                "--test",
                "tests/js/authored-contracts.test.mjs",
                "tests/js/generated-contracts.test.mjs",
                "tests/js/sdk-client.test.mjs",
            ],
            "fix the failing mutation sentinel or black-box SDK behavior",
        ),
        CommandGate::new(
            "rust.format",
            "cargo",
            ["fmt", "--all", "--", "--check"],
            "run cargo fmt --all and review the resulting source changes",
        ),
        CommandGate::new(
            "rust.clippy",
            "cargo",
            cargo_args(
                locked,
                ["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
            ),
            "fix all workspace clippy diagnostics without suppressing contract safety checks",
        ),
        CommandGate::new(
            "rust.workspace_tests",
            "cargo",
            cargo_args(locked, ["test", "--workspace"]),
            "fix the failing Rust workspace test before rerunning",
        ),
        CommandGate::new(
            "rust.application_conformance",
            "cargo",
            cargo_args(
                locked,
                [
                    "test",
                    "-p",
                    "fasti-application",
                    "--features",
                    "conformance-fixture",
                    "--test",
                    "b1_conformance",
                ],
            ),
            "fix the feature-gated application conformance fixture or its explicit no-durability tests",
        ),
        CommandGate::new(
            "rust.http_conformance",
            "cargo",
            cargo_args(
                locked,
                [
                    "test",
                    "-p",
                    "fasti-api",
                    "--features",
                    "conformance-fixture",
                ],
            ),
            "fix the feature-gated Utoipa/router conformance tests without mounting fixture routes in production",
        ),
        CommandGate::new(
            "rust.workspace_build",
            "cargo",
            cargo_args(locked, ["build", "--workspace", "--all-targets"]),
            "fix the workspace package build before rerunning",
        ),
        CommandGate::new(
            "package.repository_truth",
            "bash",
            ["scripts/check-repository-truth.sh"],
            "remove false runtime, player, or retired-boundary claims from active repository surfaces",
        ),
        CommandGate::new(
            "package.no_publish",
            "bash",
            ["scripts/check-no-publish.sh"],
            "remove public publishing permissions and commands before the later release gate",
        ),
        CommandGate::new(
            "package.no_publish_mutation",
            "bash",
            ["scripts/test-no-publish-policy.sh"],
            "repair the no-publish policy so deliberate publishing mutations fail",
        ),
        CommandGate::new(
            "package.workspace_manifest",
            "node",
            ["scripts/check-js-workspace.mjs"],
            "repair package names, workspace membership, or package entrypoints",
        ),
        CommandGate::new(
            "package.sdk_import",
            "node",
            [
                "--input-type=module",
                "--eval",
                "const sdk = await import('./packages/sdk/dist/transport.js'); if (typeof sdk.FastiClient !== 'function' || typeof sdk.parseProblemDetails !== 'function') throw new Error('generated SDK entrypoint omits required exports');",
            ],
            "rebuild @fasti/sdk and restore its generated transport and problem parser exports",
        ),
        CommandGate::new(
            "package.cli_help",
            "cargo",
            cargo_args(locked, ["run", "--quiet", "-p", "fasti-cli", "--", "--help"]),
            "restore the packaged fasti CLI entrypoint without enabling unavailable operations",
        ),
    ]);
    gates
}

fn cargo_args<const N: usize>(locked: bool, args: [&str; N]) -> Vec<OsString> {
    let mut rendered: Vec<OsString> = args.into_iter().map(Into::into).collect();
    if locked {
        rendered.insert(1, "--locked".into());
        rendered.insert(2, "--offline".into());
    }
    rendered
}

#[derive(Debug, Eq, PartialEq)]
struct SourceState {
    git_commit: String,
    dirty: bool,
}

fn read_source_state(root: &Path) -> anyhow::Result<SourceState> {
    let commit = git_output(root, ["rev-parse", "--verify", "HEAD"])?;
    let status = git_output(root, ["status", "--porcelain=v1", "--untracked-files=all"])?;
    Ok(SourceState {
        git_commit: commit.trim().to_owned(),
        dirty: !status.trim().is_empty(),
    })
}

fn git_output<const N: usize>(root: &Path, args: [&str; N]) -> anyhow::Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .context("failed to start git while binding the verification receipt to source")?;
    ensure!(
        output.status.success(),
        "git could not bind the verification receipt to source: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    String::from_utf8(output.stdout).context("git emitted non-UTF-8 source metadata")
}

fn write_receipt(
    root: &Path,
    locked: bool,
    facts: &VerificationFacts,
    passed_gates: &[String],
    source: &SourceState,
) -> anyhow::Result<PathBuf> {
    let path = root.join(RECEIPT_PATH);
    ensure!(
        !source.dirty,
        "source tree is dirty after all verification gates; commit or stash every source change and rerun; no receipt was emitted"
    );
    let parent = path
        .parent()
        .context("verification receipt path has no parent")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create receipt directory {}", parent.display()))?;
    let verified_at_unix_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_secs();
    let gates: Vec<_> = passed_gates
        .iter()
        .map(|id| json!({ "id": id, "status": "pass" }))
        .collect();
    let receipt = json!({
        "receipt_version": "1.0.0",
        "kind": "fasti.b1.contract-verification",
        "verified_at_unix_seconds": verified_at_unix_seconds,
        "contract": {
            "version": facts.contract_version,
            "capability_count": facts.capability_count,
            "surface_profile_count": facts.surface_profile_count,
            "generated_artifact_count": facts.generated_artifact_count,
            "example_count": facts.example_count,
        },
        "dependency_lock_enforcement": {
            "requested": locked,
            "passed": locked,
            "offline_requested": locked,
            "offline_passed": locked,
        },
        "source": {
            "git_commit": source.git_commit,
            "dirty": source.dirty,
        },
        "scope": {
            "software_only": true,
            "performance_verified": false,
            "physical_hardware_verified": false,
            "declaration": "This receipt proves software contract gates only. It is not performance or physical-device evidence.",
        },
        "gate_count": gates.len(),
        "gates": gates,
    });

    let mut temporary = tempfile::NamedTempFile::new_in(parent).with_context(|| {
        format!(
            "failed to create a temporary receipt in {}",
            parent.display()
        )
    })?;
    {
        let mut writer = BufWriter::new(temporary.as_file_mut());
        serde_json::to_writer_pretty(&mut writer, &receipt)
            .context("verification receipt is not serializable")?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }
    temporary.as_file().sync_all()?;
    temporary
        .persist(&path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to atomically publish receipt {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeExecutor {
        calls: Vec<String>,
        fail_at: Option<usize>,
    }

    impl GateExecutor for FakeExecutor {
        fn execute(&mut self, _root: &Path, gate: &CommandGate) -> anyhow::Result<GateOutcome> {
            self.calls.push(gate.id.to_owned());
            let failed = self.fail_at == Some(self.calls.len());
            Ok(GateOutcome {
                success: !failed,
                status: if failed {
                    "exit status: 7"
                } else {
                    "exit status: 0"
                }
                .to_owned(),
            })
        }
    }

    #[test]
    fn locked_mode_pins_cargo_and_pnpm_commands() {
        let gates = command_gates(true);
        let pnpm_lock = gates
            .iter()
            .find(|gate| gate.id == "lockfiles.pnpm")
            .expect("locked mode adds a pnpm lock gate");
        assert!(pnpm_lock.args.iter().any(|arg| arg == "--frozen-lockfile"));
        assert!(pnpm_lock.args.iter().any(|arg| arg == "--offline"));
        for gate in gates
            .iter()
            .filter(|gate| gate.program == "cargo" && gate.id != "rust.format")
        {
            assert!(
                gate.args.iter().any(|arg| arg == "--locked"),
                "{} must honor --locked",
                gate.id
            );
            assert!(
                gate.args.iter().any(|arg| arg == "--offline"),
                "{} must resolve dependencies offline in locked mode",
                gate.id
            );
        }
        for gate in gates
            .iter()
            .filter(|gate| gate.program == "pnpm" && gate.id != "lockfiles.pnpm")
        {
            assert_eq!(gate.args.first(), Some(&OsString::from("run")));
            assert!(!gate.args.iter().any(|arg| arg == "--frozen-lockfile"));
        }
    }

    #[test]
    fn required_gate_inventory_is_explicit_and_has_no_shell_interpolation() {
        let gates = command_gates(true);
        let actual: BTreeSet<_> = gates.iter().map(|gate| gate.id).collect();
        let expected: BTreeSet<_> = [
            "lockfiles.pnpm",
            "examples.semantic",
            "contracts.authored",
            "contracts.generated",
            "contracts.okf_uat",
            "javascript.format",
            "javascript.typecheck",
            "javascript.mutation_sdk_tests",
            "rust.format",
            "rust.clippy",
            "rust.workspace_tests",
            "rust.application_conformance",
            "rust.http_conformance",
            "rust.workspace_build",
            "package.repository_truth",
            "package.no_publish",
            "package.no_publish_mutation",
            "package.workspace_manifest",
            "package.sdk_import",
            "package.cli_help",
        ]
        .into_iter()
        .collect();
        assert_eq!(actual, expected);
        assert_eq!(actual.len(), gates.len(), "gate IDs must be unique");
        for gate in gates {
            assert_ne!(gate.program, "sh");
            assert_ne!(gate.program, "zsh");
            assert!(!gate.args.iter().any(|arg| arg == "-c"));
            if gate.program == "bash" {
                assert_eq!(gate.args.len(), 1);
                assert!(gate.args[0].to_string_lossy().starts_with("scripts/"));
            }
        }
    }

    #[test]
    fn command_failure_is_actionable_and_stops_later_gates() {
        let gates = vec![
            CommandGate::new("first", "one", ["a"], "fix first"),
            CommandGate::new("second", "two", ["b"], "fix second"),
            CommandGate::new("third", "three", ["c"], "fix third"),
        ];
        let mut executor = FakeExecutor {
            fail_at: Some(2),
            ..FakeExecutor::default()
        };
        let error = run_command_gates(Path::new("."), &gates, &mut executor)
            .expect_err("second gate fails");
        assert_eq!(executor.calls, ["first", "second"]);
        let message = error.to_string();
        assert!(message.contains("gate second failed"));
        assert!(message.contains("fix second"));
        assert!(message.contains("\"two\" \"b\""));
    }

    #[test]
    fn example_inventory_reports_missing_and_unregistered_files() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let registry_path = root.path().join(GENERATED_REGISTRY_PATH);
        fs::create_dir_all(registry_path.parent().expect("registry parent"))
            .expect("create registry directory");
        fs::write(
            &registry_path,
            r#"{"capabilities":[{"id":"system.health","examples":["system.health.success"]}]}"#,
        )
        .expect("write registry");
        let examples = root.path().join(EXAMPLES_DIRECTORY);
        fs::create_dir_all(&examples).expect("create examples directory");
        fs::write(examples.join("orphan.example.json"), "{}\n").expect("write orphan");

        let error = verify_examples(root.path()).expect_err("inventory must disagree");
        let message = error.to_string();
        assert!(message.contains("system.health.success"));
        assert!(message.contains("orphan.example"));
    }

    #[test]
    fn receipt_is_atomic_and_explicitly_excludes_performance_claims() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let facts = VerificationFacts {
            contract_version: "1.0.0".to_owned(),
            capability_count: 22,
            surface_profile_count: 6,
            generated_artifact_count: 7,
            example_count: 12,
        };
        let source = SourceState {
            git_commit: "0123456789abcdef".to_owned(),
            dirty: false,
        };
        let gates = vec![
            "registry.validate".to_owned(),
            "examples.inventory".to_owned(),
        ];
        let path = write_receipt(root.path(), true, &facts, &gates, &source)
            .expect("write verification receipt");
        let receipt: Value = serde_json::from_reader(File::open(path).expect("open receipt"))
            .expect("parse receipt");
        assert_eq!(receipt.pointer("/source/dirty"), Some(&Value::Bool(false)));
        assert_eq!(
            receipt.pointer("/scope/software_only"),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            receipt.pointer("/scope/performance_verified"),
            Some(&Value::Bool(false))
        );
        assert_eq!(receipt.get("gate_count").and_then(Value::as_u64), Some(2));
        assert_eq!(
            receipt.pointer("/dependency_lock_enforcement/offline_passed"),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            fs::read_dir(root.path().join("target/fasti-receipts"))
                .expect("read receipt directory")
                .count(),
            1,
            "the atomic writer leaves no temporary receipt"
        );
    }

    #[test]
    fn dirty_source_cannot_emit_a_receipt() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let facts = VerificationFacts {
            contract_version: "1.0.0".to_owned(),
            capability_count: 22,
            surface_profile_count: 6,
            generated_artifact_count: 7,
            example_count: 12,
        };
        let source = SourceState {
            git_commit: "0123456789abcdef".to_owned(),
            dirty: true,
        };
        let error = write_receipt(root.path(), true, &facts, &[], &source)
            .expect_err("dirty source must fail closed");
        assert!(error.to_string().contains("source tree is dirty"));
        assert!(!root.path().join(RECEIPT_PATH).exists());
    }

    #[test]
    fn stale_receipt_is_removed_before_verification() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let receipt = root.path().join(RECEIPT_PATH);
        fs::create_dir_all(receipt.parent().expect("receipt parent"))
            .expect("create receipt directory");
        fs::write(&receipt, "stale\n").expect("write stale receipt");
        clear_receipt(root.path()).expect("clear stale receipt");
        assert!(!receipt.exists());
    }
}
