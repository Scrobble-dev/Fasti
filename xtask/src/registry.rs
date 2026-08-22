use anyhow::{ensure, Context};
use fasti_application::{
    AuthorizationKind, AuthorizationRequirement, CapabilityBody, CapabilityKey, ContractState,
    ProblemCode, RuntimeAvailability, ScopeKey,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::Path;

const REGISTRY_PATH: &str = "contracts/registry/v1/capabilities.yaml";
const EXPECTED_PROFILES: [&str; 7] = [
    "b1_http_fixture",
    "b1_observation_accept",
    "b1_receipt_replay",
    "b1_receipt_stream",
    "health",
    "later_b2",
    "later_b3",
];

#[derive(Debug)]
pub struct ValidationSummary {
    pub contract_version: String,
    pub capability_count: usize,
    pub surface_profile_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Registry {
    contract_version: String,
    capability_base_uri: String,
    surface_profiles: BTreeMap<String, SurfaceProfile>,
    capabilities: Vec<Capability>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SurfaceProfile {
    domain_application: SurfaceDisposition,
    http_openapi: SurfaceDisposition,
    sse_asyncapi: SurfaceDisposition,
    cli: SurfaceDisposition,
    json_schema: SurfaceDisposition,
    json_ld: SurfaceDisposition,
    okf: SurfaceDisposition,
    sdk: SurfaceDisposition,
    knowledge: SurfaceDisposition,
    package_smoke: SurfaceDisposition,
    ui: SurfaceDisposition,
}

impl SurfaceProfile {
    fn surfaces(&self) -> [(&'static str, &SurfaceDisposition); 11] {
        [
            ("domain_application", &self.domain_application),
            ("http_openapi", &self.http_openapi),
            ("sse_asyncapi", &self.sse_asyncapi),
            ("cli", &self.cli),
            ("json_schema", &self.json_schema),
            ("json_ld", &self.json_ld),
            ("okf", &self.okf),
            ("sdk", &self.sdk),
            ("knowledge", &self.knowledge),
            ("package_smoke", &self.package_smoke),
            ("ui", &self.ui),
        ]
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SurfaceDisposition {
    state: SurfaceState,
    #[serde(skip_serializing_if = "Option::is_none")]
    binding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<CapabilityBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SurfaceState {
    Required,
    LaterBody,
    NotApplicable,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Capability {
    #[serde(skip_serializing)]
    application_key: CapabilityKey,
    id: String,
    bounded_context: String,
    contract_body: CapabilityBody,
    runtime_body: CapabilityBody,
    authorization: AuthorizationKind,
    lifecycle: RegistryLifecycle,
    surface_profile: String,
    scopes: Vec<ScopeKey>,
    problems: Vec<String>,
    examples: Vec<String>,
    uat: Vec<UatOwnership>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryLifecycle {
    introduced_in: CapabilityBody,
    contract_state: ContractState,
    runtime_availability: RuntimeAvailability,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UatOwnership {
    id: String,
    relationship: UatRelationship,
    owner_body: CapabilityBody,
    reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum UatRelationship {
    Direct,
    Split,
    Deferred,
}

pub fn validate(workspace_root: &Path) -> anyhow::Result<ValidationSummary> {
    let registry = load_validated(workspace_root)?;

    Ok(ValidationSummary {
        contract_version: registry.contract_version,
        capability_count: registry.capabilities.len(),
        surface_profile_count: registry.surface_profiles.len(),
    })
}

pub(crate) fn normalized_public_json(workspace_root: &Path) -> anyhow::Result<serde_json::Value> {
    let mut registry = load_validated(workspace_root)?;
    registry
        .capabilities
        .sort_by(|left, right| left.id.cmp(&right.id));
    for capability in &mut registry.capabilities {
        capability
            .scopes
            .sort_by_key(|scope| serde_json::to_string(scope).unwrap_or_default());
        capability.problems.sort();
        capability.examples.sort();
        capability.uat.sort_by(|left, right| left.id.cmp(&right.id));
    }

    let mut public =
        serde_json::to_value(registry).context("public capability registry is not serializable")?;
    let profiles = public
        .get_mut("surface_profiles")
        .and_then(serde_json::Value::as_object_mut)
        .context("public surface_profiles must be an object")?;
    for profile in profiles.values_mut() {
        let surfaces = profile
            .as_object_mut()
            .context("public surface profile must be an object")?;
        for disposition in surfaces.values_mut() {
            let disposition = disposition
                .as_object_mut()
                .context("public surface disposition must be an object")?;
            let visibility = match disposition
                .get("binding")
                .and_then(serde_json::Value::as_str)
            {
                Some("application:{application_key}") => {
                    disposition.remove("binding");
                    Some("internal")
                }
                Some(_) => Some("public"),
                None => None,
            };
            if let Some(visibility) = visibility {
                disposition.insert(
                    "binding_visibility".to_owned(),
                    serde_json::Value::String(visibility.to_owned()),
                );
            }
        }
    }
    Ok(public)
}

#[derive(Debug)]
pub(crate) struct RequiredBinding {
    pub capability_id: String,
    pub surface: &'static str,
    pub binding: String,
}

pub(crate) fn finalized_b1_required_bindings(
    workspace_root: &Path,
) -> anyhow::Result<Vec<RequiredBinding>> {
    let registry = load_validated(workspace_root)?;
    let mut bindings = Vec::new();
    for capability in registry.capabilities {
        if capability.contract_body != CapabilityBody::B1
            || capability.lifecycle.contract_state != ContractState::Finalized
        {
            continue;
        }
        let profile = registry
            .surface_profiles
            .get(&capability.surface_profile)
            .context("validated capability profile disappeared")?;
        for (surface, disposition) in profile.surfaces() {
            if matches!(disposition.state, SurfaceState::Required) {
                bindings.push(RequiredBinding {
                    capability_id: capability.id.clone(),
                    surface,
                    binding: disposition
                        .binding
                        .clone()
                        .context("validated required binding disappeared")?,
                });
            }
        }
    }
    bindings.sort_by(|left, right| {
        (&left.capability_id, left.surface).cmp(&(&right.capability_id, right.surface))
    });
    Ok(bindings)
}

pub(crate) fn internal_key_id_pairs(
    workspace_root: &Path,
) -> anyhow::Result<Vec<(CapabilityKey, String)>> {
    let registry = load_validated(workspace_root)?;
    let mut pairs: Vec<_> = registry
        .capabilities
        .into_iter()
        .map(|capability| (capability.application_key, capability.id))
        .collect();
    pairs.sort_by_key(|(key, _)| format!("{key:?}"));
    Ok(pairs)
}

fn load_validated(workspace_root: &Path) -> anyhow::Result<Registry> {
    let path = workspace_root.join(REGISTRY_PATH);
    let source =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let registry: Registry = serde_saphyr::from_str(&source)
        .with_context(|| format!("{} is not strict registry YAML", path.display()))?;

    validate_header(&registry)?;
    validate_surface_profiles(&registry.surface_profiles)?;
    validate_capabilities(&registry)?;

    Ok(registry)
}

fn validate_header(registry: &Registry) -> anyhow::Result<()> {
    ensure!(
        registry.contract_version == "1.0.0",
        "contract_version must be exactly 1.0.0 for the v1 registry"
    );
    ensure!(
        registry.capability_base_uri == "https://fasti.scrobble.dev/ns/capabilities/v1/",
        "capability_base_uri must be the governed v1 HTTPS namespace"
    );
    Ok(())
}

fn validate_surface_profiles(profiles: &BTreeMap<String, SurfaceProfile>) -> anyhow::Result<()> {
    let actual: BTreeSet<_> = profiles.keys().map(String::as_str).collect();
    let expected: BTreeSet<_> = EXPECTED_PROFILES.into_iter().collect();
    ensure!(
        actual == expected,
        "surface profile keys differ: expected {expected:?}, found {actual:?}"
    );

    for (profile_name, profile) in profiles {
        for (surface_name, disposition) in profile.surfaces() {
            validate_disposition(profile_name, surface_name, disposition)?;
        }
    }
    Ok(())
}

fn validate_disposition(
    profile_name: &str,
    surface_name: &str,
    disposition: &SurfaceDisposition,
) -> anyhow::Result<()> {
    let location = format!("surface_profiles.{profile_name}.{surface_name}");
    match disposition.state {
        SurfaceState::Required => {
            ensure!(
                nonempty(disposition.binding.as_deref()),
                "{location}: required needs one non-empty binding"
            );
            ensure!(
                disposition.body.is_none() && disposition.reason.is_none(),
                "{location}: required allows binding only"
            );
        }
        SurfaceState::LaterBody => {
            ensure!(
                disposition.body.is_some() && nonempty(disposition.reason.as_deref()),
                "{location}: later_body needs body and reason"
            );
            ensure!(
                disposition.binding.is_none(),
                "{location}: later_body must not claim a binding"
            );
        }
        SurfaceState::NotApplicable => {
            ensure!(
                nonempty(disposition.reason.as_deref()),
                "{location}: not_applicable needs a reason"
            );
            ensure!(
                disposition.binding.is_none() && disposition.body.is_none(),
                "{location}: not_applicable allows reason only"
            );
        }
    }
    Ok(())
}

fn validate_capabilities(registry: &Registry) -> anyhow::Result<()> {
    let problem_codes: BTreeSet<_> = ProblemCode::ALL.iter().map(|code| code.as_str()).collect();
    let finalized_problem_codes: BTreeSet<_> = ProblemCode::ALL
        .iter()
        .filter(|code| code.contract_state() == ContractState::Finalized)
        .map(|code| code.as_str())
        .collect();
    let all_scopes: HashSet<_> = ScopeKey::ALL.iter().copied().collect();
    let mut seen_keys = HashSet::new();
    let mut seen_ids = BTreeSet::new();
    let mut used_profiles = BTreeSet::new();
    let mut used_problems = BTreeSet::new();
    let mut used_scopes = HashSet::new();

    for capability in &registry.capabilities {
        let label = format!("capability {:?}", capability.application_key);
        ensure!(
            seen_keys.insert(capability.application_key),
            "{label}: duplicate application_key"
        );
        ensure!(
            seen_ids.insert(capability.id.as_str()),
            "{label}: duplicate public ID {}",
            capability.id
        );
        ensure!(
            public_id_is_well_formed(&capability.id),
            "{label}: public ID must contain at least two lowercase dot-separated segments: {}",
            capability.id
        );
        ensure!(
            public_id_is_well_formed(&capability.bounded_context),
            "{label}: bounded_context must be a lowercase dot-separated identifier"
        );
        ensure!(
            capability.contract_body == capability.application_key.contract_body(),
            "{label}: contract_body disagrees with fasti-application"
        );
        ensure!(
            capability.runtime_body == capability.application_key.runtime_body(),
            "{label}: runtime_body disagrees with fasti-application"
        );
        ensure!(
            capability.authorization == capability.application_key.authorization_kind(),
            "{label}: authorization disagrees with fasti-application"
        );
        let requirement = AuthorizationRequirement::for_capability(capability.application_key);
        ensure!(
            match capability.authorization {
                AuthorizationKind::Unauthenticated => requirement.is_unauthenticated(),
                AuthorizationKind::BootstrapOnly => requirement.is_bootstrap_only(),
                AuthorizationKind::Scoped => {
                    !requirement.is_unauthenticated() && !requirement.is_bootstrap_only()
                }
            },
            "{label}: authorization disagrees with effective AuthorizationRequirement"
        );
        ensure!(
            body_rank(capability.lifecycle.introduced_in) <= body_rank(capability.contract_body),
            "{label}: introduced_in cannot follow contract_body"
        );
        validate_lifecycle(capability)?;

        ensure!(
            registry
                .surface_profiles
                .contains_key(&capability.surface_profile),
            "{label}: unknown surface_profile {}",
            capability.surface_profile
        );
        let expected_profile = expected_surface_profile(capability.application_key);
        ensure!(
            capability.surface_profile == expected_profile,
            "{label}: surface_profile violates contract policy; expected {expected_profile}, found {}",
            capability.surface_profile
        );
        used_profiles.insert(capability.surface_profile.as_str());

        let mut capability_scopes = HashSet::new();
        for scope in &capability.scopes {
            ensure!(
                all_scopes.contains(scope),
                "{label}: scope {scope:?} is not owned by ScopeKey::ALL"
            );
            ensure!(
                capability_scopes.insert(*scope),
                "{label}: duplicate scope {scope:?}"
            );
            used_scopes.insert(*scope);
        }
        ensure!(
            capability.scopes.as_slice() == requirement.required_scopes(),
            "{label}: scopes disagree with effective AuthorizationRequirement; expected {:?}, found {:?}",
            requirement.required_scopes(),
            capability.scopes
        );

        let mut capability_problems = BTreeSet::new();
        for problem in &capability.problems {
            ensure!(
                problem_codes.contains(problem.as_str()),
                "{label}: problem code {problem:?} is not owned by ProblemCode::ALL"
            );
            let problem_code = ProblemCode::from_code(problem)
                .with_context(|| format!("{label}: unknown problem code {problem:?}"))?;
            ensure!(
                problem_code.contract_state() == ContractState::Finalized,
                "{label}: reserved problem code {problem:?} cannot enter the public registry before {}",
                problem_code.introduced_in().as_str()
            );
            ensure!(
                body_rank(problem_code.introduced_in()) <= body_rank(capability.contract_body),
                "{label}: problem code {problem:?} cannot precede its owning body {}",
                problem_code.introduced_in().as_str()
            );
            ensure!(
                capability_problems.insert(problem.as_str()),
                "{label}: duplicate problem code {problem:?}"
            );
            used_problems.insert(problem.as_str());
        }
        let expected_problems: BTreeSet<_> = capability
            .application_key
            .allowed_problem_codes()
            .iter()
            .map(|problem| problem.as_str())
            .collect();
        ensure!(
            capability_problems == expected_problems,
            "{label}: problems disagree with fasti-application; expected {expected_problems:?}, found {capability_problems:?}"
        );

        let mut examples = BTreeSet::new();
        for example in &capability.examples {
            ensure!(
                public_id_is_well_formed(example),
                "{label}: example key must be a lowercase dot-separated identifier"
            );
            ensure!(
                examples.insert(example.as_str()),
                "{label}: duplicate example key {example:?}"
            );
        }
        if capability.contract_body == CapabilityBody::B1
            && capability.lifecycle.contract_state == ContractState::Finalized
        {
            ensure!(
                !examples.is_empty(),
                "{label}: every finalized B1 capability must own at least one example"
            );
        }
        validate_uat(&label, &capability.uat)?;
    }

    let expected_keys: HashSet<_> = CapabilityKey::ALL.iter().copied().collect();
    ensure!(
        seen_keys == expected_keys,
        "registry application keys differ from CapabilityKey::ALL: missing={:?}, unexpected={:?}",
        expected_keys.difference(&seen_keys).collect::<Vec<_>>(),
        seen_keys.difference(&expected_keys).collect::<Vec<_>>()
    );
    ensure!(
        used_scopes == all_scopes,
        "every ScopeKey::ALL value must be used: unused={:?}",
        all_scopes.difference(&used_scopes).collect::<Vec<_>>()
    );
    ensure!(
        used_problems == finalized_problem_codes,
        "every finalized ProblemCode value must be used: unused={:?}",
        finalized_problem_codes
            .difference(&used_problems)
            .collect::<Vec<_>>()
    );
    let expected_profiles: BTreeSet<_> = EXPECTED_PROFILES.into_iter().collect();
    ensure!(
        used_profiles == expected_profiles,
        "every governed surface profile must be used: unused={:?}",
        expected_profiles
            .difference(&used_profiles)
            .collect::<Vec<_>>()
    );
    Ok(())
}

fn validate_lifecycle(capability: &Capability) -> anyhow::Result<()> {
    let expected = (
        capability.application_key.contract_state(),
        capability.application_key.runtime_availability(),
    );
    ensure!(
        (
            capability.lifecycle.contract_state,
            capability.lifecycle.runtime_availability
        ) == expected,
        "capability {:?}: lifecycle disagrees with fasti-application; expected {:?}",
        capability.application_key,
        expected
    );
    Ok(())
}

fn validate_uat(label: &str, entries: &[UatOwnership]) -> anyhow::Result<()> {
    let mut seen = BTreeSet::new();
    for entry in entries {
        ensure!(
            uat_id_is_well_formed(&entry.id),
            "{label}: UAT ID must use ID-NNN: {}",
            entry.id
        );
        ensure!(
            seen.insert(entry.id.as_str()),
            "{label}: duplicate UAT ownership {}",
            entry.id
        );
        ensure!(
            nonempty(Some(&entry.reason)),
            "{label}: UAT {} needs an ownership reason",
            entry.id
        );
        match entry.relationship {
            UatRelationship::Direct | UatRelationship::Split | UatRelationship::Deferred => {}
        }
        ensure!(
            body_rank(entry.owner_body) >= body_rank(CapabilityBody::B1),
            "{label}: UAT {} cannot be owned before B1",
            entry.id
        );
    }
    Ok(())
}

fn public_id_is_well_formed(value: &str) -> bool {
    let mut segments = value.split('.');
    let Some(first) = segments.next() else {
        return false;
    };
    let Some(second) = segments.next() else {
        return false;
    };
    segment_is_well_formed(first)
        && segment_is_well_formed(second)
        && segments.all(segment_is_well_formed)
}

fn segment_is_well_formed(segment: &str) -> bool {
    let mut chars = segment.chars();
    matches!(chars.next(), Some('a'..='z'))
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn uat_id_is_well_formed(value: &str) -> bool {
    value.len() == 6
        && value.starts_with("ID-")
        && value[3..]
            .chars()
            .all(|character| character.is_ascii_digit())
}

fn nonempty(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

const fn expected_surface_profile(key: CapabilityKey) -> &'static str {
    match key {
        CapabilityKey::SystemHealth => "health",
        CapabilityKey::AcceptObservation => "b1_observation_accept",
        CapabilityKey::ReplayReceipt => "b1_receipt_replay",
        CapabilityKey::StreamReceipts => "b1_receipt_stream",
        _ => match key.contract_body() {
            CapabilityBody::B1 => "b1_http_fixture",
            CapabilityBody::B2 => "later_b2",
            CapabilityBody::B3 => "later_b3",
            CapabilityBody::B0 => "health",
        },
    }
}

const fn body_rank(body: CapabilityBody) -> u8 {
    match body {
        CapabilityBody::B0 => 0,
        CapabilityBody::B1 => 1,
        CapabilityBody::B2 => 2,
        CapabilityBody::B3 => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_ids_are_deliberately_narrow() {
        assert!(public_id_is_well_formed("identity.record.create"));
        assert!(!public_id_is_well_formed("identity"));
        assert!(!public_id_is_well_formed("Identity.record"));
        assert!(!public_id_is_well_formed("identity..record"));
    }

    #[test]
    fn uat_ids_are_fixed_width() {
        assert!(uat_id_is_well_formed("ID-065"));
        assert!(!uat_id_is_well_formed("ID-65"));
    }

    #[test]
    fn surface_profile_policy_separates_finalized_and_later_contracts() {
        assert_eq!(
            expected_surface_profile(CapabilityKey::InitializeNode),
            "b1_http_fixture"
        );
        assert_eq!(
            expected_surface_profile(CapabilityKey::AcceptObservation),
            "b1_observation_accept"
        );
        assert_eq!(
            expected_surface_profile(CapabilityKey::CreateRecord),
            "later_b2"
        );
        assert_eq!(
            expected_surface_profile(CapabilityKey::ExportWorkspace),
            "later_b3"
        );
    }

    #[test]
    fn cross_body_surface_profile_mutations_are_rejected() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask lives under workspace root");
        let mut finalized = load_validated(root).expect("authored registry is valid");
        finalized
            .capabilities
            .iter_mut()
            .find(|capability| capability.application_key == CapabilityKey::InitializeNode)
            .expect("initialize capability")
            .surface_profile = "later_b2".to_owned();
        assert!(validate_capabilities(&finalized).is_err());

        let mut reserved = load_validated(root).expect("authored registry is valid");
        reserved
            .capabilities
            .iter_mut()
            .find(|capability| capability.application_key == CapabilityKey::CreateRecord)
            .expect("create-record capability")
            .surface_profile = "b1_http_fixture".to_owned();
        assert!(validate_capabilities(&reserved).is_err());
    }

    #[test]
    fn finalized_b1_capabilities_without_examples_are_rejected() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask lives under workspace root");
        let mut registry = load_validated(root).expect("authored registry is valid");
        registry
            .capabilities
            .iter_mut()
            .find(|capability| capability.application_key == CapabilityKey::DiscoverCapabilities)
            .expect("capability discovery")
            .examples
            .clear();
        assert!(validate_capabilities(&registry).is_err());
    }

    #[test]
    fn authorization_mutations_are_rejected() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask lives under workspace root");
        let mut registry = load_validated(root).expect("authored registry is valid");
        registry
            .capabilities
            .iter_mut()
            .find(|capability| capability.application_key == CapabilityKey::InitializeNode)
            .expect("initialize capability")
            .authorization = AuthorizationKind::Scoped;
        assert!(validate_capabilities(&registry).is_err());
    }

    #[test]
    fn reserved_problem_codes_stay_out_of_the_b1_registry() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask lives under workspace root");
        let public = normalized_public_json(root).expect("public registry");
        let rendered = serde_json::to_string(&public).expect("serialize public registry");

        assert!(!rendered.contains(ProblemCode::AuthenticationFailed.as_str()));
        assert!(!rendered.contains(ProblemCode::StorageUnavailable.as_str()));
        assert_eq!(
            ProblemCode::AuthenticationFailed.contract_state(),
            ContractState::Reserved
        );
    }

    #[test]
    fn public_registry_redacts_private_application_bindings() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask lives under workspace root");
        let public = normalized_public_json(root).expect("public registry");
        let rendered = serde_json::to_string(&public).expect("serialize public registry");
        assert!(!rendered.contains("application_key"));
        assert!(!rendered.contains("application:{application_key}"));
        assert!(rendered.contains("\"binding_visibility\":\"internal\""));
    }
}
