use anyhow::{ensure, Context};
use fasti_application::{
    CapabilityBody, CapabilityKey, ContractState, ProblemCode, RuntimeAvailability, ScopeKey,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::Path;

const REGISTRY_PATH: &str = "contracts/registry/v1/capabilities.yaml";
const EXPECTED_PROFILES: [&str; 6] = [
    "b1_http_fixture",
    "b1_listener",
    "b1_receipt_replay",
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
    json_ld_okf: SurfaceDisposition,
    sdk: SurfaceDisposition,
    knowledge: SurfaceDisposition,
    package_smoke: SurfaceDisposition,
    ui: SurfaceDisposition,
}

impl SurfaceProfile {
    fn surfaces(&self) -> [(&'static str, &SurfaceDisposition); 10] {
        [
            ("domain_application", &self.domain_application),
            ("http_openapi", &self.http_openapi),
            ("sse_asyncapi", &self.sse_asyncapi),
            ("cli", &self.cli),
            ("json_schema", &self.json_schema),
            ("json_ld_okf", &self.json_ld_okf),
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

    serde_json::to_value(registry).context("public capability registry is not serializable")
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
            capability.scopes.as_slice() == capability.application_key.required_scopes(),
            "{label}: scopes disagree with fasti-application; expected {:?}, found {:?}",
            capability.application_key.required_scopes(),
            capability.scopes
        );

        let mut capability_problems = BTreeSet::new();
        for problem in &capability.problems {
            ensure!(
                problem_codes.contains(problem.as_str()),
                "{label}: problem code {problem:?} is not owned by ProblemCode::ALL"
            );
            ensure!(
                capability_problems.insert(problem.as_str()),
                "{label}: duplicate problem code {problem:?}"
            );
            used_problems.insert(problem.as_str());
        }

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
        used_problems == problem_codes,
        "every ProblemCode::ALL value must be used: unused={:?}",
        problem_codes.difference(&used_problems).collect::<Vec<_>>()
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
}
