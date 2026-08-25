use clap::ValueEnum;
use fasti_application::CapabilityKey;
use fasti_contracts::public_capability_id;
use serde_json::{json, Map, Value};
use std::fmt::{self, Write as _};

const DISCOVERY_CAPABILITY_ID: &str = public_capability_id(CapabilityKey::DiscoverCapabilities);
const PUBLIC_REGISTRY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../contracts/generated/v1/capabilities.json"
));
const RUNTIME_NOTICE: &str = "Registry lifecycle values describe the checked-in contract; they do not claim that later-body runtime behavior is available.";

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub(crate) enum OutputFormat {
    #[default]
    Human,
    Json,
}

#[derive(Debug)]
pub(crate) enum CliFailure {
    Capability {
        code: &'static str,
        capability_id: &'static str,
        detail: String,
        next_action: String,
    },
    Local {
        diagnostic: &'static str,
        detail: String,
        next_action: String,
    },
}

impl CliFailure {
    pub(crate) fn new(
        code: &'static str,
        capability_id: &'static str,
        detail: impl Into<String>,
        next_action: impl Into<String>,
    ) -> Self {
        Self::Capability {
            code,
            capability_id,
            detail: detail.into(),
            next_action: next_action.into(),
        }
    }

    fn registry(detail: impl Into<String>) -> Self {
        Self::local(
            "registry_invalid",
            detail,
            "Regenerate and validate the checked-in public contract registry, then rebuild Fasti.",
        )
    }

    pub(crate) fn local(
        diagnostic: &'static str,
        detail: impl Into<String>,
        next_action: impl Into<String>,
    ) -> Self {
        Self::Local {
            diagnostic,
            detail: detail.into(),
            next_action: next_action.into(),
        }
    }
}

impl fmt::Display for CliFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capability {
                code,
                capability_id,
                detail,
                next_action,
            } => write!(
                formatter,
                "code={code} capability_id={capability_id} safe_state=no_mutation detail={detail:?} next_action={next_action:?}"
            ),
            Self::Local {
                diagnostic,
                detail,
                next_action,
            } => write!(
                formatter,
                "diagnostic={diagnostic} scope=cli_local detail={detail:?} next_action={next_action:?}"
            ),
        }
    }
}

impl std::error::Error for CliFailure {}

pub(crate) struct CapabilityCatalog {
    contract_version: String,
    capability_base_uri: String,
    resources: Vec<Value>,
}

impl CapabilityCatalog {
    pub(crate) fn load() -> Result<Self, CliFailure> {
        let registry: Value = serde_json::from_str(PUBLIC_REGISTRY).map_err(|error| {
            CliFailure::registry(format!(
                "The embedded public registry is not valid JSON: {error}"
            ))
        })?;
        reject_private_keys(&registry)?;

        let root = registry
            .as_object()
            .ok_or_else(|| CliFailure::registry("The public registry root must be an object."))?;
        let contract_version = required_string(root, "contract_version")?.to_owned();
        let capability_base_uri = required_string(root, "capability_base_uri")?.to_owned();
        let profiles = root
            .get("surface_profiles")
            .and_then(Value::as_object)
            .ok_or_else(|| CliFailure::registry("surface_profiles must be an object."))?;
        let capabilities = root
            .get("capabilities")
            .and_then(Value::as_array)
            .ok_or_else(|| CliFailure::registry("capabilities must be an array."))?;

        let mut resources = Vec::with_capacity(capabilities.len());
        for capability in capabilities {
            let mut resource = capability
                .as_object()
                .cloned()
                .ok_or_else(|| CliFailure::registry("Each capability must be an object."))?;
            let id = required_string(&resource, "id")?.to_owned();
            let profile_id = required_string(&resource, "surface_profile")?.to_owned();
            let profile = profiles
                .get(&profile_id)
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    CliFailure::registry(format!(
                        "Capability {id} references unknown surface profile {profile_id}."
                    ))
                })?;

            normalize_resource_arrays(&id, &mut resource)?;
            resource.insert(
                "surface_dispositions".to_owned(),
                Value::Object(public_surface_dispositions(&id, profile)?),
            );
            resources.push(Value::Object(resource));
        }

        resources.sort_by(|left, right| resource_id(left).cmp(resource_id(right)));
        if resources
            .windows(2)
            .any(|pair| resource_id(&pair[0]) == resource_id(&pair[1]))
        {
            return Err(CliFailure::registry(
                "The public registry contains duplicate capability identifiers.",
            ));
        }

        Ok(Self {
            contract_version,
            capability_base_uri,
            resources,
        })
    }

    pub(crate) fn list(&self, output: OutputFormat) -> Result<String, CliFailure> {
        match output {
            OutputFormat::Human => self.render_list_human(),
            OutputFormat::Json => render_json(&json!({
                "operation_capability_id": DISCOVERY_CAPABILITY_ID,
                "contract_version": self.contract_version,
                "capability_base_uri": self.capability_base_uri,
                "resource_count": self.resources.len(),
                "runtime_notice": RUNTIME_NOTICE,
                "resources": self.resources,
            })),
        }
    }

    pub(crate) fn show(&self, id: &str, output: OutputFormat) -> Result<String, CliFailure> {
        let resource = self
            .resources
            .binary_search_by(|resource| resource_id(resource).cmp(id))
            .ok()
            .and_then(|index| self.resources.get(index))
            .ok_or_else(|| {
                CliFailure::local(
                    "resource_not_found",
                    format!("No public capability resource has id {id:?}."),
                    "Run `fasti capability list` to inspect the stable public identifiers.",
                )
            })?;

        match output {
            OutputFormat::Human => self.render_show_human(resource),
            OutputFormat::Json => render_json(&json!({
                "operation_capability_id": DISCOVERY_CAPABILITY_ID,
                "contract_version": self.contract_version,
                "capability_base_uri": self.capability_base_uri,
                "runtime_notice": RUNTIME_NOTICE,
                "resource": resource,
            })),
        }
    }

    fn render_list_human(&self) -> Result<String, CliFailure> {
        let mut output = String::new();
        writeln!(output, "operation_capability_id: {DISCOVERY_CAPABILITY_ID}")
            .map_err(format_failure)?;
        writeln!(output, "contract_version: {}", self.contract_version).map_err(format_failure)?;
        writeln!(output, "capability_base_uri: {}", self.capability_base_uri)
            .map_err(format_failure)?;
        writeln!(output, "resource_count: {}", self.resources.len()).map_err(format_failure)?;
        writeln!(output, "runtime_notice: {RUNTIME_NOTICE}").map_err(format_failure)?;

        for resource in &self.resources {
            writeln!(output).map_err(format_failure)?;
            render_resource_human(&mut output, resource)?;
        }
        Ok(output.trim_end().to_owned())
    }

    fn render_show_human(&self, resource: &Value) -> Result<String, CliFailure> {
        let mut output = String::new();
        writeln!(output, "operation_capability_id: {DISCOVERY_CAPABILITY_ID}")
            .map_err(format_failure)?;
        writeln!(output, "contract_version: {}", self.contract_version).map_err(format_failure)?;
        writeln!(output, "capability_base_uri: {}", self.capability_base_uri)
            .map_err(format_failure)?;
        writeln!(output, "runtime_notice: {RUNTIME_NOTICE}").map_err(format_failure)?;
        writeln!(output).map_err(format_failure)?;
        render_resource_human(&mut output, resource)?;
        Ok(output.trim_end().to_owned())
    }
}

fn required_string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, CliFailure> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| CliFailure::registry(format!("Registry field {key:?} must be a string.")))
}

fn resource_id(resource: &Value) -> &str {
    resource
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn normalize_resource_arrays(
    id: &str,
    resource: &mut Map<String, Value>,
) -> Result<(), CliFailure> {
    for field in ["scopes", "problems", "examples"] {
        let values = resource
            .get_mut(field)
            .and_then(Value::as_array_mut)
            .ok_or_else(|| {
                CliFailure::registry(format!("Capability {id} field {field} must be an array."))
            })?;
        if values.iter().any(|value| !value.is_string()) {
            return Err(CliFailure::registry(format!(
                "Capability {id} field {field} may contain only strings."
            )));
        }
        values.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
    }

    let uat = resource
        .get_mut("uat")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            CliFailure::registry(format!("Capability {id} field uat must be an array."))
        })?;
    if uat
        .iter()
        .any(|entry| entry.get("id").and_then(Value::as_str).is_none())
    {
        return Err(CliFailure::registry(format!(
            "Capability {id} UAT entries must have string ids."
        )));
    }
    uat.sort_by(|left, right| {
        left.get("id")
            .and_then(Value::as_str)
            .cmp(&right.get("id").and_then(Value::as_str))
    });
    Ok(())
}

fn public_surface_dispositions(
    capability_id: &str,
    profile: &Map<String, Value>,
) -> Result<Map<String, Value>, CliFailure> {
    let mut surfaces = Map::new();
    for (surface, disposition) in profile {
        let mut public = disposition.as_object().cloned().ok_or_else(|| {
            CliFailure::registry(format!("Surface disposition {surface} must be an object."))
        })?;
        if serde_json::to_string(&public).is_ok_and(|rendered| rendered.contains("application_key"))
        {
            return Err(CliFailure::registry(
                "A public surface disposition contains an internal application binding.",
            ));
        }
        if public.get("binding_visibility").and_then(Value::as_str) == Some("public") {
            if let Some(binding) = public.get("binding").and_then(Value::as_str) {
                public.insert(
                    "binding".to_owned(),
                    Value::String(binding.replace("{capability_id}", capability_id)),
                );
            }
        }
        surfaces.insert(surface.clone(), Value::Object(public));
    }
    Ok(surfaces)
}

fn reject_private_keys(value: &Value) -> Result<(), CliFailure> {
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                if key == "application_key" {
                    return Err(CliFailure::registry(
                        "The public registry contains an internal-only key.",
                    ));
                }
                reject_private_keys(nested)?;
            }
        }
        Value::Array(values) => {
            for nested in values {
                reject_private_keys(nested)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn render_resource_human(output: &mut String, resource: &Value) -> Result<(), CliFailure> {
    let object = resource
        .as_object()
        .ok_or_else(|| CliFailure::registry("A capability resource must be an object."))?;
    let lifecycle = object
        .get("lifecycle")
        .and_then(Value::as_object)
        .ok_or_else(|| CliFailure::registry("A capability lifecycle must be an object."))?;

    writeln!(output, "id: {}", required_string(object, "id")?).map_err(format_failure)?;
    writeln!(
        output,
        "bounded_context: {}",
        required_string(object, "bounded_context")?
    )
    .map_err(format_failure)?;
    writeln!(
        output,
        "contract_body: {}",
        required_string(object, "contract_body")?
    )
    .map_err(format_failure)?;
    writeln!(
        output,
        "runtime_body: {}",
        required_string(object, "runtime_body")?
    )
    .map_err(format_failure)?;
    writeln!(
        output,
        "lifecycle: contract_state={} introduced_in={} runtime_availability={}",
        required_string(lifecycle, "contract_state")?,
        required_string(lifecycle, "introduced_in")?,
        required_string(lifecycle, "runtime_availability")?
    )
    .map_err(format_failure)?;
    writeln!(output, "scopes: {}", string_array(object, "scopes")?).map_err(format_failure)?;
    writeln!(output, "problems: {}", string_array(object, "problems")?).map_err(format_failure)?;
    writeln!(output, "examples: {}", string_array(object, "examples")?).map_err(format_failure)?;
    writeln!(
        output,
        "surface_profile: {}",
        required_string(object, "surface_profile")?
    )
    .map_err(format_failure)?;

    let uat = object
        .get("uat")
        .and_then(Value::as_array)
        .ok_or_else(|| CliFailure::registry("Capability uat must be an array."))?;
    writeln!(
        output,
        "uat: {}",
        if uat.is_empty() {
            "none".to_owned()
        } else {
            uat.iter()
                .filter_map(|entry| entry.get("id").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(", ")
        }
    )
    .map_err(format_failure)?;
    writeln!(output, "surface_dispositions:").map_err(format_failure)?;

    let surfaces = object
        .get("surface_dispositions")
        .and_then(Value::as_object)
        .ok_or_else(|| CliFailure::registry("surface_dispositions must be an object."))?;
    for (surface, disposition) in surfaces {
        let disposition = disposition.as_object().ok_or_else(|| {
            CliFailure::registry(format!("Surface disposition {surface} must be an object."))
        })?;
        write!(
            output,
            "  {surface}: state={}",
            required_string(disposition, "state")?
        )
        .map_err(format_failure)?;
        for field in ["binding", "binding_visibility", "body", "reason"] {
            if let Some(value) = disposition.get(field).and_then(Value::as_str) {
                write!(output, " {field}={value:?}").map_err(format_failure)?;
            }
        }
        writeln!(output).map_err(format_failure)?;
    }
    Ok(())
}

fn string_array(object: &Map<String, Value>, field: &str) -> Result<String, CliFailure> {
    let values = object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| CliFailure::registry(format!("Capability {field} must be an array.")))?;
    if values.is_empty() {
        return Ok("none".to_owned());
    }
    values
        .iter()
        .map(|value| {
            value.as_str().ok_or_else(|| {
                CliFailure::registry(format!("Capability {field} may contain only strings."))
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|values| values.join(", "))
}

fn render_json(value: &Value) -> Result<String, CliFailure> {
    serde_json::to_string_pretty(value).map_err(|error| {
        CliFailure::local(
            "output_failed",
            format!("The capability result could not be encoded as JSON: {error}"),
            "Validate the checked-in registry and retry the command.",
        )
    })
}

fn format_failure(_: fmt::Error) -> CliFailure {
    CliFailure::local(
        "output_failed",
        "The capability result could not be formatted.",
        "Validate the checked-in registry and retry the command.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_is_sorted_and_private_bindings_are_redacted() {
        let catalog = CapabilityCatalog::load().expect("generated registry should load");
        let ids = catalog
            .resources
            .iter()
            .map(resource_id)
            .collect::<Vec<_>>();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);

        let rendered = catalog
            .list(OutputFormat::Json)
            .expect("catalog should render");
        assert!(!rendered.contains("application_key"));
        assert!(rendered.contains("binding_visibility"));
    }
}
