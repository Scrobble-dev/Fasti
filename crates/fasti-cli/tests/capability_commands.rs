use serde_json::Value;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fasti"))
        .args(args)
        .env("HTTP_PROXY", "http://127.0.0.1:9")
        .env("HTTPS_PROXY", "http://127.0.0.1:9")
        .env("ALL_PROXY", "http://127.0.0.1:9")
        .env("NO_PROXY", "")
        .output()
        .expect("fasti CLI should start")
}

fn success(args: &[&str]) -> String {
    let output = run(args);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "success must not write stderr");
    String::from_utf8(output.stdout).expect("stdout should be UTF-8")
}

#[test]
fn list_human_is_stably_sorted_and_resource_complete() {
    let output = success(&["capability", "list"]);
    assert!(output
        .starts_with("operation_capability_id: system.capabilities.discover\ncontract_version:"));
    for field in [
        "bounded_context:",
        "contract_body:",
        "runtime_body:",
        "lifecycle:",
        "scopes:",
        "problems:",
        "examples:",
        "surface_profile:",
        "surface_dispositions:",
    ] {
        assert!(output.contains(field), "missing {field}");
    }
    assert!(output.contains("do not claim that later-body runtime behavior is available"));

    let ids = output
        .lines()
        .filter_map(|line| line.strip_prefix("id: "))
        .collect::<Vec<_>>();
    assert!(ids.len() > 20, "expected the complete public registry");
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted);
}

#[test]
fn show_human_exposes_public_lifecycle_and_surface_dispositions() {
    let output = success(&["capability", "show", "system.health"]);
    assert!(output.contains("operation_capability_id: system.capabilities.discover"));
    assert!(output.contains("id: system.health"));
    assert!(output.contains("contract_state=finalized"));
    assert!(output.contains("runtime_availability=implemented"));
    assert!(output.contains("http_openapi: state=required"));
    assert!(output.contains("binding=\"openapi:system.health\""));
    assert!(output.contains("domain_application: state=required"));
    assert!(output.contains("binding_visibility=\"internal\""));
}

#[test]
fn json_output_is_deterministic_sorted_and_private() {
    let first = success(&["capability", "list", "--output", "json"]);
    let second = success(&["capability", "list", "--output", "json"]);
    assert_eq!(first, second);
    assert!(!first.contains("application_key"));
    assert!(!first.contains("discover_capabilities"));
    assert!(!first.contains("system_health"));

    let document: Value = serde_json::from_str(&first).expect("list output should be JSON");
    assert_eq!(
        document["operation_capability_id"],
        "system.capabilities.discover"
    );
    let resources = document["resources"]
        .as_array()
        .expect("resources should be an array");
    assert_eq!(
        document["resource_count"].as_u64(),
        Some(resources.len() as u64)
    );
    let ids = resources
        .iter()
        .map(|resource| resource["id"].as_str().expect("resource id"))
        .collect::<Vec<_>>();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted);
    for resource in resources {
        let dispositions = resource["surface_dispositions"]
            .as_object()
            .expect("every resource should expose its surface dispositions");
        assert!(!dispositions.is_empty());
        assert!(dispositions
            .values()
            .all(|disposition| disposition["state"].is_string()));
    }

    let shown = success(&["capability", "show", "receipt.stream", "--output", "json"]);
    let shown: Value = serde_json::from_str(&shown).expect("show output should be JSON");
    assert_eq!(shown["resource"]["id"], "receipt.stream");
    assert!(shown["resource"]["surface_dispositions"]["sse_asyncapi"].is_object());
}

#[test]
fn unknown_id_is_typed_actionable_and_stderr_only() {
    let output = run(&["capability", "show", "not.a.capability"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "failure must not write stdout");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("code=capability_not_found"));
    assert!(stderr.contains("capability_id=system.capabilities.discover"));
    assert!(stderr.contains("safe_state=no_mutation"));
    assert!(stderr.contains("fasti capability list"));
}

#[test]
fn command_help_binds_inspection_to_discovery_capability() {
    for args in [
        &["capability", "--help"][..],
        &["capability", "list", "--help"][..],
        &["capability", "show", "--help"][..],
    ] {
        let output = success(args);
        assert!(output.contains("system.capabilities.discover"));
    }
}
