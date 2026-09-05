//! Regression: registered Search handlers also need the packaged WebView grant.

#[test]
fn packaged_search_commands_are_allowed_by_the_generated_runtime_permission() {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../gen/schemas/acl-manifests.json"))
            .expect("generated packaged ACL manifest");
    let permission = &manifest["__app-acl__"]["permissions"]["main-runtime"];
    let allowed = permission["commands"]["allow"]
        .as_array()
        .expect("runtime command allow list");
    for command in [
        "search_records",
        "search_provider_page",
        "read_search_candidate",
        "save_search_candidate",
        "save_provider_identifier",
    ] {
        assert_eq!(
            allowed
                .iter()
                .filter(|value| value.as_str() == Some(command))
                .count(),
            1,
            "packaged Search command {command} must be allowed exactly once"
        );
    }
}
