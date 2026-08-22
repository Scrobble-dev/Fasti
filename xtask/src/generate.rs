use anyhow::{ensure, Context};
use fasti_application::{CapabilityKey, ProblemCode, ProblemParamPolicy};
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
const PROBLEM_CATALOG_PATH: &str = "contracts/generated/v1/problems.json";
const CAPABILITY_DISCOVERY_EXAMPLE_PATH: &str =
    "contracts/examples/v1/system.capabilities.success.json";
const HEALTH_SCHEMA_PATH: &str = "packages/schemas/schemas/health-response.json";
const PROBLEM_SCHEMA_PATH: &str = "packages/schemas/schemas/problem-details.json";
const SDK_GENERATED_PATH: &str = "packages/sdk/src/generated.ts";
const RUST_CAPABILITY_IDS_PATH: &str = "crates/fasti-contracts/src/generated_capability_ids.rs";
const ASYNCAPI_PATH: &str = "contracts/asyncapi/v1/transport.yaml";
const EXAMPLES_DIRECTORY: &str = "contracts/examples/v1";
const DOCUMENTATION_BASE: &str = "https://fasti.scrobble.dev";
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
    let capability_keys: BTreeMap<_, _> = registry::internal_key_id_pairs(workspace_root)?
        .into_iter()
        .map(|(key, id)| (id, key))
        .collect();
    let problem_catalog = canonical_problem_catalog(&public_registry, &capability_keys)?;
    let capability_discovery_example = capability_discovery_example(&public_registry)?;
    let health_schema = draft_2020_12_schema::<HealthResponse>()?;
    let problem_schema = draft_2020_12_schema::<ProblemDetails>()?;
    let asyncapi = load_yaml(workspace_root, ASYNCAPI_PATH)?;
    let mut production_openapi = serde_json::to_value(fasti_api::openapi())
        .context("production OpenAPI is not serializable")?;
    enrich_production_health_openapi(workspace_root, &mut production_openapi, &public_registry)?;
    let mut conformance_openapi = serde_json::to_value(fasti_api::b1_conformance_openapi())
        .context("B1 conformance OpenAPI is not serializable")?;
    enrich_conformance_openapi(
        workspace_root,
        &mut conformance_openapi,
        &public_registry,
        &capability_keys,
        &capability_discovery_example,
    )?;
    validate_problem_schema_parity(&problem_schema, &conformance_openapi)?;
    validate_required_b1_bindings(
        workspace_root,
        &capability_keys,
        &production_openapi,
        &conformance_openapi,
        &asyncapi,
        &problem_catalog,
        &health_schema,
    )?;
    insert(&mut artifacts, OPENAPI_PATH, production_openapi)?;
    insert(
        &mut artifacts,
        CAPABILITY_REGISTRY_PATH,
        public_registry.clone(),
    )?;
    insert(
        &mut artifacts,
        PROBLEM_CATALOG_PATH,
        problem_catalog.clone(),
    )?;
    insert(
        &mut artifacts,
        CAPABILITY_DISCOVERY_EXAMPLE_PATH,
        capability_discovery_example,
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
            &problem_catalog,
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

fn canonical_problem_catalog(
    public_registry: &Value,
    capability_keys: &BTreeMap<String, CapabilityKey>,
) -> anyhow::Result<Value> {
    let mut entries = Vec::new();
    for capability in array_at(public_registry, "/capabilities")? {
        let capability_id = string_at(capability, "/id")?;
        let capability_key = *capability_keys.get(capability_id).with_context(|| {
            format!("registry capability {capability_id} has no application key")
        })?;
        for problem in array_at(capability, "/problems")? {
            let code = problem
                .as_str()
                .context("registry problem code must be a string")?;
            let code = ProblemCode::from_code(code).with_context(|| {
                format!("registry problem code {code} has no canonical contract")
            })?;
            let contract = code.contract();
            let param_policy = contract.param_policy();
            if param_policy == ProblemParamPolicy::ReceiptIdentifierByCapability {
                ensure!(
                    matches!(
                        capability_key,
                        CapabilityKey::ReplayReceipt | CapabilityKey::StreamReceipts
                    ),
                    "{} cannot resolve receipt identifier parameters for {capability_id}",
                    code.as_str()
                );
            }
            let action = contract.default_next_action();
            entries.push(serde_json::json!({
                "capability_id": capability_id,
                "code": code.as_str(),
                "type": format!(
                    "{DOCUMENTATION_BASE}/{}",
                    contract.documentation_path()
                ),
                "title": contract.title(),
                "status": contract.status(),
                "detail": contract.detail(capability_key),
                "safe_state": contract.safe_state().as_str(),
                "retryability": contract.retryability().as_str(),
                "next_actions": [{
                    "id": action.id(),
                    "label": action.label(),
                }],
                "param_policy": param_policy.as_str(),
                "param": param_policy.resolve(capability_key),
            }));
        }
    }
    ensure!(
        !entries.is_empty(),
        "canonical problem catalog cannot be empty"
    );
    Ok(serde_json::json!({
        "contract_version": string_at(public_registry, "/contract_version")?,
        "documentation_base": DOCUMENTATION_BASE,
        "problems": entries,
    }))
}

fn capability_discovery_example(public_registry: &Value) -> anyhow::Result<Value> {
    let capabilities = array_at(public_registry, "/capabilities")?;
    ensure!(
        capabilities.len() == CapabilityKey::ALL.len(),
        "capability discovery example must expose every application capability"
    );
    let ids: Vec<_> = capabilities
        .iter()
        .map(|capability| string_at(capability, "/id"))
        .collect::<anyhow::Result<_>>()?;
    ensure!(
        ids.windows(2).all(|pair| pair[0] < pair[1]),
        "capability discovery example must remain canonically sorted"
    );
    Ok(serde_json::json!({
        "conformance": {
            "availability": "fixture_only",
            "durability": "none",
        },
        "contract_version": string_at(public_registry, "/contract_version")?,
        "capability_base_uri": string_at(public_registry, "/capability_base_uri")?,
        "surface_profiles": object_at(public_registry, "/surface_profiles")?,
        "capabilities": capabilities,
    }))
}

fn enrich_conformance_openapi(
    workspace_root: &Path,
    openapi: &mut Value,
    public_registry: &Value,
    capability_keys: &BTreeMap<String, CapabilityKey>,
    capability_discovery_example: &Value,
) -> anyhow::Result<()> {
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
        let problems = array_at(capability, "/problems")?.to_vec();
        let examples = array_at(capability, "/examples")?.to_vec();
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
            "x-fasti-authorization".to_owned(),
            Value::String(string_at(capability, "/authorization")?.to_owned()),
        );
        operation.insert(
            "x-fasti-problem-codes".to_owned(),
            Value::Array(problems.clone()),
        );
        operation.insert(
            "x-fasti-example-ids".to_owned(),
            Value::Array(examples.clone()),
        );
        operation.insert(
            "x-fasti-runtime-availability".to_owned(),
            Value::String(runtime_availability),
        );
        validate_problem_responses(operation, expected, capability_keys, &problems)?;
        bind_governed_examples(
            workspace_root,
            operation,
            public_registry,
            capability,
            expected,
            capability_keys,
            &examples,
            capability_discovery_example,
        )?;
    }
    enrich_discovery_collection_schema(openapi, public_registry)?;
    Ok(())
}

fn enrich_discovery_collection_schema(
    openapi: &mut Value,
    public_registry: &Value,
) -> anyhow::Result<()> {
    let capabilities = array_at(public_registry, "/capabilities")?;
    let capability_vocabulary = |pointer: &str| -> anyhow::Result<Vec<Value>> {
        let values: BTreeSet<_> = capabilities
            .iter()
            .map(|capability| string_at(capability, pointer).map(ToOwned::to_owned))
            .collect::<anyhow::Result<_>>()?;
        Ok(values.into_iter().map(Value::String).collect())
    };
    let array_vocabulary = |pointer: &str| -> anyhow::Result<Vec<Value>> {
        let mut values = BTreeSet::new();
        for capability in capabilities {
            for value in array_at(capability, pointer)? {
                values.insert(
                    value
                        .as_str()
                        .with_context(|| format!("{pointer} vocabulary must contain strings"))?
                        .to_owned(),
                );
            }
        }
        Ok(values.into_iter().map(Value::String).collect())
    };
    let profile_names: Vec<_> = object_at(public_registry, "/surface_profiles")?
        .keys()
        .cloned()
        .map(Value::String)
        .collect();
    let surface_names: Vec<_> = [
        "cli",
        "domain_application",
        "http_openapi",
        "json_ld",
        "json_schema",
        "knowledge",
        "okf",
        "package_smoke",
        "sdk",
        "sse_asyncapi",
        "ui",
    ]
    .into_iter()
    .map(|value| Value::String(value.to_owned()))
    .collect();
    let profile_count = profile_names.len();
    let surface_count = surface_names.len();
    let profiles_schema = openapi
        .pointer_mut("/components/schemas/CapabilityDiscoveryResponse/properties/surface_profiles")
        .and_then(Value::as_object_mut)
        .context("CapabilityDiscoveryResponse surface_profiles schema is absent")?;
    profiles_schema.insert("minProperties".to_owned(), profile_count.into());
    profiles_schema.insert("maxProperties".to_owned(), profile_count.into());
    profiles_schema.insert(
        "propertyNames".to_owned(),
        serde_json::json!({ "type": "string", "enum": profile_names }),
    );
    let disposition_map = profiles_schema
        .get_mut("additionalProperties")
        .and_then(Value::as_object_mut)
        .context("surface profile values must have a schema")?;
    disposition_map.insert("minProperties".to_owned(), surface_count.into());
    disposition_map.insert("maxProperties".to_owned(), surface_count.into());
    disposition_map.insert(
        "propertyNames".to_owned(),
        serde_json::json!({ "type": "string", "enum": surface_names }),
    );

    let capability_count = capabilities.len();
    let capabilities_schema = openapi
        .pointer_mut("/components/schemas/CapabilityDiscoveryResponse/properties/capabilities")
        .and_then(Value::as_object_mut)
        .context("CapabilityDiscoveryResponse capabilities schema is absent")?;
    capabilities_schema.insert("minItems".to_owned(), capability_count.into());
    capabilities_schema.insert("maxItems".to_owned(), capability_count.into());
    capabilities_schema.insert("uniqueItems".to_owned(), Value::Bool(true));

    for (pointer, values) in [
        (
            "/components/schemas/CapabilityDescriptorDto/properties/id",
            capability_vocabulary("/id")?,
        ),
        (
            "/components/schemas/CapabilityDescriptorDto/properties/authorization",
            capability_vocabulary("/authorization")?,
        ),
        (
            "/components/schemas/CapabilityDescriptorDto/properties/contract_body",
            capability_vocabulary("/contract_body")?,
        ),
        (
            "/components/schemas/CapabilityDescriptorDto/properties/runtime_body",
            capability_vocabulary("/runtime_body")?,
        ),
        (
            "/components/schemas/CapabilityDescriptorDto/properties/surface_profile",
            profile_names.clone(),
        ),
        (
            "/components/schemas/CapabilityLifecycleDto/properties/introduced_in",
            capability_vocabulary("/lifecycle/introduced_in")?,
        ),
        (
            "/components/schemas/CapabilityLifecycleDto/properties/contract_state",
            capability_vocabulary("/lifecycle/contract_state")?,
        ),
        (
            "/components/schemas/CapabilityLifecycleDto/properties/runtime_availability",
            capability_vocabulary("/lifecycle/runtime_availability")?,
        ),
    ] {
        openapi
            .pointer_mut(pointer)
            .and_then(Value::as_object_mut)
            .with_context(|| format!("discovery schema omits {pointer}"))?
            .insert("enum".to_owned(), Value::Array(values));
    }
    for (field, values) in [
        ("scopes", array_vocabulary("/scopes")?),
        ("problems", array_vocabulary("/problems")?),
        ("examples", array_vocabulary("/examples")?),
    ] {
        let schema = openapi
            .pointer_mut(&format!(
                "/components/schemas/CapabilityDescriptorDto/properties/{field}"
            ))
            .and_then(Value::as_object_mut)
            .with_context(|| format!("capability {field} schema is absent"))?;
        schema.insert("uniqueItems".to_owned(), Value::Bool(true));
        schema.insert(
            "items".to_owned(),
            serde_json::json!({ "type": "string", "enum": values }),
        );
    }
    for (pointer, pattern) in [
        (
            "/components/schemas/CapabilityDescriptorDto/properties/id",
            r"^[a-z][a-z0-9_]*(?:\.[a-z][a-z0-9_]*)+$",
        ),
        (
            "/components/schemas/CapabilityDescriptorDto/properties/bounded_context",
            r"^[a-z][a-z0-9_]*(?:\.[a-z][a-z0-9_]*)+$",
        ),
        (
            "/components/schemas/CapabilityUatDto/properties/id",
            r"^ID-[0-9]{3}$",
        ),
    ] {
        openapi
            .pointer_mut(pointer)
            .and_then(Value::as_object_mut)
            .with_context(|| format!("discovery schema omits {pointer}"))?
            .insert("pattern".to_owned(), Value::String(pattern.to_owned()));
    }
    for (pointer, values) in [
        (
            "/components/schemas/CapabilitySurfaceDispositionDto/properties/state",
            vec!["later_body", "not_applicable", "required"],
        ),
        (
            "/components/schemas/CapabilitySurfaceDispositionDto/properties/binding_visibility",
            vec!["internal", "public"],
        ),
        (
            "/components/schemas/CapabilitySurfaceDispositionDto/properties/body",
            vec!["b0", "b1", "b2", "b3"],
        ),
        (
            "/components/schemas/CapabilityUatDto/properties/relationship",
            vec!["deferred", "direct", "split"],
        ),
        (
            "/components/schemas/CapabilityUatDto/properties/owner_body",
            vec!["b1", "b2", "b3"],
        ),
    ] {
        openapi
            .pointer_mut(pointer)
            .and_then(Value::as_object_mut)
            .with_context(|| format!("discovery schema omits {pointer}"))?
            .insert(
                "enum".to_owned(),
                Value::Array(
                    values
                        .into_iter()
                        .map(|value| Value::String(value.to_owned()))
                        .collect(),
                ),
            );
    }
    Ok(())
}

fn validate_problem_schema_parity(
    json_schema: &Value,
    conformance_openapi: &Value,
) -> anyhow::Result<()> {
    let openapi_problem = value_at(conformance_openapi, "/components/schemas/ProblemDetails")?;
    let openapi_violation = value_at(conformance_openapi, "/components/schemas/ViolationDto")?;
    for (label, draft_pointer, openapi_schema, openapi_pointer) in [
        (
            "ProblemDetails.actual",
            "/properties/actual/type",
            openapi_problem,
            "/properties/actual/type",
        ),
        (
            "ViolationDto.actual",
            "/$defs/ViolationDto/properties/actual/type",
            openapi_violation,
            "/properties/actual/type",
        ),
    ] {
        ensure!(
            string_at(json_schema, draft_pointer)? == "null"
                && string_at(openapi_schema, openapi_pointer)? == "null",
            "{label} must be explicit JSON null in JSON Schema and OpenAPI"
        );
    }
    let draft_status = value_at(json_schema, "/properties/status")?;
    let openapi_status = value_at(openapi_problem, "/properties/status")?;
    for (label, pointer) in [("minimum", "/minimum"), ("maximum", "/maximum")] {
        ensure!(
            u64_at(draft_status, pointer)? == u64_at(openapi_status, pointer)?,
            "ProblemDetails.status {label} differs between JSON Schema and OpenAPI"
        );
    }
    ensure!(
        string_at(draft_status, "/type")? == "integer"
            && string_at(openapi_status, "/type")? == "integer"
            && string_at(draft_status, "/format")? == "uint16"
            && string_at(openapi_status, "/format")? == "uint16",
        "ProblemDetails.status type/format differs between JSON Schema and OpenAPI"
    );
    Ok(())
}

fn enrich_production_health_openapi(
    workspace_root: &Path,
    openapi: &mut Value,
    public_registry: &Value,
) -> anyhow::Result<()> {
    let capability = array_at(public_registry, "/capabilities")?
        .iter()
        .find(|capability| string_at(capability, "/id").ok() == Some("system.health"))
        .context("public registry omits system.health")?;
    let operation = openapi
        .pointer_mut("/paths/~1api~1v1~1health/get")
        .and_then(Value::as_object_mut)
        .context("production OpenAPI omits GET /api/v1/health")?;
    operation.insert(
        "x-fasti-capability-id".to_owned(),
        Value::String("system.health".to_owned()),
    );
    operation.insert(
        "x-fasti-required-scopes".to_owned(),
        Value::Array(array_at(capability, "/scopes")?.clone()),
    );
    operation.insert(
        "x-fasti-authorization".to_owned(),
        Value::String(string_at(capability, "/authorization")?.to_owned()),
    );
    operation.insert(
        "x-fasti-problem-codes".to_owned(),
        Value::Array(array_at(capability, "/problems")?.clone()),
    );
    let example_ids = array_at(capability, "/examples")?.clone();
    operation.insert(
        "x-fasti-example-ids".to_owned(),
        Value::Array(example_ids.clone()),
    );
    operation.insert(
        "x-fasti-runtime-availability".to_owned(),
        Value::String(string_at(capability, "/lifecycle/runtime_availability")?.to_owned()),
    );
    ensure!(
        example_ids.len() == 1 && example_ids[0].as_str() == Some("system.health.success"),
        "production health must own exactly the governed health success example"
    );
    let example = load_governed_example(workspace_root, "system.health.success", &Value::Null)?;
    ensure!(
        example.media_type == "application/json",
        "system.health.success must be an application/json example"
    );
    let media = operation
        .get_mut("responses")
        .and_then(Value::as_object_mut)
        .and_then(|responses| responses.get_mut("200"))
        .and_then(|response| response.get_mut("content"))
        .and_then(Value::as_object_mut)
        .and_then(|content| content.get_mut("application/json"))
        .and_then(Value::as_object_mut)
        .context("production health 200 response omits application/json")?;
    media.insert(
        "examples".to_owned(),
        serde_json::json!({
            "system.health.success": { "value": example.payload }
        }),
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_required_b1_bindings(
    workspace_root: &Path,
    capability_keys: &BTreeMap<String, CapabilityKey>,
    production_openapi: &Value,
    conformance_openapi: &Value,
    asyncapi: &Value,
    problem_catalog: &Value,
    health_schema: &Value,
) -> anyhow::Result<()> {
    for required in registry::finalized_b1_required_bindings(workspace_root)? {
        resolve_required_binding(
            workspace_root,
            required.surface,
            &required.binding,
            &required.capability_id,
            capability_keys,
            production_openapi,
            conformance_openapi,
            asyncapi,
            problem_catalog,
            health_schema,
        )
        .with_context(|| {
            format!(
                "required binding {} does not resolve for {}.{}",
                required.binding, required.capability_id, required.surface
            )
        })?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn resolve_required_binding(
    workspace_root: &Path,
    surface: &str,
    binding: &str,
    capability_id: &str,
    capability_keys: &BTreeMap<String, CapabilityKey>,
    production_openapi: &Value,
    conformance_openapi: &Value,
    asyncapi: &Value,
    problem_catalog: &Value,
    health_schema: &Value,
) -> anyhow::Result<()> {
    match surface {
        "domain_application" => {
            ensure!(
                binding == "application:{application_key}"
                    && capability_keys.contains_key(capability_id),
                "application capability key is absent"
            );
        }
        "http_openapi" => {
            ensure!(
                binding == "openapi:{capability_id}",
                "unknown OpenAPI binding"
            );
            ensure!(
                openapi_has_capability(production_openapi, capability_id)?
                    || openapi_has_capability(conformance_openapi, capability_id)?,
                "OpenAPI operation is absent"
            );
        }
        "sse_asyncapi" => {
            ensure!(
                binding == "asyncapi:{capability_id}",
                "unknown AsyncAPI binding"
            );
            ensure!(
                object_at(asyncapi, "/operations")?
                    .values()
                    .any(|operation| {
                        string_at(operation, "/x-fasti-capability-id").ok() == Some(capability_id)
                    }),
                "AsyncAPI operation is absent"
            );
        }
        "cli" => {
            ensure!(binding == "cli:capability-discovery", "unknown CLI binding");
            let source =
                fs::read_to_string(workspace_root.join("crates/fasti-cli/src/capabilities.rs"))?;
            let main_source =
                fs::read_to_string(workspace_root.join("crates/fasti-cli/src/main.rs"))?;
            let tests = fs::read_to_string(
                workspace_root.join("crates/fasti-cli/tests/capability_commands.rs"),
            )?;
            ensure!(
                source.contains("PUBLIC_REGISTRY")
                    && source.contains("CapabilityCatalog")
                    && source.contains("public_capability_id(CapabilityKey::DiscoverCapabilities)")
                    && !source.contains("\"system.capabilities.discover\"")
                    && source.contains("scope=cli_local")
                    && !source.contains("CliFailure::new(")
                    && main_source.matches("CliFailure::new(").count() == 1
                    && main_source.contains("fn unavailable(")
                    && tests.contains("for resource in resources")
                    && tests.contains("document[\"resource_count\"]")
                    && capability_keys.contains_key(capability_id),
                "CLI capability discovery does not generically cover this capability or still claims local failures as capability problems"
            );
        }
        "json_schema" => match binding {
            "schema:health-response" => ensure!(
                health_schema.get("$schema").is_some(),
                "health response schema is absent"
            ),
            "schema:openapi-operation:{capability_id}" => ensure!(
                openapi_has_capability(conformance_openapi, capability_id)?,
                "conformance operation schema is absent"
            ),
            "schema:asyncapi-message:receiptCommitted" => ensure!(
                asyncapi
                    .pointer("/components/messages/receiptCommitted/payload/schema")
                    .is_some(),
                "receiptCommitted AsyncAPI message schema is absent"
            ),
            _ => anyhow::bail!("unknown JSON Schema binding"),
        },
        "json_ld" => {
            ensure!(
                binding == "json-ld:observation-receipt"
                    && workspace_root
                        .join("contracts/jsonld/v1/context.jsonld")
                        .is_file(),
                "observation receipt JSON-LD context is absent"
            );
        }
        "okf" => {
            ensure!(
                binding == "okf:capability-catalog"
                    && workspace_root
                        .join("contracts/okf/v1/capabilities.md")
                        .is_file(),
                "OKF capability catalog is absent"
            );
        }
        "sdk" => {
            ensure!(
                binding == "sdk:{capability_id}" || binding == "sdk:system.health",
                "unknown SDK binding"
            );
            ensure!(
                capability_id == "system.health"
                    || capability_id == "receipt.stream"
                    || CONFORMANCE_OPERATIONS
                        .iter()
                        .any(|operation| operation.capability_id == capability_id),
                "generated SDK capability is absent"
            );
        }
        "knowledge" => {
            ensure!(
                binding == "knowledge:problem-catalog",
                "unknown knowledge binding"
            );
            ensure!(
                array_at(problem_catalog, "/problems")?
                    .iter()
                    .any(|problem| {
                        string_at(problem, "/capability_id").ok() == Some(capability_id)
                    }),
                "canonical problem catalog entry is absent"
            );
        }
        "package_smoke" => match binding {
            "package-smoke:production-health" => {
                let smoke = fs::read_to_string(workspace_root.join("scripts/smoke-oci.sh"))?;
                ensure!(
                    smoke.contains("/api/v1/health"),
                    "production health smoke is absent"
                );
            }
            "package-smoke:b1-conformance-fixture" => {
                let test = fs::read_to_string(workspace_root.join("tests/js/sdk-client.test.mjs"))?;
                let sdk_method = b1_sdk_method(capability_id).with_context(|| {
                    format!("no capability-specific B1 package smoke mapping for {capability_id}")
                })?;
                ensure!(
                    test.contains("loopback Rust fixture")
                        && test.contains("withRustFixture")
                        && test.contains(&format!(".{sdk_method}(")),
                    "B1 conformance package smoke does not exercise {capability_id} through {sdk_method}"
                );
            }
            _ => anyhow::bail!("unknown package-smoke binding"),
        },
        other => anyhow::bail!("unsupported required surface {other}"),
    }
    Ok(())
}

fn b1_sdk_method(capability_id: &str) -> Option<&'static str> {
    match capability_id {
        "system.capabilities.discover" => Some("discoverCapabilities"),
        "profile.select" => Some("selectProfile"),
        "credential.rotate" => Some("rotateCredential"),
        "credential.revoke" => Some("revokeCredential"),
        "listener.configure" => Some("configureListener"),
        "node.initialize" => Some("initializeNode"),
        "client.enroll" => Some("enrollFirstClient"),
        "observation.accept" => Some("acceptObservation"),
        "receipt.replay" => Some("replayReceipt"),
        "receipt.stream" => Some("receiptEvents"),
        _ => None,
    }
}

fn openapi_has_capability(openapi: &Value, capability_id: &str) -> anyhow::Result<bool> {
    Ok(object_at(openapi, "/paths")?.values().any(|path| {
        path.as_object().is_some_and(|methods| {
            methods.values().any(|operation| {
                string_at(operation, "/x-fasti-capability-id").ok() == Some(capability_id)
            })
        })
    }))
}

fn validate_problem_responses(
    operation: &Map<String, Value>,
    expected: ConformanceOperation,
    capability_keys: &BTreeMap<String, CapabilityKey>,
    problems: &[Value],
) -> anyhow::Result<()> {
    let capability_key = *capability_keys
        .get(expected.capability_id)
        .with_context(|| {
            format!(
                "conformance capability {} has no application key",
                expected.capability_id
            )
        })?;
    let responses = operation
        .get("responses")
        .and_then(Value::as_object)
        .context("conformance operation responses must be an object")?;
    let mut represented_statuses = BTreeSet::new();
    for problem in problems {
        let raw_code = problem
            .as_str()
            .context("registry problem code must be a string")?;
        let code = ProblemCode::from_code(raw_code).with_context(|| {
            format!(
                "conformance capability {} claims unknown problem {raw_code}",
                expected.capability_id
            )
        })?;
        if code.contract().param_policy() == ProblemParamPolicy::ReceiptIdentifierByCapability {
            ensure!(
                matches!(
                    capability_key,
                    CapabilityKey::ReplayReceipt | CapabilityKey::StreamReceipts
                ),
                "conformance capability {} cannot represent problem {raw_code}",
                expected.capability_id
            );
        }
        let status = code.contract().status().to_string();
        let response = responses.get(&status).with_context(|| {
            format!(
                "{} {} cannot represent governed problem {raw_code}: response {status} is absent",
                expected.method, expected.path
            )
        })?;
        ensure!(
            string_at(response, "/content/application~1problem+json/schema/$ref")?
                == "#/components/schemas/ProblemDetails",
            "{} {} cannot represent governed problem {raw_code} as ProblemDetails",
            expected.method,
            expected.path
        );
        represented_statuses.insert(status);
    }

    let documented_problem_statuses: BTreeSet<_> = responses
        .iter()
        .filter_map(|(status, response)| {
            response
                .pointer("/content/application~1problem+json")
                .is_some()
                .then_some(status.clone())
        })
        .collect();
    ensure!(
        documented_problem_statuses == represented_statuses,
        "{} {} problem responses drift from registry claims: documented={documented_problem_statuses:?}, governed={represented_statuses:?}",
        expected.method,
        expected.path
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn bind_governed_examples(
    workspace_root: &Path,
    operation: &mut Map<String, Value>,
    public_registry: &Value,
    capability: &Value,
    expected: ConformanceOperation,
    capability_keys: &BTreeMap<String, CapabilityKey>,
    examples: &[Value],
    capability_discovery_example: &Value,
) -> anyhow::Result<()> {
    for example in examples {
        let example_id = example
            .as_str()
            .context("registry example ID must be a string")?;
        let governed =
            load_governed_example(workspace_root, example_id, capability_discovery_example)?;
        if governed.media_type == "application/ld+json" {
            let profile = string_at(capability, "/surface_profile")?;
            let profile_pointer = format!(
                "/surface_profiles/{}/json_ld/state",
                escape_pointer(profile)
            );
            ensure!(
                string_at(public_registry, &profile_pointer)? == "required",
                "linked-data example {example_id} is not owned by a required JSON-LD surface"
            );
            continue;
        }

        let (status, media_type) = if let Some(code) = governed.payload.get("code") {
            let code = code
                .as_str()
                .context("problem example code must be a string")?;
            ensure!(
                array_at(capability, "/problems")?
                    .iter()
                    .any(|problem| problem.as_str() == Some(code)),
                "example {example_id} uses ungoverned problem {code}"
            );
            ensure!(
                string_at(&governed.payload, "/capability_id")? == expected.capability_id,
                "example {example_id} claims another capability"
            );
            let status = u64_at(&governed.payload, "/status")?;
            let canonical = ProblemCode::from_code(code)
                .with_context(|| format!("example {example_id} uses unknown problem {code}"))?;
            ensure!(
                status == u64::from(canonical.contract().status()),
                "example {example_id} status differs from canonical problem {code}"
            );
            validate_problem_example_semantics(
                example_id,
                &governed.payload,
                expected.capability_id,
                *capability_keys
                    .get(expected.capability_id)
                    .with_context(|| {
                        format!("example {example_id} capability has no application key")
                    })?,
                canonical,
            )?;
            (status.to_string(), "application/problem+json")
        } else {
            ensure!(
                example_id == "system.capabilities.success",
                "finite HTTP example {example_id} has no deterministic response binding rule"
            );
            ("200".to_owned(), "application/json")
        };
        insert_openapi_example(
            operation,
            &status,
            media_type,
            example_id,
            governed.payload,
            expected,
        )?;
    }
    Ok(())
}

fn validate_problem_example_semantics(
    example_id: &str,
    payload: &Value,
    capability_id: &str,
    capability_key: CapabilityKey,
    code: ProblemCode,
) -> anyhow::Result<()> {
    let contract = code.contract();
    let action = contract.default_next_action();
    let expected = serde_json::json!({
        "type": format!("{DOCUMENTATION_BASE}/{}", contract.documentation_path()),
        "title": contract.title(),
        "status": contract.status(),
        "detail": contract.detail(capability_key),
        "code": code.as_str(),
        "capability_id": capability_id,
        "safe_state": contract.safe_state().as_str(),
        "retryability": contract.retryability().as_str(),
        "next_actions": [{ "id": action.id(), "label": action.label() }],
        "param": contract.param_policy().resolve(capability_key),
        "actual": null,
    });
    for field in [
        "type",
        "title",
        "status",
        "detail",
        "code",
        "capability_id",
        "safe_state",
        "retryability",
        "next_actions",
        "param",
        "actual",
    ] {
        ensure!(
            payload.get(field) == expected.get(field),
            "problem example {example_id} field {field} differs from its canonical application contract"
        );
    }
    if let Some(violation) = code.representation_violation() {
        ensure!(
            payload.get("violations")
                == Some(&serde_json::json!([{
                    "code": violation.code(),
                    "pointer": violation.pointer(),
                    "reason": violation.reason(),
                    "expected": violation.expected(),
                    "actual": null,
                }])),
            "problem example {example_id} validation violations differ from the runtime representation-rejection contract"
        );
    }
    Ok(())
}

struct GovernedExample {
    media_type: &'static str,
    payload: Value,
}

fn load_governed_example(
    workspace_root: &Path,
    example_id: &str,
    capability_discovery_example: &Value,
) -> anyhow::Result<GovernedExample> {
    if example_id == "system.capabilities.success" {
        return Ok(GovernedExample {
            media_type: "application/json",
            payload: capability_discovery_example.clone(),
        });
    }
    let candidates = [
        ("json", "application/json"),
        ("jsonld", "application/ld+json"),
    ];
    let present: Vec<_> = candidates
        .into_iter()
        .filter_map(|(extension, media_type)| {
            let path = workspace_root
                .join(EXAMPLES_DIRECTORY)
                .join(format!("{example_id}.{extension}"));
            path.is_file().then_some((path, media_type))
        })
        .collect();
    ensure!(
        present.len() == 1,
        "example {example_id} must resolve to exactly one governed JSON or JSON-LD file"
    );
    let (path, media_type) = &present[0];
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read governed example {}", path.display()))?;
    let payload = serde_json::from_slice(&bytes)
        .with_context(|| format!("governed example {} is not JSON", path.display()))?;
    Ok(GovernedExample {
        media_type,
        payload,
    })
}

fn insert_openapi_example(
    operation: &mut Map<String, Value>,
    status: &str,
    media_type: &str,
    example_id: &str,
    payload: Value,
    expected: ConformanceOperation,
) -> anyhow::Result<()> {
    let response = operation
        .get_mut("responses")
        .and_then(Value::as_object_mut)
        .and_then(|responses| responses.get_mut(status))
        .with_context(|| {
            format!(
                "example {example_id} cannot bind: {} {} response {status} is absent",
                expected.method, expected.path
            )
        })?;
    let media = response
        .get_mut("content")
        .and_then(Value::as_object_mut)
        .and_then(|content| content.get_mut(media_type))
        .and_then(Value::as_object_mut)
        .with_context(|| {
            format!(
                "example {example_id} cannot bind: {} {} response {status} omits {media_type}",
                expected.method, expected.path
            )
        })?;
    let examples = media
        .entry("examples")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .context("OpenAPI response examples must be an object")?;
    ensure!(
        examples
            .insert(
                example_id.to_owned(),
                serde_json::json!({ "value": payload })
            )
            .is_none(),
        "OpenAPI response already contains example {example_id}"
    );
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

fn validate_receipt_stream_metadata(asyncapi: &Value) -> anyhow::Result<()> {
    ensure!(
        string_at(
            asyncapi,
            "/components/messages/receiptCommitted/x-fasti-sse-id-pointer"
        )? == "$message.payload#/receipt_id",
        "receipt SSE id must be governed by the payload receipt_id"
    );
    ensure!(
        string_at(
            asyncapi,
            "/operations/sendReceiptCommitted/x-fasti-durability"
        )? == "none",
        "B1 receipt stream durability must remain explicitly none"
    );
    ensure!(
        string_at(
            asyncapi,
            "/operations/sendReceiptCommitted/x-fasti-fixture-delivery"
        )? == "finite_replay_then_close",
        "B1 receipt fixture must declare finite replay then clean close"
    );
    Ok(())
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
    problem_catalog: &Value,
    health_schema: &Value,
    problem_schema: &Value,
    asyncapi: &Value,
    conformance_openapi: &Value,
) -> anyhow::Result<String> {
    validate_receipt_stream_metadata(asyncapi)?;
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
    let problem_catalog_json = serde_json::to_string_pretty(&sort_json(problem_catalog.clone()))?;
    writeln!(
        output,
        "// prettier-ignore\nexport const PUBLIC_PROBLEM_CATALOG = {problem_catalog_json} as const;\n"
    )?;
    output.push_str(
        "export const CAPABILITY_REGISTRY = PUBLIC_CAPABILITY_REGISTRY.capabilities;\nexport const SURFACE_PROFILES = PUBLIC_CAPABILITY_REGISTRY.surface_profiles;\nexport type CapabilityMetadata = (typeof CAPABILITY_REGISTRY)[number];\nexport type SurfaceProfileMetadata = typeof SURFACE_PROFILES;\nexport type CanonicalProblemMetadata = (typeof PUBLIC_PROBLEM_CATALOG.problems)[number];\n\n",
    );

    let stream_path = string_at(asyncapi, "/channels/receiptEvents/address")?;
    let event_name = string_at(asyncapi, "/components/messages/receiptCommitted/name")?;
    ensure!(
        event_name == "receiptCommitted",
        "receipt event name changed; update the generated envelope contract deliberately"
    );
    let sse_id_pointer = string_at(
        asyncapi,
        "/components/messages/receiptCommitted/x-fasti-sse-id-pointer",
    )?;
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
    let registry_stream_problems: BTreeSet<_> = array_at(registry_capability, "/problems")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .context("registry stream problem must be a string")
                .map(ToOwned::to_owned)
        })
        .collect::<anyhow::Result<_>>()?;
    let async_stream_problems: BTreeSet<_> = array_at(
        asyncapi,
        "/operations/sendReceiptCommitted/x-fasti-http-problems/responses",
    )?
    .iter()
    .map(|response| string_at(response, "/code").map(ToOwned::to_owned))
    .collect::<anyhow::Result<_>>()?;
    ensure!(
        async_stream_problems == registry_stream_problems,
        "AsyncAPI receipt problems must exactly equal the registry-owned problem set"
    );
    let scopes_json = serde_json::to_string(&async_scopes)?;
    let stream_problems_json = serde_json::to_string(&registry_stream_problems)?;
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
    let durability = string_at(
        asyncapi,
        "/operations/sendReceiptCommitted/x-fasti-durability",
    )?;
    let fixture_delivery = string_at(
        asyncapi,
        "/operations/sendReceiptCommitted/x-fasti-fixture-delivery",
    )?;
    ensure!(
        runtime_availabilities.contains(runtime_availability),
        "AsyncAPI runtime availability is absent from the registry vocabulary"
    );
    writeln!(
        output,
        "export const RECEIPT_STREAM_CONTRACT = {{\n  path: {},\n  eventName: {},\n  sseIdPointer: {},\n  capabilityId: {},\n  requiredScopes: {},\n  problemCodes: {},\n  runtimeAvailability: {},\n  durability: {},\n  fixtureDelivery: {},\n  maximumReplayBatch: {},\n  retryPolicy: {},\n}} as const;\n",
        json_string(stream_path)?,
        json_string(event_name)?,
        json_string(sse_id_pointer)?,
        json_string(capability_id)?,
        scopes_json,
        stream_problems_json,
        json_string(runtime_availability)?,
        json_string(durability)?,
        json_string(fixture_delivery)?,
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
    let problem_correlation_pattern =
        string_at(problem_schema, "/properties/correlation_id/pattern")?;
    ensure!(
        problem_correlation_pattern == correlation_pattern,
        "ProblemDetails and receipt events must use one canonical correlation ID pattern"
    );
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
  patternString(object, "correlation_id", CORRELATION_ID, "ProblemDetails");
  integerField(object, "status", "ProblemDetails", 0, 65_535);
  nullableStringField(object, "param", "ProblemDetails");
  exactNullField(object, "actual", "ProblemDetails");
  const actions = arrayField(object, "next_actions", "ProblemDetails");
  if (actions.length !== 1) {{
    throw new FastiContractParseError("ProblemDetails.next_actions must contain exactly one canonical action");
  }}
  actions.forEach(parseProblemAction);
  const violations = arrayField(object, "violations", "ProblemDetails");
  if (violations.length > 32) {{
    throw new FastiContractParseError("ProblemDetails.violations exceeds the bounded violation count");
  }}
  violations.forEach(parseViolation);
  return object as unknown as ProblemDetails;
}}

// prettier-ignore
export function parseProblemDetailsForOperation(
  value: unknown,
  capabilityId: CapabilityId,
  allowedCodes: readonly ProblemCode[],
): ProblemDetails {{
  const problem = parseProblemDetails(value);
  if (problem.capability_id !== capabilityId) {{
    throw new FastiContractParseError("ProblemDetails capability does not match the requested operation");
  }}
  if (!allowedCodes.includes(problem.code)) {{
    throw new FastiContractParseError("ProblemDetails code is not governed for the requested operation");
  }}
  const canonical = PUBLIC_PROBLEM_CATALOG.problems.find(
    (entry) => entry.capability_id === capabilityId && entry.code === problem.code,
  );
  if (canonical === undefined) {{
    throw new FastiContractParseError("ProblemDetails has no canonical capability problem contract");
  }}
  if (
    problem.type !== canonical.type ||
    problem.title !== canonical.title ||
    problem.status !== canonical.status ||
    problem.detail !== canonical.detail ||
    problem.safe_state !== canonical.safe_state ||
    problem.retryability !== canonical.retryability ||
    (problem.param ?? null) !== canonical.param ||
    problem.next_actions.length !== canonical.next_actions.length ||
    problem.next_actions.some((action, index) =>
      action.id !== canonical.next_actions[index]?.id ||
      action.label !== canonical.next_actions[index]?.label
    )
  ) {{
    throw new FastiContractParseError("ProblemDetails differs from its canonical application contract");
  }}
  return problem;
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
  exactNullField(object, "actual", "ViolationDto");
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
function exactNullField(object: JsonObject, field: string, label: string): void {{
  if (object[field] !== null) {{
    throw new FastiContractParseError(`${{label}}.${{field}} must be null`);
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
        let problem_codes = array_at(operation, "/x-fasti-problem-codes")?;
        let problem_codes_json = serde_json::to_string(problem_codes)?;
        let example_ids = array_at(operation, "/x-fasti-example-ids")?;
        let example_ids_json = serde_json::to_string(example_ids)?;
        let runtime_availability = string_at(operation, "/x-fasti-runtime-availability")?;
        let authorization = string_at(operation, "/x-fasti-authorization")?;
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
            "  {alias}: {{ operationId: {}, method: {}, path: {}, capabilityId: {}, authorization: {}, requiredScopes: {required_scopes_json}, problemCodes: {problem_codes_json}, exampleIds: {example_ids_json}, authenticated: {authenticated}, runtimeAvailability: {}, durability: \"none\", retry: {}, requestSchema: {}, responseSchema: {} }},",
            json_string(operation_id)?,
            json_string(&method.to_ascii_uppercase())?,
            json_string(path)?,
            json_string(capability_id)?,
            json_string(authorization)?,
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
  const response = parseConformanceDto<CapabilityDiscoveryResponse>("CapabilityDiscoveryResponse", value);
  if (
    response.contract_version !== PUBLIC_CAPABILITY_REGISTRY.contract_version ||
    response.capability_base_uri !== PUBLIC_CAPABILITY_REGISTRY.capability_base_uri ||
    !contractJsonEqual(response.surface_profiles, PUBLIC_CAPABILITY_REGISTRY.surface_profiles) ||
    !contractJsonEqual(response.capabilities, PUBLIC_CAPABILITY_REGISTRY.capabilities)
  ) {
    throw new FastiContractParseError("CapabilityDiscoveryResponse differs from the complete generated registry handshake");
  }
  return response;
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
    if (typeof schema.minItems === "number" && value.length < schema.minItems) {
      throw new FastiContractParseError(`${path} has fewer than its bounded items`);
    }
    if (typeof schema.maxItems === "number" && value.length > schema.maxItems) {
      throw new FastiContractParseError(`${path} exceeds its bounded items`);
    }
    value.forEach((item, index) => validateOpenApiValue(item, schema.items, `${path}[${index}]`));
    return;
  }
  if (schemaTypes.includes("object")) {
    if (!isPlainObject(value)) {
      throw new FastiContractParseError(`${path} must be a plain object`);
    }
    const object = value as Record<string, unknown>;
    const keys = Object.keys(object);
    if (typeof schema.minProperties === "number" && keys.length < schema.minProperties) {
      throw new FastiContractParseError(`${path} has fewer than its bounded properties`);
    }
    if (typeof schema.maxProperties === "number" && keys.length > schema.maxProperties) {
      throw new FastiContractParseError(`${path} exceeds its bounded properties`);
    }
    const properties = isPlainObject(schema.properties)
      ? (schema.properties as Record<string, unknown>)
      : {};
    for (const key of keys) {
      if (isPlainObject(schema.propertyNames)) {
        validateOpenApiValue(key, schema.propertyNames, `${path} property name`);
      }
      if (!Object.hasOwn(properties, key)) {
        if (schema.additionalProperties === false) {
          throw new FastiContractParseError(`${path} contains unknown field ${key}`);
        }
        if (isPlainObject(schema.additionalProperties)) {
          validateOpenApiValue(object[key], schema.additionalProperties, `${path}.${key}`);
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
function contractJsonEqual(left: unknown, right: unknown): boolean {
  if (left === right) return true;
  if (Array.isArray(left) || Array.isArray(right)) {
    return Array.isArray(left) && Array.isArray(right) &&
      left.length === right.length && left.every((value, index) => contractJsonEqual(value, right[index]));
  }
  if (!isPlainObject(left) || !isPlainObject(right)) return false;
  const leftKeys = Object.keys(left).sort();
  const rightKeys = Object.keys(right).sort();
  return leftKeys.length === rightKeys.length &&
    leftKeys.every((key, index) => key === rightKeys[index] && contractJsonEqual(left[key], right[key]));
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
            "object" => {
                let additional = value_at(schema, "/additionalProperties")?;
                ensure!(
                    !additional.is_boolean(),
                    "generated object map must define a value schema"
                );
                Ok(format!(
                    "Readonly<Record<string, {}>>",
                    typescript_type(additional)?
                ))
            }
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
            Path::new(PROBLEM_CATALOG_PATH),
            Path::new(CAPABILITY_DISCOVERY_EXAMPLE_PATH),
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
            let authorization = string_at(operation, "/x-fasti-authorization")
                .expect("authorization annotation is present");
            let scopes = array_at(operation, "/x-fasti-required-scopes")
                .expect("scope annotation is present");
            if expected.capability_id == "node.initialize" {
                assert_eq!(authorization, "bootstrap_only");
                assert!(scopes.is_empty());
            } else {
                assert_eq!(authorization, "scoped");
                assert!(!scopes.is_empty());
            }
            assert_eq!(
                string_at(operation, "/x-fasti-runtime-availability")
                    .expect("runtime annotation is present"),
                "fixture_only"
            );
            assert!(!array_at(operation, "/x-fasti-problem-codes")
                .expect("problem annotation is present")
                .is_empty());
            assert!(operation.get("x-fasti-example-ids").is_some());
            for example in
                array_at(operation, "/x-fasti-example-ids").expect("example annotation is present")
            {
                let example = example.as_str().expect("example ID is a string");
                if example == "observation.accept.receipt" {
                    continue;
                }
                assert!(operation
                    .pointer("/responses")
                    .expect("responses")
                    .to_string()
                    .contains(&format!("\"{example}\"")));
            }
        }
    }

    #[test]
    fn production_health_has_exact_registry_annotations_and_example() {
        let artifacts = build(workspace_root()).expect("contract generation succeeds");
        let openapi: Value = serde_json::from_slice(
            artifacts
                .get(Path::new(OPENAPI_PATH))
                .expect("production OpenAPI generated"),
        )
        .expect("production OpenAPI JSON");
        let operation =
            value_at(&openapi, "/paths/~1api~1v1~1health/get").expect("health operation");
        assert_eq!(
            string_at(operation, "/x-fasti-capability-id").expect("capability ID"),
            "system.health"
        );
        assert_eq!(
            string_at(operation, "/x-fasti-runtime-availability").expect("availability"),
            "implemented"
        );
        assert_eq!(
            string_at(operation, "/x-fasti-authorization").expect("authorization"),
            "unauthenticated"
        );
        assert!(array_at(operation, "/x-fasti-required-scopes")
            .expect("scopes")
            .is_empty());
        assert_eq!(
            string_at(
                operation,
                "/responses/200/content/application~1json/examples/system.health.success/value/status"
            )
            .expect("embedded health example"),
            "healthy"
        );
    }

    #[test]
    fn discovery_openapi_exposes_finite_registry_vocabularies() {
        let artifacts = build(workspace_root()).expect("contract generation succeeds");
        let openapi: Value = serde_json::from_slice(
            artifacts
                .get(Path::new(CONFORMANCE_OPENAPI_PATH))
                .expect("conformance OpenAPI generated"),
        )
        .expect("conformance OpenAPI JSON");
        for pointer in [
            "/components/schemas/CapabilityDescriptorDto/properties/id/enum",
            "/components/schemas/CapabilityDescriptorDto/properties/authorization/enum",
            "/components/schemas/CapabilityDescriptorDto/properties/contract_body/enum",
            "/components/schemas/CapabilityDescriptorDto/properties/runtime_body/enum",
            "/components/schemas/CapabilityDescriptorDto/properties/surface_profile/enum",
            "/components/schemas/CapabilityLifecycleDto/properties/contract_state/enum",
            "/components/schemas/CapabilityLifecycleDto/properties/runtime_availability/enum",
            "/components/schemas/CapabilitySurfaceDispositionDto/properties/state/enum",
            "/components/schemas/CapabilitySurfaceDispositionDto/properties/binding_visibility/enum",
            "/components/schemas/CapabilityUatDto/properties/relationship/enum",
        ] {
            assert!(
                !array_at(&openapi, pointer)
                    .unwrap_or_else(|_| panic!("missing finite vocabulary {pointer}"))
                    .is_empty(),
                "finite vocabulary {pointer} cannot be empty"
            );
        }
        assert_eq!(
            value_at(
                &openapi,
                "/components/schemas/CapabilityDescriptorDto/properties/scopes/uniqueItems"
            )
            .expect("scope uniqueness"),
            &Value::Bool(true)
        );
    }

    #[test]
    fn validation_example_violation_mutations_are_rejected() {
        let path = workspace_root()
            .join(EXAMPLES_DIRECTORY)
            .join("observation.accept.validation_failed.json");
        let baseline: Value = serde_json::from_slice(&fs::read(path).expect("validation example"))
            .expect("validation example JSON");
        for (field, replacement) in [
            ("code", "another_code"),
            ("pointer", "/another"),
            ("reason", "another reason"),
            ("expected", "another expectation"),
        ] {
            let mut mutated = baseline.clone();
            mutated["violations"][0][field] = Value::String(replacement.to_owned());
            assert!(validate_problem_example_semantics(
                "observation.accept.validation_failed",
                &mutated,
                "observation.accept",
                CapabilityKey::AcceptObservation,
                ProblemCode::ValidationFailed,
            )
            .is_err());
        }
    }

    #[test]
    fn receipt_stream_metadata_mutations_are_rejected() {
        let baseline = load_yaml(workspace_root(), ASYNCAPI_PATH).expect("authored AsyncAPI");
        assert!(validate_receipt_stream_metadata(&baseline).is_ok());
        for (pointer, replacement) in [
            (
                "/components/messages/receiptCommitted/x-fasti-sse-id-pointer",
                "another-pointer",
            ),
            (
                "/operations/sendReceiptCommitted/x-fasti-durability",
                "durable",
            ),
            (
                "/operations/sendReceiptCommitted/x-fasti-fixture-delivery",
                "wait_forever",
            ),
        ] {
            let mut mutated = baseline.clone();
            *mutated.pointer_mut(pointer).expect("mutation pointer") =
                Value::String(replacement.to_owned());
            assert!(validate_receipt_stream_metadata(&mutated).is_err());
        }
    }

    #[test]
    fn problem_catalog_and_discovery_example_are_complete_and_sorted() {
        let artifacts = build(workspace_root()).expect("contract generation succeeds");
        let registry: Value = serde_json::from_slice(
            artifacts
                .get(Path::new(CAPABILITY_REGISTRY_PATH))
                .expect("public registry generated"),
        )
        .expect("registry JSON");
        let example: Value = serde_json::from_slice(
            artifacts
                .get(Path::new(CAPABILITY_DISCOVERY_EXAMPLE_PATH))
                .expect("discovery example generated"),
        )
        .expect("example JSON");
        assert_eq!(
            array_at(&example, "/capabilities").expect("example capabilities"),
            array_at(&registry, "/capabilities").expect("registry capabilities")
        );
        assert_eq!(
            string_at(&example, "/contract_version").expect("example version"),
            string_at(&registry, "/contract_version").expect("registry version")
        );
        assert_eq!(
            value_at(&example, "/surface_profiles").expect("example profiles"),
            value_at(&registry, "/surface_profiles").expect("registry profiles")
        );
        assert_eq!(
            array_at(&example, "/capabilities")
                .expect("example capabilities")
                .len(),
            CapabilityKey::ALL.len()
        );

        let catalog: Value = serde_json::from_slice(
            artifacts
                .get(Path::new(PROBLEM_CATALOG_PATH))
                .expect("problem catalog generated"),
        )
        .expect("problem catalog JSON");
        let governed_pairs: usize = array_at(&registry, "/capabilities")
            .expect("registry capabilities")
            .iter()
            .map(|capability| {
                array_at(capability, "/problems")
                    .expect("capability problems")
                    .len()
            })
            .sum();
        assert_eq!(
            array_at(&catalog, "/problems")
                .expect("catalog problems")
                .len(),
            governed_pairs
        );
        assert!(catalog.to_string().contains("\"param_policy\""));
        assert!(!catalog.to_string().contains("application_key"));
    }

    fn workspace_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask lives under workspace root")
    }
}
