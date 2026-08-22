use crate::{orchestration, verify};
use anyhow::{bail, ensure, Context};
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

const SCHEMA_ID: &str = "https://fasti.scrobble.dev/schemas/evidence/manifest-v1.json";
const EVIDENCE_SUPPORT_FILES: &[&str] = &[
    "benchmarks/b1/Dockerfile",
    "benchmarks/b1/budgets.json",
    "benchmarks/b1/budgets.schema.json",
    "benchmarks/b1/device-hypotheses.json",
    "benchmarks/b1/device-hypotheses.schema.json",
    "benchmarks/b1/evidence.schema.json",
    "benchmarks/b1/physical-profiles.json",
    "benchmarks/b1/physical-profiles.schema.json",
    "benchmarks/b1/validate-evidence.mjs",
    "benchmarks/b1/tauri-shell/evidence.schema.json",
    "benchmarks/b1/tauri-shell/fixture-policy.json",
    "benchmarks/b1/tauri-shell/fixture-policy.schema.json",
    "benchmarks/b1/tauri-shell/src-tauri/Cargo.lock",
    "benchmarks/b1/tauri-shell/validate-evidence.mjs",
    "scripts/benchmark-tauri-b1.py",
    "scripts/lib/strict-json.mjs",
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub(crate) enum Body {
    B0,
    B1,
    B2,
    B3,
}

impl Body {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::B0 => "B0",
            Self::B1 => "B1",
            Self::B2 => "B2",
            Self::B3 => "B3",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ResultStatus {
    Pass,
    Fail,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum DesignReviewStatus {
    Pass,
    Fail,
    NotApplicable,
}

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
enum EvidenceKind {
    B1ContractVerification,
    B1DeviceLedger,
    B1DeviceQualification,
    B1PerformancePi5,
    B1PerformanceJ4125,
    B1TauriShell,
    QaReview,
    RawResult,
    BuiltArtifact,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct SchemaBinding {
    id: String,
    sha256: String,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceBinding {
    git_commit: String,
    git_tree: String,
    tree_state: String,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct CiBinding {
    provider: String,
    run: String,
    job: String,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct RunnerBinding {
    runner_id: String,
    platform: String,
    architecture: String,
    tool_versions: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct EnvironmentBinding {
    declaration: String,
    network: String,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct CorpusBinding {
    seed: String,
    sha256: String,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewBinding {
    status: ResultStatus,
    evidence_id: String,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct DesignReviewBinding {
    status: DesignReviewStatus,
    reason: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct EvidenceEntry {
    id: String,
    kind: EvidenceKind,
    path: PathBuf,
    sha256: String,
    status: ResultStatus,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct Summary {
    status: ResultStatus,
    pass: usize,
    fail: usize,
    unsupported: usize,
    bound_files: usize,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct EvidenceManifest {
    schema: SchemaBinding,
    body: Body,
    source: SourceBinding,
    ci: CiBinding,
    command: String,
    runner: RunnerBinding,
    environment: EnvironmentBinding,
    corpus: CorpusBinding,
    qa: ReviewBinding,
    design_review: DesignReviewBinding,
    evidence_roots: Vec<PathBuf>,
    evidence: Vec<EvidenceEntry>,
    summary: Summary,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct EvidenceEnvelope {
    manifest: EvidenceManifest,
    manifest_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QaReceipt {
    schema_version: String,
    kind: String,
    body: Body,
    status: ResultStatus,
    reviewed_commit: String,
    reviewed_tree: String,
    review_command: String,
    open_findings: usize,
    rendered_ui_or_ux_changed: bool,
    design_review: QaDesignReview,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QaDesignReview {
    status: DesignReviewStatus,
    reason: String,
}

#[derive(Debug)]
pub(crate) struct VerifiedManifest {
    manifest: EvidenceManifest,
}

pub(crate) fn print_schema() -> anyhow::Result<()> {
    let schema = schema_for!(EvidenceEnvelope);
    let digest = schema_digest()?;
    println!("schema_id={SCHEMA_ID}");
    println!("schema_sha256={digest}");
    println!("{}", serde_json::to_string_pretty(&schema)?);
    Ok(())
}

pub(crate) fn create_b1_milestone_manifest(
    root: &Path,
    manifest_path: &Path,
) -> anyhow::Result<PathBuf> {
    let manifest_path = safe_manifest_output_path(root, manifest_path)?;
    let candidate_path = root.join("target/fasti-evidence/b1-incomplete-candidate.json");
    remove_if_present(&manifest_path)?;
    remove_if_present(&candidate_path)?;

    match build_b1_milestone_manifest(root, &manifest_path) {
        Ok(path) => match verify_b1_milestone(root, &path) {
            Ok(()) => Ok(path),
            Err(error) => {
                remove_if_present(&manifest_path)?;
                write_incomplete_candidate(root, &candidate_path, &error)?;
                Err(error.context(format!(
                    "generated B1 manifest failed immediate verification and was removed; incomplete candidate={}",
                    candidate_path.display()
                )))
            }
        },
        Err(error) => {
            write_incomplete_candidate(root, &candidate_path, &error)?;
            Err(error.context(format!(
                "B1 milestone manifest was not emitted; incomplete candidate={}",
                candidate_path.display()
            )))
        }
    }
}

fn safe_manifest_output_path(root: &Path, requested: &Path) -> anyhow::Result<PathBuf> {
    let absolute = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        root.join(requested)
    };
    let relative = absolute.strip_prefix(root).with_context(|| {
        format!(
            "milestone manifest output must remain inside {}",
            root.join("target/fasti-evidence").display()
        )
    })?;
    validate_relative_path(relative)?;
    ensure!(
        relative.starts_with("target/fasti-evidence")
            && relative != Path::new("target/fasti-evidence")
            && relative.extension().and_then(|value| value.to_str()) == Some("json"),
        "milestone manifest output must be a .json file below target/fasti-evidence"
    );
    reject_symlink_components(root, relative)?;
    Ok(absolute)
}

fn build_b1_milestone_manifest(root: &Path, manifest_path: &Path) -> anyhow::Result<PathBuf> {
    let source = current_source_binding(root)?;
    verify_source_binding(root, &source)?;

    let contract_path = PathBuf::from("target/fasti-receipts/b1-contract-verification.json");
    let portable_path = PathBuf::from("target/fasti-receipts/b1-portable.json");
    let deep_path = PathBuf::from("target/fasti-receipts/b1-deep.json");
    let ledger_path = PathBuf::from("benchmarks/b1/device-hypotheses.json");
    let qualification_path =
        PathBuf::from("target/fasti-evidence/qualification/b1-device-qualification.json");
    let qa_path = PathBuf::from("target/fasti-evidence/qa/b1-qa.json");
    let physical_root = PathBuf::from("benchmarks/b1/evidence");
    let tauri_root = PathBuf::from("benchmarks/b1/tauri-shell/evidence");
    let contract_root = PathBuf::from("target/fasti-receipts");
    let qa_root = PathBuf::from("target/fasti-evidence/qa");

    for required in [
        &contract_path,
        &portable_path,
        &deep_path,
        &ledger_path,
        &qa_path,
    ] {
        ensure!(
            root.join(required).is_file(),
            "required B1 evidence is missing: {}",
            required.display()
        );
    }
    let (pi_path, j4125_path) = physical_evidence_paths(root)?;
    write_device_qualification(root, &qualification_path, &pi_path, &j4125_path)?;
    let physical_artifacts = physical_retained_artifacts(root, [&pi_path, &j4125_path])?;
    let tauri_path = exactly_one_json(root, &tauri_root, "Tauri")?;
    let tauri_receipt = read_json(root.join(&tauri_path))?;
    let tauri_artifact_path = PathBuf::from(
        tauri_receipt
            .pointer("/artifact/path")
            .and_then(Value::as_str)
            .context("Tauri receipt artifact path is missing")?,
    );
    validate_relative_path(&tauri_artifact_path)?;

    let mut entries = vec![
        evidence_entry(
            root,
            "b1-contract-verification",
            EvidenceKind::B1ContractVerification,
            contract_path,
        )?,
        evidence_entry(
            root,
            "b1-device-ledger",
            EvidenceKind::B1DeviceLedger,
            ledger_path,
        )?,
        evidence_entry(
            root,
            "b1-device-qualification",
            EvidenceKind::B1DeviceQualification,
            qualification_path,
        )?,
        evidence_entry(root, "b1-deep-gates", EvidenceKind::RawResult, deep_path)?,
        evidence_entry(
            root,
            "b1-performance-j4125",
            EvidenceKind::B1PerformanceJ4125,
            j4125_path,
        )?,
        evidence_entry(
            root,
            "b1-performance-pi5",
            EvidenceKind::B1PerformancePi5,
            pi_path,
        )?,
        evidence_entry(root, "b1-qa", EvidenceKind::QaReview, qa_path)?,
        evidence_entry(
            root,
            "b1-portable-gates",
            EvidenceKind::RawResult,
            portable_path,
        )?,
        evidence_entry(
            root,
            "b1-tauri-shell",
            EvidenceKind::B1TauriShell,
            tauri_path,
        )?,
        evidence_entry(
            root,
            "b1-tauri-shell-artifact",
            EvidenceKind::BuiltArtifact,
            tauri_artifact_path,
        )?,
    ];
    for (path, expected_digest) in physical_artifacts {
        let artifact_id = format!("b1-physical-artifact-{expected_digest}");
        let artifact = evidence_entry(root, &artifact_id, EvidenceKind::BuiltArtifact, path)?;
        ensure!(
            artifact.sha256 == expected_digest,
            "physical receipt and retained artifact digest disagree"
        );
        entries.push(artifact);
    }
    entries.sort_by(|left, right| (&left.id, &left.path).cmp(&(&right.id, &right.path)));
    let qualification_root = PathBuf::from("target/fasti-evidence/qualification");
    let mut evidence_roots = vec![
        contract_root,
        physical_root,
        tauri_root,
        qa_root,
        qualification_root,
    ];
    evidence_roots.sort();

    let snapshot = snapshot_evidence_files(root, &source, &entries)?;
    let ci = current_ci_binding()?;
    verify_evidence_inventory(root, &evidence_roots, &entries)?;
    for entry in &entries {
        validate_entry_semantics(root, snapshot.path(), entry, &source, &ci)?;
    }
    verify_b1_device_qualification(snapshot.path(), &entries)?;
    let qa = validate_qa_receipt(snapshot.path(), &entries, &source)?;
    verify_source_binding(root, &source)?;

    let corpus_bytes = fs::read(snapshot.path().join("benchmarks/b1/budgets.json"))
        .context("failed to read the governed B1 budget/corpus seed input")?;
    let count = entries.len();
    let manifest = EvidenceManifest {
        schema: SchemaBinding {
            id: SCHEMA_ID.to_owned(),
            sha256: schema_digest()?,
        },
        body: Body::B1,
        source,
        ci,
        command: "cargo xtask test milestone --body B1".to_owned(),
        runner: current_runner_binding(root)?,
        environment: EnvironmentBinding {
            declaration: "B1 live contract/deep gates plus digest-bound physical and benchmark-only process evidence".to_owned(),
            network: "per-receipt isolation; orchestration itself makes no global network-denied claim".to_owned(),
        },
        corpus: CorpusBinding {
            seed: "b1-empty-process-and-contract-v1".to_owned(),
            sha256: sha256_bytes(&corpus_bytes),
        },
        qa: ReviewBinding {
            status: qa.status,
            evidence_id: "b1-qa".to_owned(),
        },
        design_review: DesignReviewBinding {
            status: qa.design_review.status,
            reason: qa.design_review.reason,
        },
        evidence_roots,
        evidence: entries,
        summary: Summary {
            status: ResultStatus::Pass,
            pass: count,
            fail: 0,
            unsupported: 0,
            bound_files: count,
        },
    };
    let canonical = serde_json_canonicalizer::to_vec(&manifest)
        .context("failed to canonicalize generated B1 milestone evidence")?;
    let envelope = EvidenceEnvelope {
        manifest,
        manifest_sha256: sha256_bytes(&canonical),
    };
    write_json_atomic(manifest_path, &envelope)?;
    println!(
        "PASS: generated canonical B1 milestone manifest {}",
        manifest_path.display()
    );
    Ok(manifest_path.to_path_buf())
}

fn verify_envelope(root: &Path, manifest_path: &Path) -> anyhow::Result<VerifiedManifest> {
    let source = fs::read_to_string(manifest_path).with_context(|| {
        format!(
            "failed to read evidence manifest {}",
            manifest_path.display()
        )
    })?;
    let envelope: EvidenceEnvelope = serde_json::from_str(&source).with_context(|| {
        format!(
            "{} does not conform to the strict evidence-manifest schema",
            manifest_path.display()
        )
    })?;

    validate_manifest_shape(&envelope)?;
    verify_manifest_digest(&envelope)?;
    verify_source_binding(root, &envelope.manifest.source)?;
    verify_evidence_inventory(
        root,
        &envelope.manifest.evidence_roots,
        &envelope.manifest.evidence,
    )?;

    Ok(VerifiedManifest {
        manifest: envelope.manifest,
    })
}

pub(crate) fn verify(root: &Path, manifest_path: &Path) -> anyhow::Result<VerifiedManifest> {
    let verified = verify_envelope(root, manifest_path)?;
    match verified.manifest.body {
        Body::B1 => verify_b1_manifest_requirements(root, &verified.manifest)?,
        body => bail!(
            "{} evidence verification has no implemented body-specific closure policy",
            body.as_str()
        ),
    }
    println!(
        "PASS: verified {} bound evidence files and body-specific closure for {} without making a publisher-authenticity claim",
        verified.manifest.evidence.len(),
        verified.manifest.body.as_str()
    );
    Ok(verified)
}

pub(crate) fn verify_b1_milestone(root: &Path, manifest_path: &Path) -> anyhow::Result<()> {
    let verified = verify(root, manifest_path)?;
    ensure!(
        verified.manifest.body == Body::B1,
        "milestone B1 requires a B1 manifest"
    );
    Ok(())
}

fn verify_b1_manifest_requirements(root: &Path, manifest: &EvidenceManifest) -> anyhow::Result<()> {
    ensure!(
        manifest.command == "cargo xtask test milestone --body B1",
        "B1 evidence manifest must bind the exact milestone command"
    );
    ensure!(
        manifest.summary.status == ResultStatus::Pass,
        "B1 evidence manifest contains a failing or unsupported result"
    );
    ensure!(
        manifest.qa.status == ResultStatus::Pass,
        "mandatory B1 QA is not recorded as passing"
    );
    ensure!(
        manifest.design_review.status == DesignReviewStatus::NotApplicable
            && !manifest.design_review.reason.trim().is_empty(),
        "headless B1 must record design review as unsupported/not applicable with a reason"
    );

    let required = [
        EvidenceKind::B1ContractVerification,
        EvidenceKind::B1DeviceLedger,
        EvidenceKind::B1DeviceQualification,
        EvidenceKind::B1PerformancePi5,
        EvidenceKind::B1PerformanceJ4125,
        EvidenceKind::B1TauriShell,
        EvidenceKind::QaReview,
    ];
    for kind in required {
        let count = manifest
            .evidence
            .iter()
            .filter(|entry| entry.kind == kind && entry.status == ResultStatus::Pass)
            .count();
        ensure!(
            count == 1,
            "B1 milestone requires exactly one passing {kind:?} evidence entry; found {count}"
        );
    }
    let raw_results = manifest
        .evidence
        .iter()
        .filter(|entry| entry.kind == EvidenceKind::RawResult && entry.status == ResultStatus::Pass)
        .collect::<Vec<_>>();
    ensure!(
        raw_results.len() == 2
            && raw_results
                .iter()
                .any(|entry| entry.id == "b1-portable-gates")
            && raw_results.iter().any(|entry| entry.id == "b1-deep-gates"),
        "B1 milestone requires exactly the portable and deep passing raw-result receipts"
    );

    let qa_entry = manifest
        .evidence
        .iter()
        .find(|entry| entry.id == manifest.qa.evidence_id)
        .context("qa.evidence_id does not resolve to a bound evidence entry")?;
    ensure!(
        qa_entry.kind == EvidenceKind::QaReview && qa_entry.status == ResultStatus::Pass,
        "qa.evidence_id must resolve to the passing QA review entry"
    );

    let snapshot = snapshot_evidence_files(root, &manifest.source, &manifest.evidence)?;
    for entry in &manifest.evidence {
        validate_entry_semantics(root, snapshot.path(), entry, &manifest.source, &manifest.ci)?;
    }
    validate_qa_receipt(snapshot.path(), &manifest.evidence, &manifest.source)?;
    verify_b1_device_qualification(snapshot.path(), &manifest.evidence)?;
    verify_physical_artifact_bindings(snapshot.path(), &manifest.evidence)?;
    verify_tauri_artifact_binding(snapshot.path(), &manifest.evidence)?;
    let corpus_bytes = fs::read(snapshot.path().join("benchmarks/b1/budgets.json"))
        .context("failed to read the snapshotted B1 budget/corpus seed input")?;
    ensure!(
        manifest.corpus.seed == "b1-empty-process-and-contract-v1"
            && manifest.corpus.sha256 == sha256_bytes(&corpus_bytes),
        "B1 corpus binding does not recompute from the governed budget seed"
    );
    verify_source_binding(root, &manifest.source)?;
    println!("PASS: B1 milestone evidence is complete, physical, current, and fail-closed");
    Ok(())
}

fn validate_manifest_shape(envelope: &EvidenceEnvelope) -> anyhow::Result<()> {
    let manifest = &envelope.manifest;
    ensure!(
        manifest.schema.id == SCHEMA_ID,
        "unexpected evidence schema ID"
    );
    let expected_schema_digest = schema_digest()?;
    ensure!(
        manifest.schema.sha256 == expected_schema_digest,
        "evidence schema digest is stale or substituted; expected {expected_schema_digest}"
    );
    ensure!(
        manifest.source.tree_state == "clean",
        "source.tree_state must be clean"
    );
    ensure!(
        !manifest.command.trim().is_empty(),
        "command must not be empty"
    );
    for value in [
        &manifest.ci.provider,
        &manifest.ci.run,
        &manifest.ci.job,
        &manifest.runner.runner_id,
        &manifest.runner.platform,
        &manifest.runner.architecture,
        &manifest.environment.declaration,
        &manifest.environment.network,
        &manifest.corpus.seed,
    ] {
        ensure!(
            !value.trim().is_empty(),
            "manifest binding strings must not be empty"
        );
    }
    validate_ci_binding(&manifest.ci)?;
    ensure!(
        !manifest.runner.tool_versions.is_empty()
            && manifest
                .runner
                .tool_versions
                .iter()
                .all(|value| !value.trim().is_empty()),
        "runner.tool_versions must bind at least one exact tool version"
    );
    ensure_sha256(&manifest.schema.sha256, "schema.sha256")?;
    ensure_sha256(&manifest.corpus.sha256, "corpus.sha256")?;
    ensure_sha256(&envelope.manifest_sha256, "manifest_sha256")?;
    ensure!(
        manifest
            .evidence
            .windows(2)
            .all(|pair| { (&pair[0].id, &pair[0].path) < (&pair[1].id, &pair[1].path) }),
        "evidence entries must be uniquely sorted by id then path; reordered evidence is rejected"
    );
    let ids: BTreeSet<_> = manifest.evidence.iter().map(|entry| &entry.id).collect();
    let paths: BTreeSet<_> = manifest.evidence.iter().map(|entry| &entry.path).collect();
    ensure!(
        ids.len() == manifest.evidence.len(),
        "duplicate evidence IDs are forbidden"
    );
    ensure!(
        paths.len() == manifest.evidence.len(),
        "duplicate evidence paths are forbidden"
    );
    for entry in &manifest.evidence {
        ensure!(!entry.id.trim().is_empty(), "evidence ID must not be empty");
        ensure_sha256(&entry.sha256, "evidence.sha256")?;
        validate_relative_path(&entry.path)?;
    }
    ensure!(
        !manifest.evidence_roots.is_empty(),
        "evidence_roots must declare at least one exhaustively bound directory"
    );
    ensure!(
        manifest
            .evidence_roots
            .windows(2)
            .all(|pair| pair[0] < pair[1] && !pair[1].starts_with(&pair[0])),
        "evidence_roots must be uniquely sorted and must not overlap"
    );
    for root in &manifest.evidence_roots {
        validate_relative_path(root)?;
    }

    let pass = manifest
        .evidence
        .iter()
        .filter(|entry| entry.status == ResultStatus::Pass)
        .count();
    let fail = manifest
        .evidence
        .iter()
        .filter(|entry| entry.status == ResultStatus::Fail)
        .count();
    let unsupported = manifest
        .evidence
        .iter()
        .filter(|entry| entry.status == ResultStatus::Unsupported)
        .count();
    let status = if fail > 0 {
        ResultStatus::Fail
    } else if unsupported > 0 {
        ResultStatus::Unsupported
    } else {
        ResultStatus::Pass
    };
    ensure!(
        manifest.summary.pass == pass,
        "summary.pass was not recomputed correctly"
    );
    ensure!(
        manifest.summary.fail == fail,
        "summary.fail was not recomputed correctly"
    );
    ensure!(
        manifest.summary.unsupported == unsupported,
        "summary.unsupported was not recomputed correctly"
    );
    ensure!(
        manifest.summary.bound_files == manifest.evidence.len(),
        "summary.bound_files must equal the exact evidence inventory"
    );
    ensure!(
        manifest.summary.status == status,
        "summary.status was not recomputed correctly"
    );
    Ok(())
}

fn verify_manifest_digest(envelope: &EvidenceEnvelope) -> anyhow::Result<()> {
    let canonical = serde_json_canonicalizer::to_vec(&envelope.manifest)
        .context("failed to canonicalize the evidence manifest as RFC 8785 JSON")?;
    let expected = sha256_bytes(&canonical);
    ensure!(
        expected == envelope.manifest_sha256,
        "manifest_sha256 does not match the RFC 8785 canonical manifest bytes; expected {expected}"
    );
    Ok(())
}

fn verify_source_binding(root: &Path, source: &SourceBinding) -> anyhow::Result<()> {
    ensure_hex(&source.git_commit, 40, "source.git_commit")?;
    ensure_hex(&source.git_tree, 40, "source.git_tree")?;
    let current_commit = git_output(root, &["rev-parse", "--verify", "HEAD"])?;
    let current_tree = git_output(root, &["rev-parse", "HEAD^{tree}"])?;
    ensure!(
        source.git_commit == current_commit,
        "evidence source commit is stale"
    );
    ensure!(
        source.git_tree == current_tree,
        "evidence source tree is stale"
    );
    let status = git_output(root, &["status", "--porcelain=v1", "--untracked-files=all"])?;
    ensure!(
        status.is_empty(),
        "working tree is dirty; evidence cannot bind uncommitted source"
    );
    Ok(())
}

#[cfg(test)]
fn verify_entry_files(root: &Path, entries: &[EvidenceEntry]) -> anyhow::Result<()> {
    for entry in entries {
        let bytes = read_bound_file_once(root, &entry.path)?;
        ensure!(
            sha256_bytes(&bytes) == entry.sha256,
            "bound evidence digest mismatch: {}",
            entry.path.display()
        );
    }
    Ok(())
}

fn snapshot_evidence_files(
    root: &Path,
    source: &SourceBinding,
    entries: &[EvidenceEntry],
) -> anyhow::Result<tempfile::TempDir> {
    let snapshot_parent = root.join("target/fasti-verifier-snapshots");
    fs::create_dir_all(&snapshot_parent).context("failed to create verifier snapshot parent")?;
    let snapshot = tempfile::tempdir_in(snapshot_parent)
        .context("failed to create verifier-owned evidence snapshot")?;
    for entry in entries {
        let bytes = match read_tracked_file_at(root, &source.git_commit, &entry.path)? {
            Some(bytes) => bytes,
            None => read_bound_file_once(root, &entry.path)?,
        };
        ensure!(
            sha256_bytes(&bytes) == entry.sha256,
            "bound evidence digest mismatch: {}",
            entry.path.display()
        );
        write_snapshot_file(snapshot.path(), &entry.path, &bytes)?;
    }
    for relative in EVIDENCE_SUPPORT_FILES {
        let relative = Path::new(relative);
        if snapshot.path().join(relative).exists() {
            continue;
        }
        let bytes =
            read_tracked_file_at(root, &source.git_commit, relative)?.with_context(|| {
                format!(
                    "evidence validator support file is not tracked at {}: {}",
                    source.git_commit,
                    relative.display()
                )
            })?;
        write_snapshot_file(snapshot.path(), relative, &bytes)?;
    }
    Ok(snapshot)
}

fn read_tracked_file_at(
    root: &Path,
    commit: &str,
    relative: &Path,
) -> anyhow::Result<Option<Vec<u8>>> {
    let relative = relative
        .to_str()
        .context("tracked evidence path is not UTF-8")?;
    let listing = Command::new("git")
        .args(["ls-tree", commit, "--", relative])
        .current_dir(root)
        .output()
        .context("failed to inspect the bound Git tree for evidence bytes")?;
    ensure!(
        listing.status.success(),
        "git ls-tree failed for bound evidence path {relative}: {}",
        String::from_utf8_lossy(&listing.stderr).trim()
    );
    if listing.stdout.is_empty() {
        return Ok(None);
    }
    let listing = String::from_utf8(listing.stdout)
        .context("git ls-tree emitted non-UTF-8 evidence metadata")?;
    let mut lines = listing.lines();
    let line = lines.next().context("git ls-tree emitted an empty entry")?;
    ensure!(
        lines.next().is_none(),
        "bound evidence path resolved to more than one Git entry: {relative}"
    );
    let (metadata, listed_path) = line
        .split_once('\t')
        .context("git ls-tree evidence entry is malformed")?;
    ensure!(
        listed_path == relative,
        "git ls-tree substituted the bound evidence path"
    );
    let mut metadata = metadata.split_whitespace();
    let mode = metadata.next().context("git ls-tree entry omits mode")?;
    let kind = metadata.next().context("git ls-tree entry omits kind")?;
    let object = metadata
        .next()
        .context("git ls-tree entry omits object ID")?;
    ensure!(
        matches!(mode, "100644" | "100755") && kind == "blob" && metadata.next().is_none(),
        "bound tracked evidence is not a regular file: {relative}"
    );
    let blob = Command::new("git")
        .args(["cat-file", "blob", object])
        .current_dir(root)
        .output()
        .context("failed to read bound evidence bytes from Git")?;
    ensure!(
        blob.status.success(),
        "git cat-file failed for bound evidence path {relative}: {}",
        String::from_utf8_lossy(&blob.stderr).trim()
    );
    Ok(Some(blob.stdout))
}

fn read_bound_file_once(root: &Path, relative: &Path) -> anyhow::Result<Vec<u8>> {
    let canonical_root = root
        .canonicalize()
        .context("failed to canonicalize workspace root")?;
    reject_symlink_components(root, relative)?;
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path)
        .with_context(|| format!("bound evidence is missing: {}", relative.display()))?;
    ensure!(
        metadata.is_file(),
        "bound evidence is not a regular file: {}",
        relative.display()
    );
    ensure!(
        !metadata.file_type().is_symlink(),
        "bound evidence must not be a symlink: {}",
        relative.display()
    );
    let canonical = path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", relative.display()))?;
    ensure!(
        canonical.starts_with(&canonical_root),
        "bound evidence escapes the workspace: {}",
        relative.display()
    );
    fs::read(&canonical).with_context(|| format!("failed to read {}", relative.display()))
}

fn write_snapshot_file(root: &Path, relative: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let path = root.join(relative);
    let parent = path
        .parent()
        .context("evidence snapshot path has no parent")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create evidence snapshot {}", parent.display()))?;
    fs::write(&path, bytes)
        .with_context(|| format!("failed to write evidence snapshot {}", relative.display()))
}

fn verify_evidence_inventory(
    root: &Path,
    evidence_roots: &[PathBuf],
    entries: &[EvidenceEntry],
) -> anyhow::Result<()> {
    let mut actual = BTreeSet::new();
    for evidence_root in evidence_roots {
        reject_symlink_components(root, evidence_root)?;
        let path = root.join(evidence_root);
        ensure!(
            path.is_dir(),
            "evidence root is missing or is not a directory: {}",
            evidence_root.display()
        );
        collect_inventory(root, &path, &mut actual)?;
    }

    let mut expected = BTreeSet::new();
    for entry in entries {
        let root_count = evidence_roots
            .iter()
            .filter(|evidence_root| entry.path.starts_with(evidence_root))
            .count();
        if entry.kind == EvidenceKind::B1DeviceLedger {
            ensure!(
                root_count == 0,
                "the canonical device ledger is governed directly and must remain outside evidence_roots"
            );
        } else {
            ensure!(
                root_count == 1,
                "bound evidence must live under exactly one evidence_root: {}",
                entry.path.display()
            );
            expected.insert(entry.path.clone());
        }
    }

    let unbound: Vec<_> = actual.difference(&expected).collect();
    let missing: Vec<_> = expected.difference(&actual).collect();
    ensure!(
        unbound.is_empty() && missing.is_empty(),
        "evidence inventory mismatch: unbound={unbound:?}, missing={missing:?}"
    );
    Ok(())
}

fn collect_inventory(
    workspace_root: &Path,
    directory: &Path,
    inventory: &mut BTreeSet<PathBuf>,
) -> anyhow::Result<()> {
    let mut children = fs::read_dir(directory)
        .with_context(|| format!("failed to inspect evidence root {}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()?;
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        let path = child.path();
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect evidence path {}", path.display()))?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "evidence inventory contains a symlink: {}",
            path.display()
        );
        if metadata.is_dir() {
            collect_inventory(workspace_root, &path, inventory)?;
        } else if metadata.is_file() {
            inventory.insert(
                path.strip_prefix(workspace_root)
                    .context("evidence inventory escaped the workspace")?
                    .to_path_buf(),
            );
        } else {
            bail!(
                "evidence inventory contains a non-file entry: {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn reject_symlink_components(root: &Path, relative: &Path) -> anyhow::Result<()> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            bail!(
                "evidence path contains a forbidden component: {}",
                relative.display()
            );
        };
        current.push(component);
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            ensure!(
                !metadata.file_type().is_symlink(),
                "evidence path contains a symlink component: {}",
                relative.display()
            );
        }
    }
    Ok(())
}

fn validate_entry_semantics(
    source_root: &Path,
    evidence_root: &Path,
    entry: &EvidenceEntry,
    source: &SourceBinding,
    ci: &CiBinding,
) -> anyhow::Result<()> {
    match entry.kind {
        EvidenceKind::B1DeviceLedger => {
            ensure!(
                entry.path == Path::new("benchmarks/b1/device-hypotheses.json"),
                "the B1 device ledger kind must bind the canonical ledger path"
            );
            run_with_evidence_root(
                source_root,
                evidence_root,
                "node",
                &["benchmarks/b1/validate-evidence.mjs", "--static"],
            )
        }
        EvidenceKind::B1ContractVerification => {
            ensure!(
                entry.path == Path::new("target/fasti-receipts/b1-contract-verification.json"),
                "the B1 contract receipt kind must bind the canonical receipt path"
            );
            let value = read_json(evidence_root.join(&entry.path))?;
            ensure!(
                value.get("receipt_version").and_then(Value::as_str) == Some("2.0.0")
                    && value.get("kind").and_then(Value::as_str)
                        == Some("fasti.b1.contract-verification"),
                "contract receipt version or kind is invalid"
            );
            ensure!(
                value.get("kind").and_then(Value::as_str) == Some("fasti.b1.contract-verification"),
                "contract receipt kind is invalid"
            );
            ensure!(
                value.pointer("/source/git_commit").and_then(Value::as_str)
                    == Some(source.git_commit.as_str())
                    && value.pointer("/source/git_tree").and_then(Value::as_str)
                        == Some(source.git_tree.as_str())
                    && value.pointer("/source/dirty").and_then(Value::as_bool) == Some(false),
                "contract receipt is stale or was produced from dirty source"
            );
            let expected = verify::contract_gate_inventory(true)?;
            validate_gate_records(&value, true, &expected)?;
            validate_contract_internal_gate_facts(&value)?;
            validate_receipt_ci(&value, ci)?;
            ensure!(
                value
                    .pointer("/dependency_lock_enforcement/passed")
                    .and_then(Value::as_bool)
                    == Some(true)
                    && value
                        .pointer("/dependency_lock_enforcement/offline_passed")
                        .and_then(Value::as_bool)
                        == Some(true),
                "contract receipt did not enforce locked offline dependency resolution"
            );
            Ok(())
        }
        EvidenceKind::B1PerformancePi5 | EvidenceKind::B1PerformanceJ4125 => {
            let receipt_path = evidence_root.join(&entry.path);
            let receipt_argument = receipt_path
                .to_str()
                .context("physical B1 evidence path is not UTF-8")?;
            run_with_evidence_root(
                source_root,
                evidence_root,
                "node",
                &["benchmarks/b1/validate-evidence.mjs", receipt_argument],
            )?;
            let value = read_json(receipt_path)?;
            ensure_receipt_source(&value, source)?;
            ensure!(
                value.pointer("/status").and_then(Value::as_str) == Some("complete"),
                "physical B1 evidence must be a complete receipt"
            );
            let expected = if entry.kind == EvidenceKind::B1PerformancePi5 {
                "raspberry_pi_5_champion"
            } else {
                "j4125_calibrated"
            };
            ensure!(
                value
                    .pointer("/runner/hardware_profile")
                    .and_then(Value::as_str)
                    == Some(expected),
                "physical evidence kind does not match its measured hardware profile"
            );
            ensure!(
                value
                    .pointer("/runner/physicality/status")
                    .and_then(Value::as_str)
                    == Some("physical"),
                "physical B1 evidence does not prove a physical runner"
            );
            Ok(())
        }
        EvidenceKind::B1TauriShell => {
            let receipt_path = evidence_root.join(&entry.path);
            let receipt_argument = receipt_path
                .to_str()
                .context("Tauri evidence path is not UTF-8")?;
            run_with_evidence_root(
                source_root,
                evidence_root,
                "node",
                &[
                    "benchmarks/b1/tauri-shell/validate-evidence.mjs",
                    receipt_argument,
                ],
            )?;
            let value = read_json(receipt_path)?;
            ensure_receipt_source(&value, source)?;
            ensure!(
                value.get("status").and_then(Value::as_str) == Some("complete"),
                "Tauri evidence must be a complete receipt, not a test fixture"
            );
            let fixture_spec = format!("{}:benchmarks/b1/tauri-shell", source.git_commit);
            let fixture_tree = git_output(source_root, &["rev-parse", &fixture_spec])?;
            ensure!(
                value
                    .pointer("/source/fixture_tree")
                    .and_then(Value::as_str)
                    == Some(fixture_tree.as_str()),
                "Tauri receipt fixture tree is stale or substituted"
            );
            verify_json_file_binding(
                evidence_root,
                &value,
                "benchmarks/b1/tauri-shell/src-tauri/Cargo.lock",
                "/source/cargo_lock_sha256",
            )?;
            verify_json_file_binding(
                evidence_root,
                &value,
                "scripts/benchmark-tauri-b1.py",
                "/source/harness_script_sha256",
            )?;
            run_with_evidence_root(
                source_root,
                evidence_root,
                "python3",
                &["-B", "scripts/benchmark-tauri-b1.py", "policy-check"],
            )?;
            Ok(())
        }
        EvidenceKind::RawResult => {
            let value = read_json(evidence_root.join(&entry.path))?;
            let (expected_path, expected_kind, expected_command, expected_gates) =
                match entry.id.as_str() {
                    "b1-portable-gates" => (
                        "target/fasti-receipts/b1-portable.json",
                        "fasti.b1.portable-gates",
                        "cargo xtask test pr",
                        verify::process_gate_inventory(&orchestration::portable_b1_gates())?,
                    ),
                    "b1-deep-gates" => (
                        "target/fasti-receipts/b1-deep.json",
                        "fasti.b1.deep-gates",
                        "cargo xtask test deep",
                        verify::process_gate_inventory(&orchestration::deep_b1_gates())?,
                    ),
                    _ => bail!("unexpected B1 raw-result evidence ID: {}", entry.id),
                };
            ensure!(
                entry.path == Path::new(expected_path)
                    && value.get("receipt_version").and_then(Value::as_str) == Some("1.0.0")
                    && value.get("kind").and_then(Value::as_str) == Some(expected_kind)
                    && value.get("command").and_then(Value::as_str) == Some(expected_command),
                "B1 raw-result receipt path, version, kind, or command is invalid"
            );
            ensure_receipt_source(&value, source)?;
            validate_receipt_ci(&value, ci)?;
            ensure!(
                value.pointer("/source/dirty").and_then(Value::as_bool) == Some(false),
                "B1 raw-result receipt was produced from dirty source"
            );
            validate_gate_records(&value, false, &expected_gates)
        }
        EvidenceKind::B1DeviceQualification
        | EvidenceKind::QaReview
        | EvidenceKind::BuiltArtifact => Ok(()),
    }
}

fn validate_gate_records(
    receipt: &Value,
    allow_in_process: bool,
    expected: &[verify::GateInventoryEntry],
) -> anyhow::Result<()> {
    let gates = receipt
        .get("gates")
        .and_then(Value::as_array)
        .context("gate receipt omits gates")?;
    ensure!(
        !gates.is_empty()
            && receipt.get("gate_count").and_then(Value::as_u64) == Some(gates.len() as u64),
        "gate receipt count is missing or stale"
    );
    ensure!(
        gates.len() == expected.len(),
        "gate receipt inventory count differs from the canonical suite"
    );
    let mut ids = BTreeSet::new();
    for (gate, (expected_id, expected_execution, expected_command)) in gates.iter().zip(expected) {
        let id = gate
            .get("id")
            .and_then(Value::as_str)
            .context("gate record omits id")?;
        ensure!(
            !id.trim().is_empty() && ids.insert(id),
            "gate IDs must be nonempty and unique"
        );
        let execution = gate
            .get("execution")
            .and_then(Value::as_str)
            .context("gate record omits execution")?;
        ensure!(
            execution == "process" || (allow_in_process && execution == "in_process"),
            "gate record has an unsupported execution mode"
        );
        let command = gate
            .get("command")
            .and_then(Value::as_array)
            .context("gate record omits structured command argv")?;
        ensure!(
            !command.is_empty()
                && command
                    .iter()
                    .all(|part| part.as_str().is_some_and(|part| !part.is_empty())),
            "gate record command argv is empty or invalid"
        );
        let actual_command = command
            .iter()
            .map(|part| part.as_str().expect("command strings checked above"))
            .collect::<Vec<_>>();
        ensure!(
            id == expected_id
                && execution == expected_execution
                && actual_command
                    == expected_command.iter().map(String::as_str).collect::<Vec<_>>(),
            "gate receipt inventory, order, execution mode, or exact argv differs from the canonical suite"
        );
        ensure!(
            gate.get("status").and_then(Value::as_str) == Some("pass")
                && gate.get("exit_code").and_then(Value::as_i64) == Some(0)
                && gate
                    .get("tool_version")
                    .and_then(Value::as_str)
                    .is_some_and(|version| !version.trim().is_empty()),
            "gate record is not a passing, version-bound execution"
        );
        for (output_key, digest_key) in [("stdout", "stdout_sha256"), ("stderr", "stderr_sha256")] {
            let output = gate
                .get(output_key)
                .and_then(Value::as_str)
                .with_context(|| format!("gate record omits {output_key}"))?;
            let digest = gate
                .get(digest_key)
                .and_then(Value::as_str)
                .with_context(|| format!("gate record omits {digest_key}"))?;
            ensure_sha256(digest, digest_key)?;
            ensure!(
                sha256_bytes(output.as_bytes()) == digest,
                "gate record {output_key} digest does not recompute"
            );
        }
    }
    Ok(())
}

fn validate_contract_internal_gate_facts(receipt: &Value) -> anyhow::Result<()> {
    let facts = receipt
        .get("contract")
        .context("contract receipt omits contract facts")?;
    let canonical = serde_json_canonicalizer::to_vec(facts)
        .context("failed to canonicalize contract receipt facts")?;
    let facts_sha256 = sha256_bytes(&canonical);
    for gate in receipt
        .get("gates")
        .and_then(Value::as_array)
        .context("contract receipt omits gates")?
    {
        if gate.get("execution").and_then(Value::as_str) != Some("in_process") {
            continue;
        }
        let id = gate
            .get("id")
            .and_then(Value::as_str)
            .context("internal gate omits id")?;
        let expected = format!("PASS [{id}] facts_sha256={facts_sha256}\n");
        ensure!(
            gate.get("stdout").and_then(Value::as_str) == Some(expected.as_str()),
            "internal contract gate is not bound to the receipt facts"
        );
    }
    Ok(())
}

fn validate_ci_binding(ci: &CiBinding) -> anyhow::Result<()> {
    match ci.provider.as_str() {
        "local" => ensure!(
            ci.run == "local-unpublished" && ci.job == "local-milestone",
            "local evidence CI binding is not canonical"
        ),
        "github_actions" => ensure!(
            !ci.run.is_empty()
                && ci.run.bytes().all(|byte| byte.is_ascii_digit())
                && !ci.job.trim().is_empty(),
            "GitHub Actions evidence CI binding is incomplete or invalid"
        ),
        _ => bail!("unsupported evidence CI provider: {}", ci.provider),
    }
    Ok(())
}

fn validate_receipt_ci(receipt: &Value, expected: &CiBinding) -> anyhow::Result<()> {
    ensure!(
        receipt.pointer("/ci/provider").and_then(Value::as_str) == Some(expected.provider.as_str())
            && receipt.pointer("/ci/run").and_then(Value::as_str) == Some(expected.run.as_str())
            && receipt.pointer("/ci/job").and_then(Value::as_str) == Some(expected.job.as_str()),
        "gate receipt CI binding does not match the evidence manifest"
    );
    validate_ci_binding(expected)
}

fn verify_b1_device_qualification(
    evidence_root: &Path,
    entries: &[EvidenceEntry],
) -> anyhow::Result<()> {
    let qualification = entries
        .iter()
        .find(|entry| entry.kind == EvidenceKind::B1DeviceQualification)
        .context("B1 generated device qualification entry is missing")?;
    let pi = entries
        .iter()
        .find(|entry| entry.kind == EvidenceKind::B1PerformancePi5)
        .context("B1 Raspberry Pi 5 receipt entry is missing")?;
    let j4125 = entries
        .iter()
        .find(|entry| entry.kind == EvidenceKind::B1PerformanceJ4125)
        .context("B1 J4125 receipt entry is missing")?;
    let expected = build_device_qualification(
        evidence_root,
        &evidence_root.join(&pi.path),
        &evidence_root.join(&j4125.path),
    )?;
    let actual = read_json(evidence_root.join(&qualification.path))?;
    ensure!(
        actual == expected,
        "generated B1 device qualification does not recompute from the two bound physical receipts"
    );
    Ok(())
}

fn verify_tauri_artifact_binding(root: &Path, entries: &[EvidenceEntry]) -> anyhow::Result<()> {
    let receipt_entry = entries
        .iter()
        .find(|entry| entry.kind == EvidenceKind::B1TauriShell)
        .context("B1 Tauri receipt entry is missing")?;
    let receipt = read_json(root.join(&receipt_entry.path))?;
    let artifact_path = PathBuf::from(
        receipt
            .pointer("/artifact/path")
            .and_then(Value::as_str)
            .context("Tauri receipt artifact path is missing")?,
    );
    let artifact_digest = receipt
        .pointer("/artifact/sha256")
        .and_then(Value::as_str)
        .context("Tauri receipt artifact digest is missing")?;
    let bound = entries
        .iter()
        .filter(|entry| {
            entry.kind == EvidenceKind::BuiltArtifact
                && entry.status == ResultStatus::Pass
                && entry.path == artifact_path
        })
        .collect::<Vec<_>>();
    ensure!(
        bound.len() == 1,
        "Tauri receipt must resolve to exactly one passing BuiltArtifact entry; found {}",
        bound.len()
    );
    ensure!(
        bound[0].sha256 == artifact_digest,
        "Tauri receipt and BuiltArtifact entry digests disagree"
    );
    Ok(())
}

fn verify_physical_artifact_bindings(root: &Path, entries: &[EvidenceEntry]) -> anyhow::Result<()> {
    let receipt_paths = [
        entries
            .iter()
            .find(|entry| entry.kind == EvidenceKind::B1PerformancePi5)
            .context("B1 Raspberry Pi 5 receipt entry is missing")?
            .path
            .clone(),
        entries
            .iter()
            .find(|entry| entry.kind == EvidenceKind::B1PerformanceJ4125)
            .context("B1 J4125 receipt entry is missing")?
            .path
            .clone(),
    ];
    for (path, digest) in physical_retained_artifacts(root, receipt_paths.iter())? {
        let bound = entries
            .iter()
            .filter(|entry| {
                entry.kind == EvidenceKind::BuiltArtifact
                    && entry.status == ResultStatus::Pass
                    && entry.path == path
            })
            .collect::<Vec<_>>();
        ensure!(
            bound.len() == 1,
            "physical retained artifact must resolve to exactly one passing BuiltArtifact entry; found {} for {}",
            bound.len(),
            path.display()
        );
        ensure!(
            bound[0].sha256 == digest,
            "physical receipt and BuiltArtifact entry digests disagree"
        );
    }
    Ok(())
}

fn physical_retained_artifacts<'a>(
    root: &Path,
    receipt_paths: impl IntoIterator<Item = &'a PathBuf>,
) -> anyhow::Result<Vec<(PathBuf, String)>> {
    let mut artifacts = BTreeMap::<PathBuf, String>::new();
    for receipt_path in receipt_paths {
        let receipt = read_json(root.join(receipt_path))?;
        let references = receipt
            .get("retained_artifacts")
            .and_then(Value::as_object)
            .context("physical receipt omits retained_artifacts")?;
        ensure!(
            references.len() == 2,
            "physical receipt must bind exactly the OCI image and contract-pack artifacts"
        );
        for reference in references.values() {
            let relative = PathBuf::from(
                reference
                    .get("path")
                    .and_then(Value::as_str)
                    .context("physical retained artifact path is missing")?,
            );
            validate_relative_path(&relative)?;
            ensure!(
                relative.starts_with("artifacts/sha256"),
                "physical retained artifact is outside its content-addressed package"
            );
            let digest = reference
                .get("sha256")
                .and_then(Value::as_str)
                .context("physical retained artifact digest is missing")?
                .to_owned();
            ensure_sha256(&digest, "physical retained artifact sha256")?;
            let workspace_path = PathBuf::from("benchmarks/b1/evidence").join(relative);
            if let Some(previous) = artifacts.insert(workspace_path.clone(), digest.clone()) {
                ensure!(
                    previous == digest,
                    "physical receipts disagree about retained artifact {}",
                    workspace_path.display()
                );
            }
        }
    }
    Ok(artifacts.into_iter().collect())
}

fn validate_qa_receipt(
    root: &Path,
    entries: &[EvidenceEntry],
    source: &SourceBinding,
) -> anyhow::Result<QaReceipt> {
    let entry = entries
        .iter()
        .find(|entry| entry.kind == EvidenceKind::QaReview)
        .context("B1 QA receipt entry is missing")?;
    let bytes = fs::read(root.join(&entry.path))
        .with_context(|| format!("failed to read QA receipt {}", entry.path.display()))?;
    let receipt: QaReceipt = serde_json::from_slice(&bytes)
        .context("B1 QA receipt does not match the strict machine-readable shape")?;
    ensure!(
        receipt.schema_version == "fasti.qa-review.v1"
            && receipt.kind == "fasti.qa-review"
            && receipt.body == Body::B1,
        "B1 QA receipt schema, kind, or body is invalid"
    );
    ensure!(
        receipt.status == ResultStatus::Pass,
        "mandatory B1 QA did not pass"
    );
    ensure!(
        receipt.reviewed_commit == source.git_commit && receipt.reviewed_tree == source.git_tree,
        "B1 QA receipt is stale for the reviewed source"
    );
    ensure!(
        receipt.review_command == "/qa",
        "B1 QA receipt must bind /qa"
    );
    ensure!(
        receipt.open_findings == 0,
        "B1 QA receipt has open findings"
    );
    ensure!(
        !receipt.rendered_ui_or_ux_changed
            && receipt.design_review.status == DesignReviewStatus::NotApplicable
            && !receipt.design_review.reason.trim().is_empty(),
        "headless B1 QA must record design review N/A with a reason"
    );
    Ok(receipt)
}

fn evidence_entry(
    root: &Path,
    id: &str,
    kind: EvidenceKind,
    path: PathBuf,
) -> anyhow::Result<EvidenceEntry> {
    let bytes = fs::read(root.join(&path))
        .with_context(|| format!("required B1 evidence is missing: {}", path.display()))?;
    Ok(EvidenceEntry {
        id: id.to_owned(),
        kind,
        path,
        sha256: sha256_bytes(&bytes),
        status: ResultStatus::Pass,
    })
}

fn physical_evidence_paths(root: &Path) -> anyhow::Result<(PathBuf, PathBuf)> {
    let relative = PathBuf::from("benchmarks/b1/evidence");
    let directory = root.join(&relative);
    ensure!(
        directory.is_dir(),
        "required physical B1 evidence directory is missing: {}",
        relative.display()
    );
    let mut by_profile = BTreeMap::new();
    for entry in fs::read_dir(&directory)
        .with_context(|| format!("failed to inspect {}", relative.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_file()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        let value = read_json(entry.path())?;
        let profile = value
            .pointer("/runner/hardware_profile")
            .and_then(Value::as_str)
            .context("physical B1 receipt omits runner.hardware_profile")?;
        ensure!(
            matches!(profile, "raspberry_pi_5_champion" | "j4125_calibrated"),
            "physical evidence directory contains an unexpected profile: {profile}"
        );
        let path = relative.join(entry.file_name());
        ensure!(
            by_profile.insert(profile.to_owned(), path).is_none(),
            "physical evidence directory contains more than one {profile} receipt"
        );
    }
    let pi = by_profile
        .remove("raspberry_pi_5_champion")
        .context("B1 Raspberry Pi 5 physical receipt is missing")?;
    let j4125 = by_profile
        .remove("j4125_calibrated")
        .context("B1 J4125 physical receipt is missing")?;
    ensure!(
        by_profile.is_empty(),
        "physical evidence directory contains an unexpected receipt"
    );
    Ok((pi, j4125))
}

fn build_device_qualification(
    evidence_root: &Path,
    pi_path: &Path,
    j4125_path: &Path,
) -> anyhow::Result<Value> {
    let script = evidence_root.join("benchmarks/b1/validate-evidence.mjs");
    let script = script
        .to_str()
        .context("B1 qualification script path is not UTF-8")?;
    let pi_path = pi_path
        .to_str()
        .context("Raspberry Pi evidence path is not UTF-8")?;
    let j4125_path = j4125_path
        .to_str()
        .context("J4125 evidence path is not UTF-8")?;
    let output = Command::new("node")
        .args([script, "--build-qualification", pi_path, j4125_path])
        .current_dir(evidence_root)
        .env("FASTI_EVIDENCE_WORKSPACE_ROOT", evidence_root)
        .output()
        .context("failed to start the canonical B1 device qualification builder")?;
    ensure!(
        output.status.success(),
        "B1 device qualification could not be derived: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    serde_json::from_slice(&output.stdout)
        .context("canonical B1 device qualification builder emitted invalid JSON")
}

fn write_device_qualification(
    root: &Path,
    output_path: &Path,
    pi_path: &Path,
    j4125_path: &Path,
) -> anyhow::Result<()> {
    let qualification = build_device_qualification(root, pi_path, j4125_path)?;
    remove_if_present(&root.join(output_path))?;
    write_json_atomic(&root.join(output_path), &qualification)
}

fn exactly_one_json(root: &Path, relative: &Path, label: &str) -> anyhow::Result<PathBuf> {
    let directory = root.join(relative);
    ensure!(
        directory.is_dir(),
        "required {label} evidence directory is missing: {}",
        relative.display()
    );
    let mut receipts = fs::read_dir(&directory)
        .with_context(|| format!("failed to inspect {}", relative.display()))?
        .collect::<Result<Vec<_>, _>>()?;
    receipts.retain(|entry| {
        entry.file_type().is_ok_and(|kind| kind.is_file())
            && entry.path().extension().and_then(|value| value.to_str()) == Some("json")
    });
    ensure!(
        receipts.len() == 1,
        "B1 requires exactly one {label} JSON receipt in {}; found {}",
        relative.display(),
        receipts.len()
    );
    Ok(relative.join(receipts.remove(0).file_name()))
}

fn current_source_binding(root: &Path) -> anyhow::Result<SourceBinding> {
    Ok(SourceBinding {
        git_commit: git_output(root, &["rev-parse", "--verify", "HEAD"])?,
        git_tree: git_output(root, &["rev-parse", "HEAD^{tree}"])?,
        tree_state: "clean".to_owned(),
    })
}

fn current_ci_binding() -> anyhow::Result<CiBinding> {
    if env::var_os("GITHUB_ACTIONS").is_some() {
        let run =
            env::var("GITHUB_RUN_ID").context("GitHub Actions evidence omits GITHUB_RUN_ID")?;
        let job = env::var("GITHUB_JOB").context("GitHub Actions evidence omits GITHUB_JOB")?;
        ensure!(
            !run.is_empty() && run.bytes().all(|byte| byte.is_ascii_digit()),
            "GitHub Actions evidence has an invalid GITHUB_RUN_ID"
        );
        ensure!(
            !job.trim().is_empty(),
            "GitHub Actions evidence has an empty GITHUB_JOB"
        );
        Ok(CiBinding {
            provider: "github_actions".to_owned(),
            run,
            job,
        })
    } else {
        Ok(CiBinding {
            provider: "local".to_owned(),
            run: "local-unpublished".to_owned(),
            job: "local-milestone".to_owned(),
        })
    }
}

fn current_runner_binding(root: &Path) -> anyhow::Result<RunnerBinding> {
    let runner_id = match env::var("FASTI_EVIDENCE_RUNNER_ID").or_else(|_| env::var("RUNNER_NAME"))
    {
        Ok(value) => value,
        Err(_) => git_output(root, &["config", "user.name"])?,
    };
    ensure!(!runner_id.trim().is_empty(), "evidence runner ID is empty");
    Ok(RunnerBinding {
        runner_id,
        platform: env::consts::OS.to_owned(),
        architecture: env::consts::ARCH.to_owned(),
        tool_versions: vec![
            command_output(root, "cargo", &["--version"])?,
            command_output(root, "rustc", &["--version"])?,
            command_output(root, "node", &["--version"])?,
            command_output(root, "python3", &["--version"])?,
        ],
    })
}

fn command_output(root: &Path, program: &str, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .with_context(|| format!("failed to start {program} for evidence runner binding"))?;
    ensure!(
        output.status.success(),
        "{program} failed while binding tool versions"
    );
    let stdout = String::from_utf8(output.stdout).context("tool version output was not UTF-8")?;
    let stderr =
        String::from_utf8(output.stderr).context("tool version error output was not UTF-8")?;
    let rendered = if stdout.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };
    ensure!(!rendered.is_empty(), "{program} emitted an empty version");
    Ok(rendered.to_owned())
}

fn write_incomplete_candidate(
    root: &Path,
    path: &Path,
    error: &anyhow::Error,
) -> anyhow::Result<()> {
    let source = current_source_binding(root).ok();
    let candidate = serde_json::json!({
        "schema_version": "fasti.b1.milestone-candidate.v1",
        "kind": "fasti.b1.milestone-candidate",
        "status": "incomplete",
        "source": source.map(|value| serde_json::json!({
            "git_commit": value.git_commit,
            "git_tree": value.git_tree,
        })),
        "blocking_reason": format!("{error:#}"),
        "declaration": "This candidate is diagnostic only. It is not a passing evidence manifest and cannot satisfy the B1 milestone gate."
    });
    write_json_atomic(path, &candidate)
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .context("evidence output path has no parent")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create evidence directory {}", parent.display()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).with_context(|| {
        format!(
            "failed to create temporary evidence in {}",
            parent.display()
        )
    })?;
    {
        let mut writer = BufWriter::new(temporary.as_file_mut());
        serde_json::to_writer_pretty(&mut writer, value)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to atomically write {}", path.display()))?;
    Ok(())
}

fn remove_if_present(path: &Path) -> anyhow::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("failed to remove stale {}", path.display()))
        }
    }
}

fn ensure_receipt_source(value: &Value, source: &SourceBinding) -> anyhow::Result<()> {
    ensure!(
        value.pointer("/source/git_commit").and_then(Value::as_str)
            == Some(source.git_commit.as_str()),
        "evidence receipt commit does not match the manifest"
    );
    ensure!(
        value.pointer("/source/git_tree").and_then(Value::as_str) == Some(source.git_tree.as_str()),
        "evidence receipt tree does not match the manifest"
    );
    Ok(())
}

fn verify_json_file_binding(
    root: &Path,
    receipt: &Value,
    relative: &str,
    pointer: &str,
) -> anyhow::Result<()> {
    let bytes = fs::read(root.join(relative))
        .with_context(|| format!("failed to read Tauri provenance input {relative}"))?;
    ensure!(
        receipt.pointer(pointer).and_then(Value::as_str) == Some(sha256_bytes(&bytes).as_str()),
        "Tauri provenance digest does not recompute for {relative}"
    );
    Ok(())
}

fn read_json(path: PathBuf) -> anyhow::Result<Value> {
    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("{} is not valid JSON", path.display()))
}

fn run_with_evidence_root(
    source_root: &Path,
    evidence_root: &Path,
    program: &str,
    args: &[&str],
) -> anyhow::Result<()> {
    run_with_optional_evidence_root(source_root, Some(evidence_root), program, args)
}

fn run_with_optional_evidence_root(
    root: &Path,
    evidence_root: Option<&Path>,
    program: &str,
    args: &[&str],
) -> anyhow::Result<()> {
    let rendered = std::iter::once(program)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");
    println!("RUN [evidence.semantic]: {rendered}");
    let mut command = Command::new(program);
    let current_directory = evidence_root.unwrap_or(root);
    command
        .args(args)
        .current_dir(current_directory)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(evidence_root) = evidence_root {
        command.env("FASTI_EVIDENCE_WORKSPACE_ROOT", evidence_root);
    }
    let status = command
        .status()
        .with_context(|| format!("failed to start evidence validator `{rendered}`"))?;
    ensure!(status.success(), "evidence validator failed: {rendered}");
    Ok(())
}

fn validate_relative_path(path: &Path) -> anyhow::Result<()> {
    ensure!(
        !path.as_os_str().is_empty(),
        "evidence path must not be empty"
    );
    ensure!(
        !path.is_absolute(),
        "evidence path must be workspace-relative"
    );
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            bail!(
                "evidence path contains a forbidden component: {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn ensure_sha256(value: &str, label: &str) -> anyhow::Result<()> {
    ensure_hex(value, 64, label)
}

fn ensure_hex(value: &str, length: usize, label: &str) -> anyhow::Result<()> {
    ensure!(
        value.len() == length
            && value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
        "{label} must be exactly {length} lowercase hexadecimal characters"
    );
    Ok(())
}

fn schema_digest() -> anyhow::Result<String> {
    let schema = schema_for!(EvidenceEnvelope);
    let canonical = serde_json_canonicalizer::to_vec(&schema)
        .context("failed to canonicalize the evidence schema")?;
    Ok(sha256_bytes(&canonical))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn git_output(root: &Path, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .context("failed to start git while verifying evidence source bindings")?;
    ensure!(
        output.status.success(),
        "git failed while verifying evidence: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(String::from_utf8(output.stdout)
        .context("git emitted non-UTF-8 output")?
        .trim()
        .to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(entry: EvidenceEntry) -> EvidenceEnvelope {
        let mut envelope = EvidenceEnvelope {
            manifest: EvidenceManifest {
                schema: SchemaBinding {
                    id: SCHEMA_ID.to_owned(),
                    sha256: schema_digest().expect("schema digest"),
                },
                body: Body::B1,
                source: SourceBinding {
                    git_commit: "a".repeat(40),
                    git_tree: "b".repeat(40),
                    tree_state: "clean".to_owned(),
                },
                ci: CiBinding {
                    provider: "local".to_owned(),
                    run: "local-unpublished".to_owned(),
                    job: "local-milestone".to_owned(),
                },
                command: "cargo xtask test milestone --body B1".to_owned(),
                runner: RunnerBinding {
                    runner_id: "test".to_owned(),
                    platform: "test".to_owned(),
                    architecture: "test".to_owned(),
                    tool_versions: vec!["cargo 1.97.1".to_owned()],
                },
                environment: EnvironmentBinding {
                    declaration: "unit test".to_owned(),
                    network: "denied".to_owned(),
                },
                corpus: CorpusBinding {
                    seed: "B1-empty-process".to_owned(),
                    sha256: "c".repeat(64),
                },
                qa: ReviewBinding {
                    status: ResultStatus::Pass,
                    evidence_id: entry.id.clone(),
                },
                design_review: DesignReviewBinding {
                    status: DesignReviewStatus::NotApplicable,
                    reason: "headless non-product body".to_owned(),
                },
                evidence_roots: vec![PathBuf::from("target")],
                evidence: vec![entry],
                summary: Summary {
                    status: ResultStatus::Pass,
                    pass: 1,
                    fail: 0,
                    unsupported: 0,
                    bound_files: 1,
                },
            },
            manifest_sha256: String::new(),
        };
        envelope.manifest_sha256 = sha256_bytes(
            &serde_json_canonicalizer::to_vec(&envelope.manifest).expect("canonical manifest"),
        );
        envelope
    }

    fn entry(id: &str, path: &str) -> EvidenceEntry {
        EvidenceEntry {
            id: id.to_owned(),
            kind: EvidenceKind::QaReview,
            path: PathBuf::from(path),
            sha256: "d".repeat(64),
            status: ResultStatus::Pass,
        }
    }

    fn gate_receipt() -> (Value, Vec<verify::GateInventoryEntry>) {
        let stdout = "verified\n";
        let stderr = "";
        (
            serde_json::json!({
                "gate_count": 1,
                "gates": [{
                    "id": "gate.one",
                    "execution": "process",
                    "command": ["tool", "check"],
                    "status": "pass",
                    "exit_code": 0,
                    "stdout_sha256": sha256_bytes(stdout.as_bytes()),
                    "stderr_sha256": sha256_bytes(stderr.as_bytes()),
                    "tool_version": "tool 1.0.0",
                    "stdout": stdout,
                    "stderr": stderr,
                }],
            }),
            vec![(
                "gate.one".to_owned(),
                "process".to_owned(),
                vec!["tool".to_owned(), "check".to_owned()],
            )],
        )
    }

    #[test]
    fn gate_receipts_recompute_outputs_and_exact_inventory() {
        let (receipt, expected) = gate_receipt();
        validate_gate_records(&receipt, false, &expected).expect("valid gate receipt");

        let mut output_mutation = receipt.clone();
        output_mutation["gates"][0]["stdout"] = Value::String("substituted\n".to_owned());
        let error = validate_gate_records(&output_mutation, false, &expected)
            .expect_err("output mutation fails");
        assert!(error.to_string().contains("digest does not recompute"));

        let mut command_mutation = receipt;
        command_mutation["gates"][0]["command"][1] = Value::String("publish".to_owned());
        let error = validate_gate_records(&command_mutation, false, &expected)
            .expect_err("command mutation fails");
        assert!(error.to_string().contains("exact argv"));
    }

    #[test]
    fn receipt_ci_must_equal_the_manifest_binding() {
        let expected = CiBinding {
            provider: "local".to_owned(),
            run: "local-unpublished".to_owned(),
            job: "local-milestone".to_owned(),
        };
        let receipt = serde_json::json!({
            "ci": {
                "provider": "github_actions",
                "run": "12345",
                "job": "b1-deep",
            }
        });
        let error =
            validate_receipt_ci(&receipt, &expected).expect_err("substituted receipt CI must fail");
        assert!(error.to_string().contains("does not match"));
    }

    #[test]
    fn strict_shape_recomputes_summary_and_schema_binding() {
        let mut envelope = manifest(entry("qa", "target/qa.json"));
        validate_manifest_shape(&envelope).expect("valid manifest shape");
        envelope.manifest.summary.pass = 0;
        let error = validate_manifest_shape(&envelope).expect_err("invented summary fails");
        assert!(error.to_string().contains("summary.pass"));

        let mut envelope = manifest(entry("qa", "target/qa.json"));
        envelope.manifest.schema.sha256 = "0".repeat(64);
        let error = validate_manifest_shape(&envelope).expect_err("substituted schema fails");
        assert!(error.to_string().contains("schema digest"));
    }

    #[test]
    fn evidence_inventory_rejects_duplicates_reordering_and_traversal() {
        let mut envelope = manifest(entry("b", "target/b.json"));
        envelope.manifest.evidence.push(entry("a", "target/a.json"));
        envelope.manifest.summary.pass = 2;
        envelope.manifest.summary.bound_files = 2;
        let error = validate_manifest_shape(&envelope).expect_err("reordered evidence fails");
        assert!(error.to_string().contains("uniquely sorted"));

        let envelope = manifest(entry("qa", "../outside.json"));
        let error = validate_manifest_shape(&envelope).expect_err("traversal fails");
        assert!(error.to_string().contains("forbidden component"));

        let mut envelope = manifest(entry("qa", "target/qa.json"));
        envelope
            .manifest
            .evidence
            .push(entry("qa-copy", "target/qa.json"));
        envelope.manifest.summary.pass = 2;
        envelope.manifest.summary.bound_files = 2;
        let error = validate_manifest_shape(&envelope).expect_err("duplicate path fails");
        assert!(error.to_string().contains("duplicate evidence paths"));
    }

    #[test]
    fn inventory_rejects_unbound_files_and_digest_mutation() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let directory = root.path().join("evidence");
        fs::create_dir(&directory).expect("create evidence directory");
        fs::write(directory.join("bound.json"), b"{}\n").expect("write bound file");
        fs::write(directory.join("extra.json"), b"{}\n").expect("write unbound file");
        let entry = EvidenceEntry {
            id: "bound".to_owned(),
            kind: EvidenceKind::RawResult,
            path: PathBuf::from("evidence/bound.json"),
            sha256: sha256_bytes(b"{}\n"),
            status: ResultStatus::Pass,
        };
        verify_entry_files(root.path(), std::slice::from_ref(&entry))
            .expect("bound digest is valid");
        let error = verify_evidence_inventory(
            root.path(),
            &[PathBuf::from("evidence")],
            std::slice::from_ref(&entry),
        )
        .expect_err("unbound file fails");
        assert!(error.to_string().contains("unbound"));

        let mut mutated = entry;
        mutated.sha256 = "0".repeat(64);
        let error = verify_entry_files(root.path(), &[mutated]).expect_err("digest mutation fails");
        assert!(error.to_string().contains("digest mismatch"));
    }

    #[test]
    fn physical_receipts_deduplicate_content_addressed_artifacts() {
        let root = tempfile::tempdir().expect("temporary workspace");
        fs::create_dir_all(root.path().join("benchmarks/b1/evidence"))
            .expect("create evidence directory");
        let shared = "1".repeat(64);
        let pi_only = "2".repeat(64);
        let j_only = "3".repeat(64);
        let pi = PathBuf::from("benchmarks/b1/evidence/pi.json");
        let j4125 = PathBuf::from("benchmarks/b1/evidence/j4125.json");
        for (path, first, second) in [(&pi, &shared, &pi_only), (&j4125, &shared, &j_only)] {
            fs::write(
                root.path().join(path),
                serde_json::to_vec(&serde_json::json!({
                    "retained_artifacts": {
                        "oci_image_compressed": {
                            "path": format!("artifacts/sha256/{first}.tar.gz"),
                            "sha256": first,
                            "size_bytes": 1
                        },
                        "contract_pack_compressed": {
                            "path": format!("artifacts/sha256/{second}.tar.gz"),
                            "sha256": second,
                            "size_bytes": 1
                        }
                    }
                }))
                .expect("serialize receipt"),
            )
            .expect("write receipt");
        }
        let artifacts = physical_retained_artifacts(root.path(), [&pi, &j4125])
            .expect("collect retained artifacts");
        assert_eq!(artifacts.len(), 3);
        assert!(artifacts.iter().any(|(_, digest)| digest == &shared));
    }

    #[test]
    fn tauri_receipt_requires_a_matching_built_artifact_entry() {
        let root = tempfile::tempdir().expect("temporary workspace");
        fs::create_dir(root.path().join("evidence")).expect("create evidence directory");
        let digest = "4".repeat(64);
        fs::write(
            root.path().join("evidence/tauri.json"),
            serde_json::to_vec(&serde_json::json!({
                "artifact": {
                    "path": "evidence/tauri.bin",
                    "sha256": digest,
                    "size_bytes": 1
                }
            }))
            .expect("serialize receipt"),
        )
        .expect("write receipt");
        let receipt = EvidenceEntry {
            id: "tauri".to_owned(),
            kind: EvidenceKind::B1TauriShell,
            path: PathBuf::from("evidence/tauri.json"),
            sha256: "5".repeat(64),
            status: ResultStatus::Pass,
        };
        let mut artifact = EvidenceEntry {
            id: "tauri-artifact".to_owned(),
            kind: EvidenceKind::BuiltArtifact,
            path: PathBuf::from("evidence/tauri.bin"),
            sha256: digest,
            status: ResultStatus::Pass,
        };
        verify_tauri_artifact_binding(root.path(), &[receipt.clone(), artifact.clone()])
            .expect("matching artifact binding");
        artifact.sha256 = "6".repeat(64);
        let error = verify_tauri_artifact_binding(root.path(), &[receipt, artifact])
            .expect_err("digest mismatch must fail");
        assert!(error.to_string().contains("digests disagree"));
    }

    #[cfg(unix)]
    #[test]
    fn inventory_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("temporary workspace");
        let outside = tempfile::tempdir().expect("outside directory");
        fs::write(outside.path().join("receipt.json"), b"{}\n").expect("outside receipt");
        fs::create_dir(root.path().join("evidence")).expect("evidence directory");
        symlink(
            outside.path().join("receipt.json"),
            root.path().join("evidence/receipt.json"),
        )
        .expect("create symlink");
        let entry = EvidenceEntry {
            id: "receipt".to_owned(),
            kind: EvidenceKind::RawResult,
            path: PathBuf::from("evidence/receipt.json"),
            sha256: sha256_bytes(b"{}\n"),
            status: ResultStatus::Pass,
        };
        let error = verify_entry_files(root.path(), &[entry]).expect_err("symlink fails");
        assert!(error.to_string().contains("symlink"));
    }

    #[test]
    fn source_binding_rejects_stale_and_dirty_git_state() {
        let root = tempfile::tempdir().expect("temporary workspace");
        initialize_git(root.path());
        let source = SourceBinding {
            git_commit: git_output(root.path(), &["rev-parse", "HEAD"]).expect("commit"),
            git_tree: git_output(root.path(), &["rev-parse", "HEAD^{tree}"]).expect("tree"),
            tree_state: "clean".to_owned(),
        };
        verify_source_binding(root.path(), &source).expect("current clean source");

        let stale = SourceBinding {
            git_commit: "0".repeat(40),
            git_tree: source.git_tree.clone(),
            tree_state: "clean".to_owned(),
        };
        let error = verify_source_binding(root.path(), &stale).expect_err("stale source fails");
        assert!(error.to_string().contains("stale"));

        fs::write(root.path().join("tracked.txt"), "dirty\n").expect("dirty tracked file");
        let error = verify_source_binding(root.path(), &source).expect_err("dirty source fails");
        assert!(error.to_string().contains("dirty"));
    }

    #[test]
    fn tracked_snapshot_bytes_come_from_the_bound_commit_not_the_live_checkout() {
        let root = tempfile::tempdir().expect("temporary workspace");
        initialize_git(root.path());
        let commit = git_output(root.path(), &["rev-parse", "HEAD"]).expect("commit");
        fs::write(root.path().join("tracked.txt"), "substituted\n").expect("mutate live file");

        let bytes = read_tracked_file_at(root.path(), &commit, Path::new("tracked.txt"))
            .expect("read bound Git blob")
            .expect("tracked file exists at commit");
        assert_eq!(bytes, b"one\n");
    }

    #[test]
    fn missing_evidence_emits_only_an_incomplete_candidate() {
        let root = tempfile::tempdir().expect("temporary workspace");
        initialize_git(root.path());
        let manifest_path = root.path().join("target/fasti-evidence/b1-manifest.json");
        let error = create_b1_milestone_manifest(root.path(), &manifest_path)
            .expect_err("missing evidence blocks generation");
        assert!(error.to_string().contains("manifest was not emitted"));
        assert!(!manifest_path.exists());
        let candidate_path = root
            .path()
            .join("target/fasti-evidence/b1-incomplete-candidate.json");
        let candidate = read_json(candidate_path).expect("read incomplete candidate");
        assert_eq!(
            candidate.get("status").and_then(Value::as_str),
            Some("incomplete")
        );
        assert!(candidate
            .get("declaration")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("cannot satisfy")));
    }

    #[test]
    fn milestone_output_cannot_escape_its_target_directory() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let outside = root.path().join("../outside.json");
        let error = safe_manifest_output_path(root.path(), &outside)
            .expect_err("outside output must be rejected");
        assert!(error.to_string().contains("forbidden component"));

        let source_output = root.path().join("manifest.json");
        let error = safe_manifest_output_path(root.path(), &source_output)
            .expect_err("source-tree output must be rejected");
        assert!(error.to_string().contains("below target/fasti-evidence"));

        let allowed = root.path().join("target/fasti-evidence/custom-b1.json");
        assert_eq!(
            safe_manifest_output_path(root.path(), &allowed).expect("safe target output"),
            allowed
        );
    }

    #[test]
    fn public_verify_rejects_a_partial_b1_envelope() {
        let root = tempfile::tempdir().expect("temporary workspace");
        initialize_git(root.path());
        fs::create_dir(root.path().join("evidence")).expect("create evidence root");
        fs::write(root.path().join("evidence/artifact.bin"), b"artifact").expect("write artifact");
        let status = Command::new("git")
            .args(["add", "evidence/artifact.bin"])
            .current_dir(root.path())
            .status()
            .expect("stage artifact");
        assert!(status.success());
        let status = Command::new("git")
            .args(["commit", "--quiet", "-m", "bind artifact"])
            .current_dir(root.path())
            .status()
            .expect("commit artifact");
        assert!(status.success());

        let mut envelope = manifest(EvidenceEntry {
            id: "only-artifact".to_owned(),
            kind: EvidenceKind::BuiltArtifact,
            path: PathBuf::from("evidence/artifact.bin"),
            sha256: sha256_bytes(b"artifact"),
            status: ResultStatus::Pass,
        });
        envelope.manifest.source = current_source_binding(root.path()).expect("source binding");
        envelope.manifest.evidence_roots = vec![PathBuf::from("evidence")];
        envelope.manifest.qa.evidence_id = "only-artifact".to_owned();
        envelope.manifest_sha256 = sha256_bytes(
            &serde_json_canonicalizer::to_vec(&envelope.manifest).expect("canonical manifest"),
        );
        let manifest_path = root.path().join(".git/partial-b1.json");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&envelope).expect("serialize envelope"),
        )
        .expect("write envelope");

        let error = verify(root.path(), &manifest_path).expect_err("partial B1 must fail");
        assert!(error.to_string().contains("requires exactly one passing"));
    }

    fn initialize_git(root: &Path) {
        for args in [
            vec!["init", "--quiet"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["config", "user.name", "Fasti Test"],
        ] {
            let status = Command::new("git")
                .args(args)
                .current_dir(root)
                .status()
                .expect("start git");
            assert!(status.success());
        }
        fs::write(root.join("tracked.txt"), "one\n").expect("write tracked file");
        for args in [
            vec!["add", "tracked.txt"],
            vec!["commit", "--quiet", "-m", "test"],
        ] {
            let status = Command::new("git")
                .args(args)
                .current_dir(root)
                .status()
                .expect("start git");
            assert!(status.success());
        }
    }

    #[test]
    fn b1_milestone_requires_generated_qualification_inputs_not_a_tracked_assignment() {
        let root = tempfile::tempdir().expect("temporary workspace");
        fs::create_dir_all(root.path().join("benchmarks/b1/evidence"))
            .expect("create physical evidence directory");
        let error = physical_evidence_paths(root.path())
            .expect_err("missing physical receipts block generated qualification");
        assert!(error
            .to_string()
            .contains("Raspberry Pi 5 physical receipt"));
    }

    #[test]
    fn canonical_manifest_digest_changes_when_bound_claim_changes() {
        let mut envelope = manifest(entry("qa", "target/qa.json"));
        let original = envelope.manifest_sha256.clone();
        envelope.manifest.command.push_str(" --mutated");
        let mutated = sha256_bytes(
            &serde_json_canonicalizer::to_vec(&envelope.manifest).expect("canonical manifest"),
        );
        assert_ne!(original, mutated);
        let error = verify_manifest_digest(&envelope).expect_err("stale manifest digest fails");
        assert!(error.to_string().contains("manifest_sha256"));
    }
}
