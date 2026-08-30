use anyhow::Context;
use clap::ValueEnum;
use schemars::{generate::SchemaSettings, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::net::IpAddr;
use std::path::{Component, Path, PathBuf};

const MANIFEST_API_VERSION: &str = "fasti.scrobble.dev/addons/v0.1";
const MANIFEST_SCHEMA_VERSION: &str = "0.1.0";
const FIXTURE_API_VERSION: &str = "fasti.scrobble.dev/addons/fixture/v0.1";
const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
const MAX_FIXTURE_BYTES: u64 = 8 * 1024 * 1024;
const GENERATED_INPUT_ROOTS: [&str; 3] = [
    "contracts/generated",
    "contracts/addons/generated",
    "packages/schemas",
];
const REQUIRED_DENIED_NETWORK_CLASSES: [&str; 7] = [
    "documentation",
    "link_local",
    "loopback",
    "multicast",
    "private",
    "reserved",
    "unspecified",
];

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub(crate) enum OutputFormat {
    #[default]
    Human,
    Json,
}

#[derive(Debug)]
pub(crate) enum CheckFailure {
    Validation(CheckProblem),
    Tool(anyhow::Error),
}

impl CheckFailure {
    pub(crate) const fn exit_code(&self) -> u8 {
        match self {
            Self::Validation(_) => 2,
            Self::Tool(_) => 1,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckProblem {
    code: &'static str,
    location: String,
    detail: &'static str,
    next_action: &'static str,
}

impl CheckProblem {
    fn validation(location: impl Into<String>, detail: &'static str) -> Self {
        Self {
            code: "validation_failed",
            location: location.into(),
            detail,
            next_action:
                "Correct the authored manifest or fixture and run the focused check again.",
        }
    }

    fn response(location: impl Into<String>, detail: &'static str) -> Self {
        Self {
            code: "provider_response_invalid",
            location: location.into(),
            detail,
            next_action: "Replace the fixture with a bounded response that matches the declared provider contract.",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckReport {
    status: &'static str,
    kind: &'static str,
    source: String,
    sha256: String,
    fixture_count: usize,
    fixture_sha256: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct FailureReport<'a> {
    status: &'static str,
    problem: &'a CheckProblem,
}

pub(crate) fn render_success(report: &CheckReport, output: OutputFormat) -> anyhow::Result<String> {
    match output {
        OutputFormat::Human => Ok(format!(
            "PASS: metadata provider manifest {} fixtures={} sha256={}\n",
            report.source, report.fixture_count, report.sha256
        )),
        OutputFormat::Json => serde_json::to_string(report)
            .map(|line| format!("{line}\n"))
            .context("failed to serialize the integration check report"),
    }
}

pub(crate) fn render_validation_failure(
    problem: &CheckProblem,
    output: OutputFormat,
) -> anyhow::Result<String> {
    match output {
        OutputFormat::Human => Ok(format!(
            "FAIL: {} at {}. {} Next action: {}\n",
            problem.code, problem.location, problem.detail, problem.next_action
        )),
        OutputFormat::Json => serde_json::to_string(&FailureReport {
            status: "fail",
            problem,
        })
        .map(|line| format!("{line}\n"))
        .context("failed to serialize the integration check failure"),
    }
}

pub(crate) fn provider_manifest_schema() -> anyhow::Result<Value> {
    let schema = SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<ProviderManifest>();
    serde_json::to_value(schema).context("provider manifest schema is not serializable")
}

pub(crate) fn check(workspace_root: &Path, input: &Path) -> Result<CheckReport, CheckFailure> {
    let workspace_root = workspace_root
        .canonicalize()
        .map_err(|error| CheckFailure::Tool(error.into()))?;
    let relative = resolve_input(&workspace_root, input)?;
    reject_generated_input(&relative)?;
    let path = workspace_root.join(&relative);
    let source = read_bounded_utf8(&path, MAX_MANIFEST_BYTES, "/")?;
    let manifest: ProviderManifest = serde_saphyr::from_str(&source).map_err(|_| {
        CheckFailure::Validation(CheckProblem::validation(
            "/",
            "The file is not a strict v0.1 metadata provider manifest.",
        ))
    })?;
    validate_manifest(&manifest)?;

    let mut fixture_sha256 = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for (case, reference) in manifest.conformance.fixtures.entries() {
        let fixture_relative = resolve_reference(
            &workspace_root,
            relative.parent().unwrap_or_else(|| Path::new("")),
            reference,
            &format!("/conformance/fixtures/{case}"),
        )?;
        reject_generated_input(&fixture_relative)?;
        if !seen.insert(fixture_relative.clone()) {
            return Err(CheckFailure::Validation(CheckProblem::validation(
                format!("/conformance/fixtures/{case}"),
                "Each required response case must use a distinct authored fixture.",
            )));
        }
        let fixture_path = workspace_root.join(&fixture_relative);
        let fixture_source = read_bounded_utf8(
            &fixture_path,
            manifest.limits.max_response_bytes.min(MAX_FIXTURE_BYTES),
            &format!("/conformance/fixtures/{case}"),
        )?;
        let fixture: ProviderFixture = serde_json::from_str(&fixture_source).map_err(|_| {
            CheckFailure::Validation(CheckProblem::response(
                format!("/conformance/fixtures/{case}"),
                "The fixture is not a strict provider-response fixture.",
            ))
        })?;
        validate_fixture(case, &fixture, &manifest)?;
        fixture_sha256.insert(case.to_owned(), sha256(fixture_source.as_bytes()));
    }

    Ok(CheckReport {
        status: "pass",
        kind: "metadata_source",
        source: path_display(&relative),
        sha256: sha256(source.as_bytes()),
        fixture_count: fixture_sha256.len(),
        fixture_sha256,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProviderManifest {
    api_version: String,
    kind: String,
    metadata: ProviderMetadata,
    compatibility: Compatibility,
    permissions: Permissions,
    authentication: Authentication,
    record_support: Vec<RecordSupport>,
    operations: BTreeMap<String, ProviderOperation>,
    normalization: Normalization,
    limits: Limits,
    errors: BTreeMap<String, ErrorMapping>,
    runtime: Runtime,
    conformance: Conformance,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProviderMetadata {
    id: String,
    name: String,
    version: String,
    status: String,
    description: String,
    licence: String,
    sources: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Compatibility {
    fasti: String,
    manifest_schema: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Permissions {
    network: NetworkPermission,
    secrets: Vec<String>,
    capabilities: Vec<String>,
    writes: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NetworkPermission {
    hosts: Vec<String>,
    schemes: Vec<String>,
    methods: Vec<String>,
    classes: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Authentication {
    #[serde(rename = "type")]
    kind: String,
    header: String,
    secret_ref: String,
    required: bool,
    sources: Vec<CredentialSource>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CredentialSource {
    environment: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RecordSupport {
    record_type: String,
    result_kind: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProviderOperation {
    method: String,
    path: String,
    query: BTreeMap<String, Value>,
    headers: BTreeMap<String, String>,
    items_path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Normalization {
    candidate: CandidateNormalization,
    local_record_write: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CandidateNormalization {
    provider: String,
    provider_id: String,
    title: String,
    kind: String,
    authors: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Limits {
    max_results: u64,
    max_query_bytes: u64,
    max_response_bytes: u64,
    timeout_seconds: u64,
    redirects: String,
    denied_network_classes: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ErrorMapping {
    code: String,
    user_action: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Runtime {
    trusted_hosts: Vec<String>,
    browser_enabled: bool,
    transport: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Conformance {
    fixtures: FixtureReferences,
    deterministic: Vec<String>,
    live_smoke: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FixtureReferences {
    success: String,
    empty: String,
    rate_limited: String,
    invalid_response: String,
}

impl FixtureReferences {
    fn entries(&self) -> [(&'static str, &str); 4] {
        [
            ("success", &self.success),
            ("empty", &self.empty),
            ("rate_limited", &self.rate_limited),
            ("invalid_response", &self.invalid_response),
        ]
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderFixture {
    api_version: String,
    kind: String,
    case: String,
    status: u16,
    headers: BTreeMap<String, String>,
    body: Value,
}

fn validate_manifest(manifest: &ProviderManifest) -> Result<(), CheckFailure> {
    require(
        manifest.api_version == MANIFEST_API_VERSION,
        "/api_version",
        "The manifest API version must be fasti.scrobble.dev/addons/v0.1.",
    )?;
    require(
        manifest.kind == "MetadataProvider",
        "/kind",
        "The focused M1 lane accepts MetadataProvider manifests only.",
    )?;
    require(
        valid_provider_id(&manifest.metadata.id),
        "/metadata/id",
        "The provider ID must use lowercase ASCII letters, digits, and single hyphens.",
    )?;
    require(
        nonempty(&manifest.metadata.name)
            && nonempty(&manifest.metadata.description)
            && nonempty(&manifest.metadata.licence),
        "/metadata",
        "Provider name, description, and licence disposition must be present.",
    )?;
    require(
        valid_version(&manifest.metadata.version),
        "/metadata/version",
        "The provider version must contain three numeric components.",
    )?;
    require(
        manifest.metadata.status == "review",
        "/metadata/status",
        "The first provider-authoring contract must remain in review status.",
    )?;
    require(
        !manifest.metadata.sources.is_empty()
            && manifest
                .metadata
                .sources
                .iter()
                .all(|source| safe_https_source(source)),
        "/metadata/sources",
        "At least one credential-free HTTPS primary source is required.",
    )?;
    require(
        nonempty(&manifest.compatibility.fasti)
            && manifest.compatibility.manifest_schema == MANIFEST_SCHEMA_VERSION,
        "/compatibility",
        "Fasti compatibility and manifest schema 0.1.0 must be declared.",
    )?;

    let network = &manifest.permissions.network;
    require(
        !network.hosts.is_empty() && network.hosts.iter().all(|host| safe_public_hostname(host)),
        "/permissions/network/hosts",
        "Network hosts must be explicit public DNS names without schemes, paths, or credentials.",
    )?;
    require(
        network.schemes == ["https"] && network.methods == ["GET"] && network.classes == ["public"],
        "/permissions/network",
        "Metadata-source network permission is limited to public HTTPS GET.",
    )?;
    require(
        manifest.permissions.secrets.len() == 1
            && valid_secret_name(&manifest.permissions.secrets[0]),
        "/permissions/secrets",
        "The M1 example must declare one bounded header credential.",
    )?;
    require(
        manifest.permissions.capabilities == ["metadata.search"]
            && manifest.permissions.writes.is_empty(),
        "/permissions",
        "The M1 example owns metadata.search and no provider write.",
    )?;

    let authentication = &manifest.authentication;
    require(
        authentication.kind == "header"
            && authentication.required
            && valid_header_name(&authentication.header)
            && authentication.secret_ref.starts_with("secret:providers/")
            && !authentication.secret_ref.contains(['?', '#']),
        "/authentication",
        "The credential must be a required named header backed by an opaque secret reference.",
    )?;
    require(
        authentication.sources.len() == 1
            && valid_environment_name(&authentication.sources[0].environment),
        "/authentication/sources",
        "The example must declare one uppercase environment fallback name.",
    )?;
    require(
        manifest.record_support.len() == 1
            && nonempty(&manifest.record_support[0].record_type)
            && manifest.record_support[0].record_type == manifest.record_support[0].result_kind,
        "/record_support",
        "The example must declare one source-neutral record/result grain.",
    )?;

    require(
        manifest.operations.len() == 1 && manifest.operations.contains_key("search"),
        "/operations",
        "The focused metadata-source lane requires exactly one search operation.",
    )?;
    let operation = &manifest.operations["search"];
    require(
        operation.method == "GET"
            && safe_absolute_path(&operation.path)
            && valid_json_path(&operation.items_path),
        "/operations/search",
        "Search must be a bounded GET with an absolute path and a simple response items path.",
    )?;
    require(
        !operation.query.is_empty()
            && operation
                .query
                .iter()
                .all(|(key, value)| safe_query_entry(key, value)),
        "/operations/search/query",
        "Query parameters must contain no credential names or secret placeholders.",
    )?;
    let secret = &manifest.permissions.secrets[0];
    require(
        operation.headers.len() == 1
            && operation.headers.get(&authentication.header)
                == Some(&format!("${{secret.{secret}}}")),
        "/operations/search/headers",
        "The declared secret must appear only in the configured request header.",
    )?;

    require(
        manifest.normalization.candidate.provider == manifest.metadata.id
            && manifest.normalization.candidate.kind == manifest.record_support[0].result_kind
            && valid_json_path(&manifest.normalization.candidate.provider_id)
            && valid_json_path(&manifest.normalization.candidate.title)
            && valid_json_path(&manifest.normalization.candidate.authors)
            && manifest.normalization.local_record_write == "none",
        "/normalization",
        "Normalization must preserve provider evidence, use declared grain, and perform no local Record write.",
    )?;

    require(
        (1..=100).contains(&manifest.limits.max_results)
            && (1..=4096).contains(&manifest.limits.max_query_bytes)
            && (1..=MAX_FIXTURE_BYTES).contains(&manifest.limits.max_response_bytes)
            && (1..=60).contains(&manifest.limits.timeout_seconds)
            && manifest.limits.redirects == "none",
        "/limits",
        "Request, response, timeout, result, and redirect limits must remain within the M1 hard bounds.",
    )?;
    let denied: BTreeSet<_> = manifest
        .limits
        .denied_network_classes
        .iter()
        .map(String::as_str)
        .collect();
    require(
        denied == REQUIRED_DENIED_NETWORK_CLASSES.into_iter().collect(),
        "/limits/denied_network_classes",
        "All unsafe network classes must be denied exactly once.",
    )?;

    for (key, expected) in [
        ("400", "provider_credential_invalid"),
        ("401", "provider_credential_invalid"),
        ("403", "provider_credential_invalid"),
        ("429", "provider_rate_limited"),
        ("invalid_response", "provider_response_invalid"),
    ] {
        require(
            manifest.errors.get(key).is_some_and(|mapping| {
                mapping.code == expected && nonempty(&mapping.user_action)
            }),
            format!("/errors/{key}"),
            "Every required provider failure must use its canonical underscore problem code and a safe action.",
        )?;
    }
    require(
        manifest.errors.len() == 5,
        "/errors",
        "The focused example must declare only the five governed error mappings.",
    )?;
    require(
        !manifest.runtime.trusted_hosts.is_empty()
            && !manifest.runtime.browser_enabled
            && manifest.runtime.transport == "tauri_ipc",
        "/runtime",
        "Provider execution must remain in a declared trusted host and out of the browser.",
    )?;
    require(
        !manifest.conformance.deterministic.is_empty()
            && manifest.conformance.live_smoke == "optional",
        "/conformance",
        "Deterministic checks are required and live smoke must remain optional.",
    )?;
    Ok(())
}

fn validate_fixture(
    expected_case: &str,
    fixture: &ProviderFixture,
    manifest: &ProviderManifest,
) -> Result<(), CheckFailure> {
    let location = format!("/conformance/fixtures/{expected_case}");
    require_response(
        fixture.api_version == FIXTURE_API_VERSION
            && fixture.kind == "MetadataProviderResponse"
            && fixture.case == expected_case,
        &location,
        "Fixture version, kind, and case must match the declared response slot.",
    )?;
    require_response(
        fixture
            .headers
            .get("content-type")
            .is_some_and(|value| value.eq_ignore_ascii_case("application/json")),
        &location,
        "Every response fixture must declare application/json content.",
    )?;

    match expected_case {
        "success" => {
            require_response(
                fixture.status == 200,
                &location,
                "Success must use HTTP 200.",
            )?;
            validate_success_body(&fixture.body, manifest).map_err(CheckFailure::Validation)?;
        }
        "empty" => {
            require_response(fixture.status == 200, &location, "Empty must use HTTP 200.")?;
            let items = json_path(&fixture.body, &manifest.operations["search"].items_path);
            require_response(
                items.is_none() || items.and_then(Value::as_array).is_some_and(Vec::is_empty),
                &location,
                "Empty must contain no candidate items and does not mean delete.",
            )?;
        }
        "rate_limited" => {
            require_response(
                fixture.status == 429 && manifest.errors["429"].code == "provider_rate_limited",
                &location,
                "Rate-limited must use HTTP 429 and provider_rate_limited.",
            )?;
        }
        "invalid_response" => {
            require_response(
                fixture.status == 200
                    && manifest.errors["invalid_response"].code
                        == "provider_response_invalid"
                    && validate_success_body(&fixture.body, manifest).is_err(),
                &location,
                "Invalid response must fail candidate normalization and map to provider_response_invalid.",
            )?;
        }
        _ => unreachable!("fixture references expose exactly four cases"),
    }
    Ok(())
}

fn validate_success_body(body: &Value, manifest: &ProviderManifest) -> Result<(), CheckProblem> {
    let operation = &manifest.operations["search"];
    let items = json_path(body, &operation.items_path)
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| {
            CheckProblem::response(
                "/conformance/fixtures/success/body",
                "Success must contain at least one item at the declared items path.",
            )
        })?;
    let candidate = &items[0];
    let mapping = &manifest.normalization.candidate;
    if json_path(candidate, &mapping.provider_id)
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
        || json_path(candidate, &mapping.title)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        || json_path(candidate, &mapping.authors)
            .and_then(Value::as_array)
            .is_none_or(|authors| {
                authors
                    .iter()
                    .any(|author| author.as_str().is_none_or(str::is_empty))
            })
    {
        return Err(CheckProblem::response(
            "/conformance/fixtures/success/body",
            "Success does not satisfy the declared provider ID, title, and authors mappings.",
        ));
    }
    Ok(())
}

fn require(
    condition: bool,
    location: impl Into<String>,
    detail: &'static str,
) -> Result<(), CheckFailure> {
    condition
        .then_some(())
        .ok_or_else(|| CheckFailure::Validation(CheckProblem::validation(location, detail)))
}

fn require_response(
    condition: bool,
    location: impl Into<String>,
    detail: &'static str,
) -> Result<(), CheckFailure> {
    condition
        .then_some(())
        .ok_or_else(|| CheckFailure::Validation(CheckProblem::response(location, detail)))
}

fn resolve_input(workspace_root: &Path, input: &Path) -> Result<PathBuf, CheckFailure> {
    let relative = if input.is_absolute() {
        input
            .strip_prefix(workspace_root)
            .map(PathBuf::from)
            .map_err(|_| {
                CheckFailure::Validation(CheckProblem::validation(
                    "/",
                    "The integration input must be an authored file inside this workspace.",
                ))
            })?
    } else {
        normalize_relative(Path::new(""), input, "/")?
    };
    reject_symlink_components(workspace_root, &relative, "/")?;
    let metadata = fs::metadata(workspace_root.join(&relative)).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            CheckFailure::Validation(CheckProblem::validation(
                "/",
                "The integration input does not exist.",
            ))
        } else {
            CheckFailure::Tool(error.into())
        }
    })?;
    require(
        metadata.is_file()
            && matches!(
                relative.extension().and_then(OsStr::to_str),
                Some("yaml" | "yml")
            ),
        "/",
        "The focused integration input must be one authored YAML provider manifest.",
    )?;
    Ok(relative)
}

fn resolve_reference(
    workspace_root: &Path,
    manifest_parent: &Path,
    reference: &str,
    location: &str,
) -> Result<PathBuf, CheckFailure> {
    require(
        !reference.is_empty() && reference.ends_with(".json"),
        location,
        "Fixture references must name authored JSON files.",
    )?;
    let relative = normalize_relative(manifest_parent, Path::new(reference), location)?;
    reject_symlink_components(workspace_root, &relative, location)?;
    Ok(relative)
}

fn normalize_relative(base: &Path, input: &Path, location: &str) -> Result<PathBuf, CheckFailure> {
    let mut normalized = base.to_path_buf();
    for component in input.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir if normalized.pop() => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(CheckFailure::Validation(CheckProblem::validation(
                    location,
                    "The path must remain inside the workspace and may not escape its authored subtree.",
                )));
            }
        }
    }
    Ok(normalized)
}

fn reject_symlink_components(
    workspace_root: &Path,
    relative: &Path,
    location: &str,
) -> Result<(), CheckFailure> {
    let mut current = workspace_root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err(CheckFailure::Validation(CheckProblem::validation(
                location,
                "The path must use normalized workspace-relative components.",
            )));
        };
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(CheckFailure::Validation(CheckProblem::validation(
                    location,
                    "Authored integration inputs and fixtures may not use symlinks.",
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(CheckFailure::Validation(CheckProblem::validation(
                    location,
                    "The referenced authored file does not exist.",
                )));
            }
            Err(error) => return Err(CheckFailure::Tool(error.into())),
        }
    }
    Ok(())
}

fn reject_generated_input(relative: &Path) -> Result<(), CheckFailure> {
    if GENERATED_INPUT_ROOTS
        .iter()
        .any(|root| relative.starts_with(root))
    {
        return Err(CheckFailure::Validation(CheckProblem::validation(
            "/",
            "Generated contract output is not an authored integration input.",
        )));
    }
    Ok(())
}

fn read_bounded_utf8(path: &Path, maximum: u64, location: &str) -> Result<String, CheckFailure> {
    let metadata = fs::metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            CheckFailure::Validation(CheckProblem::validation(
                location,
                "The referenced authored file does not exist.",
            ))
        } else {
            CheckFailure::Tool(error.into())
        }
    })?;
    require(
        metadata.len() <= maximum,
        location,
        "The authored file exceeds the bounded integration-check size.",
    )?;
    let bytes = fs::read(path).map_err(|error| CheckFailure::Tool(error.into()))?;
    String::from_utf8(bytes).map_err(|_| {
        CheckFailure::Validation(CheckProblem::validation(
            location,
            "The authored file must be UTF-8.",
        ))
    })
}

fn valid_provider_id(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_version(value: &str) -> bool {
    value.split('.').count() == 3
        && value
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn safe_https_source(value: &str) -> bool {
    value.starts_with("https://")
        && !value.contains('@')
        && !value.contains('#')
        && !value.contains(['\r', '\n'])
}

fn safe_public_hostname(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value.parse::<IpAddr>().is_err()
        && !value.eq_ignore_ascii_case("localhost")
        && value.contains('.')
        && !value.contains(['/', ':', '@', '?', '#', '\r', '\n'])
        && value.split('.').all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn valid_secret_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_environment_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_header_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn safe_absolute_path(value: &str) -> bool {
    value.starts_with('/')
        && !value.starts_with("//")
        && !value.contains("..")
        && !value.contains(['?', '#', '\r', '\n'])
}

fn safe_query_entry(key: &str, value: &Value) -> bool {
    let lower = key.to_ascii_lowercase();
    ![
        "key",
        "token",
        "secret",
        "password",
        "passwd",
        "credential",
        "signature",
        "access",
    ]
    .iter()
    .any(|denied| lower.contains(denied))
        && lower != "auth"
        && !lower.starts_with("auth_")
        && !lower.ends_with("_auth")
        && !lower.contains("authorization")
        && !value.to_string().contains("${secret.")
}

fn valid_json_path(value: &str) -> bool {
    value == "$"
        || value.strip_prefix("$.").is_some_and(|rest| {
            !rest.is_empty()
                && rest.split('.').all(|part| {
                    !part.is_empty()
                        && part
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                })
        })
}

fn json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path == "$" {
        return Some(value);
    }
    path.strip_prefix("$.")?
        .split('.')
        .try_fold(value, |current, part| current.get(part))
}

fn nonempty(value: &str) -> bool {
    !value.trim().is_empty()
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hexadecimal = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut hexadecimal, "{byte:02x}").expect("writing to String cannot fail");
    }
    hexadecimal
}

fn path_display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const SECRET_SENTINEL: &str = "do-not-print-this-secret";

    fn write_file(path: &Path, source: &str) {
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        let mut file = fs::File::create(path).expect("create file");
        file.write_all(source.as_bytes()).expect("write file");
    }

    fn fixture(case: &str, status: u16, body: &str) -> String {
        format!(
            r#"{{"api_version":"{FIXTURE_API_VERSION}","kind":"MetadataProviderResponse","case":"{case}","status":{status},"headers":{{"content-type":"application/json"}},"body":{body}}}"#
        )
    }

    fn manifest(query: &str) -> String {
        format!(
            r#"api_version: {MANIFEST_API_VERSION}
kind: MetadataProvider
metadata:
  id: example
  name: Example
  version: 0.1.0
  status: review
  description: Synthetic metadata source.
  licence: Synthetic fixtures only.
  sources: [https://example.com/docs]
compatibility:
  fasti: ">=0.1.0 <0.2.0"
  manifest_schema: {MANIFEST_SCHEMA_VERSION}
permissions:
  network:
    hosts: [api.example.com]
    schemes: [https]
    methods: [GET]
    classes: [public]
  secrets: [example_api_key]
  capabilities: [metadata.search]
  writes: []
authentication:
  type: header
  header: X-Example-Key
  secret_ref: secret:providers/example/api-key
  required: true
  sources:
    - environment: EXAMPLE_API_KEY
record_support:
  - record_type: book
    result_kind: book
operations:
  search:
    method: GET
    path: /search
    query:
      q: {query}
    headers:
      X-Example-Key: ${{secret.example_api_key}}
    items_path: $.items
normalization:
  candidate:
    provider: example
    provider_id: $.id
    title: $.title
    kind: book
    authors: $.authors
  local_record_write: none
limits:
  max_results: 10
  max_query_bytes: 256
  max_response_bytes: 2000000
  timeout_seconds: 15
  redirects: none
  denied_network_classes: [loopback, private, link_local, multicast, unspecified, documentation, reserved]
errors:
  "400": {{ code: provider_credential_invalid, user_action: Replace the credential. }}
  "401": {{ code: provider_credential_invalid, user_action: Replace the credential. }}
  "403": {{ code: provider_credential_invalid, user_action: Replace the credential. }}
  "429": {{ code: provider_rate_limited, user_action: Wait and retry. }}
  invalid_response: {{ code: provider_response_invalid, user_action: Retry later. }}
runtime:
  trusted_hosts: [tauri_desktop]
  browser_enabled: false
  transport: tauri_ipc
conformance:
  fixtures:
    success: fixtures/success.json
    empty: fixtures/empty.json
    rate_limited: fixtures/rate-limited.json
    invalid_response: fixtures/invalid-response.json
  deterministic: [normalizes-synthetic-candidate]
  live_smoke: optional
"#
        )
    }

    fn workspace(query: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("temporary workspace");
        write_file(&root.path().join("provider.yaml"), &manifest(query));
        write_file(
            &root.path().join("fixtures/success.json"),
            &fixture(
                "success",
                200,
                r#"{"items":[{"id":"synthetic-1","title":"Synthetic title","authors":["Fixture author"]}]}"#,
            ),
        );
        write_file(
            &root.path().join("fixtures/empty.json"),
            &fixture("empty", 200, r#"{"items":[]}"#),
        );
        write_file(
            &root.path().join("fixtures/rate-limited.json"),
            &fixture("rate_limited", 429, r#"{"error":"bounded fixture"}"#),
        );
        write_file(
            &root.path().join("fixtures/invalid-response.json"),
            &fixture("invalid_response", 200, r#"{"items":"invalid"}"#),
        );
        root
    }

    #[test]
    fn validates_one_manifest_and_exactly_four_offline_fixtures() {
        let root = workspace("${input.query}");
        let report = check(root.path(), Path::new("provider.yaml")).expect("valid contract");
        assert_eq!(report.fixture_count, 4);
        assert_eq!(report.fixture_sha256.len(), 4);
    }

    #[test]
    fn rejects_query_credentials_without_echoing_them() {
        let root = workspace(&format!("${{secret.{SECRET_SENTINEL}}}"));
        let failure = check(root.path(), Path::new("provider.yaml")).expect_err("secret fails");
        assert_eq!(failure.exit_code(), 2);
        let CheckFailure::Validation(problem) = failure else {
            panic!("expected validation failure");
        };
        let rendered = render_validation_failure(&problem, OutputFormat::Json).expect("JSON");
        assert!(!rendered.contains(SECRET_SENTINEL));
    }

    #[test]
    fn rejects_credential_shaped_query_parameter_names() {
        for key in [
            "api_key",
            "token",
            "secret",
            "password",
            "passwd",
            "auth",
            "authorization",
            "credential",
            "signature",
            "access_token",
        ] {
            assert!(!safe_query_entry(key, &Value::String("literal".to_owned())));
        }
        assert!(safe_query_entry(
            "q",
            &Value::String("${input.query}".to_owned())
        ));
        assert!(safe_query_entry(
            "author",
            &Value::String("${input.author}".to_owned())
        ));
    }

    #[test]
    fn rejects_generated_inputs() {
        let root = workspace("${input.query}");
        write_file(
            &root.path().join("contracts/addons/generated/provider.yaml"),
            &manifest("${input.query}"),
        );
        let failure = check(
            root.path(),
            Path::new("contracts/addons/generated/provider.yaml"),
        )
        .expect_err("generated input fails");
        assert_eq!(failure.exit_code(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_inputs() {
        use std::os::unix::fs::symlink;

        let root = workspace("${input.query}");
        symlink("provider.yaml", root.path().join("linked.yaml")).expect("create symlink");
        let failure = check(root.path(), Path::new("linked.yaml")).expect_err("symlink fails");
        assert_eq!(failure.exit_code(), 2);
    }

    #[test]
    fn dotted_problem_codes_fail_closed() {
        let root = workspace("${input.query}");
        let path = root.path().join("provider.yaml");
        let source = fs::read_to_string(&path)
            .expect("read manifest")
            .replace("provider_rate_limited", "provider.rate_limited");
        write_file(&path, &source);
        let failure = check(root.path(), Path::new("provider.yaml")).expect_err("dotted fails");
        assert_eq!(failure.exit_code(), 2);
    }

    #[test]
    fn malformed_fixture_is_validation_not_tool_failure() {
        let root = workspace("${input.query}");
        write_file(&root.path().join("fixtures/empty.json"), "not json");
        let failure = check(root.path(), Path::new("provider.yaml")).expect_err("fixture fails");
        assert_eq!(failure.exit_code(), 2);
    }

    #[test]
    fn tool_failures_use_exit_one() {
        let failure = CheckFailure::Tool(anyhow::anyhow!("synthetic environment failure"));
        assert_eq!(failure.exit_code(), 1);
    }
}
