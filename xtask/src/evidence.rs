use crate::{orchestration, verify};
use anyhow::{bail, ensure, Context};
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
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
    "scripts/bench-daemon-idle.sh",
    "scripts/bench-envelope.sh",
    "scripts/benchmark-b1.py",
    "scripts/benchmark-tauri-b1.py",
    "scripts/lib/strict-json.mjs",
];
const MAX_SAMPLE_LATENESS_NS: u64 = 500_000_000;
const OCI_UNPACKED_SAFETY_CEILING_BYTES: u64 = 400 * 1024 * 1024;
const OCI_ARCHIVE_METADATA_ALLOWANCE_BYTES: u64 = 16 * 1024 * 1024;
const OCI_ARCHIVE_METADATA_FILE_LIMIT_BYTES: u64 = 1024 * 1024;
const OCI_ARCHIVE_ENTRY_LIMIT: u64 = 4096;
const VERIFIER_SOURCE_INPUTS_PATH: &str = ".fasti-verifier/source-inputs.json";

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub(crate) enum Body {
    B0,
    B1,
    B2,
    B3,
    B8a,
    B8b,
}

impl Body {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::B0 => "B0",
            Self::B1 => "B1",
            Self::B2 => "B2",
            Self::B3 => "B3",
            Self::B8a => "B8a",
            Self::B8b => "B8b",
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
    B1ArtifactBudgets,
    B1ContractVerification,
    B1DeviceLedger,
    B1PerformanceEnvelope,
    B1TauriShell,
    /// A B8b release-readiness receipt (checksums, SBOM, provenance,
    /// security review, or release notes) bound to source via
    /// `ensure_receipt_source`.
    B8bReceipt,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PerformanceEnvelopeReceipt {
    schema_version: String,
    kind: String,
    status: ResultStatus,
    source: PerformanceEnvelopeSource,
    ci: PerformanceEnvelopeCi,
    runner: PerformanceEnvelopeRunner,
    envelope: PerformanceEnvelopeLimits,
    measurement: PerformanceEnvelopeMeasurement,
    policy: PerformanceEnvelopePolicy,
    artifact: PerformanceEnvelopeArtifact,
    artifact_budget_receipt: Option<PerformanceEnvelopeArtifactBudgetBinding>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PerformanceEnvelopeSource {
    git_commit: String,
    git_tree: String,
    dirty: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PerformanceEnvelopeCi {
    provider: String,
    repository: String,
    workflow_ref: String,
    workflow_sha: String,
    event: String,
    r#ref: String,
    run: String,
    run_attempt: String,
    job: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PerformanceEnvelopeRunner {
    architecture: String,
    kernel_release: String,
    cgroup_version: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PerformanceEnvelopeLimits {
    memory_max_bytes: u64,
    memory_swap_max_bytes: u64,
    cpu_quota_micros: u64,
    cpu_period_micros: u64,
    memory_swap_peak_bytes: u64,
    oom_event_count: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PerformanceEnvelopeMeasurement {
    profile: String,
    target: String,
    budget_bytes: u64,
    peak_memory_bytes: u64,
    steady_memory_peak_bytes: u64,
    warmup_seconds: u64,
    measurement_seconds: u64,
    sample_interval_ms: u64,
    max_sample_lateness_ns: u64,
    actual_warmup_ns: u64,
    actual_measurement_ns: u64,
    cpu_average_basis_points: u64,
    cpu_p95_basis_points: u64,
    observations: Vec<PerformanceEnvelopeObservation>,
    network_isolation: String,
    command_exit_code: i64,
    command: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PerformanceEnvelopeObservation {
    sequence: u64,
    elapsed_ns: u64,
    interval_ns: u64,
    memory_current_bytes: u64,
    cpu_usage_delta_micros: u64,
    cpu_basis_points: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PerformanceEnvelopePolicy {
    budgets_sha256: String,
    harness_sha256: String,
    workload_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PerformanceEnvelopeArtifact {
    source_path: PathBuf,
    path: PathBuf,
    sha256: String,
    size_bytes: u64,
    build_profile: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PerformanceEnvelopeArtifactBudgetBinding {
    path: PathBuf,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactBudgetReceipt {
    schema_version: String,
    kind: String,
    status: ResultStatus,
    source: ArtifactBudgetSource,
    runner: ArtifactBudgetRunner,
    policy: ArtifactBudgetPolicy,
    oci_image_id: String,
    artifact_sizes: ArtifactBudgetSizes,
    artifact_budget_verdicts: Vec<ArtifactBudgetVerdict>,
    retained_artifacts: BTreeMap<String, ArtifactBudgetRetainedArtifact>,
    commands: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactBudgetSource {
    git_commit: String,
    git_tree: String,
    contract_ref: String,
    build_recipe_sha256: String,
    build_context_archive_sha256: String,
    dirty: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactBudgetRunner {
    architecture: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactBudgetPolicy {
    budgets_sha256: String,
    harness_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactBudgetSizes {
    native_fastid_binary_bytes: u64,
    oci_fastid_binary_bytes: u64,
    oci_fasti_cli_binary_bytes: u64,
    oci_image_bytes: u64,
    native_runtime_installed_bytes: Option<u64>,
    native_archive_compressed_bytes: Option<u64>,
    oci_image_compressed_bytes: u64,
    oci_image_compressed_sha256: String,
    contract_pack_compressed_bytes: u64,
    contract_pack_compressed_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactBudgetVerdict {
    budget: String,
    limit_bytes: u64,
    measured_bytes: Option<u64>,
    status: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactBudgetRetainedArtifact {
    path: PathBuf,
    sha256: String,
    size_bytes: u64,
}

#[derive(Debug)]
struct SavedOciArchiveFile {
    position: u64,
    size: u64,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OciLayout {
    image_layout_version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OciIndex {
    schema_version: u64,
    media_type: Option<String>,
    manifests: Vec<OciDescriptor>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OciDescriptor {
    media_type: String,
    digest: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OciImageManifest {
    schema_version: u64,
    media_type: Option<String>,
    config: OciDescriptor,
    layers: Vec<OciDescriptor>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DockerArchiveManifestEntry {
    config: String,
    layers: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VerifierSourceInputs {
    contract_ref: String,
    build_context_archive_sha256: String,
}

#[derive(Debug)]
struct VerifiedPerformanceEnvelope {
    architecture: String,
    run: String,
    run_attempt: String,
    artifact_path: PathBuf,
    artifact_sha256: String,
    artifact_budget_path: PathBuf,
    artifact_budget_sha256: String,
    artifact_budget_artifacts: Vec<(PathBuf, String)>,
}

type PerformancePackageFile = (String, String, EvidenceKind, PathBuf, String);

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
                write_incomplete_candidate(root, &candidate_path, &error, Body::B1)?;
                Err(error.context(format!(
                    "generated B1 manifest failed immediate verification and was removed; incomplete candidate={}",
                    candidate_path.display()
                )))
            }
        },
        Err(error) => {
            write_incomplete_candidate(root, &candidate_path, &error, Body::B1)?;
            Err(error.context(format!(
                "B1 milestone manifest was not emitted; incomplete candidate={}",
                candidate_path.display()
            )))
        }
    }
}

pub(crate) fn create_b8b_milestone_manifest(
    root: &Path,
    manifest_path: &Path,
) -> anyhow::Result<PathBuf> {
    let manifest_path = safe_manifest_output_path(root, manifest_path)?;
    let candidate_path = root.join("target/fasti-evidence/b8b-incomplete-candidate.json");
    remove_if_present(&manifest_path)?;
    remove_if_present(&candidate_path)?;

    match build_b8b_milestone_manifest(root, &manifest_path) {
        Ok(path) => match verify_b8b_milestone(root, &path) {
            Ok(()) => Ok(path),
            Err(error) => {
                remove_if_present(&manifest_path)?;
                write_incomplete_candidate(root, &candidate_path, &error, Body::B8b)?;
                Err(error.context(format!(
                    "generated B8b manifest failed immediate verification and was removed; incomplete candidate={}",
                    candidate_path.display()
                )))
            }
        },
        Err(error) => {
            write_incomplete_candidate(root, &candidate_path, &error, Body::B8b)?;
            Err(error.context(format!(
                "B8b milestone manifest was not emitted; incomplete candidate={}",
                candidate_path.display()
            )))
        }
    }
}

fn build_b8b_milestone_manifest(root: &Path, manifest_path: &Path) -> anyhow::Result<PathBuf> {
    let source = current_source_binding(root)?;
    verify_source_binding(root, &source)?;

    let evidence_root = PathBuf::from("target/fasti-evidence/b8b");
    let sbom_root = evidence_root.join("sbom");
    let security_review_path = evidence_root.join("security-review/b8b-security-review.json");
    let provenance_path = evidence_root.join("provenance/provenance-statement.json");
    let release_notes_path = evidence_root.join("release-notes.md");
    let qa_path = evidence_root.join("qa/b8b-qa.json");

    for required in [
        &security_review_path,
        &provenance_path,
        &release_notes_path,
        &qa_path,
    ] {
        ensure!(
            root.join(required).is_file(),
            "required B8b evidence is missing: {}",
            required.display()
        );
    }

    let mut entries = vec![
        evidence_entry(
            root,
            "b8b-security-review",
            EvidenceKind::B8bReceipt,
            security_review_path.clone(),
        )?,
        evidence_entry(
            root,
            "b8b-provenance",
            EvidenceKind::B8bReceipt,
            provenance_path,
        )?,
        evidence_entry(
            root,
            "b8b-release-notes",
            EvidenceKind::BuiltArtifact,
            release_notes_path,
        )?,
        evidence_entry(root, "b8b-qa", EvidenceKind::QaReview, qa_path)?,
    ];

    for architecture in ["x86_64", "aarch64"] {
        let path = evidence_root.join(format!("checksums/checksums-{architecture}.sha256"));
        ensure!(
            root.join(&path).is_file(),
            "required B8b checksums manifest is missing: {}",
            path.display()
        );
        entries.push(evidence_entry(
            root,
            &format!("b8b-checksums-{architecture}"),
            EvidenceKind::BuiltArtifact,
            path,
        )?);
    }

    let sbom_files = list_files_with_extension(&root.join(&sbom_root), ".cdx.json")?;
    ensure!(
        !sbom_files.is_empty(),
        "no SBOM files were found under {}",
        sbom_root.display()
    );
    for absolute in sbom_files {
        let relative = absolute
            .strip_prefix(root)
            .context("SBOM file escaped the workspace root")?
            .to_path_buf();
        let file_name = relative
            .file_name()
            .and_then(|value| value.to_str())
            .context("SBOM file name is not UTF-8")?;
        let stem = file_name.trim_end_matches(".cdx.json");
        entries.push(evidence_entry(
            root,
            &format!("b8b-sbom-{stem}"),
            EvidenceKind::BuiltArtifact,
            relative,
        )?);
    }

    entries.sort_by(|left, right| (&left.id, &left.path).cmp(&(&right.id, &right.path)));
    let evidence_roots = vec![evidence_root];

    let snapshot = snapshot_evidence_files(root, &source, &entries)?;
    let ci = current_ci_binding()?;
    verify_evidence_inventory(root, &evidence_roots, &entries)?;
    for entry in &entries {
        validate_entry_semantics(root, snapshot.path(), entry, &source, &ci)?;
    }
    let qa = validate_qa_receipt(
        snapshot.path(),
        &entries,
        &source,
        Body::B8b,
        "/qa",
        DesignReviewStatus::Pass,
    )?;
    verify_source_binding(root, &source)?;

    let security_review_bytes = fs::read(snapshot.path().join(&security_review_path))
        .context("failed to read the snapshotted B8b security-review receipt")?;
    let count = entries.len();
    let manifest = EvidenceManifest {
        schema: SchemaBinding {
            id: SCHEMA_ID.to_owned(),
            sha256: schema_digest()?,
        },
        body: Body::B8b,
        source,
        ci,
        command: "cargo xtask test milestone --body B8b".to_owned(),
        runner: current_runner_binding(root)?,
        environment: EnvironmentBinding {
            declaration: "B8b non-publishing release-readiness evidence: content-digest checksums, SBOM, provenance statement, final security review, and release notes".to_owned(),
            network: "per-receipt isolation; no step in this milestone publishes, signs, or attests anything".to_owned(),
        },
        corpus: CorpusBinding {
            seed: "b8b-release-readiness-v1".to_owned(),
            sha256: sha256_bytes(&security_review_bytes),
        },
        qa: ReviewBinding {
            status: qa.status,
            evidence_id: "b8b-qa".to_owned(),
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
        .context("failed to canonicalize generated B8b milestone evidence")?;
    let envelope = EvidenceEnvelope {
        manifest,
        manifest_sha256: sha256_bytes(&canonical),
    };
    write_json_atomic(manifest_path, &envelope)?;
    println!(
        "PASS: generated canonical B8b milestone manifest {}",
        manifest_path.display()
    );
    Ok(manifest_path.to_path_buf())
}

fn list_files_with_extension(directory: &Path, suffix: &str) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to read directory {}", directory.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().and_then(|value| value.to_str());
        if name.is_some_and(|value| value.ends_with(suffix)) {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

pub(crate) fn verify_b8b_milestone(root: &Path, manifest_path: &Path) -> anyhow::Result<()> {
    let verified = verify(root, manifest_path)?;
    ensure!(
        verified.manifest.body == Body::B8b,
        "manifest does not declare body B8b"
    );
    Ok(())
}

fn verify_b8b_manifest_requirements(
    root: &Path,
    manifest: &EvidenceManifest,
) -> anyhow::Result<()> {
    ensure!(
        manifest.command == "cargo xtask test milestone --body B8b",
        "B8b evidence manifest must bind the exact milestone command"
    );
    ensure!(
        manifest.summary.status == ResultStatus::Pass,
        "B8b evidence manifest contains a failing or unsupported result"
    );
    ensure!(
        manifest.qa.status == ResultStatus::Pass,
        "mandatory B8b QA is not recorded as passing"
    );
    ensure!(
        manifest.design_review.status == DesignReviewStatus::Pass
            && !manifest.design_review.reason.trim().is_empty(),
        "B8b requires a passing design review with a reason, not a headless N/A claim"
    );

    let receipt_count = manifest
        .evidence
        .iter()
        .filter(|entry| {
            entry.kind == EvidenceKind::B8bReceipt && entry.status == ResultStatus::Pass
        })
        .count();
    ensure!(
        receipt_count == 2,
        "B8b milestone requires exactly two passing source-bound receipts (security review, provenance); found {receipt_count}"
    );
    let checksums_count = manifest
        .evidence
        .iter()
        .filter(|entry| {
            entry.kind == EvidenceKind::BuiltArtifact
                && entry.status == ResultStatus::Pass
                && entry.id.starts_with("b8b-checksums-")
        })
        .count();
    ensure!(
        checksums_count == 2,
        "B8b milestone requires exactly two passing per-architecture checksums receipts; found {checksums_count}"
    );
    let sbom_count = manifest
        .evidence
        .iter()
        .filter(|entry| {
            entry.kind == EvidenceKind::BuiltArtifact
                && entry.status == ResultStatus::Pass
                && entry.id.starts_with("b8b-sbom-")
        })
        .count();
    ensure!(
        sbom_count >= 2,
        "B8b milestone requires at least two passing SBOM receipts (Rust and npm); found {sbom_count}"
    );
    let release_notes_count = manifest
        .evidence
        .iter()
        .filter(|entry| entry.id == "b8b-release-notes" && entry.status == ResultStatus::Pass)
        .count();
    ensure!(
        release_notes_count == 1,
        "B8b milestone requires exactly one passing release-notes receipt; found {release_notes_count}"
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
    validate_qa_receipt(
        snapshot.path(),
        &manifest.evidence,
        &manifest.source,
        Body::B8b,
        "/qa",
        DesignReviewStatus::Pass,
    )?;
    let security_review_entry = manifest
        .evidence
        .iter()
        .find(|entry| entry.id == "b8b-security-review")
        .context("b8b-security-review evidence entry is missing")?;
    let security_review_bytes = fs::read(snapshot.path().join(&security_review_entry.path))
        .context("failed to read the snapshotted B8b security-review receipt")?;
    ensure!(
        manifest.corpus.seed == "b8b-release-readiness-v1"
            && manifest.corpus.sha256 == sha256_bytes(&security_review_bytes),
        "B8b corpus binding does not recompute from the security-review receipt"
    );
    verify_source_binding(root, &manifest.source)?;
    println!(
        "PASS: B8b milestone evidence is complete, envelope-enforced, current, and fail-closed"
    );
    Ok(())
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
    let qa_path = PathBuf::from("target/fasti-evidence/qa/b1-qa.json");
    let performance_root = PathBuf::from("target/fasti-evidence/envelope");
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
    let performance_paths = performance_envelope_paths(root)?;
    let performance_artifacts = performance_envelope_artifacts(root, &performance_paths)?;
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
        evidence_entry(root, "b1-deep-gates", EvidenceKind::RawResult, deep_path)?,
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
    for (architecture, path) in &performance_paths {
        entries.push(evidence_entry(
            root,
            &format!("b1-performance-envelope-{architecture}"),
            EvidenceKind::B1PerformanceEnvelope,
            path.clone(),
        )?);
    }
    for (architecture, label, kind, path, expected_digest) in performance_artifacts {
        let artifact_id = format!("b1-performance-envelope-{architecture}-{label}");
        let artifact = evidence_entry(root, &artifact_id, kind, path)?;
        ensure!(
            artifact.sha256 == expected_digest,
            "performance package receipt and retained file digest disagree"
        );
        entries.push(artifact);
    }
    entries.sort_by(|left, right| (&left.id, &left.path).cmp(&(&right.id, &right.path)));
    let mut evidence_roots = vec![contract_root, performance_root, qa_root, tauri_root];
    evidence_roots.sort();

    let snapshot = snapshot_evidence_files(root, &source, &entries)?;
    let ci = current_ci_binding()?;
    verify_evidence_inventory(root, &evidence_roots, &entries)?;
    for entry in &entries {
        validate_entry_semantics(root, snapshot.path(), entry, &source, &ci)?;
    }
    verify_performance_envelope_set(snapshot.path(), &entries, &source)?;
    let qa = validate_qa_receipt(
        snapshot.path(),
        &entries,
        &source,
        Body::B1,
        "/qa",
        DesignReviewStatus::NotApplicable,
    )?;
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
            declaration: "B1 live contract/deep gates plus digest-bound low-hardware envelope and benchmark-only process evidence".to_owned(),
            network: "per-receipt isolation; orchestration itself makes no global network-denied claim".to_owned(),
        },
        corpus: CorpusBinding {
            seed: "b1-low-hardware-envelope-and-contract-v1".to_owned(),
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
        Body::B8b => verify_b8b_manifest_requirements(root, &verified.manifest)?,
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
    let artifact_budget_count = manifest
        .evidence
        .iter()
        .filter(|entry| {
            entry.kind == EvidenceKind::B1ArtifactBudgets && entry.status == ResultStatus::Pass
        })
        .count();
    ensure!(
        artifact_budget_count == 2,
        "B1 milestone requires exactly two passing architecture artifact-budget receipts; found {artifact_budget_count}"
    );
    let performance_count = manifest
        .evidence
        .iter()
        .filter(|entry| {
            entry.kind == EvidenceKind::B1PerformanceEnvelope && entry.status == ResultStatus::Pass
        })
        .count();
    ensure!(
        performance_count == 2,
        "B1 milestone requires exactly two passing low-hardware envelope receipts; found {performance_count}"
    );
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
    validate_qa_receipt(
        snapshot.path(),
        &manifest.evidence,
        &manifest.source,
        Body::B1,
        "/qa",
        DesignReviewStatus::NotApplicable,
    )?;
    verify_performance_envelope_set(snapshot.path(), &manifest.evidence, &manifest.source)?;
    verify_tauri_artifact_binding(snapshot.path(), &manifest.evidence)?;
    let corpus_bytes = fs::read(snapshot.path().join("benchmarks/b1/budgets.json"))
        .context("failed to read the snapshotted B1 budget/corpus seed input")?;
    ensure!(
        manifest.corpus.seed == "b1-low-hardware-envelope-and-contract-v1"
            && manifest.corpus.sha256 == sha256_bytes(&corpus_bytes),
        "B1 corpus binding does not recompute from the governed budget seed"
    );
    verify_source_binding(root, &manifest.source)?;
    println!(
        "PASS: B1 milestone evidence is complete, envelope-enforced, current, and fail-closed"
    );
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
    let contract_object = format!("{}:contracts", source.git_commit);
    let contract_ref = git_output(root, &["rev-parse", &contract_object])?;
    ensure_hex(&contract_ref, 40, "bound contracts tree")?;
    let mut archive = Command::new("git")
        .args(["archive", "--format=tar", &source.git_commit])
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to archive the bound source commit")?;
    let (build_context_archive_sha256, _) = sha256_reader(
        archive
            .stdout
            .as_mut()
            .context("bound source archive stdout is unavailable")?,
        "bound source archive",
    )?;
    let archive = archive
        .wait_with_output()
        .context("failed to wait for the bound source archive")?;
    ensure!(
        archive.status.success(),
        "git archive failed for the bound source commit: {}",
        String::from_utf8_lossy(&archive.stderr).trim()
    );
    let source_inputs = serde_json::to_vec(&VerifierSourceInputs {
        contract_ref,
        build_context_archive_sha256,
    })?;
    write_snapshot_file(
        snapshot.path(),
        Path::new(VERIFIER_SOURCE_INPUTS_PATH),
        &source_inputs,
    )?;
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
        EvidenceKind::B1PerformanceEnvelope => {
            validate_performance_envelope_receipt(evidence_root, &entry.path, source)?;
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
        EvidenceKind::B8bReceipt => {
            let value = read_json(evidence_root.join(&entry.path))?;
            ensure_receipt_source(&value, source)
        }
        EvidenceKind::B1ArtifactBudgets | EvidenceKind::QaReview | EvidenceKind::BuiltArtifact => {
            Ok(())
        }
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

fn validate_performance_envelope_receipt(
    root: &Path,
    receipt_path: &Path,
    source: &SourceBinding,
) -> anyhow::Result<VerifiedPerformanceEnvelope> {
    let bytes = fs::read(root.join(receipt_path)).with_context(|| {
        format!(
            "failed to read performance envelope receipt {}",
            receipt_path.display()
        )
    })?;
    let receipt: PerformanceEnvelopeReceipt = serde_json::from_slice(&bytes)
        .context("performance envelope receipt does not match the strict machine-readable shape")?;
    ensure!(
        receipt.schema_version == "fasti.b1.performance-envelope.v1"
            && receipt.kind == "fasti.b1.performance-envelope"
            && receipt.status == ResultStatus::Pass,
        "performance envelope receipt schema, kind, or status is invalid"
    );
    ensure!(
        receipt.source.git_commit == source.git_commit
            && receipt.source.git_tree == source.git_tree
            && !receipt.source.dirty,
        "performance envelope receipt is stale or was produced from dirty source"
    );
    ensure!(
        receipt.ci.provider == "github_actions"
            && receipt.ci.repository == "Scrobble-dev/Fasti"
            && receipt.ci.workflow_ref
                == "Scrobble-dev/Fasti/.github/workflows/ci.yml@refs/heads/dev"
            && receipt.ci.workflow_sha == source.git_commit
            && receipt.ci.event == "push"
            && receipt.ci.r#ref == "refs/heads/dev"
            && receipt.ci.run.bytes().all(|byte| byte.is_ascii_digit())
            && !receipt.ci.run.is_empty()
            && receipt
                .ci
                .run_attempt
                .parse::<u64>()
                .is_ok_and(|attempt| attempt > 0)
            && receipt.ci.job == "low-hardware-envelope",
        "performance envelope receipt is not from the canonical exact-dev-push CI job"
    );
    ensure!(
        matches!(receipt.runner.architecture.as_str(), "x86_64" | "aarch64")
            && !receipt.runner.kernel_release.trim().is_empty()
            && receipt.runner.cgroup_version == "v2",
        "performance envelope runner architecture, kernel, or cgroup version is invalid"
    );

    let budgets_path = root.join("benchmarks/b1/budgets.json");
    let budget_bytes = fs::read(&budgets_path).context("failed to read governed B1 budgets")?;
    let budgets: Value =
        serde_json::from_slice(&budget_bytes).context("governed B1 budgets are not valid JSON")?;
    let idle_budget = budgets
        .pointer("/memory_bytes/idle_target")
        .and_then(Value::as_u64)
        .context("B1 idle memory budget is missing")?;
    let ceiling = budgets
        .pointer("/memory_bytes/absolute_ceiling")
        .and_then(Value::as_u64)
        .context("B1 absolute memory ceiling is missing")?;
    let artifact_limit = budgets
        .pointer("/artifact_bytes/native_runtime_installed")
        .and_then(Value::as_u64)
        .context("B1 native runtime artifact budget is missing")?;
    let warmup_seconds = budgets
        .pointer("/timing_seconds/idle_warmup")
        .and_then(Value::as_u64)
        .context("B1 idle warm-up duration is missing")?;
    let measurement_seconds = budgets
        .pointer("/timing_seconds/idle_measurement")
        .and_then(Value::as_u64)
        .context("B1 idle measurement duration is missing")?;
    let sample_interval_ms = budgets
        .pointer("/timing_seconds/sample_interval_ms")
        .and_then(Value::as_u64)
        .context("B1 idle sample interval is missing")?;
    let cpu_average_limit_bp = cpu_limit_basis_points(
        budgets
            .pointer("/idle_cpu_percent_one_core/average")
            .and_then(Value::as_f64)
            .context("B1 idle CPU average budget is missing")?,
    )?;
    let cpu_p95_limit_bp = cpu_limit_basis_points(
        budgets
            .pointer("/idle_cpu_percent_one_core/p95")
            .and_then(Value::as_f64)
            .context("B1 idle CPU p95 budget is missing")?,
    )?;
    ensure!(
        receipt.policy.budgets_sha256 == sha256_bytes(&budget_bytes)
            && receipt.policy.harness_sha256
                == sha256_bytes(
                    &fs::read(root.join("scripts/bench-envelope.sh"))
                        .context("failed to read envelope harness policy")?,
                )
            && receipt.policy.workload_sha256
                == sha256_bytes(
                    &fs::read(root.join("scripts/bench-daemon-idle.sh"))
                        .context("failed to read envelope workload policy")?,
                ),
        "performance envelope policy digests do not bind the reviewed source"
    );
    ensure!(
        receipt.envelope.memory_max_bytes == ceiling
            && receipt.envelope.memory_swap_max_bytes == 0
            && receipt.envelope.memory_swap_peak_bytes == 0
            && receipt.envelope.cpu_quota_micros > 0
            && receipt.envelope.cpu_quota_micros == receipt.envelope.cpu_period_micros
            && receipt.envelope.oom_event_count == 0,
        "performance envelope did not apply the 192 MiB, one-vCPU, zero-swap limits cleanly"
    );
    ensure!(
        receipt.measurement.profile == "canonical_idle_v1"
            && receipt.measurement.target == "idle"
            && receipt.measurement.budget_bytes == idle_budget
            && receipt.measurement.peak_memory_bytes <= ceiling
            && receipt.measurement.warmup_seconds == warmup_seconds
            && receipt.measurement.measurement_seconds == measurement_seconds
            && receipt.measurement.sample_interval_ms == sample_interval_ms
            && receipt.measurement.max_sample_lateness_ns == MAX_SAMPLE_LATENESS_NS
            && receipt.measurement.actual_warmup_ns
                >= warmup_seconds.saturating_mul(1_000_000_000)
            && receipt.measurement.actual_warmup_ns
                <= warmup_seconds
                    .saturating_mul(1_000_000_000)
                    .saturating_add(MAX_SAMPLE_LATENESS_NS)
            && receipt.measurement.actual_measurement_ns
                >= measurement_seconds.saturating_mul(1_000_000_000)
            && receipt.measurement.actual_measurement_ns
                <= measurement_seconds
                    .saturating_mul(1_000_000_000)
                    .saturating_add(MAX_SAMPLE_LATENESS_NS)
            && receipt.measurement.network_isolation == "route_less_user_network_namespace"
            && receipt.measurement.command_exit_code == 0
            && receipt.measurement.command
                == [
                    "bash",
                    "scripts/bench-daemon-idle.sh",
                    "target/release/fastid",
                ],
        "performance envelope canonical idle profile, timing, command, or ceiling verdict is invalid"
    );
    ensure!(
        sample_interval_ms > 0
            && measurement_seconds.saturating_mul(1000) % sample_interval_ms == 0,
        "governed B1 idle timing does not contain a whole number of samples"
    );
    let expected_samples = measurement_seconds.saturating_mul(1000) / sample_interval_ms;
    let sample_interval_ns = sample_interval_ms
        .checked_mul(1_000_000)
        .context("performance envelope sample interval overflowed")?;
    ensure!(
        receipt.measurement.observations.len() as u64 == expected_samples && expected_samples > 0,
        "performance envelope canonical idle sample count is invalid"
    );
    let mut previous_elapsed = 0_u64;
    let mut steady_memory_peak = 0_u64;
    let mut total_cpu_micros = 0_u128;
    let mut cpu_samples = Vec::with_capacity(receipt.measurement.observations.len());
    for (index, observation) in receipt.measurement.observations.iter().enumerate() {
        let expected_sequence = index as u64 + 1;
        let expected_elapsed = expected_sequence
            .checked_mul(sample_interval_ns)
            .context("performance envelope sample deadline overflowed")?;
        ensure!(
            observation.sequence == expected_sequence
                && observation.interval_ns > 0
                && observation.elapsed_ns > previous_elapsed
                && observation.elapsed_ns - previous_elapsed == observation.interval_ns
                && observation.elapsed_ns >= expected_elapsed
                && observation.elapsed_ns
                    <= expected_elapsed.saturating_add(MAX_SAMPLE_LATENESS_NS),
            "performance envelope canonical idle observation sequence or timing is invalid"
        );
        let expected_cpu_bp = cpu_basis_points(
            observation.cpu_usage_delta_micros as u128,
            observation.interval_ns as u128,
        )?;
        ensure!(
            observation.cpu_basis_points == expected_cpu_bp,
            "performance envelope observation CPU result does not recompute"
        );
        previous_elapsed = observation.elapsed_ns;
        steady_memory_peak = steady_memory_peak.max(observation.memory_current_bytes);
        total_cpu_micros += observation.cpu_usage_delta_micros as u128;
        cpu_samples.push(expected_cpu_bp);
    }
    ensure!(
        previous_elapsed == receipt.measurement.actual_measurement_ns,
        "performance envelope measurement duration does not recompute from observations"
    );
    let average_cpu_bp = cpu_basis_points(
        total_cpu_micros,
        receipt.measurement.actual_measurement_ns as u128,
    )?;
    cpu_samples.sort_unstable();
    let p95_index = (95 * cpu_samples.len()).div_ceil(100) - 1;
    let p95_cpu_bp = cpu_samples[p95_index];
    ensure!(
        receipt.measurement.steady_memory_peak_bytes == steady_memory_peak
            && steady_memory_peak <= idle_budget
            && receipt.measurement.cpu_average_basis_points == average_cpu_bp
            && receipt.measurement.cpu_p95_basis_points == p95_cpu_bp
            && average_cpu_bp <= cpu_average_limit_bp
            && p95_cpu_bp <= cpu_p95_limit_bp,
        "performance envelope canonical idle memory or CPU verdict does not recompute or exceeds policy"
    );
    ensure!(
        receipt.artifact.source_path == Path::new("target/release/fastid")
            && receipt.artifact.build_profile == "release"
            && receipt.artifact.size_bytes > 0
            && receipt.artifact.size_bytes <= artifact_limit,
        "performance envelope measured artifact identity or size is invalid"
    );
    validate_relative_path(&receipt.artifact.path)?;
    ensure!(
        receipt.artifact.path.starts_with("artifacts")
            && receipt.artifact.path.components().count() == 2,
        "performance envelope artifact must be a direct child of its artifacts directory"
    );
    ensure_sha256(&receipt.artifact.sha256, "performance artifact sha256")?;
    let artifact_path = receipt_path
        .parent()
        .context("performance receipt path has no parent")?
        .join(&receipt.artifact.path);
    validate_relative_path(&artifact_path)?;
    let artifact = fs::read(root.join(&artifact_path)).with_context(|| {
        format!(
            "performance envelope retained artifact is missing: {}",
            artifact_path.display()
        )
    })?;
    ensure!(
        artifact.len() as u64 == receipt.artifact.size_bytes
            && sha256_bytes(&artifact) == receipt.artifact.sha256,
        "performance envelope retained artifact size or digest does not recompute"
    );
    ensure!(
        elf_architecture(&artifact)? == receipt.runner.architecture,
        "performance envelope retained artifact architecture does not match the runner"
    );
    let artifact_budget_binding = receipt
        .artifact_budget_receipt
        .as_ref()
        .context("canonical performance envelope omits the artifact budget receipt")?;
    validate_relative_path(&artifact_budget_binding.path)?;
    ensure!(
        artifact_budget_binding.path == Path::new("artifact-budgets/evidence.json"),
        "performance envelope artifact budget receipt path is not canonical"
    );
    ensure_sha256(
        &artifact_budget_binding.sha256,
        "artifact budget receipt sha256",
    )?;
    let artifact_budget_path = receipt_path
        .parent()
        .context("performance receipt path has no parent")?
        .join(&artifact_budget_binding.path);
    let artifact_budget_artifacts = validate_artifact_budget_receipt(
        root,
        &artifact_budget_path,
        &artifact_budget_binding.sha256,
        source,
        &receipt.runner.architecture,
        receipt.artifact.size_bytes,
        &budget_bytes,
        &budgets,
    )?;
    Ok(VerifiedPerformanceEnvelope {
        architecture: receipt.runner.architecture,
        run: receipt.ci.run,
        run_attempt: receipt.ci.run_attempt,
        artifact_path,
        artifact_sha256: receipt.artifact.sha256,
        artifact_budget_path,
        artifact_budget_sha256: artifact_budget_binding.sha256.clone(),
        artifact_budget_artifacts,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_artifact_budget_receipt(
    root: &Path,
    receipt_path: &Path,
    expected_digest: &str,
    source: &SourceBinding,
    architecture: &str,
    native_binary_size: u64,
    budget_bytes: &[u8],
    budgets: &Value,
) -> anyhow::Result<Vec<(PathBuf, String)>> {
    let bytes = fs::read(root.join(receipt_path))
        .context("performance envelope artifact budget receipt is missing")?;
    ensure!(
        sha256_bytes(&bytes) == expected_digest,
        "artifact budget receipt digest does not recompute"
    );
    let receipt: ArtifactBudgetReceipt = serde_json::from_slice(&bytes)
        .context("artifact budget receipt does not match its strict shape")?;
    ensure!(
        receipt.schema_version == "fasti.b1.artifact-budgets.v1"
            && receipt.kind == "fasti.b1.artifact-budgets"
            && receipt.status == ResultStatus::Pass
            && receipt.source.git_commit == source.git_commit
            && receipt.source.git_tree == source.git_tree
            && !receipt.source.dirty
            && receipt.runner.architecture == architecture,
        "artifact budget receipt schema, source, or architecture is invalid"
    );
    ensure_hex(
        &receipt.source.contract_ref,
        40,
        "artifact budget contract ref",
    )?;
    ensure_sha256(
        &receipt.source.build_recipe_sha256,
        "artifact budget build recipe sha256",
    )?;
    ensure_sha256(
        &receipt.source.build_context_archive_sha256,
        "artifact budget build context sha256",
    )?;
    let source_inputs: VerifierSourceInputs = serde_json::from_slice(
        &fs::read(root.join(VERIFIER_SOURCE_INPUTS_PATH))
            .context("verifier-owned source inputs are missing")?,
    )
    .context("verifier-owned source inputs are invalid")?;
    ensure!(
        receipt.source.contract_ref == source_inputs.contract_ref
            && receipt.source.build_recipe_sha256
                == sha256_bytes(
                    &fs::read(root.join("benchmarks/b1/Dockerfile"))
                        .context("failed to read the bound OCI build recipe")?,
                )
            && receipt.source.build_context_archive_sha256
                == source_inputs.build_context_archive_sha256,
        "artifact budget contract, recipe, or build context does not match the bound source"
    );
    ensure!(
        receipt.policy.budgets_sha256 == sha256_bytes(budget_bytes)
            && receipt.policy.harness_sha256
                == sha256_bytes(
                    &fs::read(root.join("scripts/benchmark-b1.py"))
                        .context("failed to read artifact budget harness")?,
                ),
        "artifact budget receipt policy digests do not bind the reviewed source"
    );
    ensure!(
        receipt.oci_image_id.starts_with("sha256:")
            && receipt.oci_image_id.len() == 71
            && receipt.oci_image_id[7..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            && receipt.artifact_sizes.native_fastid_binary_bytes == native_binary_size
            && receipt.artifact_sizes.oci_fastid_binary_bytes > 0
            && receipt.artifact_sizes.oci_fasti_cli_binary_bytes > 0
            && receipt
                .artifact_sizes
                .native_runtime_installed_bytes
                .is_none()
            && receipt
                .artifact_sizes
                .native_archive_compressed_bytes
                .is_none()
            && !receipt.commands.is_empty()
            && receipt
                .commands
                .iter()
                .all(|command| !command.trim().is_empty()),
        "artifact budget image, binary inventory, or command evidence is invalid"
    );

    let limits = budgets
        .get("artifact_bytes")
        .and_then(Value::as_object)
        .context("B1 artifact budgets are missing")?;
    ensure!(
        receipt
            .retained_artifacts
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>()
            == ["contract_pack_compressed", "oci_image_compressed"],
        "artifact budget receipt retained artifact inventory is invalid"
    );
    let mut retained = Vec::new();
    let mut recomputed_oci_image_bytes = None;
    for (name, reference) in &receipt.retained_artifacts {
        validate_relative_path(&reference.path)?;
        ensure!(
            reference.path.starts_with("artifacts/sha256")
                && reference.path.components().count() == 3,
            "artifact budget retained path is not canonical"
        );
        ensure_sha256(&reference.sha256, "artifact budget retained sha256")?;
        let path = receipt_path
            .parent()
            .context("artifact budget receipt path has no parent")?
            .join(&reference.path);
        validate_relative_path(&path)?;
        let retained_path = root.join(&path);
        let retained_metadata = fs::metadata(&retained_path).with_context(|| {
            format!(
                "artifact budget retained file is missing: {}",
                path.display()
            )
        })?;
        let (expected_size, expected_sha, budget_name) = if name == "oci_image_compressed" {
            (
                receipt.artifact_sizes.oci_image_compressed_bytes,
                &receipt.artifact_sizes.oci_image_compressed_sha256,
                "oci_image_compressed",
            )
        } else {
            (
                receipt.artifact_sizes.contract_pack_compressed_bytes,
                &receipt.artifact_sizes.contract_pack_compressed_sha256,
                "contract_pack_compressed",
            )
        };
        let retained_limit = limits
            .get(budget_name)
            .and_then(Value::as_u64)
            .context("retained artifact budget limit is missing")?;
        ensure!(
            retained_metadata.is_file()
                && retained_metadata.len() == reference.size_bytes
                && reference.size_bytes == expected_size
                && reference.size_bytes <= retained_limit
                && &reference.sha256 == expected_sha,
            "artifact budget retained file does not bind its measured result"
        );
        let retained_bytes = fs::read(&retained_path)
            .context("failed to read bounded artifact budget retained file")?;
        ensure!(
            sha256_bytes(&retained_bytes) == reference.sha256,
            "artifact budget retained file digest does not recompute"
        );
        if name == "oci_image_compressed" {
            recomputed_oci_image_bytes = Some(validate_saved_oci_archive(
                &retained_path,
                &receipt,
                OCI_UNPACKED_SAFETY_CEILING_BYTES,
            )?);
        }
        retained.push((path, reference.sha256.clone()));
    }
    let recomputed_oci_image_bytes = recomputed_oci_image_bytes
        .context("artifact budget receipt omits the retained OCI image")?;
    ensure!(
        receipt.artifact_sizes.oci_image_bytes == recomputed_oci_image_bytes,
        "OCI unpacked image bytes do not recompute from the retained archive"
    );
    let measured = BTreeMap::from([
        (
            "native_runtime_installed",
            receipt.artifact_sizes.native_runtime_installed_bytes,
        ),
        (
            "native_archive_compressed",
            receipt.artifact_sizes.native_archive_compressed_bytes,
        ),
        (
            "oci_image_compressed",
            Some(receipt.artifact_sizes.oci_image_compressed_bytes),
        ),
        ("oci_image_unpacked", Some(recomputed_oci_image_bytes)),
        (
            "contract_pack_compressed",
            Some(receipt.artifact_sizes.contract_pack_compressed_bytes),
        ),
    ]);
    ensure!(
        receipt.artifact_budget_verdicts.len() == measured.len(),
        "artifact budget receipt verdict count is invalid"
    );
    let mut seen = BTreeSet::new();
    for verdict in &receipt.artifact_budget_verdicts {
        let expected_measurement = measured
            .get(verdict.budget.as_str())
            .with_context(|| format!("unexpected artifact budget verdict: {}", verdict.budget))?;
        let expected_limit = limits
            .get(&verdict.budget)
            .and_then(Value::as_u64)
            .context("artifact budget limit is missing")?;
        let expected_status = if expected_measurement.is_some() {
            "pass"
        } else {
            "not_applicable"
        };
        ensure!(
            seen.insert(verdict.budget.as_str())
                && verdict.limit_bytes == expected_limit
                && &verdict.measured_bytes == expected_measurement
                && verdict.status == expected_status
                && verdict
                    .measured_bytes
                    .is_none_or(|measurement| measurement <= expected_limit)
                && !verdict.reason.trim().is_empty(),
            "artifact budget verdict does not recompute or exceeds policy"
        );
    }
    Ok(retained)
}

fn validate_saved_oci_archive(
    compressed_path: &Path,
    receipt: &ArtifactBudgetReceipt,
    unpacked_safety_ceiling: u64,
) -> anyhow::Result<u64> {
    let decompressed_limit = unpacked_safety_ceiling
        .checked_add(OCI_ARCHIVE_METADATA_ALLOWANCE_BYTES)
        .context("OCI archive decompression limit overflowed")?;
    let mut child = Command::new("gzip")
        .args(["-cd", "--"])
        .arg(compressed_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to start bounded OCI archive decompression")?;
    let stdout = child.stdout.take().context("gzip stdout is unavailable")?;
    let mut decompressed = tempfile::tempfile().context("failed to create OCI archive buffer")?;
    let copied = std::io::copy(
        &mut stdout.take(decompressed_limit.saturating_add(1)),
        &mut decompressed,
    )
    .context("failed to decompress retained OCI archive")?;
    if copied > decompressed_limit {
        let _ = child.kill();
        let _ = child.wait();
        bail!("retained OCI archive exceeds the bounded decompression limit");
    }
    ensure!(
        child
            .wait()
            .context("failed to wait for OCI archive decompression")?
            .success(),
        "retained OCI archive is not valid gzip data"
    );
    decompressed
        .rewind()
        .context("failed to rewind retained OCI archive")?;

    let mut archive = tar::Archive::new(
        decompressed
            .try_clone()
            .context("failed to clone retained OCI archive buffer")?,
    );
    let mut paths = BTreeSet::new();
    let mut files = BTreeMap::new();
    let mut entry_count = 0_u64;
    for entry in archive
        .entries_with_seek()
        .context("failed to enumerate retained OCI archive")?
    {
        entry_count += 1;
        ensure!(
            entry_count <= OCI_ARCHIVE_ENTRY_LIMIT,
            "retained OCI archive has too many entries"
        );
        let mut entry = entry.context("failed to read retained OCI archive entry")?;
        let path = entry
            .path()
            .context("retained OCI archive entry path is invalid")?
            .into_owned();
        validate_relative_path(&path)?;
        ensure!(
            paths.insert(path.clone()),
            "retained OCI archive contains a duplicate path"
        );
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let position = entry.raw_file_position();
        let size = entry
            .header()
            .size()
            .context("retained OCI archive entry size is invalid")?;
        let (sha256, read_bytes) = sha256_reader(&mut entry, "retained OCI archive entry")?;
        ensure!(
            read_bytes == size,
            "retained OCI archive entry bytes do not match the tar header"
        );
        files.insert(
            path,
            SavedOciArchiveFile {
                position,
                size,
                sha256,
            },
        );
    }
    drop(archive);

    let has_layout = files.contains_key(Path::new("oci-layout"));
    let has_index = files.contains_key(Path::new("index.json"));
    if has_layout || has_index {
        ensure!(
            has_layout && has_index,
            "retained OCI layout must contain both oci-layout and index.json"
        );
        validate_oci_layout_archive(&mut decompressed, &files, receipt, unpacked_safety_ceiling)
    } else {
        validate_legacy_docker_archive(&mut decompressed, &files, receipt, unpacked_safety_ceiling)
    }
}

fn read_saved_oci_archive_file(
    archive: &mut fs::File,
    files: &BTreeMap<PathBuf, SavedOciArchiveFile>,
    path: &Path,
    label: &str,
) -> anyhow::Result<Vec<u8>> {
    let file = files
        .get(path)
        .with_context(|| format!("retained OCI archive omits {label}"))?;
    ensure!(
        file.size <= OCI_ARCHIVE_METADATA_FILE_LIMIT_BYTES,
        "retained OCI archive {label} is too large"
    );
    archive
        .seek(SeekFrom::Start(file.position))
        .with_context(|| format!("failed to seek to retained OCI archive {label}"))?;
    let capacity = usize::try_from(file.size).context("retained OCI metadata is too large")?;
    let mut bytes = Vec::with_capacity(capacity);
    archive
        .take(file.size)
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read retained OCI archive {label}"))?;
    ensure!(
        bytes.len() as u64 == file.size,
        "retained OCI archive {label} is truncated"
    );
    Ok(bytes)
}

fn docker_archive_manifest(
    archive: &mut fs::File,
    files: &BTreeMap<PathBuf, SavedOciArchiveFile>,
) -> anyhow::Result<DockerArchiveManifestEntry> {
    let bytes =
        read_saved_oci_archive_file(archive, files, Path::new("manifest.json"), "manifest.json")?;
    let mut manifests: Vec<DockerArchiveManifestEntry> =
        serde_json::from_slice(&bytes).context("retained OCI archive manifest.json is invalid")?;
    ensure!(
        manifests.len() == 1,
        "retained OCI archive must contain exactly one image"
    );
    Ok(manifests.remove(0))
}

fn validate_legacy_docker_archive(
    archive: &mut fs::File,
    files: &BTreeMap<PathBuf, SavedOciArchiveFile>,
    receipt: &ArtifactBudgetReceipt,
    unpacked_safety_ceiling: u64,
) -> anyhow::Result<u64> {
    let image_digest = oci_sha256_digest(&receipt.oci_image_id, "OCI image ID")?;
    let expected_config_path = PathBuf::from(format!("{image_digest}.json"));
    let manifest = docker_archive_manifest(archive, files)?;
    ensure!(
        manifest.config == expected_config_path.to_string_lossy(),
        "retained OCI archive config does not match the immutable image ID"
    );
    ensure!(
        !manifest.layers.is_empty(),
        "retained OCI archive layer inventory is empty"
    );
    let mut expected_layers = BTreeSet::new();
    for layer in &manifest.layers {
        let path = PathBuf::from(layer);
        validate_relative_path(&path)?;
        ensure!(
            expected_layers.insert(path),
            "retained OCI archive manifest duplicates a layer"
        );
    }
    let actual_layers = files
        .keys()
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("tar"))
        .cloned()
        .collect::<BTreeSet<_>>();
    ensure!(
        expected_layers == actual_layers,
        "retained OCI archive layer files do not match its manifest"
    );

    let config_file = files
        .get(&expected_config_path)
        .context("retained OCI archive omits image config")?;
    ensure!(
        config_file.sha256 == image_digest,
        "retained OCI archive config digest does not match the immutable image ID"
    );
    let config_bytes =
        read_saved_oci_archive_file(archive, files, &expected_config_path, "image config")?;
    let diff_ids = manifest
        .layers
        .iter()
        .map(|path| {
            files
                .get(Path::new(path))
                .map(|file| format!("sha256:{}", file.sha256))
                .context("retained OCI archive omits a manifest layer")
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    validate_saved_oci_config(&config_bytes, receipt, &diff_ids)?;

    let unpacked_bytes = manifest.layers.into_iter().try_fold(0_u64, |total, path| {
        total
            .checked_add(files[Path::new(&path)].size)
            .context("retained OCI layer size sum overflowed")
    })?;
    ensure!(
        unpacked_bytes <= unpacked_safety_ceiling,
        "retained OCI layers exceed the unpacked safety ceiling"
    );
    Ok(unpacked_bytes)
}

fn validate_oci_layout_archive(
    archive: &mut fs::File,
    files: &BTreeMap<PathBuf, SavedOciArchiveFile>,
    receipt: &ArtifactBudgetReceipt,
    unpacked_safety_ceiling: u64,
) -> anyhow::Result<u64> {
    let layout: OciLayout = serde_json::from_slice(&read_saved_oci_archive_file(
        archive,
        files,
        Path::new("oci-layout"),
        "oci-layout",
    )?)
    .context("retained OCI archive oci-layout is invalid")?;
    ensure!(
        layout.image_layout_version == "1.0.0",
        "retained OCI archive layout version is unsupported"
    );
    let index: OciIndex = serde_json::from_slice(&read_saved_oci_archive_file(
        archive,
        files,
        Path::new("index.json"),
        "index.json",
    )?)
    .context("retained OCI archive index.json is invalid")?;
    ensure!(
        index.schema_version == 2
            && index
                .media_type
                .as_deref()
                .is_none_or(is_oci_index_media_type),
        "retained OCI archive index.json media type or schema is invalid"
    );
    let direct_targets = index
        .manifests
        .iter()
        .filter(|descriptor| descriptor.digest == receipt.oci_image_id)
        .collect::<Vec<_>>();
    ensure!(
        direct_targets.len() <= 1,
        "retained OCI archive index duplicates the immutable image ID"
    );
    let uses_target_identity = direct_targets.len() == 1;
    let graph_roots = if uses_target_identity {
        direct_targets
    } else {
        index.manifests.iter().collect()
    };
    let mut visited = BTreeSet::new();
    let mut manifests = Vec::new();
    for root in graph_roots {
        collect_oci_manifests(archive, files, root, 0, &mut visited, &mut manifests)?;
    }
    let compatibility = docker_archive_manifest(archive, files)?;
    let compatibility_config_path = PathBuf::from(&compatibility.config);
    validate_relative_path(&compatibility_config_path)?;
    ensure!(
        !compatibility.layers.is_empty(),
        "retained OCI archive layer inventory is empty"
    );
    let compatibility_layer_paths = compatibility
        .layers
        .iter()
        .map(|path| {
            let path = PathBuf::from(path);
            validate_relative_path(&path)?;
            Ok(path)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    ensure!(
        compatibility_layer_paths
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            == compatibility_layer_paths.len(),
        "retained OCI archive manifest.json duplicates a layer"
    );
    let mut selected = Vec::new();
    for descriptor in manifests {
        let manifest: OciImageManifest = read_oci_descriptor_json(
            archive,
            files,
            &descriptor,
            "selected platform manifest candidate",
        )?;
        ensure!(
            manifest.schema_version == 2
                && manifest
                    .media_type
                    .as_deref()
                    .is_none_or(|media_type| media_type == descriptor.media_type),
            "retained OCI archive selected manifest media type or schema is invalid"
        );
        let config_path = oci_descriptor_path(&manifest.config.digest)?;
        let layer_paths = manifest
            .layers
            .iter()
            .map(|layer| oci_descriptor_path(&layer.digest))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let identity_matches =
            uses_target_identity || manifest.config.digest == receipt.oci_image_id;
        if identity_matches
            && config_path == compatibility_config_path
            && layer_paths == compatibility_layer_paths
        {
            selected.push(manifest);
        }
    }
    ensure!(
        selected.len() == 1,
        "retained OCI archive image graph does not bind the immutable image ID to exactly one selected platform manifest whose descriptors match manifest.json"
    );
    let manifest = selected.remove(0);
    ensure!(
        is_oci_config_media_type(&manifest.config.media_type),
        "retained OCI archive image config media type is unsupported"
    );

    verify_oci_descriptor(files, &manifest.config, "image config")?;
    let config_bytes =
        read_oci_descriptor_json_bytes(archive, files, &manifest.config, "image config")?;
    let mut unpacked_bytes = 0_u64;
    let mut diff_ids = Vec::with_capacity(manifest.layers.len());
    for descriptor in &manifest.layers {
        let file = verify_oci_descriptor(files, descriptor, "image layer")?;
        let remaining = unpacked_safety_ceiling.saturating_sub(unpacked_bytes);
        let (diff_id, size) = hash_oci_layer(archive, file, &descriptor.media_type, remaining)?;
        unpacked_bytes = unpacked_bytes
            .checked_add(size)
            .context("retained OCI layer size sum overflowed")?;
        diff_ids.push(format!("sha256:{diff_id}"));
    }
    validate_saved_oci_config(&config_bytes, receipt, &diff_ids)?;
    Ok(unpacked_bytes)
}

#[allow(clippy::too_many_arguments)]
fn collect_oci_manifests(
    archive: &mut fs::File,
    files: &BTreeMap<PathBuf, SavedOciArchiveFile>,
    descriptor: &OciDescriptor,
    depth: usize,
    visited: &mut BTreeSet<String>,
    manifests: &mut Vec<OciDescriptor>,
) -> anyhow::Result<()> {
    ensure!(
        depth <= 16,
        "retained OCI archive descriptor graph is too deep"
    );
    ensure!(
        visited.insert(descriptor.digest.clone()),
        "retained OCI archive descriptor graph is cyclic or ambiguous"
    );
    verify_oci_descriptor(files, descriptor, "descriptor graph blob")?;
    if is_oci_manifest_media_type(&descriptor.media_type) {
        manifests.push(descriptor.clone());
        return Ok(());
    }
    ensure!(
        is_oci_index_media_type(&descriptor.media_type),
        "retained OCI archive descriptor graph has an unsupported media type"
    );
    let index: OciIndex =
        read_oci_descriptor_json(archive, files, descriptor, "descriptor graph index")?;
    ensure!(
        index.schema_version == 2
            && index
                .media_type
                .as_deref()
                .is_none_or(|media_type| media_type == descriptor.media_type),
        "retained OCI archive descriptor graph index is invalid"
    );
    for child in &index.manifests {
        collect_oci_manifests(archive, files, child, depth + 1, visited, manifests)?;
    }
    Ok(())
}

fn verify_oci_descriptor<'a>(
    files: &'a BTreeMap<PathBuf, SavedOciArchiveFile>,
    descriptor: &OciDescriptor,
    label: &str,
) -> anyhow::Result<&'a SavedOciArchiveFile> {
    let path = oci_descriptor_path(&descriptor.digest)?;
    let expected_digest = oci_sha256_digest(&descriptor.digest, label)?;
    let file = files
        .get(&path)
        .with_context(|| format!("retained OCI archive omits {label} blob"))?;
    ensure!(
        file.size == descriptor.size && file.sha256 == expected_digest,
        "retained OCI archive {label} digest or size does not match its descriptor"
    );
    Ok(file)
}

fn read_oci_descriptor_json<T: for<'de> Deserialize<'de>>(
    archive: &mut fs::File,
    files: &BTreeMap<PathBuf, SavedOciArchiveFile>,
    descriptor: &OciDescriptor,
    label: &str,
) -> anyhow::Result<T> {
    serde_json::from_slice(&read_oci_descriptor_json_bytes(
        archive, files, descriptor, label,
    )?)
    .with_context(|| format!("retained OCI archive {label} is invalid JSON"))
}

fn read_oci_descriptor_json_bytes(
    archive: &mut fs::File,
    files: &BTreeMap<PathBuf, SavedOciArchiveFile>,
    descriptor: &OciDescriptor,
    label: &str,
) -> anyhow::Result<Vec<u8>> {
    verify_oci_descriptor(files, descriptor, label)?;
    read_saved_oci_archive_file(
        archive,
        files,
        &oci_descriptor_path(&descriptor.digest)?,
        label,
    )
}

fn oci_descriptor_path(digest: &str) -> anyhow::Result<PathBuf> {
    Ok(PathBuf::from("blobs")
        .join("sha256")
        .join(oci_sha256_digest(digest, "OCI descriptor")?))
}

fn oci_sha256_digest<'a>(digest: &'a str, label: &str) -> anyhow::Result<&'a str> {
    let value = digest
        .strip_prefix("sha256:")
        .with_context(|| format!("{label} is not a sha256 digest"))?;
    ensure!(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{label} is not a canonical sha256 digest"
    );
    Ok(value)
}

fn is_oci_index_media_type(media_type: &str) -> bool {
    matches!(
        media_type,
        "application/vnd.oci.image.index.v1+json"
            | "application/vnd.docker.distribution.manifest.list.v2+json"
    )
}

fn is_oci_manifest_media_type(media_type: &str) -> bool {
    matches!(
        media_type,
        "application/vnd.oci.image.manifest.v1+json"
            | "application/vnd.docker.distribution.manifest.v2+json"
    )
}

fn is_oci_config_media_type(media_type: &str) -> bool {
    matches!(
        media_type,
        "application/vnd.oci.image.config.v1+json"
            | "application/vnd.docker.container.image.v1+json"
    )
}

fn hash_oci_layer(
    archive: &mut fs::File,
    file: &SavedOciArchiveFile,
    media_type: &str,
    remaining_safety_ceiling: u64,
) -> anyhow::Result<(String, u64)> {
    archive
        .seek(SeekFrom::Start(file.position))
        .context("failed to seek to retained OCI layer")?;
    match media_type {
        "application/vnd.oci.image.layer.v1.tar"
        | "application/vnd.oci.image.layer.nondistributable.v1.tar"
        | "application/vnd.docker.image.rootfs.diff.tar" => {
            ensure!(
                file.size <= remaining_safety_ceiling,
                "retained OCI layers exceed the unpacked safety ceiling"
            );
            let (digest, bytes) =
                sha256_reader(&mut archive.take(file.size), "uncompressed OCI layer")?;
            ensure!(
                bytes == file.size,
                "retained OCI uncompressed layer is truncated"
            );
            Ok((digest, bytes))
        }
        "application/vnd.oci.image.layer.v1.tar+gzip"
        | "application/vnd.oci.image.layer.nondistributable.v1.tar+gzip"
        | "application/vnd.docker.image.rootfs.diff.tar.gzip"
        | "application/vnd.docker.image.rootfs.foreign.diff.tar.gzip" => {
            let mut compressed = tempfile::tempfile()
                .context("failed to create retained OCI compressed layer buffer")?;
            let copied = std::io::copy(&mut archive.take(file.size), &mut compressed)
                .context("failed to copy retained OCI compressed layer")?;
            ensure!(
                copied == file.size,
                "retained OCI compressed layer is truncated"
            );
            compressed
                .rewind()
                .context("failed to rewind retained OCI compressed layer")?;
            let mut child = Command::new("gzip")
                .arg("-cd")
                .stdin(Stdio::from(compressed))
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .context("failed to start retained OCI layer decompression")?;
            let stdout = child.stdout.take().context("gzip stdout is unavailable")?;
            let (digest, bytes) = sha256_reader(
                &mut stdout.take(remaining_safety_ceiling.saturating_add(1)),
                "uncompressed OCI layer",
            )?;
            if bytes > remaining_safety_ceiling {
                let _ = child.kill();
                let _ = child.wait();
                bail!("retained OCI layers exceed the unpacked safety ceiling");
            }
            ensure!(
                child
                    .wait()
                    .context("failed to wait for retained OCI layer decompression")?
                    .success(),
                "retained OCI layer does not match its declared gzip media type"
            );
            Ok((digest, bytes))
        }
        _ => bail!("retained OCI layer uses an unsupported compression media type"),
    }
}

fn docker_architecture(architecture: &str) -> anyhow::Result<&'static str> {
    match architecture {
        "x86_64" => Ok("amd64"),
        "aarch64" => Ok("arm64"),
        _ => bail!("artifact budget runner architecture is unsupported"),
    }
}

fn validate_saved_oci_config(
    config_bytes: &[u8],
    receipt: &ArtifactBudgetReceipt,
    expected_diff_ids: &[String],
) -> anyhow::Result<()> {
    let config: Value = serde_json::from_slice(config_bytes)
        .context("retained OCI archive image config is invalid")?;
    ensure!(
        config.pointer("/rootfs/type").and_then(Value::as_str) == Some("layers"),
        "retained OCI archive config rootfs type is unsupported"
    );
    let expected_architecture = docker_architecture(&receipt.runner.architecture)?;
    ensure!(
        config.get("architecture").and_then(Value::as_str) == Some(expected_architecture),
        "retained OCI archive architecture does not match the runner"
    );
    ensure!(
        config.get("os").and_then(Value::as_str) == Some("linux"),
        "retained OCI archive operating system is not Linux"
    );
    let labels = config
        .pointer("/config/Labels")
        .and_then(Value::as_object)
        .context("retained OCI archive config omits source labels")?;
    for (label, expected) in [
        (
            "org.opencontainers.image.revision",
            receipt.source.git_commit.as_str(),
        ),
        (
            "dev.scrobble.fasti.source.tree",
            receipt.source.git_tree.as_str(),
        ),
        (
            "dev.scrobble.fasti.contracts",
            receipt.source.contract_ref.as_str(),
        ),
        (
            "dev.scrobble.fasti.build.recipe.sha256",
            receipt.source.build_recipe_sha256.as_str(),
        ),
        (
            "dev.scrobble.fasti.build.context.archive.sha256",
            receipt.source.build_context_archive_sha256.as_str(),
        ),
    ] {
        ensure!(
            labels.get(label).and_then(Value::as_str) == Some(expected),
            "retained OCI archive source label is stale: {label}"
        );
    }
    let diff_ids = config
        .pointer("/rootfs/diff_ids")
        .and_then(Value::as_array)
        .context("retained OCI archive config omits layer identities")?;
    ensure!(
        diff_ids.len() == expected_diff_ids.len()
            && diff_ids
                .iter()
                .zip(expected_diff_ids)
                .all(|(actual, expected)| actual.as_str() == Some(expected)),
        "retained OCI archive config layer identities do not match the retained bytes"
    );
    Ok(())
}

fn cpu_limit_basis_points(percent: f64) -> anyhow::Result<u64> {
    let basis_points = percent * 100.0;
    ensure!(
        percent.is_finite()
            && percent >= 0.0
            && basis_points <= u64::MAX as f64
            && basis_points.fract() == 0.0,
        "B1 idle CPU limit is not an exact nonnegative basis-point value"
    );
    Ok(basis_points as u64)
}

fn cpu_basis_points(cpu_usage_micros: u128, elapsed_ns: u128) -> anyhow::Result<u64> {
    ensure!(elapsed_ns > 0, "CPU sample duration must be positive");
    let result = cpu_usage_micros
        .checked_mul(10_000_000)
        .context("CPU sample result overflowed")?
        .div_ceil(elapsed_ns);
    u64::try_from(result).context("CPU sample result does not fit in u64")
}

fn elf_architecture(bytes: &[u8]) -> anyhow::Result<&'static str> {
    ensure!(
        bytes.len() >= 20 && bytes.starts_with(b"\x7fELF") && bytes[4] == 2 && bytes[5] == 1,
        "performance envelope retained artifact is not a 64-bit little-endian ELF executable"
    );
    match u16::from_le_bytes([bytes[18], bytes[19]]) {
        62 => Ok("x86_64"),
        183 => Ok("aarch64"),
        machine => bail!("unsupported performance artifact ELF machine: {machine}"),
    }
}

fn verify_performance_envelope_set(
    root: &Path,
    entries: &[EvidenceEntry],
    source: &SourceBinding,
) -> anyhow::Result<()> {
    let receipts = entries
        .iter()
        .filter(|entry| entry.kind == EvidenceKind::B1PerformanceEnvelope)
        .map(|entry| validate_performance_envelope_receipt(root, &entry.path, source))
        .collect::<anyhow::Result<Vec<_>>>()?;
    ensure!(
        receipts.len() == 2,
        "B1 requires exactly two performance envelope receipts"
    );
    let architectures = receipts
        .iter()
        .map(|receipt| receipt.architecture.as_str())
        .collect::<BTreeSet<_>>();
    ensure!(
        architectures == BTreeSet::from(["aarch64", "x86_64"]),
        "performance envelope receipts must cover exactly aarch64 and x86_64"
    );
    ensure!(
        receipts
            .iter()
            .map(|receipt| (receipt.run.as_str(), receipt.run_attempt.as_str()))
            .collect::<BTreeSet<_>>()
            .len()
            == 1,
        "performance envelope receipts must come from the same CI run attempt"
    );
    for receipt in receipts {
        let bound = entries
            .iter()
            .filter(|entry| {
                entry.kind == EvidenceKind::BuiltArtifact
                    && entry.status == ResultStatus::Pass
                    && entry.path == receipt.artifact_path
            })
            .collect::<Vec<_>>();
        ensure!(
            bound.len() == 1 && bound[0].sha256 == receipt.artifact_sha256,
            "performance envelope retained artifact must resolve to one digest-matched BuiltArtifact entry"
        );
        let artifact_budget = entries
            .iter()
            .filter(|entry| {
                entry.kind == EvidenceKind::B1ArtifactBudgets
                    && entry.status == ResultStatus::Pass
                    && entry.path == receipt.artifact_budget_path
            })
            .collect::<Vec<_>>();
        ensure!(
            artifact_budget.len() == 1
                && artifact_budget[0].sha256 == receipt.artifact_budget_sha256,
            "performance envelope artifact budget receipt must resolve to one digest-matched entry"
        );
        for (path, digest) in receipt.artifact_budget_artifacts {
            let retained = entries
                .iter()
                .filter(|entry| {
                    entry.kind == EvidenceKind::BuiltArtifact
                        && entry.status == ResultStatus::Pass
                        && entry.path == path
                })
                .collect::<Vec<_>>();
            ensure!(
                retained.len() == 1 && retained[0].sha256 == digest,
                "artifact budget retained file must resolve to one digest-matched BuiltArtifact entry"
            );
        }
    }
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

fn validate_qa_receipt(
    root: &Path,
    entries: &[EvidenceEntry],
    source: &SourceBinding,
    expected_body: Body,
    expected_review_command: &str,
    expected_design_review_status: DesignReviewStatus,
) -> anyhow::Result<QaReceipt> {
    let entry = entries
        .iter()
        .find(|entry| entry.kind == EvidenceKind::QaReview)
        .context("QA receipt entry is missing")?;
    let bytes = fs::read(root.join(&entry.path))
        .with_context(|| format!("failed to read QA receipt {}", entry.path.display()))?;
    let receipt: QaReceipt = serde_json::from_slice(&bytes)
        .context("QA receipt does not match the strict machine-readable shape")?;
    ensure!(
        receipt.schema_version == "fasti.qa-review.v1"
            && receipt.kind == "fasti.qa-review"
            && receipt.body == expected_body,
        "QA receipt schema, kind, or body is invalid"
    );
    ensure!(
        receipt.status == ResultStatus::Pass,
        "mandatory QA did not pass"
    );
    ensure!(
        receipt.reviewed_commit == source.git_commit && receipt.reviewed_tree == source.git_tree,
        "QA receipt is stale for the reviewed source"
    );
    ensure!(
        receipt.review_command == expected_review_command,
        "QA receipt must bind {expected_review_command}"
    );
    ensure!(receipt.open_findings == 0, "QA receipt has open findings");
    if expected_design_review_status == DesignReviewStatus::NotApplicable {
        ensure!(
            !receipt.rendered_ui_or_ux_changed
                && receipt.design_review.status == DesignReviewStatus::NotApplicable
                && !receipt.design_review.reason.trim().is_empty(),
            "headless QA must record design review N/A with a reason"
        );
    } else {
        ensure!(
            receipt.design_review.status == expected_design_review_status
                && !receipt.design_review.reason.trim().is_empty(),
            "QA receipt design review status does not match the required status"
        );
    }
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

fn performance_envelope_paths(root: &Path) -> anyhow::Result<BTreeMap<String, PathBuf>> {
    let relative = PathBuf::from("target/fasti-evidence/envelope");
    let directory = root.join(&relative);
    ensure!(
        directory.is_dir(),
        "required low-hardware envelope evidence directory is missing: {}",
        relative.display()
    );
    let mut candidates = Vec::new();
    collect_named_files(root, &directory, "receipt.json", &mut candidates)?;
    let mut by_architecture = BTreeMap::new();
    for path in candidates {
        let bytes = fs::read(root.join(&path))?;
        let receipt: PerformanceEnvelopeReceipt = serde_json::from_slice(&bytes)
            .context("performance envelope receipt has an invalid shape")?;
        ensure!(
            matches!(receipt.runner.architecture.as_str(), "x86_64" | "aarch64"),
            "performance envelope receipt has an unexpected architecture: {}",
            receipt.runner.architecture
        );
        ensure!(
            by_architecture
                .insert(receipt.runner.architecture.clone(), path)
                .is_none(),
            "performance envelope directory contains more than one {} receipt",
            receipt.runner.architecture
        );
    }
    ensure!(
        by_architecture
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>()
            == ["aarch64", "x86_64"],
        "performance envelope evidence requires exactly aarch64 and x86_64 receipts"
    );
    Ok(by_architecture)
}

fn collect_named_files(
    workspace_root: &Path,
    directory: &Path,
    name: &str,
    output: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    let mut children = fs::read_dir(directory)
        .with_context(|| format!("failed to inspect {}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()?;
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        let path = child.path();
        let metadata = fs::symlink_metadata(&path)?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "performance envelope evidence contains a symlink: {}",
            path.display()
        );
        if metadata.is_dir() {
            collect_named_files(workspace_root, &path, name, output)?;
        } else if metadata.is_file() && child.file_name() == name {
            output.push(
                path.strip_prefix(workspace_root)
                    .context("performance envelope receipt escaped the workspace")?
                    .to_path_buf(),
            );
        }
    }
    Ok(())
}

fn performance_envelope_artifacts(
    root: &Path,
    receipt_paths: &BTreeMap<String, PathBuf>,
) -> anyhow::Result<Vec<PerformancePackageFile>> {
    let mut artifacts = BTreeSet::new();
    let mut bindings = Vec::new();
    for (architecture, receipt_path) in receipt_paths {
        let bytes = fs::read(root.join(receipt_path))?;
        let receipt: PerformanceEnvelopeReceipt = serde_json::from_slice(&bytes)
            .context("performance envelope receipt has an invalid shape")?;
        validate_relative_path(&receipt.artifact.path)?;
        let artifact_path = receipt_path
            .parent()
            .context("performance receipt path has no parent")?
            .join(&receipt.artifact.path);
        validate_relative_path(&artifact_path)?;
        ensure!(
            artifacts.insert(artifact_path.clone()),
            "performance envelope receipts alias one retained artifact path"
        );
        bindings.push((
            architecture.clone(),
            "daemon".to_owned(),
            EvidenceKind::BuiltArtifact,
            artifact_path,
            receipt.artifact.sha256,
        ));
        let artifact_budget = receipt
            .artifact_budget_receipt
            .context("performance envelope omits artifact budget binding")?;
        validate_relative_path(&artifact_budget.path)?;
        let artifact_budget_path = receipt_path
            .parent()
            .context("performance receipt path has no parent")?
            .join(&artifact_budget.path);
        ensure!(
            artifacts.insert(artifact_budget_path.clone()),
            "performance packages alias one artifact budget receipt path"
        );
        let artifact_budget_bytes = fs::read(root.join(&artifact_budget_path))?;
        ensure!(
            sha256_bytes(&artifact_budget_bytes) == artifact_budget.sha256,
            "artifact budget receipt digest does not recompute"
        );
        let artifact_budget_receipt: ArtifactBudgetReceipt =
            serde_json::from_slice(&artifact_budget_bytes)
                .context("artifact budget receipt has an invalid shape")?;
        bindings.push((
            architecture.clone(),
            "artifact-budgets".to_owned(),
            EvidenceKind::B1ArtifactBudgets,
            artifact_budget_path.clone(),
            artifact_budget.sha256,
        ));
        for (name, reference) in artifact_budget_receipt.retained_artifacts {
            validate_relative_path(&reference.path)?;
            let retained_path = artifact_budget_path
                .parent()
                .context("artifact budget receipt path has no parent")?
                .join(reference.path);
            ensure!(
                artifacts.insert(retained_path.clone()),
                "performance packages alias one retained artifact path"
            );
            bindings.push((
                architecture.clone(),
                name,
                EvidenceKind::BuiltArtifact,
                retained_path,
                reference.sha256,
            ));
        }
    }
    Ok(bindings)
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
    body: Body,
) -> anyhow::Result<()> {
    let source = current_source_binding(root).ok();
    let body = body.as_str();
    let candidate = serde_json::json!({
        "schema_version": format!("fasti.{}.milestone-candidate.v1", body.to_lowercase()),
        "kind": format!("fasti.{}.milestone-candidate", body.to_lowercase()),
        "status": "incomplete",
        "source": source.map(|value| serde_json::json!({
            "git_commit": value.git_commit,
            "git_tree": value.git_tree,
        })),
        "blocking_reason": format!("{error:#}"),
        "declaration": format!("This candidate is diagnostic only. It is not a passing evidence manifest and cannot satisfy the {body} milestone gate.")
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

fn sha256_reader(reader: &mut impl Read, label: &str) -> anyhow::Result<(String, u64)> {
    let mut digest = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 8192];
    loop {
        let count = reader
            .read(&mut buffer)
            .with_context(|| format!("failed to hash {label}"))?;
        if count == 0 {
            break;
        }
        bytes = bytes
            .checked_add(count as u64)
            .with_context(|| format!("{label} size overflowed"))?;
        digest.update(&buffer[..count]);
    }
    Ok((format!("{:x}", digest.finalize()), bytes))
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

    fn performance_source() -> SourceBinding {
        SourceBinding {
            git_commit: "1".repeat(40),
            git_tree: "2".repeat(40),
            tree_state: "clean".to_owned(),
        }
    }

    fn gzip_fixture(bytes: &[u8]) -> Vec<u8> {
        let mut child = Command::new("gzip")
            .args(["-n", "-9"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("start fixture gzip");
        child
            .stdin
            .take()
            .expect("fixture gzip stdin")
            .write_all(bytes)
            .expect("write fixture gzip input");
        let output = child.wait_with_output().expect("finish fixture gzip");
        assert!(output.status.success());
        output.stdout
    }

    fn saved_oci_fixture(
        architecture: &str,
        source: &SourceBinding,
        valid_layer_identity: bool,
    ) -> (Vec<u8>, String, u64) {
        let docker_architecture = match architecture {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            _ => panic!("unsupported fixture architecture"),
        };
        let layer = vec![b'x'; 1000];
        let config = serde_json::to_vec(&serde_json::json!({
            "architecture": docker_architecture,
            "os": "linux",
            "config": {"Labels": {
                "org.opencontainers.image.revision": source.git_commit,
                "dev.scrobble.fasti.source.tree": source.git_tree,
                "dev.scrobble.fasti.contracts": "3".repeat(40),
                "dev.scrobble.fasti.build.recipe.sha256": sha256_bytes(b"recipe\n"),
                "dev.scrobble.fasti.build.context.archive.sha256": "6".repeat(64)
            }},
            "rootfs": {"type": "layers", "diff_ids": [format!(
                "sha256:{}",
                if valid_layer_identity { sha256_bytes(&layer) } else { "7".repeat(64) }
            )]}
        }))
        .expect("serialize OCI config");
        let image_id = format!("sha256:{}", sha256_bytes(&config));
        let config_path = format!("{}.json", image_id.trim_start_matches("sha256:"));
        let manifest = serde_json::to_vec(&serde_json::json!([{
            "Config": config_path,
            "RepoTags": ["fasti:test"],
            "Layers": ["layer/layer.tar"]
        }]))
        .expect("serialize OCI manifest");

        let mut archive = tar::Builder::new(Vec::new());
        for (path, bytes) in [
            ("manifest.json", manifest.as_slice()),
            (config_path.as_str(), config.as_slice()),
            ("layer/layer.tar", layer.as_slice()),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_mode(0o600);
            header.set_size(bytes.len() as u64);
            header.set_cksum();
            archive
                .append_data(&mut header, path, bytes)
                .expect("append OCI fixture entry");
        }
        let archive = archive.into_inner().expect("finish OCI fixture tar");
        (gzip_fixture(&archive), image_id, layer.len() as u64)
    }

    #[derive(Clone, Copy)]
    enum OciLayoutFixtureMutation {
        None,
        WrongImageId,
        StaleSource,
        BlobDigest,
        DescriptorSize,
        DiffId,
        RootfsType,
        WrongOs,
        DuplicateDirectRoot,
        IndexBlobDigest,
        ManifestBlobDigest,
        UnknownCompression,
        AmbiguousPlatform,
        HiddenDuplicatePlatform,
        Traversal,
        CompatibilityManifest,
        CompatibilityLayers,
    }

    struct SavedOciLayoutFixture {
        archive: Vec<u8>,
        image_id: String,
        config_image_id: String,
        unpacked_bytes: u64,
    }

    fn saved_oci_layout_fixture(
        receipt: &ArtifactBudgetReceipt,
        mutation: OciLayoutFixtureMutation,
    ) -> SavedOciLayoutFixture {
        let architecture = match receipt.runner.architecture.as_str() {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            _ => panic!("unsupported fixture architecture"),
        };
        let gzip_layer = b"gzip layer bytes";
        let plain_layer = b"plain layer bytes";
        let mut gzip_blob = gzip_fixture(gzip_layer);
        let gzip_digest = sha256_bytes(&gzip_blob);
        let plain_digest = sha256_bytes(plain_layer);
        let labels = serde_json::json!({
            "org.opencontainers.image.revision": if matches!(mutation, OciLayoutFixtureMutation::StaleSource) {
                "0".repeat(40)
            } else {
                receipt.source.git_commit.clone()
            },
            "dev.scrobble.fasti.source.tree": receipt.source.git_tree,
            "dev.scrobble.fasti.contracts": receipt.source.contract_ref,
            "dev.scrobble.fasti.build.recipe.sha256": receipt.source.build_recipe_sha256,
            "dev.scrobble.fasti.build.context.archive.sha256": receipt.source.build_context_archive_sha256,
        });
        let config = serde_json::to_vec(&serde_json::json!({
            "architecture": architecture,
            "os": if matches!(mutation, OciLayoutFixtureMutation::WrongOs) {
                "windows"
            } else {
                "linux"
            },
            "config": {"Labels": labels},
            "rootfs": {
                "type": if matches!(mutation, OciLayoutFixtureMutation::RootfsType) {
                    "not-layers"
                } else {
                    "layers"
                },
                "diff_ids": [
                format!("sha256:{}", if matches!(mutation, OciLayoutFixtureMutation::DiffId) {
                    "7".repeat(64)
                } else {
                    sha256_bytes(gzip_layer)
                }),
                format!("sha256:{}", sha256_bytes(plain_layer)),
            ]}
        }))
        .expect("serialize OCI layout config");
        let config_digest = sha256_bytes(&config);
        let gzip_media_type = if matches!(mutation, OciLayoutFixtureMutation::UnknownCompression) {
            "application/vnd.oci.image.layer.v1.tar+zstd"
        } else {
            "application/vnd.oci.image.layer.v1.tar+gzip"
        };
        let config_size = config.len() as u64
            + u64::from(matches!(mutation, OciLayoutFixtureMutation::DescriptorSize));
        let config_descriptor = serde_json::json!({
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "digest": format!("sha256:{config_digest}"),
            "size": config_size,
        });
        let layer_descriptors = serde_json::json!([
            {
                "mediaType": gzip_media_type,
                "digest": format!("sha256:{gzip_digest}"),
                "size": gzip_blob.len(),
            },
            {
                "mediaType": "application/vnd.oci.image.layer.v1.tar",
                "digest": format!("sha256:{plain_digest}"),
                "size": plain_layer.len(),
            },
        ]);
        let mut manifest = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "config": config_descriptor,
            "layers": layer_descriptors,
        }))
        .expect("serialize OCI layout manifest");
        let manifest_digest = sha256_bytes(&manifest);
        let manifest_descriptor = serde_json::json!({
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "digest": format!("sha256:{manifest_digest}"),
            "size": manifest.len(),
            "platform": {"architecture": architecture, "os": "linux"},
        });
        let mut graph_manifests = vec![manifest_descriptor];
        let mut extra_manifest = None;
        if matches!(
            mutation,
            OciLayoutFixtureMutation::AmbiguousPlatform
                | OciLayoutFixtureMutation::HiddenDuplicatePlatform
        ) {
            let bytes = serde_json::to_vec(&serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "config": config_descriptor,
                "layers": layer_descriptors,
                "annotations": {"fixture": "ambiguous"},
            }))
            .expect("serialize ambiguous manifest");
            let digest = sha256_bytes(&bytes);
            let extra_architecture =
                if matches!(mutation, OciLayoutFixtureMutation::HiddenDuplicatePlatform) {
                    if architecture == "amd64" {
                        "arm64"
                    } else {
                        "amd64"
                    }
                } else {
                    architecture
                };
            graph_manifests.push(serde_json::json!({
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "digest": format!("sha256:{digest}"),
                "size": bytes.len(),
                "platform": {"architecture": extra_architecture, "os": "linux"},
            }));
            extra_manifest = Some((digest, bytes));
        }
        let mut target = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.index.v1+json",
            "manifests": graph_manifests,
        }))
        .expect("serialize OCI target index");
        let target_digest = sha256_bytes(&target);
        let target_descriptor = serde_json::json!({
            "mediaType": "application/vnd.oci.image.index.v1+json",
            "digest": format!("sha256:{target_digest}"),
            "size": target.len(),
        });
        let index_manifests = if matches!(mutation, OciLayoutFixtureMutation::DuplicateDirectRoot) {
            vec![target_descriptor.clone(), target_descriptor]
        } else {
            vec![target_descriptor]
        };
        let index = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.index.v1+json",
            "manifests": index_manifests,
        }))
        .expect("serialize OCI layout index");
        let config_path = format!("blobs/sha256/{config_digest}");
        let gzip_path = format!("blobs/sha256/{gzip_digest}");
        let plain_path = format!("blobs/sha256/{plain_digest}");
        let compatibility_config =
            if matches!(mutation, OciLayoutFixtureMutation::CompatibilityManifest) {
                format!("blobs/sha256/{}", "f".repeat(64))
            } else {
                config_path.clone()
            };
        let compatibility_layers =
            if matches!(mutation, OciLayoutFixtureMutation::CompatibilityLayers) {
                vec![plain_path.clone(), gzip_path.clone()]
            } else {
                vec![gzip_path.clone(), plain_path.clone()]
            };
        let compatibility = serde_json::to_vec(&serde_json::json!([{
            "Config": compatibility_config,
            "RepoTags": ["fasti:test"],
            "Layers": compatibility_layers,
        }]))
        .expect("serialize Docker compatibility manifest");
        if matches!(mutation, OciLayoutFixtureMutation::IndexBlobDigest) {
            target[0] ^= 0xff;
        }
        if matches!(mutation, OciLayoutFixtureMutation::ManifestBlobDigest) {
            manifest[0] ^= 0xff;
        }
        if matches!(mutation, OciLayoutFixtureMutation::BlobDigest) {
            gzip_blob[0] ^= 0xff;
        }

        let mut members = vec![
            ("manifest.json".to_owned(), compatibility),
            (
                "oci-layout".to_owned(),
                br#"{"imageLayoutVersion":"1.0.0"}"#.to_vec(),
            ),
            ("index.json".to_owned(), index),
            (format!("blobs/sha256/{target_digest}"), target),
            (format!("blobs/sha256/{manifest_digest}"), manifest),
            (config_path, config),
            (gzip_path, gzip_blob),
            (plain_path, plain_layer.to_vec()),
        ];
        if let Some((digest, bytes)) = extra_manifest {
            members.push((format!("blobs/sha256/{digest}"), bytes));
        }
        let mut archive = tar::Builder::new(Vec::new());
        for (path, bytes) in members {
            let mut header = tar::Header::new_gnu();
            header.set_mode(0o600);
            header.set_size(bytes.len() as u64);
            header.set_cksum();
            archive
                .append_data(&mut header, path, bytes.as_slice())
                .expect("append OCI layout fixture entry");
        }
        if matches!(mutation, OciLayoutFixtureMutation::Traversal) {
            let bytes = b"escape";
            let mut header = tar::Header::new_gnu();
            header.as_mut_bytes()[..9].copy_from_slice(b"../escape");
            header.set_mode(0o600);
            header.set_size(bytes.len() as u64);
            header.set_cksum();
            archive
                .append(&header, bytes.as_slice())
                .expect("append traversal fixture entry");
        }
        let archive = archive.into_inner().expect("finish OCI layout fixture tar");
        SavedOciLayoutFixture {
            archive: gzip_fixture(&archive),
            image_id: format!("sha256:{target_digest}"),
            config_image_id: format!("sha256:{config_digest}"),
            unpacked_bytes: (gzip_layer.len() + plain_layer.len()) as u64,
        }
    }

    fn write_performance_fixture_with_layer_identity(
        root: &Path,
        architecture: &str,
        run: &str,
        valid_layer_identity: bool,
    ) -> (PathBuf, PathBuf) {
        fs::create_dir_all(root.join("benchmarks/b1")).expect("create budgets directory");
        fs::create_dir_all(root.join("scripts")).expect("create scripts directory");
        fs::write(
            root.join("benchmarks/b1/budgets.json"),
            serde_json::to_vec(&serde_json::json!({
                "memory_bytes": {
                    "idle_target": 67_108_864_u64,
                    "absolute_ceiling": 201_326_592_u64
                },
                "idle_cpu_percent_one_core": {"average": 0.5, "p95": 1.0},
                "timing_seconds": {
                    "idle_warmup": 1,
                    "idle_measurement": 2,
                    "sample_interval_ms": 1000
                },
                "artifact_bytes": {
                    "native_runtime_installed": 33_554_432_u64,
                    "native_archive_compressed": 20_971_520_u64,
                    "oci_image_compressed": 52_428_800_u64,
                    "oci_image_unpacked": 104_857_600_u64,
                    "contract_pack_compressed": 5_242_880_u64
                }
            }))
            .expect("serialize budgets"),
        )
        .expect("write budgets");
        fs::write(root.join("benchmarks/b1/Dockerfile"), b"recipe\n").expect("write build recipe");
        fs::write(root.join("scripts/bench-envelope.sh"), b"harness\n").expect("write harness");
        fs::write(root.join("scripts/bench-daemon-idle.sh"), b"workload\n")
            .expect("write workload");
        fs::write(root.join("scripts/benchmark-b1.py"), b"artifact harness\n")
            .expect("write artifact harness");

        let package = root
            .join("target/fasti-evidence/envelope")
            .join(architecture);
        fs::create_dir_all(package.join("artifacts")).expect("create artifact directory");
        let mut artifact = vec![0_u8; 20];
        artifact[..4].copy_from_slice(b"\x7fELF");
        artifact[4] = 2;
        artifact[5] = 1;
        artifact[6] = 1;
        artifact[18..20].copy_from_slice(
            &(match architecture {
                "x86_64" => 62_u16,
                "aarch64" => 183_u16,
                _ => panic!("unsupported fixture architecture"),
            })
            .to_le_bytes(),
        );
        let artifact_digest = sha256_bytes(&artifact);
        let artifact_relative = PathBuf::from(format!("artifacts/sha256-{artifact_digest}-fastid"));
        fs::write(package.join(&artifact_relative), &artifact).expect("write artifact");

        let source = performance_source();
        fs::create_dir_all(root.join(".fasti-verifier")).expect("create verifier directory");
        fs::write(
            root.join(VERIFIER_SOURCE_INPUTS_PATH),
            serde_json::to_vec(&VerifierSourceInputs {
                contract_ref: "3".repeat(40),
                build_context_archive_sha256: "6".repeat(64),
            })
            .expect("serialize verifier source inputs"),
        )
        .expect("write verifier source inputs");
        let artifact_budget_root = package.join("artifact-budgets");
        fs::create_dir_all(artifact_budget_root.join("artifacts/sha256"))
            .expect("create artifact budget directory");
        let (oci_bytes, oci_image_id, oci_image_bytes) =
            saved_oci_fixture(architecture, &source, valid_layer_identity);
        let contract_bytes = b"compressed contract pack";
        let oci_digest = sha256_bytes(&oci_bytes);
        let contract_digest = sha256_bytes(contract_bytes);
        let oci_path = format!("artifacts/sha256/{oci_digest}.tar.gz");
        let contract_path = format!("artifacts/sha256/{contract_digest}.tar.gz");
        fs::write(artifact_budget_root.join(&oci_path), &oci_bytes).expect("write OCI artifact");
        fs::write(artifact_budget_root.join(&contract_path), contract_bytes)
            .expect("write contract artifact");
        let artifact_budget = serde_json::json!({
            "schema_version": "fasti.b1.artifact-budgets.v1",
            "kind": "fasti.b1.artifact-budgets",
            "status": "pass",
            "source": {
                "git_commit": source.git_commit,
                "git_tree": source.git_tree,
                "contract_ref": "3".repeat(40),
                "build_recipe_sha256": sha256_bytes(b"recipe\n"),
                "build_context_archive_sha256": "6".repeat(64),
                "dirty": false
            },
            "runner": {"architecture": architecture},
            "policy": {
                "budgets_sha256": sha256_bytes(&fs::read(root.join("benchmarks/b1/budgets.json")).unwrap()),
                "harness_sha256": sha256_bytes(b"artifact harness\n")
            },
            "oci_image_id": oci_image_id,
            "artifact_sizes": {
                "native_fastid_binary_bytes": artifact.len(),
                "oci_fastid_binary_bytes": artifact.len(),
                "oci_fasti_cli_binary_bytes": artifact.len(),
                "oci_image_bytes": oci_image_bytes,
                "native_runtime_installed_bytes": null,
                "native_archive_compressed_bytes": null,
                "oci_image_compressed_bytes": oci_bytes.len(),
                "oci_image_compressed_sha256": oci_digest,
                "contract_pack_compressed_bytes": contract_bytes.len(),
                "contract_pack_compressed_sha256": contract_digest
            },
            "artifact_budget_verdicts": [
                {"budget": "native_runtime_installed", "limit_bytes": 33_554_432_u64, "measured_bytes": null, "status": "not_applicable", "reason": "fixture"},
                {"budget": "native_archive_compressed", "limit_bytes": 20_971_520_u64, "measured_bytes": null, "status": "not_applicable", "reason": "fixture"},
                {"budget": "oci_image_compressed", "limit_bytes": 52_428_800_u64, "measured_bytes": oci_bytes.len(), "status": "pass", "reason": "fixture"},
                {"budget": "oci_image_unpacked", "limit_bytes": 104_857_600_u64, "measured_bytes": oci_image_bytes, "status": "pass", "reason": "fixture"},
                {"budget": "contract_pack_compressed", "limit_bytes": 5_242_880_u64, "measured_bytes": contract_bytes.len(), "status": "pass", "reason": "fixture"}
            ],
            "retained_artifacts": {
                "oci_image_compressed": {"path": oci_path, "sha256": oci_digest, "size_bytes": oci_bytes.len()},
                "contract_pack_compressed": {"path": contract_path, "sha256": contract_digest, "size_bytes": contract_bytes.len()}
            },
            "commands": ["fixture artifact capture"]
        });
        let artifact_budget_bytes =
            serde_json::to_vec(&artifact_budget).expect("serialize artifact budget");
        fs::write(
            artifact_budget_root.join("evidence.json"),
            &artifact_budget_bytes,
        )
        .expect("write artifact budget receipt");
        let receipt = serde_json::json!({
            "schema_version": "fasti.b1.performance-envelope.v1",
            "kind": "fasti.b1.performance-envelope",
            "status": "pass",
            "source": {
                "git_commit": source.git_commit,
                "git_tree": source.git_tree,
                "dirty": false
            },
            "ci": {
                "provider": "github_actions",
                "repository": "Scrobble-dev/Fasti",
                "workflow_ref": "Scrobble-dev/Fasti/.github/workflows/ci.yml@refs/heads/dev",
                "workflow_sha": source.git_commit,
                "event": "push",
                "ref": "refs/heads/dev",
                "run": run,
                "run_attempt": "1",
                "job": "low-hardware-envelope"
            },
            "runner": {
                "architecture": architecture,
                "kernel_release": "test-kernel",
                "cgroup_version": "v2"
            },
            "envelope": {
                "memory_max_bytes": 201_326_592_u64,
                "memory_swap_max_bytes": 0,
                "cpu_quota_micros": 100_000,
                "cpu_period_micros": 100_000,
                "memory_swap_peak_bytes": 0,
                "oom_event_count": 0
            },
            "measurement": {
                "profile": "canonical_idle_v1",
                "target": "idle",
                "budget_bytes": 67_108_864_u64,
                "peak_memory_bytes": 5_000_000,
                "steady_memory_peak_bytes": 5_000_000,
                "warmup_seconds": 1,
                "measurement_seconds": 2,
                "sample_interval_ms": 1000,
                "max_sample_lateness_ns": MAX_SAMPLE_LATENESS_NS,
                "actual_warmup_ns": 1_000_000_000_u64,
                "actual_measurement_ns": 2_000_000_000_u64,
                "cpu_average_basis_points": 10,
                "cpu_p95_basis_points": 10,
                "observations": [
                    {
                        "sequence": 1,
                        "elapsed_ns": 1_000_000_000_u64,
                        "interval_ns": 1_000_000_000_u64,
                        "memory_current_bytes": 5_000_000,
                        "cpu_usage_delta_micros": 1000,
                        "cpu_basis_points": 10
                    },
                    {
                        "sequence": 2,
                        "elapsed_ns": 2_000_000_000_u64,
                        "interval_ns": 1_000_000_000_u64,
                        "memory_current_bytes": 4_000_000,
                        "cpu_usage_delta_micros": 1000,
                        "cpu_basis_points": 10
                    }
                ],
                "network_isolation": "route_less_user_network_namespace",
                "command_exit_code": 0,
                "command": ["bash", "scripts/bench-daemon-idle.sh", "target/release/fastid"]
            },
            "policy": {
                "budgets_sha256": sha256_bytes(&fs::read(root.join("benchmarks/b1/budgets.json")).expect("read budgets")),
                "harness_sha256": sha256_bytes(b"harness\n"),
                "workload_sha256": sha256_bytes(b"workload\n")
            },
            "artifact": {
                "source_path": "target/release/fastid",
                "path": artifact_relative,
                "sha256": artifact_digest,
                "size_bytes": artifact.len(),
                "build_profile": "release"
            },
            "artifact_budget_receipt": {
                "path": "artifact-budgets/evidence.json",
                "sha256": sha256_bytes(&artifact_budget_bytes)
            }
        });
        let receipt_path = PathBuf::from(format!(
            "target/fasti-evidence/envelope/{architecture}/receipt.json"
        ));
        fs::write(
            root.join(&receipt_path),
            serde_json::to_vec(&receipt).expect("serialize receipt"),
        )
        .expect("write receipt");
        (
            receipt_path,
            PathBuf::from(format!(
                "target/fasti-evidence/envelope/{architecture}/{}",
                receipt["artifact"]["path"].as_str().expect("artifact path")
            )),
        )
    }

    fn write_performance_fixture(root: &Path, architecture: &str, run: &str) -> (PathBuf, PathBuf) {
        write_performance_fixture_with_layer_identity(root, architecture, run, true)
    }

    fn mutate_artifact_budget_fixture(
        root: &Path,
        receipt_path: &Path,
        mutation: impl FnOnce(&mut Value),
    ) {
        let receipt_file = root.join(receipt_path);
        let mut receipt = read_json(receipt_file.clone()).expect("read envelope receipt");
        let artifact_budget_path = receipt_file.parent().expect("receipt parent").join(
            receipt["artifact_budget_receipt"]["path"]
                .as_str()
                .expect("artifact budget path"),
        );
        let mut artifact_budget =
            read_json(artifact_budget_path.clone()).expect("read artifact budget receipt");
        mutation(&mut artifact_budget);
        let artifact_budget_bytes =
            serde_json::to_vec(&artifact_budget).expect("serialize artifact budget receipt");
        fs::write(artifact_budget_path, &artifact_budget_bytes)
            .expect("write artifact budget receipt");
        receipt["artifact_budget_receipt"]["sha256"] =
            Value::String(sha256_bytes(&artifact_budget_bytes));
        fs::write(
            receipt_file,
            serde_json::to_vec(&receipt).expect("serialize envelope receipt"),
        )
        .expect("write envelope receipt");
    }

    fn artifact_budget_fixture_receipt(root: &Path) -> ArtifactBudgetReceipt {
        let (receipt_path, _) = write_performance_fixture(root, "x86_64", "12345");
        let receipt_file = root.join(receipt_path);
        let receipt = read_json(receipt_file.clone()).expect("read envelope receipt");
        let artifact_budget_path = receipt_file.parent().expect("receipt parent").join(
            receipt["artifact_budget_receipt"]["path"]
                .as_str()
                .expect("artifact budget path"),
        );
        serde_json::from_slice(&fs::read(artifact_budget_path).expect("read artifact budget"))
            .expect("parse artifact budget")
    }

    #[test]
    fn performance_envelope_discovery_rejects_duplicate_architecture() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let (receipt_path, _) = write_performance_fixture(root.path(), "x86_64", "12345");
        let duplicate = root
            .path()
            .join("target/fasti-evidence/envelope/duplicate/receipt.json");
        fs::create_dir_all(duplicate.parent().expect("duplicate receipt parent"))
            .expect("create duplicate receipt directory");
        fs::copy(root.path().join(receipt_path), duplicate).expect("copy duplicate receipt");

        let error = performance_envelope_paths(root.path())
            .expect_err("duplicate architecture receipt must fail discovery");
        assert!(error.to_string().contains("more than one x86_64 receipt"));
    }

    #[test]
    fn performance_envelope_discovery_rejects_unexpected_architecture() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let (receipt_path, _) = write_performance_fixture(root.path(), "x86_64", "12345");
        let receipt_file = root.path().join(receipt_path);
        let mut receipt = read_json(receipt_file.clone()).expect("read receipt");
        receipt["runner"]["architecture"] = Value::String("riscv64".to_owned());
        fs::write(receipt_file, serde_json::to_vec(&receipt).unwrap()).expect("update receipt");

        let error = performance_envelope_paths(root.path())
            .expect_err("unexpected architecture must fail discovery");
        assert!(error.to_string().contains("unexpected architecture"));
    }

    #[test]
    fn performance_envelope_rejects_canonical_ci_identity_substitutions() {
        for (label, pointer, replacement) in [
            ("provider", "/ci/provider", "local".to_owned()),
            ("repository", "/ci/repository", "other/Fasti".to_owned()),
            (
                "workflow",
                "/ci/workflow_ref",
                "other/Fasti/.github/workflows/ci.yml@refs/heads/dev".to_owned(),
            ),
            ("workflow SHA", "/ci/workflow_sha", "0".repeat(40)),
            ("ref", "/ci/ref", "refs/pull/1/merge".to_owned()),
            ("job", "/ci/job", "untrusted-envelope".to_owned()),
        ] {
            let root = tempfile::tempdir().expect("temporary workspace");
            let (receipt_path, _) = write_performance_fixture(root.path(), "x86_64", "12345");
            let receipt_file = root.path().join(&receipt_path);
            let mut receipt = read_json(receipt_file.clone()).expect("read receipt");
            *receipt.pointer_mut(pointer).expect("fixture CI field") = Value::String(replacement);
            fs::write(receipt_file, serde_json::to_vec(&receipt).unwrap()).expect("update receipt");

            let error = validate_performance_envelope_receipt(
                root.path(),
                &receipt_path,
                &performance_source(),
            )
            .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("canonical exact-dev-push CI job"),
                "{label} substitution returned: {error:#}"
            );
        }
    }

    #[test]
    fn performance_envelope_rejects_kernel_control_substitutions() {
        for (label, pointer) in [
            ("swap limit", "/envelope/memory_swap_max_bytes"),
            ("swap peak", "/envelope/memory_swap_peak_bytes"),
            ("CPU quota", "/envelope/cpu_quota_micros"),
            ("CPU period", "/envelope/cpu_period_micros"),
            ("OOM event", "/envelope/oom_event_count"),
        ] {
            let root = tempfile::tempdir().expect("temporary workspace");
            let (receipt_path, _) = write_performance_fixture(root.path(), "x86_64", "12345");
            let receipt_file = root.path().join(&receipt_path);
            let mut receipt = read_json(receipt_file.clone()).expect("read receipt");
            *receipt
                .pointer_mut(pointer)
                .expect("fixture envelope field") = Value::from(1_u64);
            fs::write(receipt_file, serde_json::to_vec(&receipt).unwrap()).expect("update receipt");

            let error = validate_performance_envelope_receipt(
                root.path(),
                &receipt_path,
                &performance_source(),
            )
            .unwrap_err();
            assert!(
                error.to_string().contains("zero-swap limits cleanly"),
                "{label} substitution returned: {error:#}"
            );
        }
    }

    #[test]
    fn performance_envelope_rejects_idle_aggregate_and_budget_substitutions() {
        for mutation in [
            "steady peak",
            "CPU average",
            "CPU p95",
            "steady memory budget",
            "CPU aggregate budget",
        ] {
            let root = tempfile::tempdir().expect("temporary workspace");
            let (receipt_path, _) = write_performance_fixture(root.path(), "x86_64", "12345");
            let receipt_file = root.path().join(&receipt_path);
            let mut receipt = read_json(receipt_file.clone()).expect("read receipt");
            match mutation {
                "steady peak" => {
                    receipt["measurement"]["steady_memory_peak_bytes"] = Value::from(4_000_000_u64);
                }
                "CPU average" => {
                    receipt["measurement"]["cpu_average_basis_points"] = Value::from(11_u64);
                }
                "CPU p95" => {
                    receipt["measurement"]["cpu_p95_basis_points"] = Value::from(11_u64);
                }
                "steady memory budget" => {
                    receipt["measurement"]["observations"][0]["memory_current_bytes"] =
                        Value::from(67_108_865_u64);
                    receipt["measurement"]["steady_memory_peak_bytes"] =
                        Value::from(67_108_865_u64);
                }
                "CPU aggregate budget" => {
                    for observation in receipt["measurement"]["observations"]
                        .as_array_mut()
                        .expect("fixture observations")
                    {
                        observation["cpu_usage_delta_micros"] = Value::from(20_000_u64);
                        observation["cpu_basis_points"] = Value::from(200_u64);
                    }
                    receipt["measurement"]["cpu_average_basis_points"] = Value::from(200_u64);
                    receipt["measurement"]["cpu_p95_basis_points"] = Value::from(200_u64);
                }
                _ => unreachable!(),
            }
            fs::write(receipt_file, serde_json::to_vec(&receipt).unwrap()).expect("update receipt");

            let error = validate_performance_envelope_receipt(
                root.path(),
                &receipt_path,
                &performance_source(),
            )
            .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("idle memory or CPU verdict does not recompute or exceeds policy"),
                "{mutation} returned: {error:#}"
            );
        }
    }

    #[test]
    fn artifact_budget_rejects_policy_and_command_mutations() {
        for (mutation, expected) in [
            ("budgets policy", "policy digests"),
            ("harness policy", "policy digests"),
            ("missing commands", "command evidence"),
            ("blank command", "command evidence"),
        ] {
            let root = tempfile::tempdir().expect("temporary workspace");
            let (receipt_path, _) = write_performance_fixture(root.path(), "x86_64", "12345");
            mutate_artifact_budget_fixture(root.path(), &receipt_path, |receipt| match mutation {
                "budgets policy" => {
                    receipt["policy"]["budgets_sha256"] = Value::String("0".repeat(64));
                }
                "harness policy" => {
                    receipt["policy"]["harness_sha256"] = Value::String("0".repeat(64));
                }
                "missing commands" => receipt["commands"] = Value::Array(Vec::new()),
                "blank command" => receipt["commands"][0] = Value::String(String::new()),
                _ => unreachable!(),
            });

            let error = validate_performance_envelope_receipt(
                root.path(),
                &receipt_path,
                &performance_source(),
            )
            .unwrap_err();
            assert!(
                error.to_string().contains(expected),
                "{mutation} returned: {error:#}"
            );
        }
    }

    #[test]
    fn artifact_budget_rejects_verdict_mutations() {
        for mutation in [
            "unexpected budget",
            "duplicate budget",
            "wrong limit",
            "wrong measurement",
            "wrong status",
            "blank reason",
        ] {
            let root = tempfile::tempdir().expect("temporary workspace");
            let (receipt_path, _) = write_performance_fixture(root.path(), "x86_64", "12345");
            mutate_artifact_budget_fixture(root.path(), &receipt_path, |receipt| match mutation {
                "unexpected budget" => {
                    receipt["artifact_budget_verdicts"][0]["budget"] =
                        Value::String("unknown".to_owned());
                }
                "duplicate budget" => {
                    receipt["artifact_budget_verdicts"][1]["budget"] =
                        receipt["artifact_budget_verdicts"][0]["budget"].clone();
                }
                "wrong limit" => {
                    receipt["artifact_budget_verdicts"][0]["limit_bytes"] = Value::from(1_u64);
                }
                "wrong measurement" => {
                    receipt["artifact_budget_verdicts"][2]["measured_bytes"] = Value::from(1_u64);
                }
                "wrong status" => {
                    receipt["artifact_budget_verdicts"][2]["status"] =
                        Value::String("not_applicable".to_owned());
                }
                "blank reason" => {
                    receipt["artifact_budget_verdicts"][2]["reason"] = Value::String(String::new());
                }
                _ => unreachable!(),
            });

            let error = validate_performance_envelope_receipt(
                root.path(),
                &receipt_path,
                &performance_source(),
            )
            .unwrap_err();
            assert!(
                error.to_string().contains("artifact budget verdict"),
                "{mutation} returned: {error:#}"
            );
        }
    }

    #[test]
    fn performance_package_discovery_rejects_artifact_path_aliases() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let mut receipt_paths = BTreeMap::new();
        for architecture in ["aarch64", "x86_64"] {
            let (receipt_path, _) = write_performance_fixture(root.path(), architecture, "12345");
            receipt_paths.insert(architecture.to_owned(), receipt_path);
        }
        let x86_receipt_path = receipt_paths.get("x86_64").expect("x86 receipt");
        let x86_receipt_file = root.path().join(x86_receipt_path);
        let mut receipt = read_json(x86_receipt_file.clone()).expect("read x86 receipt");
        receipt["artifact_budget_receipt"]["path"] = receipt["artifact"]["path"].clone();
        fs::write(
            x86_receipt_file,
            serde_json::to_vec(&receipt).expect("serialize aliased receipt"),
        )
        .expect("write aliased receipt");

        let error = performance_envelope_artifacts(root.path(), &receipt_paths)
            .expect_err("artifact-budget receipt alias must fail");
        assert!(error
            .to_string()
            .contains("alias one artifact budget receipt path"));

        let root = tempfile::tempdir().expect("temporary workspace");
        let mut receipt_paths = BTreeMap::new();
        for architecture in ["aarch64", "x86_64"] {
            let (receipt_path, _) = write_performance_fixture(root.path(), architecture, "12345");
            receipt_paths.insert(architecture.to_owned(), receipt_path);
        }
        let x86_receipt_path = receipt_paths.get("x86_64").expect("x86 receipt");
        mutate_artifact_budget_fixture(root.path(), x86_receipt_path, |receipt| {
            receipt["retained_artifacts"]["oci_image_compressed"]["path"] =
                receipt["retained_artifacts"]["contract_pack_compressed"]["path"].clone();
        });

        let error = performance_envelope_artifacts(root.path(), &receipt_paths)
            .expect_err("retained artifact alias must fail");
        assert!(error
            .to_string()
            .contains("alias one retained artifact path"));
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
    fn performance_envelopes_require_exact_limits_architectures_run_and_artifacts() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let mut entries = Vec::new();
        let mut receipt_paths = BTreeMap::new();
        for architecture in ["aarch64", "x86_64"] {
            let (receipt_path, _) = write_performance_fixture(root.path(), architecture, "12345");
            receipt_paths.insert(architecture.to_owned(), receipt_path.clone());
            entries.push(EvidenceEntry {
                id: format!("receipt-{architecture}"),
                kind: EvidenceKind::B1PerformanceEnvelope,
                sha256: sha256_bytes(&fs::read(root.path().join(&receipt_path)).unwrap()),
                path: receipt_path,
                status: ResultStatus::Pass,
            });
        }
        for (architecture, label, kind, path, digest) in
            performance_envelope_artifacts(root.path(), &receipt_paths).unwrap()
        {
            entries.push(EvidenceEntry {
                id: format!("{architecture}-{label}"),
                kind,
                sha256: digest,
                path,
                status: ResultStatus::Pass,
            });
        }
        verify_performance_envelope_set(root.path(), &entries, &performance_source())
            .expect("valid two-architecture envelope set");

        let x86_path = root
            .path()
            .join("target/fasti-evidence/envelope/x86_64/receipt.json");
        let mut receipt = read_json(x86_path.clone()).expect("read x86 receipt");
        receipt["ci"]["event"] = Value::String("pull_request".to_owned());
        fs::write(&x86_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
        let error = verify_performance_envelope_set(root.path(), &entries, &performance_source())
            .expect_err("pull-request evidence fails");
        assert!(error.to_string().contains("exact-dev-push"));

        receipt["ci"]["event"] = Value::String("push".to_owned());
        receipt["ci"]["run"] = Value::String("99999".to_owned());
        fs::write(&x86_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
        let error = verify_performance_envelope_set(root.path(), &entries, &performance_source())
            .expect_err("mixed CI runs fail");
        assert!(error.to_string().contains("same CI run"));

        receipt["ci"]["run"] = Value::String("12345".to_owned());
        receipt["ci"]["run_attempt"] = Value::String("2".to_owned());
        fs::write(&x86_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
        let error = verify_performance_envelope_set(root.path(), &entries, &performance_source())
            .expect_err("mixed CI run attempts fail");
        assert!(error.to_string().contains("same CI run attempt"));

        receipt["ci"]["run_attempt"] = Value::String("1".to_owned());
        receipt["envelope"]["memory_max_bytes"] = Value::from(1);
        fs::write(&x86_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
        let error = verify_performance_envelope_set(root.path(), &entries, &performance_source())
            .expect_err("loose or substituted limits fail");
        assert!(error.to_string().contains("did not apply"));
    }

    #[test]
    fn performance_envelope_binary_architecture_must_match_runner() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let (receipt_path, artifact_path) =
            write_performance_fixture(root.path(), "x86_64", "12345");
        let mut artifact = fs::read(root.path().join(&artifact_path)).expect("read artifact");
        artifact[18..20].copy_from_slice(&183_u16.to_le_bytes());
        fs::write(root.path().join(&artifact_path), &artifact).expect("write wrong architecture");

        let receipt_file = root.path().join(&receipt_path);
        let mut receipt = read_json(receipt_file.clone()).expect("read receipt");
        receipt["artifact"]["sha256"] = Value::String(sha256_bytes(&artifact));
        fs::write(receipt_file, serde_json::to_vec(&receipt).unwrap()).expect("update receipt");

        let error = validate_performance_envelope_receipt(
            root.path(),
            &receipt_path,
            &performance_source(),
        )
        .expect_err("wrong executable architecture fails");
        assert!(error.to_string().contains("architecture does not match"));
    }

    #[test]
    fn artifact_budget_accepts_bound_oci_layout_descriptor_graph() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let mut receipt = artifact_budget_fixture_receipt(root.path());
        let fixture = saved_oci_layout_fixture(&receipt, OciLayoutFixtureMutation::None);
        receipt.oci_image_id = fixture.image_id;
        let archive_path = root.path().join("oci-layout.tar.gz");
        fs::write(&archive_path, fixture.archive).expect("write OCI layout fixture");

        assert_eq!(
            validate_saved_oci_archive(&archive_path, &receipt, 1024 * 1024)
                .expect("valid OCI layout descriptor graph"),
            fixture.unpacked_bytes
        );
        let budgets = read_json(root.path().join("benchmarks/b1/budgets.json"))
            .expect("read fixture budgets");
        let unpacked_policy_limit = budgets
            .pointer("/artifact_bytes/oci_image_unpacked")
            .and_then(Value::as_u64)
            .expect("fixture OCI unpacked policy limit");
        assert!(OCI_UNPACKED_SAFETY_CEILING_BYTES > unpacked_policy_limit);
        let error = validate_saved_oci_archive(&archive_path, &receipt, fixture.unpacked_bytes - 1)
            .expect_err("OCI layout must enforce its independent safety ceiling");
        assert!(error.to_string().contains("unpacked safety ceiling"));
    }

    #[test]
    fn artifact_budget_supports_config_digest_identity_without_ambiguity() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let mut receipt = artifact_budget_fixture_receipt(root.path());
        let fixture = saved_oci_layout_fixture(&receipt, OciLayoutFixtureMutation::None);
        receipt.oci_image_id = fixture.config_image_id;
        let archive_path = root.path().join("oci-layout.tar.gz");
        fs::write(&archive_path, fixture.archive).expect("write OCI layout fixture");
        assert_eq!(
            validate_saved_oci_archive(&archive_path, &receipt, 1024 * 1024)
                .expect("valid config-digest OCI identity"),
            fixture.unpacked_bytes
        );

        receipt.oci_image_id = format!("sha256:{}", "0".repeat(64));
        let error = validate_saved_oci_archive(&archive_path, &receipt, 1024 * 1024)
            .expect_err("unbound config digest must fail");
        assert!(error
            .to_string()
            .contains("does not bind the immutable image ID"));

        for (mutation, use_config_identity, expected) in [
            (
                OciLayoutFixtureMutation::AmbiguousPlatform,
                true,
                "exactly one selected platform manifest",
            ),
            (
                OciLayoutFixtureMutation::DuplicateDirectRoot,
                false,
                "duplicates the immutable image ID",
            ),
            (
                OciLayoutFixtureMutation::HiddenDuplicatePlatform,
                true,
                "exactly one selected platform manifest",
            ),
        ] {
            let fixture = saved_oci_layout_fixture(&receipt, mutation);
            receipt.oci_image_id = if use_config_identity {
                fixture.config_image_id
            } else {
                fixture.image_id
            };
            fs::write(&archive_path, fixture.archive).expect("write hostile OCI fixture");
            let error = validate_saved_oci_archive(&archive_path, &receipt, 1024 * 1024)
                .expect_err("ambiguous OCI identity must fail");
            assert!(
                error.to_string().contains(expected),
                "unexpected validation error: {error:#}"
            );
        }
    }

    #[test]
    fn artifact_budget_rejects_hostile_oci_layout_descriptor_graphs() {
        for (mutation, expected) in [
            (
                OciLayoutFixtureMutation::WrongImageId,
                "does not bind the immutable image ID",
            ),
            (
                OciLayoutFixtureMutation::StaleSource,
                "source label is stale",
            ),
            (OciLayoutFixtureMutation::BlobDigest, "digest or size"),
            (OciLayoutFixtureMutation::DescriptorSize, "digest or size"),
            (OciLayoutFixtureMutation::DiffId, "layer identities"),
            (OciLayoutFixtureMutation::RootfsType, "rootfs type"),
            (OciLayoutFixtureMutation::WrongOs, "not Linux"),
            (OciLayoutFixtureMutation::IndexBlobDigest, "digest or size"),
            (
                OciLayoutFixtureMutation::ManifestBlobDigest,
                "digest or size",
            ),
            (
                OciLayoutFixtureMutation::UnknownCompression,
                "unsupported compression",
            ),
            (
                OciLayoutFixtureMutation::AmbiguousPlatform,
                "exactly one selected platform manifest",
            ),
            (OciLayoutFixtureMutation::Traversal, "forbidden component"),
            (
                OciLayoutFixtureMutation::CompatibilityManifest,
                "descriptors match manifest.json",
            ),
            (
                OciLayoutFixtureMutation::CompatibilityLayers,
                "descriptors match manifest.json",
            ),
        ] {
            let root = tempfile::tempdir().expect("temporary workspace");
            let mut receipt = artifact_budget_fixture_receipt(root.path());
            let fixture = saved_oci_layout_fixture(&receipt, mutation);
            receipt.oci_image_id = if matches!(mutation, OciLayoutFixtureMutation::WrongImageId) {
                format!("sha256:{}", "0".repeat(64))
            } else {
                fixture.image_id
            };
            let archive_path = root.path().join("oci-layout.tar.gz");
            fs::write(&archive_path, fixture.archive).expect("write OCI layout fixture");

            let error = validate_saved_oci_archive(&archive_path, &receipt, 1024 * 1024)
                .expect_err("hostile OCI layout fixture must fail");
            assert!(
                error.to_string().contains(expected),
                "unexpected validation error: {error:#}"
            );
        }
    }

    #[test]
    fn artifact_budget_recomputes_unpacked_oci_bytes_from_retained_archive() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let (receipt_path, _) = write_performance_fixture(root.path(), "x86_64", "12345");
        let receipt_file = root.path().join(&receipt_path);
        let artifact_budget_file = receipt_file
            .parent()
            .expect("receipt parent")
            .join("artifact-budgets/evidence.json");
        let mut artifact_budget =
            read_json(artifact_budget_file.clone()).expect("read artifact budget receipt");
        artifact_budget["artifact_sizes"]["oci_image_bytes"] = Value::from(1_u64);
        artifact_budget["artifact_budget_verdicts"][3]["measured_bytes"] = Value::from(1_u64);
        let artifact_budget_bytes = serde_json::to_vec(&artifact_budget).unwrap();
        fs::write(&artifact_budget_file, &artifact_budget_bytes).unwrap();

        let mut receipt = read_json(receipt_file.clone()).expect("read envelope receipt");
        receipt["artifact_budget_receipt"]["sha256"] =
            Value::String(sha256_bytes(&artifact_budget_bytes));
        fs::write(&receipt_file, serde_json::to_vec(&receipt).unwrap()).unwrap();

        let error = validate_performance_envelope_receipt(
            root.path(),
            &receipt_path,
            &performance_source(),
        )
        .expect_err("substituted unpacked OCI size fails");
        assert!(error.to_string().contains("do not recompute"));
    }

    #[test]
    fn artifact_budget_binds_layer_bytes_to_image_diff_ids() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let (receipt_path, _) =
            write_performance_fixture_with_layer_identity(root.path(), "x86_64", "12345", false);
        let error = validate_performance_envelope_receipt(
            root.path(),
            &receipt_path,
            &performance_source(),
        )
        .expect_err("substituted OCI layer body fails");
        assert!(error.to_string().contains("layer identities do not match"));
    }

    #[test]
    fn artifact_budget_source_inputs_must_match_verifier_owned_values() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let (receipt_path, _) = write_performance_fixture(root.path(), "x86_64", "12345");
        let receipt_file = root.path().join(&receipt_path);
        let artifact_budget_file = receipt_file
            .parent()
            .expect("receipt parent")
            .join("artifact-budgets/evidence.json");
        let mut artifact_budget =
            read_json(artifact_budget_file.clone()).expect("read artifact budget receipt");
        artifact_budget["source"]["build_context_archive_sha256"] = Value::String("8".repeat(64));
        let artifact_budget_bytes = serde_json::to_vec(&artifact_budget).unwrap();
        fs::write(&artifact_budget_file, &artifact_budget_bytes).unwrap();
        let mut receipt = read_json(receipt_file.clone()).expect("read envelope receipt");
        receipt["artifact_budget_receipt"]["sha256"] =
            Value::String(sha256_bytes(&artifact_budget_bytes));
        fs::write(&receipt_file, serde_json::to_vec(&receipt).unwrap()).unwrap();

        let error = validate_performance_envelope_receipt(
            root.path(),
            &receipt_path,
            &performance_source(),
        )
        .expect_err("self-referential build context digest fails");
        assert!(error
            .to_string()
            .contains("does not match the bound source"));
    }

    #[test]
    fn performance_envelope_recomputes_canonical_idle_timing_and_cpu() {
        let root = tempfile::tempdir().expect("temporary workspace");
        let (receipt_path, _) = write_performance_fixture(root.path(), "x86_64", "12345");
        let receipt_file = root.path().join(&receipt_path);
        let original = read_json(receipt_file.clone()).expect("read receipt");

        let mut shortened = original.clone();
        shortened["measurement"]["actual_measurement_ns"] = Value::from(1_u64);
        fs::write(&receipt_file, serde_json::to_vec(&shortened).unwrap()).unwrap();
        let error = validate_performance_envelope_receipt(
            root.path(),
            &receipt_path,
            &performance_source(),
        )
        .expect_err("shortened canonical window fails");
        assert!(error.to_string().contains("canonical idle profile"));

        let mut late = original.clone();
        late["measurement"]["observations"][0]["elapsed_ns"] = Value::from(1_500_000_001_u64);
        late["measurement"]["observations"][0]["interval_ns"] = Value::from(1_500_000_001_u64);
        fs::write(&receipt_file, serde_json::to_vec(&late).unwrap()).unwrap();
        let error = validate_performance_envelope_receipt(
            root.path(),
            &receipt_path,
            &performance_source(),
        )
        .expect_err("late canonical sample fails");
        assert!(error.to_string().contains("observation sequence or timing"));

        let mut substituted_cpu = original;
        substituted_cpu["measurement"]["observations"][0]["cpu_usage_delta_micros"] =
            Value::from(100_000_u64);
        fs::write(receipt_file, serde_json::to_vec(&substituted_cpu).unwrap()).unwrap();
        let error = validate_performance_envelope_receipt(
            root.path(),
            &receipt_path,
            &performance_source(),
        )
        .expect_err("substituted CPU sample fails");
        assert!(error.to_string().contains("CPU result does not recompute"));
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
    fn evidence_snapshot_includes_artifact_budget_harness() {
        let root = tempfile::tempdir().expect("temporary workspace");
        initialize_git(root.path());
        for relative in EVIDENCE_SUPPORT_FILES {
            let path = root.path().join(relative);
            fs::create_dir_all(path.parent().expect("support file parent"))
                .expect("create support file parent");
            fs::write(path, format!("fixture {relative}\n")).expect("write support file");
        }
        fs::create_dir_all(root.path().join("contracts")).expect("create contracts tree");
        fs::write(root.path().join("contracts/fixture.json"), b"{}\n")
            .expect("write contract fixture");
        for args in [
            vec!["add", "."],
            vec!["commit", "--quiet", "-m", "support files"],
        ] {
            assert!(Command::new("git")
                .args(args)
                .current_dir(root.path())
                .status()
                .expect("commit support files")
                .success());
        }
        let source = current_source_binding(root.path()).expect("current source");
        let snapshot = snapshot_evidence_files(root.path(), &source, &[])
            .expect("create verifier-owned snapshot");
        assert_eq!(
            fs::read(snapshot.path().join("scripts/benchmark-b1.py"))
                .expect("read snapshotted artifact harness"),
            b"fixture scripts/benchmark-b1.py\n"
        );
        let source_inputs: VerifierSourceInputs = serde_json::from_slice(
            &fs::read(snapshot.path().join(VERIFIER_SOURCE_INPUTS_PATH))
                .expect("read verifier source inputs"),
        )
        .expect("parse verifier source inputs");
        let archive = Command::new("git")
            .args(["archive", "--format=tar", &source.git_commit])
            .current_dir(root.path())
            .output()
            .expect("archive fixture commit");
        assert!(archive.status.success());
        assert_eq!(
            source_inputs.build_context_archive_sha256,
            sha256_bytes(&archive.stdout)
        );
        assert_eq!(
            source_inputs.contract_ref,
            git_output(
                root.path(),
                &["rev-parse", &format!("{}:contracts", source.git_commit)]
            )
            .expect("fixture contract tree")
        );
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
    fn b8b_milestone_requires_evidence_before_generation() {
        let root = tempfile::tempdir().expect("temporary workspace");
        initialize_git(root.path());
        let manifest_path = root.path().join("target/fasti-evidence/b8b-manifest.json");
        let error = create_b8b_milestone_manifest(root.path(), &manifest_path)
            .expect_err("missing evidence blocks generation");
        assert!(error.to_string().contains("manifest was not emitted"));
        assert!(!manifest_path.exists());
        let candidate_path = root
            .path()
            .join("target/fasti-evidence/b8b-incomplete-candidate.json");
        let candidate = read_json(candidate_path).expect("read incomplete candidate");
        assert_eq!(
            candidate.get("status").and_then(Value::as_str),
            Some("incomplete")
        );
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
    fn b1_milestone_requires_both_performance_envelope_architectures() {
        let root = tempfile::tempdir().expect("temporary workspace");
        write_performance_fixture(root.path(), "x86_64", "12345");
        let error = performance_envelope_paths(root.path())
            .expect_err("missing aarch64 receipt blocks milestone evidence");
        assert!(error.to_string().contains("exactly aarch64 and x86_64"));
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
