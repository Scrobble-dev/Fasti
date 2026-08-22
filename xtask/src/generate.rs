use anyhow::{ensure, Context};
use fasti_contracts::{HealthResponse, ProblemDetails};
use schemars::{generate::SchemaSettings, JsonSchema};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use crate::registry;

pub(crate) type Artifacts = BTreeMap<PathBuf, Vec<u8>>;

const OPENAPI_PATH: &str = "contracts/generated/v1/openapi.json";
const CONFORMANCE_OPENAPI_PATH: &str = "contracts/generated/v1/conformance-openapi.json";
const CAPABILITY_REGISTRY_PATH: &str = "contracts/generated/v1/capabilities.json";
const HEALTH_SCHEMA_PATH: &str = "packages/schemas/schemas/health-response.json";
const PROBLEM_SCHEMA_PATH: &str = "packages/schemas/schemas/problem-details.json";
const SDK_GENERATED_PATH: &str = "packages/sdk/src/generated.ts";
const RUST_CAPABILITY_IDS_PATH: &str = "crates/fasti-contracts/src/generated_capability_ids.rs";
const ASYNCAPI_PATH: &str = "contracts/asyncapi/v1/transport.yaml";
const GENERATED_ONLY_DIRECTORIES: [&str; 2] =
    ["contracts/generated/v1", "packages/schemas/schemas"];

#[derive(Clone, Copy)]
struct ConformanceOperation {
    alias: &'static str,
    operation_id: &'static str,
    method: &'static str,
    path: &'static str,
    capability_id: &'static str,
    authenticated: bool,
    request: Option<&'static str>,
    response: Option<&'static str>,
    retry: &'static str,
}

const CONFORMANCE_OPERATIONS: [ConformanceOperation; 9] = [
    ConformanceOperation {
        alias: "discoverCapabilities",
        operation_id: "discover_capabilities",
        method: "get",
        path: "/api/v1/capabilities",
        capability_id: "system.capabilities.discover",
        authenticated: true,
        request: None,
        response: Some("CapabilityDiscoveryResponse"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "selectProfile",
        operation_id: "select_profile_unavailable",
        method: "put",
        path: "/api/v1/profile-selection",
        capability_id: "profile.select",
        authenticated: true,
        request: None,
        response: None,
        retry: "never",
    },
    ConformanceOperation {
        alias: "rotateCredential",
        operation_id: "rotate_credential_unavailable",
        method: "post",
        path: "/api/v1/credential-rotations",
        capability_id: "credential.rotate",
        authenticated: true,
        request: None,
        response: None,
        retry: "never",
    },
    ConformanceOperation {
        alias: "revokeCredential",
        operation_id: "revoke_credential_unavailable",
        method: "post",
        path: "/api/v1/credential-revocations",
        capability_id: "credential.revoke",
        authenticated: true,
        request: None,
        response: None,
        retry: "never",
    },
    ConformanceOperation {
        alias: "configureListener",
        operation_id: "configure_listener_unavailable",
        method: "put",
        path: "/api/v1/listener-configuration",
        capability_id: "listener.configure",
        authenticated: true,
        request: None,
        response: None,
        retry: "never",
    },
    ConformanceOperation {
        alias: "initializeNode",
        operation_id: "initialize_node",
        method: "post",
        path: "/api/v1/node/initialization",
        capability_id: "node.initialize",
        authenticated: false,
        request: Some("InitializeNodeRequest"),
        response: Some("InitializeNodeResponse"),
        retry: "never",
    },
    ConformanceOperation {
        alias: "enrollFirstClient",
        operation_id: "enroll_first_client",
        method: "post",
        path: "/api/v1/client-enrollments",
        capability_id: "client.enroll",
        authenticated: false,
        request: Some("EnrollFirstClientRequest"),
        response: Some("EnrollFirstClientResponse"),
        retry: "never",
    },
    ConformanceOperation {
        alias: "acceptObservation",
        operation_id: "accept_observation",
        method: "post",
        path: "/api/v1/observations",
        capability_id: "observation.accept",
        authenticated: true,
        request: Some("AcceptObservationRequest"),
        response: Some("AcceptObservationResponse"),
        retry: "stable_body_operation_id",
    },
    ConformanceOperation {
        alias: "replayReceipt",
        operation_id: "replay_receipt",
        method: "get",
        path: "/api/v1/receipts/{receipt_id}",
        capability_id: "receipt.replay",
        authenticated: true,
        request: None,
        response: Some("ReplayReceiptResponse"),
        retry: "safe",
    },
];

pub(crate) fn generate_checked_in(workspace_root: &Path) -> anyhow::Result<Artifacts> {
    generate_to(workspace_root, workspace_root)
}

pub(crate) fn generate_to(workspace_root: &Path, output_root: &Path) -> anyhow::Result<Artifacts> {
    let artifacts = build(workspace_root)?;
    write(output_root, &artifacts)?;
    Ok(artifacts)
}

pub(crate) fn verify_checked_in(
    workspace_root: &Path,
    generated: &Artifacts,
) -> anyhow::Result<()> {
    verify_inventory(workspace_root, generated)?;
    for (relative_path, expected) in generated {
        let checked_in_path = workspace_root.join(relative_path);
        let actual = fs::read(&checked_in_path).with_context(|| {
            format!(
                "generated artifact {} is absent; run `cargo xtask contract generate`",
                relative_path.display()
            )
        })?;
        ensure!(
            actual == *expected,
            "generated artifact {} is stale; run `cargo xtask contract generate`",
            relative_path.display()
        );
    }
    Ok(())
}

pub(crate) fn compare_outputs(
    first_root: &Path,
    second_root: &Path,
    first: &Artifacts,
    second: &Artifacts,
) -> anyhow::Result<()> {
    ensure!(
        first.keys().eq(second.keys()),
        "isolated contract generations produced different artifact inventories"
    );
    for relative_path in first.keys() {
        let first_bytes = fs::read(first_root.join(relative_path)).with_context(|| {
            format!(
                "first isolated generation omitted {}",
                relative_path.display()
            )
        })?;
        let second_bytes = fs::read(second_root.join(relative_path)).with_context(|| {
            format!(
                "second isolated generation omitted {}",
                relative_path.display()
            )
        })?;
        ensure!(
            first_bytes == second_bytes,
            "isolated contract generations differ at {}",
            relative_path.display()
        );
        ensure!(
            first.get(relative_path) == Some(&first_bytes)
                && second.get(relative_path) == Some(&second_bytes),
            "isolated generated bytes disagree with the in-memory artifact at {}",
            relative_path.display()
        );
    }
    Ok(())
}

fn build(workspace_root: &Path) -> anyhow::Result<Artifacts> {
    let mut artifacts = BTreeMap::new();
    let public_registry = registry::normalized_public_json(workspace_root)?;
    let health_schema = draft_2020_12_schema::<HealthResponse>()?;
    let problem_schema = draft_2020_12_schema::<ProblemDetails>()?;
    let asyncapi = load_yaml(workspace_root, ASYNCAPI_PATH)?;
    let mut conformance_openapi = serde_json::to_value(fasti_api::b1_conformance_openapi())
        .context("B1 conformance OpenAPI is not serializable")?;
    enrich_conformance_openapi(&mut conformance_openapi, &public_registry)?;
    insert(
        &mut artifacts,
        OPENAPI_PATH,
        serde_json::to_value(fasti_api::openapi()).context("OpenAPI is not serializable")?,
    )?;
    insert(
        &mut artifacts,
        CAPABILITY_REGISTRY_PATH,
        public_registry.clone(),
    )?;
    insert(
        &mut artifacts,
        CONFORMANCE_OPENAPI_PATH,
        conformance_openapi.clone(),
    )?;
    insert(&mut artifacts, HEALTH_SCHEMA_PATH, health_schema.clone())?;
    insert(&mut artifacts, PROBLEM_SCHEMA_PATH, problem_schema.clone())?;
    insert_bytes(
        &mut artifacts,
        SDK_GENERATED_PATH,
        typescript_sdk(
            &public_registry,
            &health_schema,
            &problem_schema,
            &asyncapi,
            &conformance_openapi,
        )?
        .into_bytes(),
    )?;
    insert_bytes(
        &mut artifacts,
        RUST_CAPABILITY_IDS_PATH,
        rust_capability_ids(workspace_root)?.into_bytes(),
    )?;
    Ok(artifacts)
}

fn insert(artifacts: &mut Artifacts, relative_path: &str, value: Value) -> anyhow::Result<()> {
    insert_bytes(artifacts, relative_path, pretty_json(value)?)
}

fn insert_bytes(
    artifacts: &mut Artifacts,
    relative_path: &str,
    bytes: Vec<u8>,
) -> anyhow::Result<()> {
    let path = PathBuf::from(relative_path);
    ensure!(
        bytes.ends_with(b"\n"),
        "generated artifact {} must end with one newline",
        path.display()
    );
    ensure!(
        artifacts.insert(path.clone(), bytes).is_none(),
        "duplicate generated artifact path {}",
        path.display()
    );
    Ok(())
}

fn draft_2020_12_schema<T: JsonSchema>() -> anyhow::Result<Value> {
    let schema = SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<T>();
    let value = serde_json::to_value(schema).context("JSON Schema is not serializable")?;
    ensure!(
        value.get("$schema").and_then(Value::as_str)
            == Some("https://json-schema.org/draft/2020-12/schema"),
        "generated JSON Schema is not explicitly Draft 2020-12"
    );
    Ok(value)
}

fn pretty_json(value: Value) -> anyhow::Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(&sort_json(value))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn enrich_conformance_openapi(openapi: &mut Value, public_registry: &Value) -> anyhow::Result<()> {
    let capabilities = array_at(public_registry, "/capabilities")?;
    for expected in CONFORMANCE_OPERATIONS {
        let capability = capabilities
            .iter()
            .find(|capability| string_at(capability, "/id").ok() == Some(expected.capability_id))
            .with_context(|| {
                format!(
                    "conformance operation {} references absent registry capability {}",
                    expected.operation_id, expected.capability_id
                )
            })?;
        let scopes = array_at(capability, "/scopes")?.to_vec();
        let runtime_availability =
            string_at(capability, "/lifecycle/runtime_availability")?.to_owned();
        let pointer = format!(
            "/paths/{}/{}",
            escape_pointer(expected.path),
            expected.method
        );
        let operation = openapi
            .pointer_mut(&pointer)
            .and_then(Value::as_object_mut)
            .with_context(|| {
                format!(
                    "conformance OpenAPI omits {} {}",
                    expected.method, expected.path
                )
            })?;
        operation.insert(
            "x-fasti-capability-id".to_owned(),
            Value::String(expected.capability_id.to_owned()),
        );
        operation.insert("x-fasti-required-scopes".to_owned(), Value::Array(scopes));
        operation.insert(
            "x-fasti-runtime-availability".to_owned(),
            Value::String(runtime_availability),
        );
    }
    Ok(())
}

fn sort_json(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let sorted: BTreeMap<_, _> = object
                .into_iter()
                .map(|(key, value)| (key, sort_json(value)))
                .collect();
            Value::Object(Map::from_iter(sorted))
        }
        Value::Array(values) => Value::Array(values.into_iter().map(sort_json).collect()),
        scalar => scalar,
    }
}

fn load_yaml(workspace_root: &Path, relative_path: &str) -> anyhow::Result<Value> {
    let path = workspace_root.join(relative_path);
    let source =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_saphyr::from_str(&source).with_context(|| format!("{} is not valid YAML", path.display()))
}

fn rust_capability_ids(workspace_root: &Path) -> anyhow::Result<String> {
    let pairs = registry::internal_key_id_pairs(workspace_root)?;
    ensure!(!pairs.is_empty(), "capability ID match cannot be empty");
    let mut output = String::from(
        "// This file is generated by `cargo xtask contract generate`. Do not edit.\n\nuse fasti_application::CapabilityKey;\n\n/// Returns the registry-owned public ID for one internal application key.\npub const fn public_capability_id(key: CapabilityKey) -> &'static str {\n    match key {\n",
    );
    for (key, public_id) in pairs {
        writeln!(
            output,
            "        CapabilityKey::{key:?} => {},",
            json_string(&public_id)?
        )?;
    }
    output.push_str("    }\n}\n");
    Ok(output)
}

fn typescript_sdk(
    public_registry: &Value,
    health_schema: &Value,
    problem_schema: &Value,
    asyncapi: &Value,
    conformance_openapi: &Value,
) -> anyhow::Result<String> {
    let mut output = String::from(
        "/* This file is generated by `cargo xtask contract generate`. Do not edit. */\n\n",
    );
    output.push_str(&render_interface("HealthResponse", health_schema)?);
    output.push('\n');

    let problem_definitions = object_at(problem_schema, "/$defs")?;
    for definition_name in ["ProblemActionDto", "ViolationDto"] {
        let definition = problem_definitions
            .get(definition_name)
            .with_context(|| format!("ProblemDetails schema omits $defs/{definition_name}"))?;
        output.push_str(&render_interface(definition_name, definition)?);
        output.push('\n');
    }
    output.push_str(&render_interface_with_overrides(
        "ProblemDetails",
        problem_schema,
        &[("capability_id", "CapabilityId"), ("code", "ProblemCode")],
    )?);
    output.push('\n');

    let receipt_schema = value_at(
        asyncapi,
        "/components/messages/receiptCommitted/payload/schema",
    )?;
    output.push_str(&render_interface("ReceiptCommittedEvent", receipt_schema)?);
    output.push('\n');
    output.push_str(
        "export interface ReceiptCommittedEnvelope {\n  readonly id: string;\n  readonly event: \"receiptCommitted\";\n  readonly data: ReceiptCommittedEvent;\n}\n\n",
    );

    output.push_str(&render_conformance_contract(conformance_openapi)?);

    let capabilities = array_at(public_registry, "/capabilities")?;
    let mut capability_ids = BTreeSet::new();
    let mut problem_codes = BTreeSet::new();
    let mut runtime_availabilities = BTreeSet::new();
    let mut contract_states = BTreeSet::new();
    let mut bodies = BTreeSet::new();
    for capability in capabilities {
        let id = string_at(capability, "/id")?;
        let contract_body = string_at(capability, "/contract_body")?;
        let runtime_body = string_at(capability, "/runtime_body")?;
        let contract_state = string_at(capability, "/lifecycle/contract_state")?;
        let runtime_availability = string_at(capability, "/lifecycle/runtime_availability")?;
        capability_ids.insert(id.to_owned());
        bodies.insert(contract_body.to_owned());
        bodies.insert(runtime_body.to_owned());
        contract_states.insert(contract_state.to_owned());
        runtime_availabilities.insert(runtime_availability.to_owned());
        for code in array_at(capability, "/problems")? {
            problem_codes.insert(
                code.as_str()
                    .context("capability problem code must be a string")?
                    .to_owned(),
            );
        }
    }

    render_string_union(&mut output, "CapabilityId", &capability_ids)?;
    render_string_union(&mut output, "CapabilityBody", &bodies)?;
    render_string_union(&mut output, "ContractState", &contract_states)?;
    render_string_union(&mut output, "RuntimeAvailability", &runtime_availabilities)?;
    render_string_union(&mut output, "ProblemCode", &problem_codes)?;
    let public_registry_json = serde_json::to_string_pretty(&sort_json(public_registry.clone()))?;
    writeln!(
        output,
        "// prettier-ignore\nexport const PUBLIC_CAPABILITY_REGISTRY = {public_registry_json} as const;\n"
    )?;
    output.push_str(
        "export const CAPABILITY_REGISTRY = PUBLIC_CAPABILITY_REGISTRY.capabilities;\nexport const SURFACE_PROFILES = PUBLIC_CAPABILITY_REGISTRY.surface_profiles;\nexport type CapabilityMetadata = (typeof CAPABILITY_REGISTRY)[number];\nexport type SurfaceProfileMetadata = typeof SURFACE_PROFILES;\n\n",
    );

    let stream_path = string_at(asyncapi, "/channels/receiptEvents/address")?;
    let event_name = string_at(asyncapi, "/components/messages/receiptCommitted/name")?;
    ensure!(
        event_name == "receiptCommitted",
        "receipt event name changed; update the generated envelope contract deliberately"
    );
    let capability_id = string_at(
        asyncapi,
        "/operations/sendReceiptCommitted/x-fasti-capability-id",
    )?;
    ensure!(
        capability_ids.contains(capability_id),
        "AsyncAPI receipt capability {capability_id} is absent from the public registry"
    );
    let async_scopes = array_at(
        asyncapi,
        "/operations/sendReceiptCommitted/x-fasti-required-scopes",
    )?
    .iter()
    .map(|value| {
        value
            .as_str()
            .context("receipt stream scope must be a string")
            .map(ToOwned::to_owned)
    })
    .collect::<anyhow::Result<BTreeSet<_>>>()?;
    ensure!(
        !async_scopes.is_empty(),
        "receipt stream must declare required scopes"
    );
    let registry_capability = capabilities
        .iter()
        .find(|capability| string_at(capability, "/id").ok() == Some(capability_id))
        .context("AsyncAPI receipt capability is absent from the public registry")?;
    let registry_scopes = array_at(registry_capability, "/scopes")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .context("registry capability scope must be a string")
                .map(ToOwned::to_owned)
        })
        .collect::<anyhow::Result<BTreeSet<_>>>()?;
    ensure!(
        async_scopes == registry_scopes,
        "AsyncAPI receipt scopes must exactly equal the registry-owned scope set"
    );
    let scopes_json = serde_json::to_string(&async_scopes)?;
    let maximum_replay = u64_at(
        asyncapi,
        "/operations/sendReceiptCommitted/x-fasti-replay/maximumBatch",
    )?;
    let retry_policy = string_at(
        asyncapi,
        "/operations/sendReceiptCommitted/x-fasti-replay/retryPolicy",
    )?;
    let runtime_availability = string_at(
        asyncapi,
        "/operations/sendReceiptCommitted/x-fasti-runtime-availability",
    )?;
    ensure!(
        runtime_availabilities.contains(runtime_availability),
        "AsyncAPI runtime availability is absent from the registry vocabulary"
    );
    writeln!(
        output,
        "export const RECEIPT_STREAM_CONTRACT = {{\n  path: {},\n  eventName: {},\n  capabilityId: {},\n  requiredScopes: {},\n  runtimeAvailability: {},\n  maximumReplayBatch: {},\n  retryPolicy: {},\n}} as const;\n",
        json_string(stream_path)?,
        json_string(event_name)?,
        json_string(capability_id)?,
        scopes_json,
        json_string(runtime_availability)?,
        maximum_replay,
        json_string(retry_policy)?,
    )?;

    let health_allowed = property_names(health_schema)?;
    let health_required = required_names(health_schema)?;
    let problem_allowed = property_names(problem_schema)?;
    let problem_required = required_names(problem_schema)?;
    let action_schema = problem_definitions
        .get("ProblemActionDto")
        .context("ProblemActionDto schema missing")?;
    let violation_schema = problem_definitions
        .get("ViolationDto")
        .context("ViolationDto schema missing")?;
    let action_allowed = property_names(action_schema)?;
    let action_required = required_names(action_schema)?;
    let violation_allowed = property_names(violation_schema)?;
    let violation_required = required_names(violation_schema)?;
    let receipt_allowed = property_names(receipt_schema)?;
    let receipt_required = required_names(receipt_schema)?;

    ensure_exact_names("HealthResponse", &health_allowed, &["status", "version"])?;
    ensure_exact_names(
        "ProblemDetails",
        &problem_allowed,
        &[
            "actual",
            "capability_id",
            "code",
            "correlation_id",
            "detail",
            "next_actions",
            "param",
            "retryability",
            "safe_state",
            "status",
            "title",
            "type",
            "violations",
        ],
    )?;
    ensure_exact_names(
        "ReceiptCommittedEvent",
        &receipt_allowed,
        &[
            "capability_id",
            "committed_at",
            "correlation_id",
            "observation_id",
            "operation_id",
            "receipt_id",
            "resolution",
        ],
    )?;

    let receipt_capability = string_at(receipt_schema, "/properties/capability_id/const")?;
    let receipt_resolution = string_at(receipt_schema, "/properties/resolution/const")?;
    let correlation_pattern = string_at(receipt_schema, "/properties/correlation_id/pattern")?;
    let receipt_pattern = string_at(receipt_schema, "/properties/receipt_id/pattern")?;
    let operation_pattern = string_at(receipt_schema, "/properties/operation_id/pattern")?;
    let observation_pattern = string_at(receipt_schema, "/properties/observation_id/pattern")?;

    output.push_str(&format!(
        r#"export class FastiContractParseError extends Error {{
  constructor(message: string) {{
    super(message);
    this.name = "FastiContractParseError";
  }}
}}

type JsonObject = Record<string, unknown>;

const HEALTH_ALLOWED = {health_allowed} as const;
const HEALTH_REQUIRED = {health_required} as const;
// prettier-ignore
const CAPABILITY_IDS = {capability_ids} as const;
// prettier-ignore
const PROBLEM_CODES = {problem_codes} as const;
// prettier-ignore
const PROBLEM_ALLOWED = {problem_allowed} as const;
// prettier-ignore
const PROBLEM_REQUIRED = {problem_required} as const;
const ACTION_ALLOWED = {action_allowed} as const;
const ACTION_REQUIRED = {action_required} as const;
// prettier-ignore
const VIOLATION_ALLOWED = {violation_allowed} as const;
const VIOLATION_REQUIRED = {violation_required} as const;
// prettier-ignore
const RECEIPT_ALLOWED = {receipt_allowed} as const;
// prettier-ignore
const RECEIPT_REQUIRED = {receipt_required} as const;
const CORRELATION_ID = new RegExp({correlation_pattern});
const RECEIPT_ID = new RegExp({receipt_pattern});
const OPERATION_ID = new RegExp({operation_pattern});
const OBSERVATION_ID = new RegExp({observation_pattern});
// prettier-ignore
const RFC3339_INSTANT = /^(\d{{4}})-(\d{{2}})-(\d{{2}})T(\d{{2}}):(\d{{2}}):(\d{{2}})(?:\.\d+)?(Z|[+-](\d{{2}}):(\d{{2}}))$/;

// prettier-ignore
export function parseHealthResponse(value: unknown): HealthResponse {{
  const object = exactObject(value, HEALTH_ALLOWED, HEALTH_REQUIRED, "HealthResponse");
  stringField(object, "status", "HealthResponse");
  stringField(object, "version", "HealthResponse");
  return object as unknown as HealthResponse;
}}

// prettier-ignore
export function parseProblemDetails(value: unknown): ProblemDetails {{
  const object = exactObject(value, PROBLEM_ALLOWED, PROBLEM_REQUIRED, "ProblemDetails");
  for (const field of [
    "type",
    "title",
    "detail",
    "code",
    "capability_id",
    "safe_state",
    "retryability",
    "correlation_id",
  ] as const) {{
    stringField(object, field, "ProblemDetails");
  }}
  knownStringField(object, "capability_id", CAPABILITY_IDS, "ProblemDetails");
  knownStringField(object, "code", PROBLEM_CODES, "ProblemDetails");
  integerField(object, "status", "ProblemDetails", 0, 65_535);
  nullableStringField(object, "param", "ProblemDetails");
  nullableStringField(object, "actual", "ProblemDetails");
  arrayField(object, "next_actions", "ProblemDetails").forEach(parseProblemAction);
  arrayField(object, "violations", "ProblemDetails").forEach(parseViolation);
  return object as unknown as ProblemDetails;
}}

// prettier-ignore
export function parseReceiptCommittedEvent(value: unknown): ReceiptCommittedEvent {{
  const object = exactObject(value, RECEIPT_ALLOWED, RECEIPT_REQUIRED, "ReceiptCommittedEvent");
  exactString(object, "capability_id", {receipt_capability}, "ReceiptCommittedEvent");
  exactString(object, "resolution", {receipt_resolution}, "ReceiptCommittedEvent");
  patternString(object, "correlation_id", CORRELATION_ID, "ReceiptCommittedEvent");
  patternString(object, "receipt_id", RECEIPT_ID, "ReceiptCommittedEvent");
  patternString(object, "operation_id", OPERATION_ID, "ReceiptCommittedEvent");
  patternString(object, "observation_id", OBSERVATION_ID, "ReceiptCommittedEvent");
  rfc3339InstantField(object, "committed_at", "ReceiptCommittedEvent");
  return object as unknown as ReceiptCommittedEvent;
}}

// prettier-ignore
function parseProblemAction(value: unknown): ProblemActionDto {{
  const object = exactObject(value, ACTION_ALLOWED, ACTION_REQUIRED, "ProblemActionDto");
  stringField(object, "id", "ProblemActionDto");
  stringField(object, "label", "ProblemActionDto");
  return object as unknown as ProblemActionDto;
}}

// prettier-ignore
function parseViolation(value: unknown): ViolationDto {{
  const object = exactObject(value, VIOLATION_ALLOWED, VIOLATION_REQUIRED, "ViolationDto");
  for (const field of ["code", "pointer", "reason", "expected"] as const) {{
    stringField(object, field, "ViolationDto");
  }}
  nullableStringField(object, "actual", "ViolationDto");
  return object as unknown as ViolationDto;
}}

// prettier-ignore
function exactObject(
  value: unknown,
  allowed: readonly string[],
  required: readonly string[],
  label: string,
): JsonObject {{
  if (!isPlainObject(value)) {{
    throw new FastiContractParseError(`${{label}} must be a plain object`);
  }}
  const object = value as JsonObject;
  for (const key of Object.keys(object)) {{
    if (!allowed.includes(key)) {{
      throw new FastiContractParseError(`${{label}} contains unknown field ${{key}}`);
    }}
  }}
  for (const key of required) {{
    if (!Object.hasOwn(object, key)) {{
      throw new FastiContractParseError(`${{label}} is missing required field ${{key}}`);
    }}
  }}
  return object;
}}

// prettier-ignore
function isPlainObject(value: unknown): value is Record<string, unknown> {{
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  return Object.getPrototypeOf(value) === Object.prototype;
}}

function stringField(object: JsonObject, field: string, label: string): string {{
  const value = object[field];
  if (typeof value !== "string") {{
    throw new FastiContractParseError(`${{label}}.${{field}} must be a string`);
  }}
  return value;
}}

// prettier-ignore
function knownStringField(
  object: JsonObject,
  field: string,
  allowed: readonly string[],
  label: string,
): void {{
  if (!allowed.includes(stringField(object, field, label))) {{
    throw new FastiContractParseError(`${{label}}.${{field}} has an unsupported value`);
  }}
}}

// prettier-ignore
function exactString(
  object: JsonObject,
  field: string,
  expected: string,
  label: string,
): void {{
  if (stringField(object, field, label) !== expected) {{
    throw new FastiContractParseError(`${{label}}.${{field}} has an unsupported value`);
  }}
}}

// prettier-ignore
function patternString(
  object: JsonObject,
  field: string,
  pattern: RegExp,
  label: string,
): void {{
  if (!pattern.test(stringField(object, field, label))) {{
    throw new FastiContractParseError(`${{label}}.${{field}} has an invalid format`);
  }}
}}

// prettier-ignore
function rfc3339InstantField(object: JsonObject, field: string, label: string): void {{
  const value = stringField(object, field, label);
  if (!isRealRfc3339Instant(value)) {{
    throw new FastiContractParseError(`${{label}}.${{field}} is not a real RFC3339 calendar instant`);
  }}
}}

// prettier-ignore
function nullableStringField(object: JsonObject, field: string, label: string): void {{
  const value = object[field];
  if (value !== undefined && value !== null && typeof value !== "string") {{
    throw new FastiContractParseError(`${{label}}.${{field}} must be a string or null`);
  }}
}}

// prettier-ignore
function integerField(
  object: JsonObject,
  field: string,
  label: string,
  minimum: number,
  maximum: number,
): void {{
  const value = object[field];
  if (typeof value !== "number" || !Number.isInteger(value) || value < minimum || value > maximum) {{
    throw new FastiContractParseError(`${{label}}.${{field}} must be an integer in range`);
  }}
}}

// prettier-ignore
function arrayField(object: JsonObject, field: string, label: string): unknown[] {{
  const value = object[field];
  if (!Array.isArray(value)) {{
    throw new FastiContractParseError(`${{label}}.${{field}} must be an array`);
  }}
  return value;
}}
"#,
        health_allowed = ts_string_array(&health_allowed)?,
        health_required = ts_string_array(&health_required)?,
        capability_ids = ts_string_array(&capability_ids)?,
        problem_codes = ts_string_array(&problem_codes)?,
        problem_allowed = ts_string_array(&problem_allowed)?,
        problem_required = ts_string_array(&problem_required)?,
        action_allowed = ts_string_array(&action_allowed)?,
        action_required = ts_string_array(&action_required)?,
        violation_allowed = ts_string_array(&violation_allowed)?,
        violation_required = ts_string_array(&violation_required)?,
        receipt_allowed = ts_string_array(&receipt_allowed)?,
        receipt_required = ts_string_array(&receipt_required)?,
        correlation_pattern = json_string(correlation_pattern)?,
        receipt_pattern = json_string(receipt_pattern)?,
        operation_pattern = json_string(operation_pattern)?,
        observation_pattern = json_string(observation_pattern)?,
        receipt_capability = json_string(receipt_capability)?,
        receipt_resolution = json_string(receipt_resolution)?,
    ));
    ensure!(
        output.ends_with('\n'),
        "generated SDK must end with a newline"
    );
    Ok(output)
}

fn render_conformance_contract(openapi: &Value) -> anyhow::Result<String> {
    ensure!(
        string_at(openapi, "/openapi")? == "3.1.0",
        "B1 conformance OpenAPI must remain 3.1.0"
    );
    let expected_paths: BTreeSet<_> = CONFORMANCE_OPERATIONS
        .iter()
        .map(|operation| operation.path)
        .collect();
    let actual_paths: BTreeSet<_> = object_at(openapi, "/paths")?
        .keys()
        .map(String::as_str)
        .collect();
    ensure!(
        actual_paths == expected_paths,
        "B1 conformance OpenAPI route inventory changed: expected {expected_paths:?}, found {actual_paths:?}"
    );

    let mut output = String::new();
    let schemas = object_at(openapi, "/components/schemas")?;
    let shared = ["ProblemActionDto", "ProblemDetails", "ViolationDto"];
    for (name, schema) in schemas {
        if shared.contains(&name.as_str()) {
            continue;
        }
        if schema.get("enum").is_some() {
            writeln!(
                output,
                "// prettier-ignore\nexport type {name} = {};\n",
                typescript_type(schema)?
            )?;
        } else {
            output.push_str(&render_interface(name, schema)?);
            output.push('\n');
        }
    }

    output.push_str("// prettier-ignore\nexport const B1_CONFORMANCE_OPERATIONS = {\n");
    for expected in CONFORMANCE_OPERATIONS {
        let ConformanceOperation {
            alias,
            operation_id,
            method,
            path,
            capability_id,
            authenticated,
            request,
            response,
            retry,
        } = expected;
        let operation_pointer = format!("/paths/{}/{method}", escape_pointer(path));
        let operation = value_at(openapi, &operation_pointer)?;
        ensure!(
            string_at(operation, "/operationId")? == operation_id,
            "conformance operation ID changed for {method} {path}"
        );
        let has_security = operation
            .get("security")
            .is_some_and(|security| security.as_array().is_some_and(|items| !items.is_empty()));
        ensure!(
            has_security == authenticated,
            "conformance security declaration changed for {method} {path}"
        );
        ensure!(
            string_at(operation, "/x-fasti-capability-id")? == capability_id,
            "conformance capability annotation changed for {method} {path}"
        );
        let required_scopes = array_at(operation, "/x-fasti-required-scopes")?;
        let required_scopes_json = serde_json::to_string(required_scopes)?;
        let runtime_availability = string_at(operation, "/x-fasti-runtime-availability")?;
        match request {
            Some(request_name) => ensure!(
                string_at(
                    operation,
                    "/requestBody/content/application~1json/schema/$ref"
                )? == format!("#/components/schemas/{request_name}"),
                "conformance request schema changed for {method} {path}"
            ),
            None => ensure!(
                operation.get("requestBody").is_none(),
                "unexpected request body for {method} {path}"
            ),
        }
        match response {
            Some(response_name) => ensure!(
                string_at(
                    operation,
                    "/responses/200/content/application~1json/schema/$ref"
                )? == format!("#/components/schemas/{response_name}"),
                "conformance success schema changed for {method} {path}"
            ),
            None => ensure!(
                operation.pointer("/responses/200").is_none(),
                "problem-only conformance binding gained a success for {method} {path}"
            ),
        }
        writeln!(
            output,
            "  {alias}: {{ operationId: {}, method: {}, path: {}, capabilityId: {}, requiredScopes: {required_scopes_json}, authenticated: {authenticated}, runtimeAvailability: {}, durability: \"none\", retry: {}, requestSchema: {}, responseSchema: {} }},",
            json_string(operation_id)?,
            json_string(&method.to_ascii_uppercase())?,
            json_string(path)?,
            json_string(capability_id)?,
            json_string(runtime_availability)?,
            json_string(retry)?,
            request.map(json_string).transpose()?.unwrap_or_else(|| "null".to_owned()),
            response
                .map(json_string)
                .transpose()?
                .unwrap_or_else(|| "null".to_owned()),
        )?;
    }
    output.push_str("} as const;\n\n");

    let schemas_json = serde_json::to_string_pretty(&sort_json(Value::Object(schemas.clone())))?;
    writeln!(
        output,
        "// prettier-ignore\nconst B1_CONFORMANCE_SCHEMAS = {schemas_json} as const;\n"
    )?;
    output.push_str(
        r##"// prettier-ignore
export function parseInitializeNodeRequest(value: unknown): InitializeNodeRequest {
  return parseConformanceDto("InitializeNodeRequest", value);
}

// prettier-ignore
export function parseInitializeNodeResponse(value: unknown): InitializeNodeResponse {
  return parseConformanceDto("InitializeNodeResponse", value);
}

// prettier-ignore
export function parseEnrollFirstClientRequest(value: unknown): EnrollFirstClientRequest {
  return parseConformanceDto("EnrollFirstClientRequest", value);
}

// prettier-ignore
export function parseEnrollFirstClientResponse(value: unknown): EnrollFirstClientResponse {
  return parseConformanceDto("EnrollFirstClientResponse", value);
}

// prettier-ignore
export function parseAcceptObservationRequest(value: unknown): AcceptObservationRequest {
  return parseConformanceDto("AcceptObservationRequest", value);
}

// prettier-ignore
export function parseAcceptObservationResponse(value: unknown): AcceptObservationResponse {
  return parseConformanceDto("AcceptObservationResponse", value);
}

// prettier-ignore
export function parseCapabilityDiscoveryResponse(value: unknown): CapabilityDiscoveryResponse {
  return parseConformanceDto("CapabilityDiscoveryResponse", value);
}

// prettier-ignore
export function parseReplayReceiptResponse(value: unknown): ReplayReceiptResponse {
  return parseConformanceDto("ReplayReceiptResponse", value);
}

// prettier-ignore
function parseConformanceDto<T>(schemaName: string, value: unknown): T {
  const schema = (B1_CONFORMANCE_SCHEMAS as Record<string, unknown>)[schemaName];
  if (schema === undefined) {
    throw new FastiContractParseError(`Unknown conformance schema ${schemaName}`);
  }
  validateOpenApiValue(value, schema, schemaName);
  return value as T;
}

// prettier-ignore
function validateOpenApiValue(value: unknown, schemaValue: unknown, path: string): void {
  const schema = schemaValue as Record<string, unknown>;
  if (typeof schema.$ref === "string") {
    const prefix = "#/components/schemas/";
    if (!schema.$ref.startsWith(prefix)) {
      throw new FastiContractParseError(`${path} has an unsupported schema reference`);
    }
    const name = schema.$ref.slice(prefix.length);
    const target = (B1_CONFORMANCE_SCHEMAS as Record<string, unknown>)[name];
    if (target === undefined) {
      throw new FastiContractParseError(`${path} references an unknown schema`);
    }
    validateOpenApiValue(value, target, path);
    return;
  }
  if (Array.isArray(schema.oneOf)) {
    let matches = 0;
    for (const candidate of schema.oneOf) {
      try {
        validateOpenApiValue(value, candidate, path);
        matches += 1;
      } catch (error) {
        if (!(error instanceof FastiContractParseError)) throw error;
      }
    }
    if (matches !== 1) {
      throw new FastiContractParseError(`${path} must match exactly one contract shape`);
    }
    return;
  }
  if (Array.isArray(schema.enum) && !schema.enum.includes(value)) {
    throw new FastiContractParseError(`${path} has an unsupported enum value`);
  }
  const schemaTypes = Array.isArray(schema.type) ? schema.type : [schema.type];
  if (schemaTypes.includes("null") && value === null) return;
  if (schemaTypes.includes("string")) {
    if (typeof value !== "string") {
      throw new FastiContractParseError(`${path} must be a string`);
    }
    if (typeof schema.minLength === "number" && value.length < schema.minLength) {
      throw new FastiContractParseError(`${path} is shorter than its minimum length`);
    }
    if (typeof schema.maxLength === "number" && value.length > schema.maxLength) {
      throw new FastiContractParseError(`${path} exceeds its maximum length`);
    }
    if (typeof schema.pattern === "string" && !new RegExp(schema.pattern).test(value)) {
      throw new FastiContractParseError(`${path} has an invalid format`);
    }
    if (schema.format === "date-time" && !isRealRfc3339Instant(value)) {
      throw new FastiContractParseError(`${path} is not a real RFC3339 instant`);
    }
    if (schema.format === "iso-date-or-rfc3339" && !isRealIsoDateOrRfc3339(value)) {
      throw new FastiContractParseError(`${path} is not a real ISO date or RFC3339 instant`);
    }
    return;
  }
  if (schemaTypes.includes("integer")) {
    if (typeof value !== "number" || !Number.isSafeInteger(value)) {
      throw new FastiContractParseError(`${path} must be a safe integer`);
    }
    if (typeof schema.minimum === "number" && value < schema.minimum) {
      throw new FastiContractParseError(`${path} is below its minimum`);
    }
    if (typeof schema.maximum === "number" && value > schema.maximum) {
      throw new FastiContractParseError(`${path} exceeds its maximum`);
    }
    return;
  }
  if (schemaTypes.includes("array")) {
    if (!Array.isArray(value)) {
      throw new FastiContractParseError(`${path} must be an array`);
    }
    value.forEach((item, index) => validateOpenApiValue(item, schema.items, `${path}[${index}]`));
    return;
  }
  if (schemaTypes.includes("object")) {
    if (!isPlainObject(value)) {
      throw new FastiContractParseError(`${path} must be a plain object`);
    }
    const object = value as Record<string, unknown>;
    const properties = isPlainObject(schema.properties)
      ? (schema.properties as Record<string, unknown>)
      : {};
    if (schema.additionalProperties === false) {
      for (const key of Object.keys(object)) {
        if (!Object.hasOwn(properties, key)) {
          throw new FastiContractParseError(`${path} contains unknown field ${key}`);
        }
      }
    }
    const required = Array.isArray(schema.required) ? schema.required : [];
    for (const field of required) {
      if (typeof field !== "string" || !Object.hasOwn(object, field)) {
        throw new FastiContractParseError(`${path} is missing a required field`);
      }
    }
    for (const [field, fieldSchema] of Object.entries(properties)) {
      if (Object.hasOwn(object, field)) {
        validateOpenApiValue(object[field], fieldSchema, `${path}.${field}`);
      }
    }
    return;
  }
  throw new FastiContractParseError(`${path} uses an unsupported schema shape`);
}

// prettier-ignore
function isRealIsoDateOrRfc3339(value: string): boolean {
  const date = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (date !== null) {
    return isRealCalendarDate(Number(date[1]), Number(date[2]), Number(date[3]));
  }
  return isRealRfc3339Instant(value);
}

// prettier-ignore
function isRealRfc3339Instant(value: string): boolean {
  const match = RFC3339_INSTANT.exec(value);
  if (match === null) return false;
  const [, yearText, monthText, dayText, hourText, minuteText, secondText, , offsetHourText, offsetMinuteText] = match;
  const hour = Number(hourText);
  const minute = Number(minuteText);
  const second = Number(secondText);
  const offsetHour = offsetHourText === undefined ? 0 : Number(offsetHourText);
  const offsetMinute = offsetMinuteText === undefined ? 0 : Number(offsetMinuteText);
  return (
    isRealCalendarDate(Number(yearText), Number(monthText), Number(dayText)) &&
    hour <= 23 &&
    minute <= 59 &&
    second <= 59 &&
    offsetHour <= 23 &&
    offsetMinute <= 59
  );
}

// prettier-ignore
function isRealCalendarDate(year: number, month: number, day: number): boolean {
  const leapYear = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
  const daysInMonth = [31, leapYear ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  return month >= 1 && month <= 12 && day >= 1 && day <= daysInMonth[month - 1]!;
}

"##,
    );
    Ok(output)
}

fn escape_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn render_interface(name: &str, schema: &Value) -> anyhow::Result<String> {
    render_interface_with_overrides(name, schema, &[])
}

fn render_interface_with_overrides(
    name: &str,
    schema: &Value,
    overrides: &[(&str, &str)],
) -> anyhow::Result<String> {
    ensure!(
        schema.get("additionalProperties").and_then(Value::as_bool) == Some(false),
        "{name} must reject unknown fields before SDK generation"
    );
    let properties = schema
        .get("properties")
        .map(|value| {
            value
                .as_object()
                .context("schema properties must be an object")
        })
        .transpose()?;
    if match properties {
        Some(properties) => properties.is_empty(),
        None => true,
    } {
        return Ok(format!("export interface {name} {{}}\n"));
    }
    let required = required_names(schema)?;
    let mut output = format!("export interface {name} {{\n");
    if let Some(properties) = properties {
        for (property_name, property_schema) in properties {
            let optional = if required.contains(property_name) {
                ""
            } else {
                "?"
            };
            writeln!(
                output,
                "  readonly {property_name}{optional}: {};",
                overrides
                    .iter()
                    .find_map(|(field, replacement)| {
                        (*field == property_name).then_some((*replacement).to_owned())
                    })
                    .map(Ok)
                    .unwrap_or_else(|| typescript_type(property_schema))?
            )?;
        }
    }
    output.push_str("}\n");
    Ok(output)
}

fn typescript_type(schema: &Value) -> anyhow::Result<String> {
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        let mut rendered = values
            .iter()
            .map(|value| match value {
                Value::String(value) => json_string(value),
                Value::Bool(_) | Value::Number(_) | Value::Null => Ok(value.to_string()),
                _ => anyhow::bail!("unsupported structured schema enum value"),
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        rendered.sort();
        rendered.dedup();
        return Ok(rendered.join(" | "));
    }
    if let Some(choices) = schema.get("oneOf").and_then(Value::as_array) {
        let mut rendered = choices
            .iter()
            .map(typescript_type)
            .collect::<anyhow::Result<Vec<_>>>()?;
        rendered.sort();
        rendered.dedup();
        return Ok(rendered.join(" | "));
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return reference
            .rsplit('/')
            .next()
            .map(str::to_owned)
            .context("schema reference has no terminal type name");
    }
    if let Some(constant) = schema.get("const") {
        return Ok(match constant {
            Value::String(value) => json_string(value)?,
            Value::Bool(_) | Value::Number(_) | Value::Null => constant.to_string(),
            _ => anyhow::bail!("unsupported structured schema const"),
        });
    }
    match schema.get("type") {
        Some(Value::String(kind)) => match kind.as_str() {
            "string" => Ok("string".to_owned()),
            "integer" | "number" => Ok("number".to_owned()),
            "boolean" => Ok("boolean".to_owned()),
            "null" => Ok("null".to_owned()),
            "array" => Ok(format!(
                "ReadonlyArray<{}>",
                typescript_type(value_at(schema, "/items")?)?
            )),
            other => anyhow::bail!("unsupported JSON Schema type {other}"),
        },
        Some(Value::Array(kinds)) => {
            let mut rendered = Vec::with_capacity(kinds.len());
            for kind in kinds {
                rendered.push(typescript_type(&serde_json::json!({ "type": kind }))?);
            }
            rendered.sort();
            rendered.dedup();
            Ok(rendered.join(" | "))
        }
        _ => anyhow::bail!("schema has no supported type or reference"),
    }
}

fn render_string_union(
    output: &mut String,
    name: &str,
    values: &BTreeSet<String>,
) -> anyhow::Result<()> {
    ensure!(!values.is_empty(), "{name} union cannot be empty");
    output.push_str("// prettier-ignore\n");
    writeln!(output, "export type {name} =")?;
    for (index, value) in values.iter().enumerate() {
        let suffix = if index + 1 == values.len() { ";" } else { "" };
        writeln!(output, "  | {}{suffix}", json_string(value)?)?;
    }
    output.push('\n');
    Ok(())
}

fn property_names(schema: &Value) -> anyhow::Result<BTreeSet<String>> {
    Ok(object_at(schema, "/properties")?.keys().cloned().collect())
}

fn required_names(schema: &Value) -> anyhow::Result<BTreeSet<String>> {
    let mut required = BTreeSet::new();
    let Some(values) = schema.get("required") else {
        return Ok(required);
    };
    let values = values
        .as_array()
        .context("schema required must be an array")?;
    for value in values {
        required.insert(
            value
                .as_str()
                .context("schema required entry must be a string")?
                .to_owned(),
        );
    }
    Ok(required)
}

fn ensure_exact_names(
    name: &str,
    actual: &BTreeSet<String>,
    expected: &[&str],
) -> anyhow::Result<()> {
    let expected: BTreeSet<_> = expected.iter().map(|value| (*value).to_owned()).collect();
    ensure!(
        actual == &expected,
        "{name} shape changed: expected {expected:?}, found {actual:?}"
    );
    Ok(())
}

fn ts_string_array(values: &BTreeSet<String>) -> anyhow::Result<String> {
    let values = values
        .iter()
        .map(|value| json_string(value))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(format!("[{}]", values.join(", ")))
}

fn json_string(value: &str) -> anyhow::Result<String> {
    serde_json::to_string(value).context("string is not JSON serializable")
}

fn value_at<'a>(value: &'a Value, pointer: &str) -> anyhow::Result<&'a Value> {
    value
        .pointer(pointer)
        .with_context(|| format!("contract value is missing {pointer}"))
}

fn object_at<'a>(value: &'a Value, pointer: &str) -> anyhow::Result<&'a Map<String, Value>> {
    value_at(value, pointer)?
        .as_object()
        .with_context(|| format!("contract value at {pointer} must be an object"))
}

fn array_at<'a>(value: &'a Value, pointer: &str) -> anyhow::Result<&'a Vec<Value>> {
    value_at(value, pointer)?
        .as_array()
        .with_context(|| format!("contract value at {pointer} must be an array"))
}

fn string_at<'a>(value: &'a Value, pointer: &str) -> anyhow::Result<&'a str> {
    value_at(value, pointer)?
        .as_str()
        .with_context(|| format!("contract value at {pointer} must be a string"))
}

fn u64_at(value: &Value, pointer: &str) -> anyhow::Result<u64> {
    value_at(value, pointer)?
        .as_u64()
        .with_context(|| format!("contract value at {pointer} must be an unsigned integer"))
}

fn write(output_root: &Path, artifacts: &Artifacts) -> anyhow::Result<()> {
    for (relative_path, bytes) in artifacts {
        let destination = output_root.join(relative_path);
        let parent = destination
            .parent()
            .with_context(|| format!("generated path {} has no parent", destination.display()))?;
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
        fs::write(&destination, bytes)
            .with_context(|| format!("failed to write {}", destination.display()))?;
    }
    Ok(())
}

fn verify_inventory(output_root: &Path, expected: &Artifacts) -> anyhow::Result<()> {
    let mut actual = BTreeSet::new();
    for relative_directory in GENERATED_ONLY_DIRECTORIES.map(PathBuf::from) {
        let directory = output_root.join(&relative_directory);
        for entry in fs::read_dir(&directory)
            .with_context(|| format!("failed to inspect {}", directory.display()))?
        {
            let entry = entry?;
            let file_type = entry.file_type()?;
            ensure!(
                file_type.is_file(),
                "generated artifact directory {} contains non-file {}",
                relative_directory.display(),
                entry.path().display()
            );
            actual.insert(relative_directory.join(entry.file_name()));
        }
    }
    let generated_only_directories: BTreeSet<_> = GENERATED_ONLY_DIRECTORIES
        .into_iter()
        .map(PathBuf::from)
        .collect();
    let expected_paths: BTreeSet<_> = expected
        .keys()
        .filter(|path| {
            path.parent()
                .is_some_and(|parent| generated_only_directories.contains(parent))
        })
        .cloned()
        .collect();
    ensure!(
        actual == expected_paths,
        "checked-in generated artifact inventory differs: missing={:?}, unexpected={:?}",
        expected_paths.difference(&actual).collect::<Vec<_>>(),
        actual.difference(&expected_paths).collect::<Vec<_>>()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_inventory_is_fixed_and_unique() {
        let artifacts = build(workspace_root()).expect("contract generation succeeds");
        let actual: BTreeSet<_> = artifacts.keys().map(|path| path.as_path()).collect();
        let expected: BTreeSet<_> = [
            Path::new(OPENAPI_PATH),
            Path::new(CONFORMANCE_OPENAPI_PATH),
            Path::new(CAPABILITY_REGISTRY_PATH),
            Path::new(HEALTH_SCHEMA_PATH),
            Path::new(PROBLEM_SCHEMA_PATH),
            Path::new(SDK_GENERATED_PATH),
            Path::new(RUST_CAPABILITY_IDS_PATH),
        ]
        .into_iter()
        .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn generation_is_byte_reproducible() {
        let first = build(workspace_root()).expect("first generation succeeds");
        let second = build(workspace_root()).expect("second generation succeeds");
        assert_eq!(first, second);
        assert!(first.values().all(|artifact| artifact.ends_with(b"\n")));
    }

    #[test]
    fn generated_mappings_keep_internal_keys_out_of_public_discovery() {
        let artifacts = build(workspace_root()).expect("contract generation succeeds");
        let public_registry = std::str::from_utf8(
            artifacts
                .get(Path::new(CAPABILITY_REGISTRY_PATH))
                .expect("public registry generated"),
        )
        .expect("public registry is UTF-8");
        assert!(!public_registry.contains("\"application_key\""));

        let rust_mapping = std::str::from_utf8(
            artifacts
                .get(Path::new(RUST_CAPABILITY_IDS_PATH))
                .expect("Rust mapping generated"),
        )
        .expect("Rust mapping is UTF-8");
        assert_eq!(
            rust_mapping.matches("CapabilityKey::").count(),
            fasti_application::CapabilityKey::ALL.len()
        );

        let sdk = std::str::from_utf8(
            artifacts
                .get(Path::new(SDK_GENERATED_PATH))
                .expect("SDK generated"),
        )
        .expect("SDK is UTF-8");
        assert!(sdk.contains("runtimeAvailability: \"fixture_only\""));
        assert!(sdk.contains("\"bounded_context\""));
        assert!(sdk.contains("\"scopes\""));
        assert!(sdk.contains("\"problems\""));
        assert!(sdk.contains("\"surface_profiles\""));
    }

    #[test]
    fn every_conformance_operation_carries_registry_parity_annotations() {
        let artifacts = build(workspace_root()).expect("contract generation succeeds");
        let openapi: Value = serde_json::from_slice(
            artifacts
                .get(Path::new(CONFORMANCE_OPENAPI_PATH))
                .expect("conformance OpenAPI generated"),
        )
        .expect("conformance OpenAPI is JSON");
        for expected in CONFORMANCE_OPERATIONS {
            let pointer = format!(
                "/paths/{}/{}",
                escape_pointer(expected.path),
                expected.method
            );
            let operation = value_at(&openapi, &pointer).expect("operation is present");
            assert_eq!(
                string_at(operation, "/x-fasti-capability-id")
                    .expect("capability annotation is present"),
                expected.capability_id
            );
            assert!(!array_at(operation, "/x-fasti-required-scopes")
                .expect("scope annotation is present")
                .is_empty());
            assert_eq!(
                string_at(operation, "/x-fasti-runtime-availability")
                    .expect("runtime annotation is present"),
                "fixture_only"
            );
        }
    }

    fn workspace_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask lives under workspace root")
    }
}
