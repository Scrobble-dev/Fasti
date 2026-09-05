use anyhow::{ensure, Context};
use fasti_application::{CapabilityKey, ProblemCode, ProblemParamPolicy, WorkspaceExportEntity};
use fasti_contracts::{ChecksummedWorkspaceManifestDto, HealthResponse, ProblemDetails};
use schemars::{generate::SchemaSettings, JsonSchema};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{integration, registry};

pub(crate) type Artifacts = BTreeMap<PathBuf, Vec<u8>>;

const OPENAPI_PATH: &str = "contracts/generated/v1/openapi.json";
const CONFORMANCE_OPENAPI_PATH: &str = "contracts/generated/v1/conformance-openapi.json";
const CAPABILITY_REGISTRY_PATH: &str = "contracts/generated/v1/capabilities.json";
const PROBLEM_CATALOG_PATH: &str = "contracts/generated/v1/problems.json";
const CAPABILITY_DISCOVERY_EXAMPLE_PATH: &str =
    "contracts/examples/v1/system.capabilities.success.json";
const HEALTH_SCHEMA_PATH: &str = "packages/schemas/schemas/health-response.json";
const PROBLEM_SCHEMA_PATH: &str = "packages/schemas/schemas/problem-details.json";
const PORTABILITY_V2_SCHEMA_PATH: &str = "contracts/portability/v2/workspace-manifest.schema.json";
const PORTABILITY_V1_EXAMPLE_PATH: &str =
    "contracts/portability/v1/workspace-manifest.example.json";
const PORTABILITY_V2_EXAMPLE_PATH: &str =
    "contracts/portability/v2/workspace-manifest.example.json";
const PORTABILITY_V3_SCHEMA_PATH: &str = "contracts/portability/v3/workspace-manifest.schema.json";
const PORTABILITY_V3_EXAMPLE_PATH: &str =
    "contracts/portability/v3/workspace-manifest.example.json";
const PORTABILITY_V4_SCHEMA_PATH: &str = "contracts/portability/v4/workspace-manifest.schema.json";
const PORTABILITY_V4_EXAMPLE_PATH: &str =
    "contracts/portability/v4/workspace-manifest.example.json";
const PORTABILITY_V5_SCHEMA_PATH: &str = "contracts/portability/v5/workspace-manifest.schema.json";
const PORTABILITY_V5_EXAMPLE_PATH: &str =
    "contracts/portability/v5/workspace-manifest.example.json";
const PORTABILITY_V6_SCHEMA_PATH: &str = "contracts/portability/v6/workspace-manifest.schema.json";
const PORTABILITY_V6_EXAMPLE_PATH: &str =
    "contracts/portability/v6/workspace-manifest.example.json";
const SDK_GENERATED_PATH: &str = "packages/sdk/src/generated.ts";
const PORTABILITY_V7_SCHEMA_PATH: &str = "contracts/portability/v7/workspace-manifest.schema.json";
const PORTABILITY_V7_EXAMPLE_PATH: &str =
    "contracts/portability/v7/workspace-manifest.example.json";
const RUST_CAPABILITY_IDS_PATH: &str = "crates/fasti-contracts/src/generated_capability_ids.rs";
const PROVIDER_MANIFEST_SCHEMA_PATH: &str =
    "contracts/addons/generated/v0.1/provider-manifest.schema.json";
const ASYNCAPI_PATH: &str = "contracts/asyncapi/v1/transport.yaml";
const EXAMPLES_DIRECTORY: &str = "contracts/examples/v1";
const DOCUMENTATION_BASE: &str = "https://fasti.scrobble.dev";
const GENERATED_ONLY_DIRECTORIES: [&str; 3] = [
    "contracts/generated/v1",
    "packages/schemas/schemas",
    "contracts/addons/generated",
];

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

const PRODUCTION_BOOTSTRAP_OPERATIONS: [ConformanceOperation; 2] = [
    ConformanceOperation {
        alias: "initializeDurableNode",
        operation_id: "initialize_node",
        method: "post",
        path: "/api/v1/node/initialization",
        capability_id: "node.initialize",
        authenticated: false,
        request: Some("InitializeNodeRequest"),
        response: Some("NodeInitializationResponse"),
        retry: "never",
    },
    ConformanceOperation {
        alias: "enrollDurableFirstClient",
        operation_id: "enroll_first_client",
        method: "post",
        path: "/api/v1/client-enrollments",
        capability_id: "client.enroll",
        authenticated: false,
        request: Some("EnrollFirstClientRequest"),
        response: Some("ClientEnrollmentResponse"),
        retry: "never",
    },
];

/// Production operations that run after bootstrap, on the durable authenticated
/// surface. Kept separate from `PRODUCTION_BOOTSTRAP_OPERATIONS` because that
/// array also drives the bootstrap-only SDK slice in
/// `render_production_bootstrap_contract`, which must not grow to include them.
const PRODUCTION_RUNTIME_OPERATIONS: [ConformanceOperation; 44] = [
    ConformanceOperation {
        alias: "searchRecords",
        operation_id: "search_local_records",
        method: "post",
        path: "/api/v1/search/records",
        capability_id: "metadata.search",
        authenticated: true,
        request: Some("LocalSearchRequestDto"),
        response: Some("LocalSearchResponseDto"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "saveSearchCandidate",
        operation_id: "save_search_candidate",
        method: "post",
        path: "/api/v1/search/candidates/{provider_id}/{grain}/{candidate_receipt_id}/actions",
        capability_id: "identity.identifier.attach",
        authenticated: true,
        request: Some("SearchCandidateActionRequest"),
        response: Some("SearchCandidateActionResponse"),
        retry: "stable_body_operation_id",
    },
    ConformanceOperation {
        alias: "readSearchCandidate",
        operation_id: "read_search_candidate",
        method: "get",
        path: "/api/v1/search/candidates/{provider_id}/{grain}/{candidate_receipt_id}",
        capability_id: "metadata.search",
        authenticated: true,
        request: None,
        response: Some("SearchCandidateDetailsResponse"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "searchProviderPage",
        operation_id: "search_provider_page",
        method: "post",
        path: "/api/v1/search/providers/{provider_id}",
        capability_id: "metadata.search",
        authenticated: true,
        request: Some("SearchProviderPageRequest"),
        response: Some("SearchProviderPageResponse"),
        retry: "never",
    },
    ConformanceOperation {
        alias: "submitObservation",
        operation_id: "submit_observation",
        method: "post",
        path: "/api/v1/observations",
        capability_id: "observation.accept",
        authenticated: true,
        request: Some("SubmitObservationRequest"),
        response: Some("SubmitObservationResponse"),
        retry: "stable_body_operation_id",
    },
    ConformanceOperation {
        alias: "nuvioWebhook",
        operation_id: "nuvio_webhook",
        method: "post",
        path: "/api/v1/integrations/nuvio/webhook",
        capability_id: "observation.accept",
        authenticated: true,
        request: Some("IntegrationObservationRequest"),
        response: Some("SubmitObservationResponse"),
        retry: "stable_body_operation_id",
    },
    ConformanceOperation {
        alias: "tautulliWebhook",
        operation_id: "tautulli_webhook",
        method: "post",
        path: "/api/v1/integrations/tautulli/webhook",
        capability_id: "observation.accept",
        authenticated: true,
        request: Some("IntegrationObservationRequest"),
        response: Some("SubmitObservationResponse"),
        retry: "stable_body_operation_id",
    },
    ConformanceOperation {
        alias: "jellyfinWebhook",
        operation_id: "jellyfin_webhook",
        method: "post",
        path: "/api/v1/integrations/jellyfin/webhook",
        capability_id: "observation.accept",
        authenticated: true,
        request: Some("IntegrationObservationRequest"),
        response: Some("SubmitObservationResponse"),
        retry: "stable_body_operation_id",
    },
    ConformanceOperation {
        alias: "embyWebhook",
        operation_id: "emby_webhook",
        method: "post",
        path: "/api/v1/integrations/emby/webhook",
        capability_id: "observation.accept",
        authenticated: true,
        request: None,
        response: Some("SubmitObservationResponse"),
        retry: "stable_body_operation_id",
    },
    ConformanceOperation {
        alias: "plexWebhook",
        operation_id: "plex_webhook",
        method: "post",
        path: "/api/v1/integrations/plex/webhook",
        capability_id: "observation.accept",
        authenticated: true,
        request: None,
        response: Some("SubmitObservationResponse"),
        retry: "stable_body_operation_id",
    },
    ConformanceOperation {
        alias: "createRecord",
        operation_id: "create_record",
        method: "post",
        path: "/api/v1/records",
        capability_id: "identity.record.create",
        authenticated: true,
        request: Some("CreateRecordRequest"),
        response: Some("CreateRecordResponse"),
        retry: "never",
    },
    ConformanceOperation {
        alias: "listRecords",
        operation_id: "list_records",
        method: "get",
        path: "/api/v1/records",
        capability_id: "identity.record.list",
        authenticated: true,
        request: None,
        response: Some("ListRecordsResponse"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "attachIdentifier",
        operation_id: "attach_identifier",
        method: "post",
        path: "/api/v1/records/identifiers",
        capability_id: "identity.identifier.attach",
        authenticated: true,
        request: Some("AttachIdentifierRequest"),
        response: Some("AttachIdentifierResponse"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "registerNamespace",
        operation_id: "register_namespace",
        method: "post",
        path: "/api/v1/namespaces",
        capability_id: "identity.namespace.register",
        authenticated: true,
        request: Some("RegisterNamespaceRequest"),
        response: Some("RegisterNamespaceResponse"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "listTrackingDispositions",
        operation_id: "list_tracking_dispositions",
        method: "get",
        path: "/api/v1/profile/record-tracking-dispositions",
        capability_id: "profile.record.tracking_disposition.list",
        authenticated: true,
        request: None,
        response: Some("ListTrackingDispositionsResponse"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "setTrackingDisposition",
        operation_id: "set_tracking_disposition",
        method: "put",
        path: "/api/v1/profile/record-tracking-dispositions/{record_id}",
        capability_id: "profile.record.tracking_disposition.set",
        authenticated: true,
        request: Some("SetTrackingDispositionRequest"),
        response: Some("TrackingDispositionStateDto"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "getNuvioCollections",
        operation_id: "get_nuvio_collections",
        method: "get",
        path: "/api/v1/profile/nuvio-collections",
        capability_id: "profile.nuvio_collections.get",
        authenticated: true,
        request: None,
        response: Some("NuvioCollectionsStateDto"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "replaceNuvioCollections",
        operation_id: "replace_nuvio_collections",
        method: "put",
        path: "/api/v1/profile/nuvio-collections",
        capability_id: "profile.nuvio_collections.replace",
        authenticated: true,
        request: Some("NuvioCollectionsDocumentDto"),
        response: Some("NuvioCollectionsStateDto"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "clearNuvioCollections",
        operation_id: "clear_nuvio_collections",
        method: "delete",
        path: "/api/v1/profile/nuvio-collections",
        capability_id: "profile.nuvio_collections.clear",
        authenticated: true,
        request: None,
        response: Some("NuvioCollectionsStateDto"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "listProviders",
        operation_id: "list_providers",
        method: "get",
        path: "/api/v1/providers",
        capability_id: "provider.list",
        authenticated: true,
        request: None,
        response: Some("ListProvidersResponse"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "configureProviderCredential",
        operation_id: "configure_provider_credential",
        method: "put",
        path: "/api/v1/providers/{provider_id}/credentials/{capability_id}",
        capability_id: "provider.credential.configure",
        authenticated: true,
        request: Some("ConfigureProviderCredentialRequest"),
        response: Some("ProviderCapabilityResponse"),
        retry: "never",
    },
    ConformanceOperation {
        alias: "removeProviderCredential",
        operation_id: "remove_provider_credential",
        method: "delete",
        path: "/api/v1/providers/{provider_id}/credentials/{capability_id}",
        capability_id: "provider.credential.configure",
        authenticated: true,
        request: None,
        response: Some("ProviderCapabilityResponse"),
        retry: "never",
    },
    ConformanceOperation {
        alias: "testProviderCredential",
        operation_id: "test_provider_credential",
        method: "post",
        path: "/api/v1/providers/{provider_id}/credentials/{capability_id}/tests",
        capability_id: "provider.credential.test",
        authenticated: true,
        request: None,
        response: Some("ProviderCapabilityResponse"),
        retry: "never",
    },
    ConformanceOperation {
        alias: "readProviderHealth",
        operation_id: "read_provider_health",
        method: "get",
        path: "/api/v1/providers/{provider_id}/health",
        capability_id: "provider.health.read",
        authenticated: true,
        request: None,
        response: Some("ProviderHealthResponse"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "refreshMetadataClaims",
        operation_id: "refresh_metadata_claims",
        method: "post",
        path: "/api/v1/metadata/claims/refresh",
        capability_id: "metadata.claim.refresh",
        authenticated: true,
        request: Some("RefreshMetadataClaimsRequest"),
        response: Some("RefreshMetadataClaimsResponse"),
        retry: "stable_body_operation_id",
    },
    ConformanceOperation {
        alias: "readMetadataProjection",
        operation_id: "read_metadata_projection",
        method: "get",
        path: "/api/v1/records/{record_id}/metadata-projection",
        capability_id: "metadata.projection.read",
        authenticated: true,
        request: None,
        response: Some("MetadataProjectionResponse"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "configureMetadataProjection",
        operation_id: "configure_metadata_projection",
        method: "put",
        path: "/api/v1/profile/metadata-projection",
        capability_id: "metadata.projection.configure",
        authenticated: true,
        request: Some("ConfigureMetadataProjectionRequest"),
        response: Some("MetadataProjectionConfigurationResponse"),
        retry: "never",
    },
    ConformanceOperation {
        alias: "resolveIdentityRoute",
        operation_id: "resolve_identity_route",
        method: "get",
        path: "/api/v1/records/{record_id}/identity-route",
        capability_id: "identity.route.resolve",
        authenticated: true,
        request: None,
        response: Some("ResolveIdentityRouteResponse"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "readAnimeGroupingPolicy",
        operation_id: "read_anime_grouping_policy",
        method: "get",
        path: "/api/v1/profile/anime-grouping-policy",
        capability_id: "profile.anime_grouping_policy.read",
        authenticated: true,
        request: None,
        response: Some("ReadAnimeGroupingPolicyResponse"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "previewAnimeGroupingPolicyChange",
        operation_id: "preview_anime_grouping_policy_change",
        method: "post",
        path: "/api/v1/profile/anime-grouping-policy/preview",
        capability_id: "profile.anime_grouping_policy.preview",
        authenticated: true,
        request: Some("PreviewAnimeGroupingPolicyChangeRequest"),
        response: Some("AnimeGroupingPolicyImpactResponse"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "applyAnimeGroupingPolicyChange",
        operation_id: "apply_anime_grouping_policy_change",
        method: "put",
        path: "/api/v1/profile/anime-grouping-policy",
        capability_id: "profile.anime_grouping_policy.apply",
        authenticated: true,
        request: Some("ApplyAnimeGroupingPolicyChangeRequest"),
        response: Some("ApplyAnimeGroupingPolicyChangeResponse"),
        retry: "stable_body_operation_id",
    },
    ConformanceOperation {
        alias: "startTrailBaseSignIn",
        operation_id: "start_trailbase_sign_in",
        method: "post",
        path: "/api/access/v1/trailbase/sign-in",
        capability_id: "browser.session.create",
        authenticated: false,
        request: Some("StartTrailBaseSignInRequest"),
        response: Some("StartTrailBaseSignInResponse"),
        retry: "never",
    },
    ConformanceOperation {
        alias: "readTrailBaseContinuation",
        operation_id: "read_trailbase_continuation",
        method: "get",
        path: "/api/access/v1/trailbase/continuation",
        capability_id: "browser.session.create",
        authenticated: false,
        request: None,
        response: Some("ReadTrailBaseContinuationResponse"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "completeTrailBaseContinuation",
        operation_id: "complete_trailbase_continuation",
        method: "post",
        path: "/api/access/v1/trailbase/continuation",
        capability_id: "browser.session.create",
        authenticated: false,
        request: Some("CompleteTrailBaseContinuationRequest"),
        response: None,
        retry: "never",
    },
    ConformanceOperation {
        alias: "cancelTrailBaseContinuation",
        operation_id: "cancel_trailbase_continuation",
        method: "delete",
        path: "/api/access/v1/trailbase/continuation",
        capability_id: "browser.session.create",
        authenticated: false,
        request: None,
        response: None,
        retry: "never",
    },
    ConformanceOperation {
        alias: "readAccessProjection",
        operation_id: "read_access_projection",
        method: "get",
        path: "/api/access/v1/projection",
        capability_id: "access.projection.read",
        authenticated: false,
        request: None,
        response: Some("AccessProjectionResponse"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "readBrowserSession",
        operation_id: "read_browser_session",
        method: "get",
        path: "/api/access/v1/browser-session",
        capability_id: "browser.session.read",
        authenticated: false,
        request: None,
        response: Some("ReadBrowserSessionResponse"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "endBrowserSession",
        operation_id: "end_browser_session",
        method: "delete",
        path: "/api/access/v1/browser-session",
        capability_id: "browser.session.end",
        authenticated: false,
        request: None,
        response: None,
        retry: "never",
    },
    ConformanceOperation {
        alias: "listBrowserSessions",
        operation_id: "list_browser_sessions",
        method: "get",
        path: "/api/access/v1/browser-sessions",
        capability_id: "browser.sessions.list",
        authenticated: false,
        request: None,
        response: Some("ListBrowserSessionsResponse"),
        retry: "safe",
    },
    ConformanceOperation {
        alias: "revokeBrowserSession",
        operation_id: "revoke_browser_session",
        method: "delete",
        path: "/api/access/v1/browser-sessions/{browser_session_id}",
        capability_id: "browser.session.revoke",
        authenticated: false,
        request: None,
        response: Some("RevokeBrowserSessionsResponse"),
        retry: "never",
    },
    ConformanceOperation {
        alias: "revokeOtherBrowserSessions",
        operation_id: "revoke_other_browser_sessions",
        method: "delete",
        path: "/api/access/v1/browser-sessions/others",
        capability_id: "browser.sessions.revoke_others",
        authenticated: false,
        request: None,
        response: Some("RevokeBrowserSessionsResponse"),
        retry: "never",
    },
    ConformanceOperation {
        alias: "revokeAllBrowserSessions",
        operation_id: "revoke_all_browser_sessions",
        method: "delete",
        path: "/api/access/v1/browser-sessions",
        capability_id: "browser.sessions.revoke_all",
        authenticated: false,
        request: None,
        response: Some("RevokeBrowserSessionsResponse"),
        retry: "never",
    },
    ConformanceOperation {
        alias: "rotateBrowserSession",
        operation_id: "rotate_browser_session",
        method: "post",
        path: "/api/access/v1/browser-session/rotation",
        capability_id: "browser.session.rotate",
        authenticated: false,
        request: None,
        response: Some("RotateBrowserSessionResponse"),
        retry: "never",
    },
    ConformanceOperation {
        alias: "selectBrowserSessionProfile",
        operation_id: "select_browser_session_profile",
        method: "put",
        path: "/api/access/v1/browser-session/profile",
        capability_id: "browser.session.profile.select",
        authenticated: false,
        request: Some("SelectBrowserSessionProfileRequest"),
        response: Some("SelectBrowserSessionProfileResponse"),
        retry: "never",
    },
];

const CREDENTIAL_ONLY_HYBRID_OPERATIONS: [&str; 5] = [
    "nuvio_webhook",
    "tautulli_webhook",
    "jellyfin_webhook",
    "emby_webhook",
    "plex_webhook",
];
const BROWSER_SESSION_PROBLEMS: [&str; 3] = [
    "browser_session_expired",
    "browser_session_revoked",
    "session_policy_changed",
];
const START_TRAILBASE_SIGN_IN_PROBLEMS: [&str; 9] = [
    "capacity_exceeded",
    "forbidden",
    "integrity_failed",
    "malformed_json",
    "payload_too_large",
    "storage_unavailable",
    "trailbase_trust_unavailable",
    "unsupported_media_type",
    "validation_failed",
];
const READ_TRAILBASE_CONTINUATION_PROBLEMS: [&str; 12] = [
    "auth_browser_binding_invalid",
    "auth_continuation_persistence_failed",
    "auth_subject_unaffiliated",
    "capacity_exceeded",
    "forbidden",
    "identity_service_unavailable",
    "integrity_failed",
    "storage_unavailable",
    "trailbase_proof_invalid",
    "trailbase_session_cleanup_failed",
    "trailbase_trust_unavailable",
    "validation_failed",
];
const COMPLETE_TRAILBASE_CONTINUATION_PROBLEMS: [&str; 16] = [
    "auth_browser_binding_invalid",
    "auth_continuation_persistence_failed",
    "auth_selection_changed",
    "auth_subject_unaffiliated",
    "capacity_exceeded",
    "forbidden",
    "identity_service_unavailable",
    "integrity_failed",
    "malformed_json",
    "payload_too_large",
    "storage_unavailable",
    "trailbase_proof_invalid",
    "trailbase_session_cleanup_failed",
    "trailbase_trust_unavailable",
    "unsupported_media_type",
    "validation_failed",
];
const CANCEL_TRAILBASE_CONTINUATION_PROBLEMS: [&str; 6] = [
    "auth_browser_binding_invalid",
    "forbidden",
    "integrity_failed",
    "storage_unavailable",
    "trailbase_proof_invalid",
    "validation_failed",
];
const BROWSER_SESSION_READ_PROBLEMS: [&str; 5] = [
    "browser_session_expired",
    "browser_session_revoked",
    "integrity_failed",
    "session_policy_changed",
    "storage_unavailable",
];
const BROWSER_SESSION_MUTATION_PROBLEMS: [&str; 6] = [
    "browser_session_expired",
    "browser_session_revoked",
    "forbidden",
    "integrity_failed",
    "session_policy_changed",
    "storage_unavailable",
];

fn production_problem_codes(
    operation: ConformanceOperation,
    capability: &Value,
) -> anyhow::Result<Vec<Value>> {
    let exact: Option<&[&str]> = match operation.operation_id {
        "start_trailbase_sign_in" => Some(&START_TRAILBASE_SIGN_IN_PROBLEMS),
        "read_trailbase_continuation" => Some(&READ_TRAILBASE_CONTINUATION_PROBLEMS),
        "complete_trailbase_continuation" => Some(&COMPLETE_TRAILBASE_CONTINUATION_PROBLEMS),
        "cancel_trailbase_continuation" => Some(&CANCEL_TRAILBASE_CONTINUATION_PROBLEMS),
        "read_access_projection" | "read_browser_session" | "list_browser_sessions" => {
            Some(&BROWSER_SESSION_READ_PROBLEMS)
        }
        "end_browser_session"
        | "revoke_other_browser_sessions"
        | "revoke_all_browser_sessions"
        | "rotate_browser_session" => Some(&BROWSER_SESSION_MUTATION_PROBLEMS),
        "revoke_browser_session" => Some(&[
            "browser_session_expired",
            "browser_session_revoked",
            "forbidden",
            "integrity_failed",
            "session_policy_changed",
            "storage_unavailable",
            "validation_failed",
        ]),
        "select_browser_session_profile" => Some(&[
            "browser_session_expired",
            "browser_session_revoked",
            "forbidden",
            "integrity_failed",
            "malformed_json",
            "payload_too_large",
            "session_policy_changed",
            "storage_unavailable",
            "unsupported_media_type",
            "validation_failed",
        ]),
        _ => None,
    };
    let governed = array_at(capability, "/problems")?;
    if let Some(exact) = exact {
        for code in exact {
            ensure!(
                governed.iter().any(|value| value.as_str() == Some(code)),
                "production operation {} claims problem {code} outside its capability",
                operation.operation_id
            );
        }
        return Ok(exact
            .iter()
            .map(|code| Value::String((*code).to_owned()))
            .collect());
    }
    Ok(governed
        .iter()
        .filter(|problem| {
            !CREDENTIAL_ONLY_HYBRID_OPERATIONS.contains(&operation.operation_id)
                || problem
                    .as_str()
                    .is_none_or(|code| !BROWSER_SESSION_PROBLEMS.contains(&code))
        })
        .cloned()
        .collect())
}

fn production_operation_authorization(
    operation: ConformanceOperation,
    capability_authorization: &str,
) -> anyhow::Result<&str> {
    if CREDENTIAL_ONLY_HYBRID_OPERATIONS.contains(&operation.operation_id) {
        ensure!(
            operation.capability_id == "observation.accept"
                && operation.path.starts_with("/api/v1/integrations/"),
            "credential-only hybrid override escaped governed webhook ingress"
        );
        Ok("scoped")
    } else {
        Ok(capability_authorization)
    }
}

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
    let provider_manifest_schema = integration::provider_manifest_schema()?;
    let portability_v2_schema = portability_v2_schema()?;
    let portability_v2_example = portability_v2_example(workspace_root)?;
    let portability_v3_schema = portability_v3_schema()?;
    let portability_v3_example = portability_v3_example(workspace_root)?;
    let portability_v4_schema = portability_v4_schema()?;
    let portability_v4_example = portability_v4_example(workspace_root)?;
    let portability_v5_schema = portability_v5_schema()?;
    let portability_v5_example = portability_v5_example(workspace_root)?;
    let portability_v6_schema = portability_v6_schema()?;
    let portability_v6_example = portability_v6_example(workspace_root)?;
    let portability_v7_schema = portability_v7_schema()?;
    let portability_v7_example = portability_v7_example(workspace_root)?;
    let asyncapi = load_yaml(workspace_root, ASYNCAPI_PATH)?;
    let mut production_openapi = serde_json::to_value(fasti_api::openapi())
        .context("production OpenAPI is not serializable")?;
    enrich_production_openapi(
        workspace_root,
        &mut production_openapi,
        &public_registry,
        &capability_keys,
    )?;
    validate_access_contract_secrets(&production_openapi)?;
    validate_trailbase_continuation_contract(&production_openapi)?;
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
    let sdk_source = typescript_sdk(
        &public_registry,
        &problem_catalog,
        &health_schema,
        &problem_schema,
        &asyncapi,
        &production_openapi,
        &conformance_openapi,
    )?;
    let sdk_transport = fs::read_to_string(workspace_root.join("packages/sdk/src/transport.ts"))
        .context("SDK transport source is unreadable")?;
    validate_required_bindings(
        workspace_root,
        &capability_keys,
        &production_openapi,
        &conformance_openapi,
        &asyncapi,
        &problem_catalog,
        &health_schema,
        &sdk_source,
        &sdk_transport,
    )?;
    insert(&mut artifacts, OPENAPI_PATH, production_openapi.clone())?;
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
    insert(
        &mut artifacts,
        PROVIDER_MANIFEST_SCHEMA_PATH,
        provider_manifest_schema,
    )?;
    insert(
        &mut artifacts,
        PORTABILITY_V2_SCHEMA_PATH,
        portability_v2_schema,
    )?;
    insert(
        &mut artifacts,
        PORTABILITY_V2_EXAMPLE_PATH,
        portability_v2_example,
    )?;
    insert(
        &mut artifacts,
        PORTABILITY_V3_SCHEMA_PATH,
        portability_v3_schema,
    )?;
    insert(
        &mut artifacts,
        PORTABILITY_V3_EXAMPLE_PATH,
        portability_v3_example,
    )?;
    insert(
        &mut artifacts,
        PORTABILITY_V4_SCHEMA_PATH,
        portability_v4_schema,
    )?;
    insert(
        &mut artifacts,
        PORTABILITY_V4_EXAMPLE_PATH,
        portability_v4_example,
    )?;
    insert(
        &mut artifacts,
        PORTABILITY_V5_SCHEMA_PATH,
        portability_v5_schema,
    )?;
    insert(
        &mut artifacts,
        PORTABILITY_V5_EXAMPLE_PATH,
        portability_v5_example,
    )?;
    insert(
        &mut artifacts,
        PORTABILITY_V6_SCHEMA_PATH,
        portability_v6_schema,
    )?;
    insert(
        &mut artifacts,
        PORTABILITY_V6_EXAMPLE_PATH,
        portability_v6_example,
    )?;
    insert(
        &mut artifacts,
        PORTABILITY_V7_SCHEMA_PATH,
        portability_v7_schema,
    )?;
    insert(
        &mut artifacts,
        PORTABILITY_V7_EXAMPLE_PATH,
        portability_v7_example,
    )?;
    insert_bytes(&mut artifacts, SDK_GENERATED_PATH, sdk_source.into_bytes())?;
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

fn portability_v2_schema() -> anyhow::Result<Value> {
    let mut schema = draft_2020_12_schema::<ChecksummedWorkspaceManifestDto>()?;
    let root = schema
        .as_object_mut()
        .context("portability schema root must be an object")?;
    root.insert(
        "$id".to_owned(),
        Value::String(
            "https://fasti.scrobble.dev/schemas/internal-staged/portability/v2/workspace-manifest.json"
                .to_owned(),
        ),
    );
    root.insert(
        "title".to_owned(),
        Value::String("InternalStagedChecksummedWorkspaceManifestV2".to_owned()),
    );
    root.insert(
        "$comment".to_owned(),
        Value::String(
            "Internal staged B3 archive v2. It extends the frozen v1 stream prefix with metadata and profile tracking state."
                .to_owned(),
        ),
    );
    root.insert(
        "x-fasti-contract-state".to_owned(),
        Value::String("internal_staged_archive_v2".to_owned()),
    );

    *schema
        .pointer_mut("/$defs/WorkspaceManifestDto/properties/format_version")
        .context("generated portability schema omits format_version")? = serde_json::json!({
        "const": 2
    });
    let entities = WorkspaceExportEntity::V2.map(WorkspaceExportEntity::as_str);
    *schema
        .pointer_mut("/$defs/WorkspaceExportEntityDto/enum")
        .context("generated portability schema omits export entity enum")? =
        serde_json::to_value(entities)?;
    *schema
        .pointer_mut("/$defs/WorkspaceManifestDto/properties/streams")
        .context("generated portability schema omits streams")? = serde_json::json!({
        "type": "array",
        "minItems": entities.len(),
        "maxItems": entities.len(),
        "prefixItems": entities.map(|entity| serde_json::json!({
            "allOf": [
                { "$ref": "#/$defs/WorkspaceStreamDescriptorDto" },
                {
                    "type": "object",
                    "properties": { "entity": { "const": entity } }
                }
            ]
        })),
        "items": false
    });
    Ok(schema)
}

fn portability_v2_example(workspace_root: &Path) -> anyhow::Result<Value> {
    let source = fs::read(workspace_root.join(PORTABILITY_V1_EXAMPLE_PATH))?;
    let mut example: Value =
        serde_json::from_slice(&source).context("archive-v1 manifest example is not valid JSON")?;
    *example
        .pointer_mut("/manifest/format_version")
        .context("archive-v1 example omits format_version")? = serde_json::json!(2);
    let streams = example
        .pointer_mut("/manifest/streams")
        .and_then(Value::as_array_mut)
        .context("archive-v1 example omits streams")?;
    let empty_digest = format!("sha256:{}", crate::evidence::sha256_bytes(&[]));
    for entity in WorkspaceExportEntity::V2[WorkspaceExportEntity::V1.len()..]
        .iter()
        .map(|entity| entity.as_str())
    {
        streams.push(serde_json::json!({
            "entity": entity,
            "row_count": 0,
            "byte_length": 0,
            "digest": empty_digest,
        }));
    }
    let manifest = example
        .get("manifest")
        .context("archive-v1 example omits manifest")?;
    let canonical = serde_json_canonicalizer::to_vec(manifest)
        .context("archive-v2 example manifest is not canonicalizable")?;
    example["manifest_digest"] = Value::String(format!(
        "sha256:{}",
        crate::evidence::sha256_bytes(&canonical)
    ));
    Ok(example)
}

fn portability_v3_schema() -> anyhow::Result<Value> {
    let mut schema = draft_2020_12_schema::<ChecksummedWorkspaceManifestDto>()?;
    let root = schema
        .as_object_mut()
        .context("portability schema root must be an object")?;
    root.insert(
        "$id".to_owned(),
        Value::String(
            "https://fasti.scrobble.dev/schemas/internal-staged/portability/v3/workspace-manifest.json"
                .to_owned(),
        ),
    );
    root.insert(
        "title".to_owned(),
        Value::String("InternalStagedChecksummedWorkspaceManifestV3".to_owned()),
    );
    root.insert(
        "$comment".to_owned(),
        Value::String(
            "Internal staged B3 archive v3. It extends the frozen v2 stream prefix with authoritative M2 metadata state. Derived projections and disposable cache state are excluded."
                .to_owned(),
        ),
    );
    root.insert(
        "x-fasti-contract-state".to_owned(),
        Value::String("internal_staged_archive_v3".to_owned()),
    );
    *schema
        .pointer_mut("/$defs/WorkspaceManifestDto/properties/format_version")
        .context("generated portability schema omits format_version")? = serde_json::json!({
        "const": 3
    });
    let entities = WorkspaceExportEntity::V3.map(WorkspaceExportEntity::as_str);
    *schema
        .pointer_mut("/$defs/WorkspaceExportEntityDto/enum")
        .context("generated portability schema omits export entity enum")? =
        serde_json::to_value(entities)?;
    *schema
        .pointer_mut("/$defs/WorkspaceManifestDto/properties/streams")
        .context("generated portability schema omits streams")? = serde_json::json!({
        "type": "array",
        "minItems": entities.len(),
        "maxItems": entities.len(),
        "prefixItems": entities.map(|entity| serde_json::json!({
            "allOf": [
                { "$ref": "#/$defs/WorkspaceStreamDescriptorDto" },
                {
                    "type": "object",
                    "properties": { "entity": { "const": entity } }
                }
            ]
        })),
        "items": false
    });
    Ok(schema)
}

fn portability_v3_example(workspace_root: &Path) -> anyhow::Result<Value> {
    let source = fs::read(workspace_root.join(PORTABILITY_V2_EXAMPLE_PATH))?;
    let mut example: Value =
        serde_json::from_slice(&source).context("archive-v2 manifest example is not valid JSON")?;
    *example
        .pointer_mut("/manifest/format_version")
        .context("archive-v2 example omits format_version")? = serde_json::json!(3);
    let streams = example
        .pointer_mut("/manifest/streams")
        .and_then(Value::as_array_mut)
        .context("archive-v2 example omits streams")?;
    let empty_digest = format!("sha256:{}", crate::evidence::sha256_bytes(&[]));
    for entity in WorkspaceExportEntity::V3[WorkspaceExportEntity::V2.len()..]
        .iter()
        .map(|entity| entity.as_str())
    {
        streams.push(serde_json::json!({
            "entity": entity,
            "row_count": 0,
            "byte_length": 0,
            "digest": empty_digest,
        }));
    }
    let manifest = example
        .get("manifest")
        .context("archive-v2 example omits manifest")?;
    let canonical = serde_json_canonicalizer::to_vec(manifest)
        .context("archive-v3 example manifest is not canonicalizable")?;
    example["manifest_digest"] = Value::String(format!(
        "sha256:{}",
        crate::evidence::sha256_bytes(&canonical)
    ));
    Ok(example)
}

fn portability_v4_schema() -> anyhow::Result<Value> {
    let mut schema = draft_2020_12_schema::<ChecksummedWorkspaceManifestDto>()?;
    let root = schema
        .as_object_mut()
        .context("portability schema root must be an object")?;
    root.insert(
        "$id".to_owned(),
        Value::String(
            "https://fasti.scrobble.dev/schemas/internal-staged/portability/v4/workspace-manifest.json"
                .to_owned(),
        ),
    );
    root.insert(
        "title".to_owned(),
        Value::String("InternalStagedChecksummedWorkspaceManifestV4".to_owned()),
    );
    root.insert(
        "$comment".to_owned(),
        Value::String(
            "Internal staged B3 archive v4. It extends the frozen v3 stream prefix with immutable metadata refresh receipts."
                .to_owned(),
        ),
    );
    root.insert(
        "x-fasti-contract-state".to_owned(),
        Value::String("internal_staged_archive_v4".to_owned()),
    );
    *schema
        .pointer_mut("/$defs/WorkspaceManifestDto/properties/format_version")
        .context("generated portability schema omits format_version")? = serde_json::json!({
        "const": 4
    });
    let entities = WorkspaceExportEntity::V4.map(WorkspaceExportEntity::as_str);
    *schema
        .pointer_mut("/$defs/WorkspaceExportEntityDto/enum")
        .context("generated portability schema omits export entity enum")? =
        serde_json::to_value(entities)?;
    *schema
        .pointer_mut("/$defs/WorkspaceManifestDto/properties/streams")
        .context("generated portability schema omits streams")? = serde_json::json!({
        "type": "array",
        "minItems": entities.len(),
        "maxItems": entities.len(),
        "prefixItems": entities.map(|entity| serde_json::json!({
            "allOf": [
                { "$ref": "#/$defs/WorkspaceStreamDescriptorDto" },
                {
                    "type": "object",
                    "properties": { "entity": { "const": entity } }
                }
            ]
        })),
        "items": false
    });
    Ok(schema)
}

fn portability_v4_example(workspace_root: &Path) -> anyhow::Result<Value> {
    let source = fs::read(workspace_root.join(PORTABILITY_V3_EXAMPLE_PATH))?;
    let mut example: Value =
        serde_json::from_slice(&source).context("archive-v3 manifest example is not valid JSON")?;
    *example
        .pointer_mut("/manifest/format_version")
        .context("archive-v3 example omits format_version")? = serde_json::json!(4);
    let streams = example
        .pointer_mut("/manifest/streams")
        .and_then(Value::as_array_mut)
        .context("archive-v3 example omits streams")?;
    let empty_digest = format!("sha256:{}", crate::evidence::sha256_bytes(&[]));
    for entity in WorkspaceExportEntity::V4[WorkspaceExportEntity::V3.len()..]
        .iter()
        .map(|entity| entity.as_str())
    {
        streams.push(serde_json::json!({
            "entity": entity,
            "row_count": 0,
            "byte_length": 0,
            "digest": empty_digest,
        }));
    }
    let manifest = example
        .get("manifest")
        .context("archive-v3 example omits manifest")?;
    let canonical = serde_json_canonicalizer::to_vec(manifest)
        .context("archive-v4 example manifest is not canonicalizable")?;
    example["manifest_digest"] = Value::String(format!(
        "sha256:{}",
        crate::evidence::sha256_bytes(&canonical)
    ));
    Ok(example)
}

fn portability_v5_schema() -> anyhow::Result<Value> {
    let mut schema = draft_2020_12_schema::<ChecksummedWorkspaceManifestDto>()?;
    let root = schema
        .as_object_mut()
        .context("portability schema root must be an object")?;
    root.insert(
        "$id".to_owned(),
        Value::String(
            "https://fasti.scrobble.dev/schemas/internal-staged/portability/v5/workspace-manifest.json"
                .to_owned(),
        ),
    );
    root.insert(
        "title".to_owned(),
        Value::String("InternalStagedChecksummedWorkspaceManifestV5".to_owned()),
    );
    root.insert(
        "$comment".to_owned(),
        Value::String(
            "Internal staged B3 archive v5. It extends the frozen v4 stream prefix with authoritative identity-routing and anime-grouping policy state."
                .to_owned(),
        ),
    );
    root.insert(
        "x-fasti-contract-state".to_owned(),
        Value::String("internal_staged_archive_v5".to_owned()),
    );
    *schema
        .pointer_mut("/$defs/WorkspaceManifestDto/properties/format_version")
        .context("generated portability schema omits format_version")? = serde_json::json!({
        "const": 5
    });
    let entities = WorkspaceExportEntity::V5
        .iter()
        .map(|entity| entity.as_str())
        .collect::<Vec<_>>();
    *schema
        .pointer_mut("/$defs/WorkspaceExportEntityDto/enum")
        .context("generated portability schema omits export entity enum")? =
        serde_json::to_value(&entities)?;
    *schema
        .pointer_mut("/$defs/WorkspaceManifestDto/properties/streams")
        .context("generated portability schema omits streams")? = serde_json::json!({
        "type": "array",
        "minItems": entities.len(),
        "maxItems": entities.len(),
        "prefixItems": entities.iter().map(|entity| serde_json::json!({
            "allOf": [
                { "$ref": "#/$defs/WorkspaceStreamDescriptorDto" },
                {
                    "type": "object",
                    "properties": { "entity": { "const": entity } }
                }
            ]
        })).collect::<Vec<_>>(),
        "items": false
    });
    Ok(schema)
}

fn portability_v5_example(workspace_root: &Path) -> anyhow::Result<Value> {
    let source = fs::read(workspace_root.join(PORTABILITY_V4_EXAMPLE_PATH))?;
    let mut example: Value =
        serde_json::from_slice(&source).context("archive-v4 manifest example is not valid JSON")?;
    *example
        .pointer_mut("/manifest/format_version")
        .context("archive-v4 example omits format_version")? = serde_json::json!(5);
    let streams = example
        .pointer_mut("/manifest/streams")
        .and_then(Value::as_array_mut)
        .context("archive-v4 example omits streams")?;
    let empty_digest = format!("sha256:{}", crate::evidence::sha256_bytes(&[]));
    for entity in WorkspaceExportEntity::V5[WorkspaceExportEntity::V4.len()..]
        .iter()
        .map(|entity| entity.as_str())
    {
        streams.push(serde_json::json!({
            "entity": entity,
            "row_count": 0,
            "byte_length": 0,
            "digest": empty_digest,
        }));
    }
    let manifest = example
        .get("manifest")
        .context("archive-v4 example omits manifest")?;
    let canonical = serde_json_canonicalizer::to_vec(manifest)
        .context("archive-v5 example manifest is not canonicalizable")?;
    example["manifest_digest"] = Value::String(format!(
        "sha256:{}",
        crate::evidence::sha256_bytes(&canonical)
    ));
    Ok(example)
}

fn portability_v6_schema() -> anyhow::Result<Value> {
    let mut schema = portability_v5_schema()?;
    schema["$id"] = serde_json::json!(
        "https://fasti.scrobble.dev/schemas/internal-staged/portability/v6/workspace-manifest.json"
    );
    schema["title"] = serde_json::json!("InternalStagedChecksummedWorkspaceManifestV6");
    schema["$comment"] = serde_json::json!("Internal staged B3 archive v6. It appends durable Search action receipts to frozen v5. Temporary Search results and authentication state remain excluded.");
    schema["x-fasti-contract-state"] = serde_json::json!("internal_staged_archive_v6");
    schema["$defs"]["WorkspaceManifestDto"]["properties"]["format_version"] =
        serde_json::json!({"const": 6});
    let entity = WorkspaceExportEntity::SearchActionReceipts.as_str();
    schema["$defs"]["WorkspaceExportEntityDto"]["enum"]
        .as_array_mut()
        .context("archive-v5 schema omits entity enum")?
        .push(serde_json::json!(entity));
    let streams = &mut schema["$defs"]["WorkspaceManifestDto"]["properties"]["streams"];
    streams["minItems"] = serde_json::json!(WorkspaceExportEntity::V6.len());
    streams["maxItems"] = serde_json::json!(WorkspaceExportEntity::V6.len());
    streams["prefixItems"]
        .as_array_mut()
        .context("archive-v5 schema omits stream prefix")?
        .push(serde_json::json!({"allOf": [
            {"$ref": "#/$defs/WorkspaceStreamDescriptorDto"},
            {"type": "object", "properties": {"entity": {"const": entity}}}
        ]}));
    Ok(schema)
}

fn portability_v6_example(workspace_root: &Path) -> anyhow::Result<Value> {
    let mut example = portability_v5_example(workspace_root)?;
    example["manifest"]["format_version"] = serde_json::json!(6);
    example["manifest"]["streams"]
        .as_array_mut()
        .context("archive-v5 example omits streams")?
        .push(serde_json::json!({
            "entity": WorkspaceExportEntity::SearchActionReceipts.as_str(),
            "row_count": 0, "byte_length": 0,
            "digest": format!("sha256:{}", crate::evidence::sha256_bytes(&[])),
        }));
    let canonical = serde_json_canonicalizer::to_vec(&example["manifest"])?;
    example["manifest_digest"] = serde_json::json!(format!(
        "sha256:{}",
        crate::evidence::sha256_bytes(&canonical)
    ));
    Ok(example)
}

fn portability_v7_schema() -> anyhow::Result<Value> {
    let mut schema = portability_v6_schema()?;
    schema["$id"] = serde_json::json!(
        "https://fasti.scrobble.dev/schemas/internal-staged/portability/v7/workspace-manifest.json"
    );
    schema["title"] = serde_json::json!("InternalStagedChecksummedWorkspaceManifestV7");
    schema["$comment"] = serde_json::json!("Internal staged B3 archive v7. MetadataClaims retain nullable response policy evidence. The 35-entity inventory is unchanged; v6 row shapes remain frozen for restore.");
    schema["x-fasti-contract-state"] = serde_json::json!("internal_staged_archive_v7");
    schema["$defs"]["WorkspaceManifestDto"]["properties"]["format_version"] =
        serde_json::json!({"const": 7});
    Ok(schema)
}

fn portability_v7_example(workspace_root: &Path) -> anyhow::Result<Value> {
    let mut example = portability_v6_example(workspace_root)?;
    example["manifest"]["format_version"] = serde_json::json!(7);
    let canonical = serde_json_canonicalizer::to_vec(&example["manifest"])?;
    example["manifest_digest"] = serde_json::json!(format!(
        "sha256:{}",
        crate::evidence::sha256_bytes(&canonical)
    ));
    Ok(example)
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
        let problems = conformance_problem_codes(capability, expected)?;
        let examples = array_at(capability, "/examples")?.to_vec();
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
            Value::String(
                if expected.operation_id == "accept_observation" {
                    "scoped"
                } else {
                    string_at(capability, "/authorization")?
                }
                .to_owned(),
            ),
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
            Value::String("fixture_only".to_owned()),
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

fn conformance_problem_codes(
    capability: &Value,
    expected: ConformanceOperation,
) -> anyhow::Result<Vec<Value>> {
    let production_only: &[&str] = match expected.capability_id {
        "node.initialize" => &[
            "already_initialized",
            "bootstrap_closed",
            "integrity_failed",
            "storage_unavailable",
        ],
        "client.enroll" => &[
            "already_initialized",
            "bootstrap_closed",
            "integrity_failed",
            "storage_unavailable",
        ],
        "observation.accept" if expected.operation_id == "accept_observation" => {
            &BROWSER_SESSION_PROBLEMS
        }
        _ => &[],
    };
    Ok(array_at(capability, "/problems")?
        .iter()
        .filter(|problem| {
            problem
                .as_str()
                .is_none_or(|code| !production_only.contains(&code))
        })
        .cloned()
        .collect())
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
            r"^(?:ID|MDN)-[0-9]{3}$",
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
            vec!["b0", "b1", "b2", "b3", "c1", "m1", "m2", "m3", "m4"],
        ),
        (
            "/components/schemas/CapabilityUatDto/properties/relationship",
            vec!["deferred", "direct", "split"],
        ),
        (
            "/components/schemas/CapabilityUatDto/properties/owner_body",
            vec!["b1", "b2", "b3", "c1", "m1", "m2", "m3"],
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

fn enrich_production_openapi(
    workspace_root: &Path,
    openapi: &mut Value,
    public_registry: &Value,
    capability_keys: &BTreeMap<String, CapabilityKey>,
) -> anyhow::Result<()> {
    validate_production_security_schemes(openapi)?;
    enrich_production_health_openapi(workspace_root, openapi, public_registry)?;
    enrich_production_integration_status_openapi(workspace_root, openapi, public_registry)?;
    let policy_changes = openapi
        .pointer_mut("/components/schemas/AnimeGroupingPolicyChangeDto/oneOf")
        .and_then(Value::as_array_mut)
        .context("AnimeGroupingPolicyChangeDto variants are absent")?;
    ensure!(
        policy_changes.len() == 3,
        "AnimeGroupingPolicyChangeDto variant count changed"
    );
    for variant in policy_changes {
        variant
            .as_object_mut()
            .context("anime grouping policy change variant must be an object")?
            .insert("additionalProperties".to_owned(), Value::Bool(false));
    }
    for (name, count) in [
        ("SearchProviderPageResponse", 3),
        ("SearchCandidateDetailsResponse", 6),
        ("SearchCandidateActionResponse", 2),
        ("SearchRecordActionDto", 2),
    ] {
        let search_outcomes = openapi
            .pointer_mut(&format!("/components/schemas/{name}/oneOf"))
            .and_then(Value::as_array_mut)
            .context("SearchProviderPageResponse variants are absent")?;
        ensure!(
            search_outcomes.len() == count,
            "Search page outcome count changed"
        );
        for variant in search_outcomes {
            variant
                .as_object_mut()
                .context("Search outcome must be an object")?
                .insert("additionalProperties".to_owned(), Value::Bool(false));
        }
    }
    let capabilities = array_at(public_registry, "/capabilities")?;
    for expected in PRODUCTION_BOOTSTRAP_OPERATIONS
        .into_iter()
        .chain(PRODUCTION_RUNTIME_OPERATIONS)
    {
        let capability = capabilities
            .iter()
            .find(|capability| string_at(capability, "/id").ok() == Some(expected.capability_id))
            .with_context(|| {
                format!(
                    "production operation {} references absent registry capability {}",
                    expected.operation_id, expected.capability_id
                )
            })?;
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
                    "production OpenAPI omits {} {}",
                    expected.method, expected.path
                )
            })?;
        operation.insert(
            "x-fasti-capability-id".to_owned(),
            Value::String(expected.capability_id.to_owned()),
        );
        operation.insert(
            "x-fasti-required-scopes".to_owned(),
            Value::Array(array_at(capability, "/scopes")?.clone()),
        );
        if expected.operation_id == "save_search_candidate" {
            let search_capability = capabilities
                .iter()
                .find(|candidate| string_at(candidate, "/id").ok() == Some("metadata.search"))
                .context("Search candidate save requires the metadata.search capability")?;
            operation.insert(
                "x-fasti-conditional-required-scopes".to_owned(),
                serde_json::json!({
                    "new_operation": array_at(search_capability, "/scopes")?
                }),
            );
        }
        operation.insert(
            "x-fasti-authorization".to_owned(),
            Value::String(
                production_operation_authorization(
                    expected,
                    string_at(capability, "/authorization")?,
                )?
                .to_owned(),
            ),
        );
        let problems = production_problem_codes(expected, capability)?;
        if expected.operation_id == "search_local_records" {
            operation.insert(
                "x-fasti-max-response-bytes".to_owned(),
                serde_json::json!(fasti_application::MAX_LOCAL_SEARCH_RESPONSE_BYTES),
            );
        }
        operation.insert(
            "x-fasti-problem-codes".to_owned(),
            Value::Array(problems.clone()),
        );
        let examples = array_at(capability, "/examples")?.clone();
        operation.insert(
            "x-fasti-example-ids".to_owned(),
            Value::Array(examples.clone()),
        );
        operation.insert(
            "x-fasti-runtime-availability".to_owned(),
            Value::String(string_at(capability, "/lifecycle/runtime_availability")?.to_owned()),
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
            &Value::Null,
        )?;
    }
    enrich_trailbase_callback_openapi(openapi, public_registry)?;
    Ok(())
}

fn enrich_trailbase_callback_openapi(
    openapi: &mut Value,
    public_registry: &Value,
) -> anyhow::Result<()> {
    let capability = array_at(public_registry, "/capabilities")?
        .iter()
        .find(|capability| string_at(capability, "/id").ok() == Some("browser.session.create"))
        .context("registry omits browser.session.create")?;
    let operation = openapi
        .pointer_mut("/paths/~1api~1access~1v1~1trailbase~1callback/get")
        .and_then(Value::as_object_mut)
        .context("production OpenAPI omits the TrailBase callback")?;
    ensure!(
        operation.get("operationId").and_then(Value::as_str)
            == Some("complete_trailbase_authentication"),
        "TrailBase callback operation ID changed"
    );
    ensure!(
        operation.get("requestBody").is_none(),
        "TrailBase callback must not accept a request body"
    );
    ensure!(
        operation.get("security") == Some(&serde_json::json!([{"auth_binding_cookie": []}])),
        "TrailBase callback must require only the binding cookie"
    );
    let responses = operation
        .get("responses")
        .and_then(Value::as_object)
        .context("TrailBase callback responses must be an object")?;
    ensure!(
        responses.len() == 1 && responses.contains_key("303"),
        "TrailBase callback must expose only its fixed 303 redirect"
    );
    let parameters = operation
        .get("parameters")
        .and_then(Value::as_array)
        .context("TrailBase callback parameters must be an array")?;
    ensure!(
        parameters.len() == 1
            && string_at(&parameters[0], "/name")? == "code"
            && string_at(&parameters[0], "/in")? == "query"
            && parameters[0].get("required") == Some(&Value::Bool(true))
            && u64_at(&parameters[0], "/schema/minLength")? == 48
            && u64_at(&parameters[0], "/schema/maxLength")? == 48
            && string_at(&parameters[0], "/schema/pattern")? == "^[A-Za-z0-9]{48}$",
        "TrailBase callback must expose one exact 48-character code query"
    );
    for (name, value) in [
        (
            "x-fasti-capability-id",
            Value::String("browser.session.create".to_owned()),
        ),
        (
            "x-fasti-required-scopes",
            Value::Array(array_at(capability, "/scopes")?.clone()),
        ),
        (
            "x-fasti-authorization",
            Value::String(string_at(capability, "/authorization")?.to_owned()),
        ),
        ("x-fasti-problem-codes", Value::Array(Vec::new())),
        (
            "x-fasti-example-ids",
            Value::Array(array_at(capability, "/examples")?.clone()),
        ),
        (
            "x-fasti-runtime-availability",
            Value::String(string_at(capability, "/lifecycle/runtime_availability")?.to_owned()),
        ),
    ] {
        operation.insert(name.to_owned(), value);
    }
    Ok(())
}

const FORBIDDEN_ACCESS_CONTRACT_PROPERTIES: [&str; 16] = [
    "access_token",
    "bootstrap_secret",
    "browser_binding",
    "browser_binding_digest",
    "code_verifier",
    "credential",
    "credential_digest",
    "csrf",
    "csrf_digest",
    "csrf_secret",
    "csrf_token",
    "id_token",
    "refresh_token",
    "session_digest",
    "session_secret",
    "vendor_token",
];

fn validate_access_contract_secrets(openapi: &Value) -> anyhow::Result<()> {
    let paths = object_at(openapi, "/paths")?;
    let mut seen_refs = BTreeSet::new();
    for (path, path_item) in paths {
        if !path.starts_with("/api/access/v1/") {
            continue;
        }
        for operation in path_item
            .as_object()
            .context("Access path item must be an object")?
            .values()
        {
            for key in ["parameters", "requestBody", "responses"] {
                if let Some(surface) = operation.get(key) {
                    validate_access_schema_node(openapi, surface, &mut seen_refs)?;
                }
            }
        }
    }
    Ok(())
}

fn validate_trailbase_continuation_contract(openapi: &Value) -> anyhow::Result<()> {
    let schemas = object_at(openapi, "/components/schemas")?;
    for (name, expected) in [
        ("StartTrailBaseSignInRequest", &["remembered"][..]),
        (
            "StartTrailBaseSignInResponse",
            &["authorization_url", "expires_at"][..],
        ),
        (
            "TrailBaseContinuationChoiceDto",
            &[
                "choice_ordinal",
                "workspace_ordinal",
                "profile_ordinal",
                "workspace_created_at",
                "profile_created_at",
                "membership_state",
                "role",
            ][..],
        ),
        (
            "ReadTrailBaseContinuationResponse",
            &["expires_at", "remembered", "candidate_revision", "choices"][..],
        ),
        (
            "CompleteTrailBaseContinuationRequest",
            &["choice_ordinal", "candidate_revision"][..],
        ),
    ] {
        let schema = schemas
            .get(name)
            .with_context(|| format!("production OpenAPI omits {name}"))?;
        ensure!(
            schema.get("additionalProperties").and_then(Value::as_bool) == Some(false),
            "{name} must reject unknown fields"
        );
        let actual: BTreeSet<_> = object_at(schema, "/properties")?
            .keys()
            .map(String::as_str)
            .collect();
        let expected: BTreeSet<_> = expected.iter().copied().collect();
        ensure!(actual == expected, "{name} exposes unexpected properties");
    }
    for (pointer, minimum, maximum) in [
        (
            "/components/schemas/TrailBaseContinuationChoiceDto/properties/choice_ordinal",
            0,
            63,
        ),
        (
            "/components/schemas/TrailBaseContinuationChoiceDto/properties/workspace_ordinal",
            1,
            64,
        ),
        (
            "/components/schemas/TrailBaseContinuationChoiceDto/properties/profile_ordinal",
            1,
            64,
        ),
        (
            "/components/schemas/CompleteTrailBaseContinuationRequest/properties/choice_ordinal",
            0,
            63,
        ),
    ] {
        let schema = value_at(openapi, pointer)?;
        ensure!(
            u64_at(schema, "/minimum")? == minimum && u64_at(schema, "/maximum")? == maximum,
            "{pointer} has the wrong numeric bounds"
        );
    }
    let choices = value_at(
        openapi,
        "/components/schemas/ReadTrailBaseContinuationResponse/properties/choices",
    )?;
    ensure!(
        u64_at(choices, "/minItems")? == 1 && u64_at(choices, "/maxItems")? == 64,
        "TrailBase continuation choices must contain 1 to 64 entries"
    );
    for pointer in [
        "/components/schemas/ReadTrailBaseContinuationResponse/properties/candidate_revision",
        "/components/schemas/CompleteTrailBaseContinuationRequest/properties/candidate_revision",
    ] {
        let revision = value_at(openapi, pointer)?;
        ensure!(
            u64_at(revision, "/minLength")? == 71
                && u64_at(revision, "/maxLength")? == 71
                && string_at(revision, "/pattern")? == r"^sha256:[0-9a-f]{64}$",
            "{pointer} must use the canonical SHA-256 revision"
        );
    }
    Ok(())
}

fn validate_access_schema_node(
    openapi: &Value,
    node: &Value,
    seen_refs: &mut BTreeSet<String>,
) -> anyhow::Result<()> {
    match node {
        Value::Array(values) => {
            for value in values {
                validate_access_schema_node(openapi, value, seen_refs)?;
            }
        }
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                ensure!(
                    reference.starts_with("#/components/schemas/"),
                    "Access contract contains a non-local schema reference"
                );
                if seen_refs.insert(reference.to_owned()) {
                    let pointer = reference
                        .strip_prefix('#')
                        .expect("local schema reference has a fragment");
                    let target = value_at(openapi, pointer).with_context(|| {
                        format!("Access schema reference {reference} is absent")
                    })?;
                    validate_access_schema_node(openapi, target, seen_refs)?;
                }
            }
            if let Some(properties) = object.get("properties").and_then(Value::as_object) {
                for name in properties.keys() {
                    ensure!(
                        !FORBIDDEN_ACCESS_CONTRACT_PROPERTIES.contains(&name.as_str())
                            && name != "token",
                        "Access contract exposes forbidden secret property {name}"
                    );
                }
            }
            for value in object.values() {
                validate_access_schema_node(openapi, value, seen_refs)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_production_security_schemes(openapi: &Value) -> anyhow::Result<()> {
    for name in ["bootstrap_bearer", "credential_bearer"] {
        let pointer = format!("/components/securitySchemes/{name}");
        let scheme = value_at(openapi, &pointer)?;
        ensure!(
            string_at(scheme, "/type")? == "http" && string_at(scheme, "/scheme")? == "bearer",
            "production security scheme {name} must be HTTP bearer"
        );
    }
    for (name, location, wire_name) in [
        ("browser_session_cookie", "cookie", "__Host-fasti_session"),
        ("csrf_cookie", "cookie", "__Host-fasti_csrf"),
        ("csrf_header", "header", "X-CSRF-Token"),
        (
            "auth_binding_cookie",
            "cookie",
            "__Secure-fasti_auth_binding",
        ),
        (
            "auth_continuation_cookie",
            "cookie",
            "__Secure-fasti_auth_continuation",
        ),
    ] {
        let pointer = format!("/components/securitySchemes/{name}");
        let scheme = value_at(openapi, &pointer)?;
        ensure!(
            string_at(scheme, "/type")? == "apiKey"
                && string_at(scheme, "/in")? == location
                && string_at(scheme, "/name")? == wire_name,
            "production security scheme {name} must be the exact {location} {wire_name}"
        );
    }
    Ok(())
}

fn validate_production_operation_security(
    operation: &Value,
    operation_id: &str,
    method: &str,
    path: &str,
) -> anyhow::Result<()> {
    let access_security = match operation_id {
        "start_trailbase_sign_in" => Some(serde_json::json!(null)),
        "read_trailbase_continuation"
        | "complete_trailbase_continuation"
        | "cancel_trailbase_continuation" => {
            Some(serde_json::json!([{"auth_continuation_cookie": []}]))
        }
        "read_access_projection" | "read_browser_session" | "list_browser_sessions" => {
            Some(serde_json::json!([{"browser_session_cookie": []}]))
        }
        "end_browser_session"
        | "revoke_browser_session"
        | "revoke_other_browser_sessions"
        | "revoke_all_browser_sessions"
        | "rotate_browser_session"
        | "select_browser_session_profile" => Some(serde_json::json!([{
            "browser_session_cookie": [],
            "csrf_cookie": [],
            "csrf_header": []
        }])),
        "submit_observation"
        | "search_provider_page"
        | "save_search_candidate"
        | "create_record"
        | "attach_identifier"
        | "register_namespace"
        | "set_tracking_disposition"
        | "replace_nuvio_collections"
        | "clear_nuvio_collections"
        | "apply_anime_grouping_policy_change" => Some(serde_json::json!([
            {"credential_bearer": []},
            {
                "browser_session_cookie": [],
                "csrf_cookie": [],
                "csrf_header": []
            }
        ])),
        _ => None,
    };
    if let Some(expected) = access_security {
        if expected.is_null() {
            ensure!(
                operation.get("security").is_none(),
                "production operation {method} {path} must not declare authentication"
            );
        } else {
            ensure!(
                operation.get("security") == Some(&expected),
                "production operation {method} {path} has the wrong browser security requirement"
            );
        }
        return Ok(());
    }
    let expected = match operation_id {
        "initialize_node" => vec!["bootstrap_bearer"],
        "list_records"
        | "read_search_candidate"
        | "search_local_records"
        | "list_tracking_dispositions"
        | "get_nuvio_collections"
        | "resolve_identity_route"
        | "read_anime_grouping_policy"
        | "preview_anime_grouping_policy_change" => {
            vec!["credential_bearer", "browser_session_cookie"]
        }
        "list_providers"
        | "configure_provider_credential"
        | "remove_provider_credential"
        | "test_provider_credential"
        | "read_provider_health"
        | "refresh_metadata_claims"
        | "read_metadata_projection"
        | "configure_metadata_projection" => vec!["credential_bearer"],
        "nuvio_webhook" | "tautulli_webhook" | "jellyfin_webhook" | "emby_webhook"
        | "plex_webhook" => vec!["credential_bearer"],
        "enroll_first_client" | "health_check" | "integration_status" => Vec::new(),
        other => anyhow::bail!("unknown production operation {other}"),
    };
    if expected.is_empty() {
        ensure!(
            operation.get("security").is_none(),
            "production operation {method} {path} must not declare authentication"
        );
        return Ok(());
    }
    let requirements = array_at(operation, "/security")?;
    ensure!(
        requirements.len() == expected.len(),
        "production operation {method} {path} has the wrong number of security requirements"
    );
    for scheme in expected {
        ensure!(
            requirements.iter().any(|requirement| {
                requirement.as_object().is_some_and(|requirement| {
                    requirement.len() == 1
                        && requirement
                            .get(scheme)
                            .and_then(Value::as_array)
                            .is_some_and(Vec::is_empty)
                })
            }),
            "production operation {method} {path} must use {scheme} without OAuth scopes"
        );
    }
    Ok(())
}

fn enrich_governed_success_operation_openapi(
    workspace_root: &Path,
    openapi: &mut Value,
    public_registry: &Value,
    capability_id: &str,
    path_pointer: &str,
    example_id: &str,
) -> anyhow::Result<()> {
    let capability = array_at(public_registry, "/capabilities")?
        .iter()
        .find(|capability| string_at(capability, "/id").ok() == Some(capability_id))
        .with_context(|| format!("public registry omits {capability_id}"))?;
    let operation = openapi
        .pointer_mut(path_pointer)
        .and_then(Value::as_object_mut)
        .with_context(|| format!("production OpenAPI omits operation at {path_pointer}"))?;
    operation.insert(
        "x-fasti-capability-id".to_owned(),
        Value::String(capability_id.to_owned()),
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
        example_ids.len() == 1 && example_ids[0].as_str() == Some(example_id),
        "production {capability_id} must own exactly the governed {example_id} example"
    );
    let example = load_governed_example(workspace_root, example_id, &Value::Null)?;
    ensure!(
        example.media_type == "application/json",
        "{example_id} must be an application/json example"
    );
    let media = operation
        .get_mut("responses")
        .and_then(Value::as_object_mut)
        .and_then(|responses| responses.get_mut("200"))
        .and_then(|response| response.get_mut("content"))
        .and_then(Value::as_object_mut)
        .and_then(|content| content.get_mut("application/json"))
        .and_then(Value::as_object_mut)
        .with_context(|| {
            format!("production {capability_id} 200 response omits application/json")
        })?;
    media.insert(
        "examples".to_owned(),
        serde_json::json!({
            example_id: { "value": example.payload }
        }),
    );
    Ok(())
}

fn enrich_production_health_openapi(
    workspace_root: &Path,
    openapi: &mut Value,
    public_registry: &Value,
) -> anyhow::Result<()> {
    enrich_governed_success_operation_openapi(
        workspace_root,
        openapi,
        public_registry,
        "system.health",
        "/paths/~1api~1v1~1health/get",
        "system.health.success",
    )
}

/// Enriches GET /api/v1/integrations directly, bypassing the generic
/// [`PRODUCTION_RUNTIME_OPERATIONS`] path. That path's example binding only
/// supports governed problem examples or the literal `system.capabilities.success`
/// id; `integration.status` has no problems and needs its own always-200
/// success example, so it is special-cased the same way `system.health` is.
fn enrich_production_integration_status_openapi(
    workspace_root: &Path,
    openapi: &mut Value,
    public_registry: &Value,
) -> anyhow::Result<()> {
    enrich_governed_success_operation_openapi(
        workspace_root,
        openapi,
        public_registry,
        "integration.status",
        "/paths/~1api~1v1~1integrations/get",
        "integration.status.success",
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_required_bindings(
    workspace_root: &Path,
    capability_keys: &BTreeMap<String, CapabilityKey>,
    production_openapi: &Value,
    conformance_openapi: &Value,
    asyncapi: &Value,
    problem_catalog: &Value,
    health_schema: &Value,
    sdk_source: &str,
    sdk_transport: &str,
) -> anyhow::Result<()> {
    for required in registry::finalized_required_bindings(workspace_root)? {
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
            sdk_source,
            sdk_transport,
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

/// True only if some operation entry for `capability_id` was actually emitted
/// into the generated SDK text, not merely declared in one of the
/// `*_OPERATIONS` arrays that feed the generator. Membership in those arrays
/// is necessary but not sufficient -- a rendering gap (a category the
/// generator forgot to render, as `PRODUCTION_RUNTIME_OPERATIONS` once was)
/// would still pass a membership-only check while shipping an SDK with no
/// method for that capability.
fn sdk_source_declares_capability(sdk_source: &str, capability_id: &str) -> bool {
    PRODUCTION_BOOTSTRAP_OPERATIONS
        .iter()
        .chain(PRODUCTION_RUNTIME_OPERATIONS.iter())
        .chain(CONFORMANCE_OPERATIONS.iter())
        .filter(|operation| operation.capability_id == capability_id)
        .any(|operation| sdk_source.contains(&format!("  {}: {{ operationId:", operation.alias)))
}

fn sdk_transport_declares_capability(sdk_transport: &str, capability_id: &str) -> bool {
    PRODUCTION_BOOTSTRAP_OPERATIONS
        .iter()
        .chain(PRODUCTION_RUNTIME_OPERATIONS.iter())
        .chain(CONFORMANCE_OPERATIONS.iter())
        .filter(|operation| operation.capability_id == capability_id)
        .any(|operation| sdk_transport.contains(&format!("\n  {}(", operation.alias)))
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
    sdk_source: &str,
    sdk_transport: &str,
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
            if binding == "cli:access-identity-bootstrap" {
                let main_source =
                    fs::read_to_string(workspace_root.join("crates/fasti-cli/src/main.rs"))?;
                ensure!(
                    capability_id == "access.identity.bootstrap"
                        && main_source.contains("BootstrapAdministrator")
                        && main_source.contains("LocalOperatorAccessRuntime")
                        && main_source.contains("--password")
                        && main_source.contains(
                            "first_administrator_cli_accepts_only_private_root_locations"
                        ),
                    "trusted CLI first-administrator bootstrap binding is absent"
                );
                return Ok(());
            }
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
            "schema:production-openapi-operation:{capability_id}" => ensure!(
                openapi_has_capability(production_openapi, capability_id)?,
                "production operation schema is absent"
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
                    || (sdk_source_declares_capability(sdk_source, capability_id)
                        && sdk_transport_declares_capability(sdk_transport, capability_id)),
                "SDK omits a generated operation entry or callable client method for this capability"
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
            "package-smoke:production-bootstrap" => {
                let smoke = fs::read_to_string(workspace_root.join("scripts/smoke-native.sh"))?;
                ensure!(
                    smoke.contains("/api/v1/node/initialization")
                        && smoke.contains("/api/v1/client-enrollments"),
                    "production bootstrap package smoke is absent"
                );
            }
            "package-smoke:production-providers" => {
                let smoke = fs::read_to_string(workspace_root.join("scripts/smoke-native.sh"))?;
                ensure!(
                    smoke.contains("/api/v1/providers")
                        && smoke.contains("len(provider_rows) != 12")
                        && smoke.contains("active_providers = {\"tmdb\", \"google-books\"}")
                        && smoke.contains("capability.get(\"credential_state\")")
                        && smoke.contains("capability.get(\"writable\")")
                        && smoke.contains("capability.get(\"testable\")"),
                    "production provider smoke is absent"
                );
            }
            "package-smoke:production-metadata" => {
                let smoke = fs::read_to_string(workspace_root.join("scripts/smoke-native.sh"))?;
                ensure!(
                    smoke.contains("/api/v1/metadata/claims/refresh")
                        && smoke.contains("\"operation_id\"")
                        && smoke.contains("/api/v1/records/")
                        && smoke.contains("/metadata-projection")
                        && smoke.contains("metadata_claim_stale")
                        && smoke.contains("projection.get(\"fields\")"),
                    "production metadata smoke is absent"
                );
            }
            "package-smoke:production-identity-routing" => {
                let smoke = fs::read_to_string(workspace_root.join("scripts/smoke-native.sh"))?;
                ensure!(
                    smoke.contains("/identity-route?intent=metadata_lookup&target_provider=tmdb")
                        && smoke.contains("/api/v1/profile/anime-grouping-policy?scope=profile")
                        && smoke.contains("/api/v1/profile/anime-grouping-policy/preview")
                        && smoke.contains("\"expected_revision\": 0")
                        && smoke.contains("immutable receipt"),
                    "production identity-routing smoke is absent"
                );
            }
            "package-smoke:c1-operator-bootstrap" => {
                let smoke = fs::read_to_string(workspace_root.join("scripts/smoke-oci.sh"))?;
                ensure!(
                    capability_id == "access.identity.bootstrap"
                        && smoke.contains("access bootstrap-administrator --help")
                        && smoke.contains("--password")
                        && smoke.contains("/missing-fasti-data-root"),
                    "C1 operator bootstrap package smoke is absent"
                );
            }
            _ => anyhow::bail!("unknown package-smoke binding"),
        },
        "ui" => match binding {
            "ui:account-security" => {
                ensure!(
                    capability_id == "access.projection.read",
                    "the Account and Security UI binding belongs only to the Access projection"
                );
                let types = fs::read_to_string(workspace_root.join("packages/ui/src/types.ts"))?;
                let host = fs::read_to_string(workspace_root.join("apps/web/src/web-host.ts"))?;
                let workbench = fs::read_to_string(
                    workspace_root.join("packages/ui/src/fasti-workbench.svelte"),
                )?;
                let view = fs::read_to_string(
                    workspace_root.join("packages/ui/src/account-security-view.svelte"),
                )?;
                let browser =
                    fs::read_to_string(workspace_root.join("tests/e2e/access-c1.spec.ts"))?;
                ensure!(
                    types.contains("readAccessProjection?")
                        && host.contains("const accessClient = new FastiClient")
                        && host.contains("accessClient.readAccessProjection")
                        && workbench.contains("function acceptAccessProjection")
                        && workbench.contains("function readAccessProjection")
                        && workbench.contains("clearProfileOwnedWorkbenchState")
                        && workbench.contains("window.addEventListener(\"focus\"")
                        && view.contains("const projection = await readAccessProjection()")
                        && view.contains("onProjection?.(undefined)")
                        && browser.contains("one shared projection read owns navigation")
                        && browser.contains("an expired browser-session deadline clears profile authority")
                        && browser.contains("window focus revalidates cached browser-session authority")
                        && browser.contains("a committed revocation cannot resurrect stale session inventory"),
                    "Account and Security UI does not preserve the governed Access projection boundary"
                );
            }
            "ui:provider-settings" => {
                let view = fs::read_to_string(
                    workspace_root.join("packages/ui/src/runtime-settings-view.svelte"),
                )?;
                ensure!(
                    view.contains("providerCredentialStatus")
                        && view.contains("saveProviderCredential")
                        && view.contains("deleteProviderCredential")
                        && view.contains("testProviderCredential")
                        && view.contains("readProviderHealth"),
                    "provider settings UI does not cover every M1 provider operation"
                );
            }
            "ui:metadata-provenance" => {
                let types = fs::read_to_string(workspace_root.join("packages/ui/src/types.ts"))?;
                let detail = fs::read_to_string(
                    workspace_root.join("packages/ui/src/media-detail-view.svelte"),
                )?;
                let settings = fs::read_to_string(
                    workspace_root.join("packages/ui/src/runtime-settings-view.svelte"),
                )?;
                ensure!(
                    types.contains("readMetadataProjection")
                        && types.contains("configureMetadataProjection")
                        && types.contains("refreshMetadataClaims")
                        && detail.contains("metadata-projection")
                        && detail.contains("metadata-field-provenance")
                        && detail.contains("metadata-rating-provenance")
                        && detail.contains("metadata-attributions")
                        && detail.contains("metadata-cache-state")
                        && detail.contains("metadata-offline-state")
                        && detail.contains("refresh-metadata-claims")
                        && settings.contains("metadata-projection-policy")
                        && settings.contains("configure-metadata-projection"),
                    "metadata UI does not cover projection, provenance, attribution, freshness, offline state, refresh, and profile policy"
                );
            }
            "ui:anime-grouping-policy" => {
                let types = fs::read_to_string(workspace_root.join("packages/ui/src/types.ts"))?;
                let host = fs::read_to_string(workspace_root.join("apps/web/src/web-host.ts"))?;
                let settings = fs::read_to_string(
                    workspace_root.join("packages/ui/src/runtime-settings-view.svelte"),
                )?;
                ensure!(
                    types.contains("readAnimeGroupingPolicy?")
                        && types.contains("previewAnimeGroupingPolicyChange?")
                        && types.contains("applyAnimeGroupingPolicyChange?")
                        && host.contains("readAnimeGroupingPolicy: (query)")
                        && host.contains("previewAnimeGroupingPolicyChange: (request)")
                        && host.contains("applyAnimeGroupingPolicyChange: (request)")
                        && settings.contains("data-testid=\"anime-grouping-policy\"")
                        && settings.contains("data-testid=\"preview-anime-grouping-policy\"")
                        && settings.contains("data-testid=\"apply-anime-grouping-policy\"")
                        && settings.contains("record.proposed_route")
                        && settings.contains("Application clients can keep a separate override"),
                    "anime grouping UI does not expose governed read, preview, apply, route evidence, and application-client semantics"
                );
            }
            _ => anyhow::bail!("unknown UI binding"),
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
    let operation_subset = expected.operation_id == "remove_provider_credential"
        && expected.capability_id == "provider.credential.configure";
    let mut governed_statuses = BTreeSet::new();
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
        governed_statuses.insert(status.clone());
        if let Some(response) = responses.get(&status) {
            ensure!(
                string_at(response, "/content/application~1problem+json/schema/$ref")?
                    == "#/components/schemas/ProblemDetails",
                "{} {} cannot represent governed problem {raw_code} as ProblemDetails",
                expected.method,
                expected.path
            );
        } else {
            ensure!(
                operation_subset,
                "{} {} cannot represent governed problem {raw_code}: response {status} is absent",
                expected.method,
                expected.path
            );
        }
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
    if operation_subset {
        let expected_subset = ["401", "403", "422", "500", "503"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        ensure!(
            documented_problem_statuses == expected_subset,
            "{} {} removal responses drift from its capability representation: documented={documented_problem_statuses:?}, expected={expected_subset:?}",
            expected.method,
            expected.path
        );
    } else {
        ensure!(
            documented_problem_statuses == governed_statuses,
            "{} {} problem responses drift from registry claims: documented={documented_problem_statuses:?}, governed={governed_statuses:?}",
            expected.method,
            expected.path
        );
    }
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
                example_id == "system.capabilities.success"
                    || example_id == "integration.status.success",
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

fn render_production_bootstrap_contract(openapi: &Value) -> anyhow::Result<String> {
    ensure!(
        string_at(openapi, "/openapi")? == "3.1.0",
        "production OpenAPI must remain 3.1.0"
    );
    let expected_paths: BTreeSet<_> = std::iter::once("/api/v1/health")
        .chain(
            PRODUCTION_BOOTSTRAP_OPERATIONS
                .iter()
                .map(|operation| operation.path),
        )
        .collect();
    let actual_paths: BTreeSet<_> = object_at(openapi, "/paths")?
        .keys()
        .map(String::as_str)
        .collect();
    ensure!(
        expected_paths.is_subset(&actual_paths),
        "production OpenAPI is missing a bootstrap route: expected {expected_paths:?}, found {actual_paths:?}"
    );

    let schemas = object_at(openapi, "/components/schemas")?;
    let mut output = String::new();
    for name in ["ClientEnrollmentResponse", "NodeInitializationResponse"] {
        let schema = schemas
            .get(name)
            .with_context(|| format!("production OpenAPI omits {name}"))?;
        output.push_str(
            &render_interface(name, schema)
                .with_context(|| format!("failed to render production DTO {name}"))?,
        );
        output.push('\n');
    }

    output.push_str("// prettier-ignore\nexport const LOCAL_BOOTSTRAP_OPERATIONS = {\n");
    for expected in PRODUCTION_BOOTSTRAP_OPERATIONS {
        let operation_pointer = format!(
            "/paths/{}/{}",
            escape_pointer(expected.path),
            expected.method
        );
        let operation = value_at(openapi, &operation_pointer)?;
        ensure!(
            string_at(operation, "/operationId")? == expected.operation_id,
            "production operation ID changed for {} {}",
            expected.method,
            expected.path
        );

        // Extract and validate request schema from OpenAPI
        let request_name = match expected.request {
            Some(expected_request) => {
                let actual_ref = string_at(
                    operation,
                    "/requestBody/content/application~1json/schema/$ref",
                )?;
                let expected_ref = format!("#/components/schemas/{}", expected_request);
                ensure!(
                    actual_ref == expected_ref,
                    "production request schema mismatch for {} {}: expected {}, found {}",
                    expected.method,
                    expected.path,
                    expected_ref,
                    actual_ref
                );
                Some(expected_request)
            }
            None => {
                ensure!(
                    operation.get("requestBody").is_none(),
                    "unexpected request body for {} {}",
                    expected.method,
                    expected.path
                );
                None
            }
        };

        // Extract and validate response schema from OpenAPI
        let response_name = match expected.response {
            Some(expected_response) => {
                let actual_ref = string_at(
                    operation,
                    "/responses/200/content/application~1json/schema/$ref",
                )?;
                let expected_ref = format!("#/components/schemas/{}", expected_response);
                ensure!(
                    actual_ref == expected_ref,
                    "production response schema mismatch for {} {}: expected {}, found {}",
                    expected.method,
                    expected.path,
                    expected_ref,
                    actual_ref
                );
                Some(expected_response)
            }
            None => None,
        };

        // Bootstrap proofs stay in request bodies. The initialization route uses its
        // separate data-root bootstrap bearer; enrollment consumes the proof body and
        // must not receive either bearer credential.
        let authorization = string_at(operation, "/x-fasti-authorization")?;
        let authenticated = expected.authenticated;
        validate_production_operation_security(
            operation,
            expected.operation_id,
            expected.method,
            expected.path,
        )?;

        let required_scopes =
            serde_json::to_string(array_at(operation, "/x-fasti-required-scopes")?)?;
        let problem_codes = serde_json::to_string(array_at(operation, "/x-fasti-problem-codes")?)?;
        let example_ids = serde_json::to_string(array_at(operation, "/x-fasti-example-ids")?)?;
        writeln!(
            output,
            "  {}: {{ operationId: {}, method: {}, path: {}, capabilityId: {}, authorization: {}, requiredScopes: {required_scopes}, problemCodes: {problem_codes}, exampleIds: {example_ids}, authenticated: {}, runtimeAvailability: {}, durability: \"durable\", retry: \"never\", requestSchema: {}, responseSchema: {} }},",
            expected.alias,
            json_string(expected.operation_id)?,
            json_string(&expected.method.to_ascii_uppercase())?,
            json_string(expected.path)?,
            json_string(expected.capability_id)?,
            json_string(authorization)?,
            authenticated,
            json_string(string_at(
                operation,
                "/x-fasti-runtime-availability"
            )?)?,
            request_name
                .map(json_string)
                .transpose()?
                .unwrap_or_else(|| "null".to_owned()),
            response_name
                .map(json_string)
                .transpose()?
                .unwrap_or_else(|| "null".to_owned()),
        )?;
    }
    output.push_str("} as const;\n\n");

    // Dumps every production schema, not just the two named above -- the
    // runtime contract rendered separately by `render_production_runtime_contract`
    // reuses this same dump rather than emitting its own, since it validates
    // against the same production OpenAPI document.
    let schemas_json = serde_json::to_string_pretty(&sort_json(Value::Object(schemas.clone())))?;
    writeln!(
        output,
        "// prettier-ignore\nconst PRODUCTION_SCHEMAS = {schemas_json} as const;\n"
    )?;
    output.push_str(
        r#"// prettier-ignore
export function parseNodeInitializationResponse(value: unknown): NodeInitializationResponse {
  return parseProductionDto("NodeInitializationResponse", value);
}

// prettier-ignore
export function parseClientEnrollmentResponse(value: unknown): ClientEnrollmentResponse {
  return parseProductionDto("ClientEnrollmentResponse", value);
}

// prettier-ignore
function parseProductionDto<T>(schemaName: string, value: unknown): T {
  const schema = (PRODUCTION_SCHEMAS as Record<string, unknown>)[schemaName];
  if (schema === undefined) {
    throw new FastiContractParseError(`Unknown production schema ${schemaName}`);
  }
  validateOpenApiValue(value, schema, schemaName, PRODUCTION_SCHEMAS as Record<string, unknown>);
  return value as T;
}

"#,
    );
    Ok(output)
}

/// Renders the durable, authenticated production-runtime surface (records,
/// observations) that runs after bootstrap. Parallels
/// `render_production_bootstrap_contract` but for `PRODUCTION_RUNTIME_OPERATIONS`,
/// and reuses that function's `PRODUCTION_SCHEMAS` dump rather than emitting
/// its own -- both validate against the same production OpenAPI document, so
/// this must run after `render_production_bootstrap_contract` in the output.
fn render_production_runtime_contract(openapi: &Value) -> anyhow::Result<String> {
    let expected_paths: BTreeSet<_> = PRODUCTION_RUNTIME_OPERATIONS
        .iter()
        .map(|operation| operation.path)
        .collect();
    let actual_paths: BTreeSet<_> = object_at(openapi, "/paths")?
        .keys()
        .map(String::as_str)
        .collect();
    ensure!(
        expected_paths.is_subset(&actual_paths),
        "production OpenAPI is missing a runtime route: expected {expected_paths:?}, found {actual_paths:?}"
    );

    let schemas = object_at(openapi, "/components/schemas")?;
    let mut output = String::new();
    for name in [
        "ObservationIngressKind",
        "TrackingDispositionDto",
        "TrackingDispositionUpdateDto",
        "ProviderKindDto",
        "CredentialRequirementDto",
        "ProviderCredentialStateDto",
        "ProviderCredentialSourceDto",
        "ProviderCapabilityStateDto",
        "ProviderCheckStateDto",
        "MetadataFieldGroupDto",
        "MetadataRefreshModeDto",
        "MetadataClaimStatusDto",
        "MetadataProjectionTierDto",
        "LastKnownGoodPolicyDto",
        "MetadataCachePurposeDto",
        "MetadataDataClassificationDto",
        "MetadataCacheInvalidationReasonDto",
        "MetadataCacheReadStateDto",
        "AccessEvidenceStateDto",
        "AccessSubjectLifecycleDto",
        "AccessMembershipLifecycleDto",
        "AccessWorkspaceRoleDto",
        "TrailBaseActivationStateDto",
        "TrailBaseActivationBlockerDto",
        "AccessAuthenticationMethodDto",
        "AccessEvidenceKindDto",
        "AccessCeremonyStateDto",
        "AccessCeremonyFailureDto",
        "AccessFirstRunStepKeyDto",
        "ResolutionIntentDto",
        "IdentityRouteStatusDto",
        "IdentityRouteKindDto",
        "IdentityAssertionRelationDto",
        "AnimeGroupingPreferenceDto",
        "AnimeGroupingPolicyScopeKindDto",
        "AnimeGroupingPolicySourceDto",
        "AnimeGroupingPolicyChangeDto",
        "SearchCacheStateDto",
        "SearchCandidateEvidenceModeDto",
        "SearchRecordActionDispositionDto",
        "SearchEvidenceStatusDto",
        "SearchRecordActionDto",
    ] {
        let schema = schemas
            .get(name)
            .with_context(|| format!("production OpenAPI omits {name}"))?;
        writeln!(
            output,
            "// prettier-ignore\nexport type {name} = {};\n",
            typescript_type(schema)
                .with_context(|| format!("failed to render production runtime type {name}"))?
        )?;
    }
    output.push_str(
        "// The Nuvio wire document intentionally preserves extension fields.\n\
         export type NuvioCollectionsDocumentDto = ReadonlyArray<Record<string, unknown>>;\n\n",
    );
    for name in [
        "ObservationIdentifierInput",
        "SubmitObservationRequest",
        "SubmitObservationResponse",
        "CreateRecordRequest",
        "CreateRecordResponse",
        "ResolvedFieldDto",
        "RecordActivityDto",
        "RecordIdentifierDto",
        "RecordSummaryDto",
        "ListRecordsResponse",
        "ListRecordsQueryParameters",
        "AttachIdentifierRequest",
        "AttachIdentifierResponse",
        "RegisterNamespaceRequest",
        "RegisterNamespaceResponse",
        "SetTrackingDispositionRequest",
        "TrackingDispositionStateDto",
        "ListTrackingDispositionsResponse",
        "NuvioCollectionsStateDto",
        "ProviderCheckDto",
        "ProviderCapabilityDto",
        "ProviderDescriptorDto",
        "ListProvidersResponse",
        "ConfigureProviderCredentialRequest",
        "ProviderCapabilityResponse",
        "ProviderHealthResponse",
        "RefreshMetadataClaimsRequest",
        "SearchProviderPageRequest",
        "LocalSearchRequestDto",
        "LocalSearchResponseDto",
        "LocalSearchCursorDto",
        "SearchProviderPageResponse",
        "SearchCandidateDetailsQueryParameters",
        "SearchCandidateDetailsResponse",
        "SearchCandidateSnapshotDto",
        "SearchCandidateActionRequest",
        "SearchCandidateActionResponse",
        "SearchCandidateActionReceiptDto",
        "SearchCandidateReceiptDto",
        "SearchCandidateDto",
        "SearchReceiptLifetimeDto",
        "MetadataClaimProvenanceDto",
        "MetadataClaimDto",
        "RatingScaleDto",
        "RatingClaimDto",
        "MetadataProjectedFieldDto",
        "EnrichmentPolicyDto",
        "MetadataCacheKeyDto",
        "MetadataCacheInvalidationDto",
        "MetadataCacheEntryDto",
        "MetadataAttributionDto",
        "RefreshMetadataClaimsResponse",
        "MetadataProjectionResponse",
        "MetadataProjectionQueryParameters",
        "MetadataOverrideMutationDto",
        "ConfigureMetadataProjectionRequest",
        "MetadataProjectionConfigurationResponse",
        "IdentityIdentifierDto",
        "AcceptedIdentityRouteAssertionDto",
        "IdentityRouteDto",
        "ResolveIdentityRouteResponse",
        "AnimeGroupingPolicyScopeDto",
        "AnimeGroupingPolicyDto",
        "ReadAnimeGroupingPolicyResponse",
        "PreviewAnimeGroupingPolicyChangeRequest",
        "AnimeGroupingRecordPreviewDto",
        "AnimeGroupingPolicyImpactResponse",
        "ApplyAnimeGroupingPolicyChangeRequest",
        "ApplyAnimeGroupingPolicyChangeResponse",
        "StartTrailBaseSignInRequest",
        "StartTrailBaseSignInResponse",
        "TrailBaseContinuationChoiceDto",
        "ReadTrailBaseContinuationResponse",
        "CompleteTrailBaseContinuationRequest",
        "SelectBrowserSessionProfileRequest",
        "BrowserSessionDto",
        "ReadBrowserSessionResponse",
        "ListBrowserSessionsResponse",
        "RevokeBrowserSessionsResponse",
        "RotateBrowserSessionResponse",
        "SelectBrowserSessionProfileResponse",
        "AccessSubjectDto",
        "AccessMembershipDto",
        "AccessProfileGrantDto",
        "BrowserSessionPolicyDto",
        "RecentAuthenticationDto",
        "AccessSessionAuthenticationDto",
        "TrailBaseActivationDto",
        "AccessFirstRunStepDto",
        "AccessEvidenceDto",
        "AccessProjectionResponse",
    ] {
        let schema = schemas
            .get(name)
            .with_context(|| format!("production OpenAPI omits {name}"))?;
        output.push_str(
            &render_interface(name, schema)
                .with_context(|| format!("failed to render production runtime DTO {name}"))?,
        );
        output.push('\n');
    }

    output.push_str("// prettier-ignore\nexport const LOCAL_RUNTIME_OPERATIONS = {\n");
    for expected in PRODUCTION_RUNTIME_OPERATIONS {
        let operation_pointer = format!(
            "/paths/{}/{}",
            escape_pointer(expected.path),
            expected.method
        );
        let operation = value_at(openapi, &operation_pointer)?;
        ensure!(
            string_at(operation, "/operationId")? == expected.operation_id,
            "production operation ID changed for {} {}",
            expected.method,
            expected.path
        );

        let request_name = match expected.request {
            Some(expected_request) => {
                let actual_ref = string_at(
                    operation,
                    "/requestBody/content/application~1json/schema/$ref",
                )?;
                let expected_ref = format!("#/components/schemas/{}", expected_request);
                ensure!(
                    actual_ref == expected_ref,
                    "production request schema mismatch for {} {}: expected {}, found {}",
                    expected.method,
                    expected.path,
                    expected_ref,
                    actual_ref
                );
                Some(expected_request)
            }
            None => {
                ensure!(
                    operation.get("requestBody").is_none(),
                    "unexpected request body for {} {}",
                    expected.method,
                    expected.path
                );
                None
            }
        };

        let response_name = match expected.response {
            Some(expected_response) => {
                let actual_ref = string_at(
                    operation,
                    "/responses/200/content/application~1json/schema/$ref",
                )?;
                let expected_ref = format!("#/components/schemas/{}", expected_response);
                ensure!(
                    actual_ref == expected_ref,
                    "production response schema mismatch for {} {}: expected {}, found {}",
                    expected.method,
                    expected.path,
                    expected_ref,
                    actual_ref
                );
                Some(expected_response)
            }
            None => None,
        };

        let authorization = string_at(operation, "/x-fasti-authorization")?;
        let authenticated = expected.authenticated;
        validate_production_operation_security(
            operation,
            expected.operation_id,
            expected.method,
            expected.path,
        )?;

        let required_scopes =
            serde_json::to_string(array_at(operation, "/x-fasti-required-scopes")?)?;
        let conditional_required_scopes = operation
            .get("x-fasti-conditional-required-scopes")
            .map(serde_json::to_string)
            .transpose()?
            .map(|value| format!(", conditionalRequiredScopes: {value}"))
            .unwrap_or_default();
        let problem_codes = serde_json::to_string(array_at(operation, "/x-fasti-problem-codes")?)?;
        let example_ids = serde_json::to_string(array_at(operation, "/x-fasti-example-ids")?)?;
        writeln!(
            output,
            "  {}: {{ operationId: {}, method: {}, path: {}, capabilityId: {}, authorization: {}, requiredScopes: {required_scopes}{conditional_required_scopes}, problemCodes: {problem_codes}, exampleIds: {example_ids}, authenticated: {}, runtimeAvailability: {}, durability: \"durable\", retry: {}, requestSchema: {}, responseSchema: {} }},",
            expected.alias,
            json_string(expected.operation_id)?,
            json_string(&expected.method.to_ascii_uppercase())?,
            json_string(expected.path)?,
            json_string(expected.capability_id)?,
            json_string(authorization)?,
            authenticated,
            json_string(string_at(
                operation,
                "/x-fasti-runtime-availability"
            )?)?,
            json_string(expected.retry)?,
            request_name
                .map(json_string)
                .transpose()?
                .unwrap_or_else(|| "null".to_owned()),
            response_name
                .map(json_string)
                .transpose()?
                .unwrap_or_else(|| "null".to_owned()),
        )?;
    }
    output.push_str("} as const;\n\n");

    writeln!(
        output,
        "export const LOCAL_SEARCH_MAX_RESPONSE_BYTES = {} as const;\n",
        value_at(
            openapi,
            "/paths/~1api~1v1~1search~1records/post/x-fasti-max-response-bytes"
        )?
        .as_u64()
        .context("local Search must publish its response byte limit")?
    )?;

    for (alias, dto) in [
        ("parseSubmitObservationRequest", "SubmitObservationRequest"),
        ("parseLocalSearchRequestDto", "LocalSearchRequestDto"),
        ("parseLocalSearchResponseDto", "LocalSearchResponseDto"),
        (
            "parseSubmitObservationResponse",
            "SubmitObservationResponse",
        ),
        ("parseCreateRecordRequest", "CreateRecordRequest"),
        ("parseCreateRecordResponse", "CreateRecordResponse"),
        ("parseListRecordsResponse", "ListRecordsResponse"),
        (
            "parseListRecordsQueryParameters",
            "ListRecordsQueryParameters",
        ),
        ("parseAttachIdentifierRequest", "AttachIdentifierRequest"),
        ("parseAttachIdentifierResponse", "AttachIdentifierResponse"),
        ("parseRegisterNamespaceRequest", "RegisterNamespaceRequest"),
        (
            "parseRegisterNamespaceResponse",
            "RegisterNamespaceResponse",
        ),
        (
            "parseSetTrackingDispositionRequest",
            "SetTrackingDispositionRequest",
        ),
        (
            "parseTrackingDispositionStateDto",
            "TrackingDispositionStateDto",
        ),
        (
            "parseListTrackingDispositionsResponse",
            "ListTrackingDispositionsResponse",
        ),
        (
            "parseNuvioCollectionsDocumentDto",
            "NuvioCollectionsDocumentDto",
        ),
        ("parseNuvioCollectionsStateDto", "NuvioCollectionsStateDto"),
        (
            "parseConfigureProviderCredentialRequest",
            "ConfigureProviderCredentialRequest",
        ),
        ("parseListProvidersResponse", "ListProvidersResponse"),
        (
            "parseProviderCapabilityResponse",
            "ProviderCapabilityResponse",
        ),
        ("parseProviderHealthResponse", "ProviderHealthResponse"),
        (
            "parseSearchProviderPageRequest",
            "SearchProviderPageRequest",
        ),
        (
            "parseSearchProviderPageResponse",
            "SearchProviderPageResponse",
        ),
        (
            "parseSearchCandidateDetailsQueryParameters",
            "SearchCandidateDetailsQueryParameters",
        ),
        (
            "parseSearchCandidateDetailsResponse",
            "SearchCandidateDetailsResponse",
        ),
        (
            "parseSearchCandidateActionRequest",
            "SearchCandidateActionRequest",
        ),
        (
            "parseSearchCandidateActionResponse",
            "SearchCandidateActionResponse",
        ),
        (
            "parseRefreshMetadataClaimsRequest",
            "RefreshMetadataClaimsRequest",
        ),
        (
            "parseRefreshMetadataClaimsResponse",
            "RefreshMetadataClaimsResponse",
        ),
        (
            "parseMetadataProjectionResponse",
            "MetadataProjectionResponse",
        ),
        (
            "parseConfigureMetadataProjectionRequest",
            "ConfigureMetadataProjectionRequest",
        ),
        (
            "parseMetadataProjectionConfigurationResponse",
            "MetadataProjectionConfigurationResponse",
        ),
        (
            "parseResolveIdentityRouteResponse",
            "ResolveIdentityRouteResponse",
        ),
        (
            "parseReadAnimeGroupingPolicyResponse",
            "ReadAnimeGroupingPolicyResponse",
        ),
        (
            "parsePreviewAnimeGroupingPolicyChangeRequest",
            "PreviewAnimeGroupingPolicyChangeRequest",
        ),
        (
            "parseAnimeGroupingPolicyImpactResponse",
            "AnimeGroupingPolicyImpactResponse",
        ),
        (
            "parseApplyAnimeGroupingPolicyChangeRequest",
            "ApplyAnimeGroupingPolicyChangeRequest",
        ),
        (
            "parseApplyAnimeGroupingPolicyChangeResponse",
            "ApplyAnimeGroupingPolicyChangeResponse",
        ),
        (
            "parseStartTrailBaseSignInRequest",
            "StartTrailBaseSignInRequest",
        ),
        (
            "parseStartTrailBaseSignInResponse",
            "StartTrailBaseSignInResponse",
        ),
        (
            "parseReadTrailBaseContinuationResponse",
            "ReadTrailBaseContinuationResponse",
        ),
        (
            "parseCompleteTrailBaseContinuationRequest",
            "CompleteTrailBaseContinuationRequest",
        ),
        (
            "parseSelectBrowserSessionProfileRequest",
            "SelectBrowserSessionProfileRequest",
        ),
        (
            "parseReadBrowserSessionResponse",
            "ReadBrowserSessionResponse",
        ),
        (
            "parseListBrowserSessionsResponse",
            "ListBrowserSessionsResponse",
        ),
        (
            "parseRevokeBrowserSessionsResponse",
            "RevokeBrowserSessionsResponse",
        ),
        (
            "parseRotateBrowserSessionResponse",
            "RotateBrowserSessionResponse",
        ),
        (
            "parseSelectBrowserSessionProfileResponse",
            "SelectBrowserSessionProfileResponse",
        ),
        ("parseAccessProjectionResponse", "AccessProjectionResponse"),
    ] {
        writeln!(
            output,
            "// prettier-ignore\nexport function {alias}(value: unknown): {dto} {{\n  return parseProductionDto(\"{dto}\", value);\n}}\n"
        )?;
    }
    Ok(output)
}

fn typescript_sdk(
    public_registry: &Value,
    problem_catalog: &Value,
    health_schema: &Value,
    problem_schema: &Value,
    asyncapi: &Value,
    production_openapi: &Value,
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

    output.push_str(&render_production_bootstrap_contract(production_openapi)?);
    output.push_str(&render_production_runtime_contract(production_openapi)?);
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
const RFC3339_INSTANT = /^(\d{{4}})-(\d{{2}})-(\d{{2}})T(\d{{2}}):(\d{{2}}):(\d{{2}})(?:\.\d{{1,9}})?(Z|[+-](\d{{2}}):(\d{{2}}))$/;

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
  validateOpenApiValue(value, schema, schemaName, B1_CONFORMANCE_SCHEMAS as Record<string, unknown>);
  return value as T;
}

// prettier-ignore
function validateOpenApiValue(value: unknown, schemaValue: unknown, path: string, schemas: Record<string, unknown>): void {
  const schema = schemaValue as Record<string, unknown>;
  if (Object.keys(schema).length === 0) return;
  if (typeof schema.$ref === "string") {
    const prefix = "#/components/schemas/";
    if (!schema.$ref.startsWith(prefix)) {
      throw new FastiContractParseError(`${path} has an unsupported schema reference`);
    }
    const name = schema.$ref.slice(prefix.length);
    const target = schemas[name];
    if (target === undefined) {
      throw new FastiContractParseError(`${path} references an unknown schema`);
    }
    validateOpenApiValue(value, target, path, schemas);
    return;
  }
  if (Array.isArray(schema.oneOf)) {
    let matches = 0;
    for (const candidate of schema.oneOf) {
      try {
        validateOpenApiValue(value, candidate, path, schemas);
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
  if (schemaTypes.includes("number")) {
    if (typeof value !== "number" || !Number.isFinite(value)) {
      throw new FastiContractParseError(`${path} must be a finite number`);
    }
    if (typeof schema.minimum === "number" && value < schema.minimum) {
      throw new FastiContractParseError(`${path} is below its minimum`);
    }
    if (typeof schema.maximum === "number" && value > schema.maximum) {
      throw new FastiContractParseError(`${path} exceeds its maximum`);
    }
    return;
  }
  if (schemaTypes.includes("boolean")) {
    if (typeof value !== "boolean") {
      throw new FastiContractParseError(`${path} must be a boolean`);
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
    value.forEach((item, index) => validateOpenApiValue(item, schema.items, `${path}[${index}]`, schemas));
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
        validateOpenApiValue(key, schema.propertyNames, `${path} property name`, schemas);
      }
      if (!Object.hasOwn(properties, key)) {
        if (schema.additionalProperties === false) {
          throw new FastiContractParseError(`${path} contains unknown field ${key}`);
        }
        if (isPlainObject(schema.additionalProperties)) {
          validateOpenApiValue(object[key], schema.additionalProperties, `${path}.${key}`, schemas);
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
        validateOpenApiValue(object[field], fieldSchema, `${path}.${field}`, schemas);
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
    if schema.get("oneOf").is_some() {
        ensure!(
            overrides.is_empty(),
            "{name} union cannot use field overrides"
        );
        return Ok(format!(
            "// prettier-ignore\nexport type {name} = {};\n",
            typescript_type(schema)?
        ));
    }
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
                if schema.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
                    let properties = object_at(schema, "/properties")?;
                    let required = required_names(schema)?;
                    let mut fields = Vec::with_capacity(properties.len());
                    for (name, property_schema) in properties {
                        let optional = if required.contains(name) { "" } else { "?" };
                        fields.push(format!(
                            "readonly {name}{optional}: {}",
                            typescript_type(property_schema)?
                        ));
                    }
                    return Ok(format!("{{ {} }}", fields.join("; ")));
                }
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
        collect_generated_files(
            output_root,
            &relative_directory,
            &output_root.join(&relative_directory),
            &mut actual,
        )?;
    }
    let generated_only_directories: BTreeSet<_> = GENERATED_ONLY_DIRECTORIES
        .into_iter()
        .map(PathBuf::from)
        .collect();
    let expected_paths: BTreeSet<_> = expected
        .keys()
        .filter(|path| {
            generated_only_directories
                .iter()
                .any(|directory| path.starts_with(directory))
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

fn collect_generated_files(
    output_root: &Path,
    generated_root: &Path,
    directory: &Path,
    files: &mut BTreeSet<PathBuf>,
) -> anyhow::Result<()> {
    let metadata = fs::symlink_metadata(directory)
        .with_context(|| format!("failed to inspect {}", directory.display()))?;
    ensure!(
        !metadata.file_type().is_symlink() && metadata.is_dir(),
        "generated artifact directory {} must be a real directory",
        directory.display()
    );
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to inspect {}", directory.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        ensure!(
            !file_type.is_symlink(),
            "generated artifact directory {} contains symlink {}",
            generated_root.display(),
            entry.path().display()
        );
        if file_type.is_dir() {
            collect_generated_files(output_root, generated_root, &entry.path(), files)?;
        } else {
            ensure!(
                file_type.is_file(),
                "generated artifact directory {} contains non-file {}",
                generated_root.display(),
                entry.path().display()
            );
            let path = entry.path();
            let relative = path.strip_prefix(output_root).with_context(|| {
                format!(
                    "generated artifact {} escaped {}",
                    path.display(),
                    output_root.display()
                )
            })?;
            files.insert(relative.to_path_buf());
        }
    }
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
            Path::new(PROVIDER_MANIFEST_SCHEMA_PATH),
            Path::new(PORTABILITY_V2_SCHEMA_PATH),
            Path::new(PORTABILITY_V2_EXAMPLE_PATH),
            Path::new(PORTABILITY_V3_SCHEMA_PATH),
            Path::new(PORTABILITY_V3_EXAMPLE_PATH),
            Path::new(PORTABILITY_V4_SCHEMA_PATH),
            Path::new(PORTABILITY_V4_EXAMPLE_PATH),
            Path::new(PORTABILITY_V5_SCHEMA_PATH),
            Path::new(PORTABILITY_V5_EXAMPLE_PATH),
            Path::new(PORTABILITY_V6_SCHEMA_PATH),
            Path::new(PORTABILITY_V6_EXAMPLE_PATH),
            Path::new(PORTABILITY_V7_SCHEMA_PATH),
            Path::new(PORTABILITY_V7_EXAMPLE_PATH),
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
    fn archive_v2_entity_enum_is_the_frozen_v2_prefix() {
        let schema = portability_v2_schema().expect("archive-v2 schema");
        let actual = schema
            .pointer("/$defs/WorkspaceExportEntityDto/enum")
            .and_then(Value::as_array)
            .expect("archive-v2 entity enum");
        let expected = WorkspaceExportEntity::V2
            .iter()
            .map(|entity| Value::String(entity.as_str().to_owned()))
            .collect::<Vec<_>>();
        assert_eq!(actual, &expected);
    }

    #[test]
    fn archive_v3_v4_and_v5_entity_enums_preserve_their_frozen_prefixes() {
        for (schema, entities) in [
            (
                portability_v3_schema().expect("archive-v3 schema"),
                WorkspaceExportEntity::V3.as_slice(),
            ),
            (
                portability_v4_schema().expect("archive-v4 schema"),
                WorkspaceExportEntity::V4.as_slice(),
            ),
            (
                portability_v5_schema().expect("archive-v5 schema"),
                WorkspaceExportEntity::V5.as_slice(),
            ),
            (
                portability_v6_schema().expect("archive-v6 schema"),
                WorkspaceExportEntity::V6.as_slice(),
            ),
            (
                portability_v7_schema().expect("archive-v7 schema"),
                WorkspaceExportEntity::ALL.as_slice(),
            ),
        ] {
            let actual = schema
                .pointer("/$defs/WorkspaceExportEntityDto/enum")
                .and_then(Value::as_array)
                .expect("archive entity enum");
            let expected = entities
                .iter()
                .map(|entity| Value::String(entity.as_str().to_owned()))
                .collect::<Vec<_>>();
            assert_eq!(actual, &expected);
        }
    }

    #[cfg(unix)]
    #[test]
    fn generated_inventory_rejects_a_symlinked_root() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("temporary root");
        let real = root.path().join("real");
        fs::create_dir(&real).expect("real directory");
        let linked = root.path().join("linked");
        symlink(&real, &linked).expect("symlink root");

        let error = collect_generated_files(root.path(), &linked, &linked, &mut BTreeSet::new())
            .expect_err("symlinked generated root must fail");
        assert!(error.to_string().contains("must be a real directory"));
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
    fn access_contract_graph_rejects_secret_properties() {
        let artifacts = build(workspace_root()).expect("contract generation succeeds");
        let mut openapi: Value = serde_json::from_slice(
            artifacts
                .get(Path::new(OPENAPI_PATH))
                .expect("production OpenAPI generated"),
        )
        .expect("production OpenAPI JSON");
        validate_access_contract_secrets(&openapi).expect("current Access graph is public-safe");
        openapi
            .pointer_mut("/components/schemas/AccessProjectionResponse/properties")
            .and_then(Value::as_object_mut)
            .expect("Access projection properties")
            .insert(
                "session_secret".to_owned(),
                serde_json::json!({"type": "string"}),
            );
        let error = validate_access_contract_secrets(&openapi)
            .expect_err("secret property must fail the Access contract gate");
        assert!(error.to_string().contains("session_secret"));
    }

    #[test]
    fn trailbase_continuation_contract_rejects_internal_identifiers() {
        let artifacts = build(workspace_root()).expect("contract generation succeeds");
        let mut openapi: Value = serde_json::from_slice(
            artifacts
                .get(Path::new(OPENAPI_PATH))
                .expect("production OpenAPI generated"),
        )
        .expect("production OpenAPI JSON");
        validate_trailbase_continuation_contract(&openapi)
            .expect("current continuation contract is identifier-free");
        openapi
            .pointer_mut("/components/schemas/TrailBaseContinuationChoiceDto/properties")
            .and_then(Value::as_object_mut)
            .expect("continuation choice properties")
            .insert(
                "workspace_id".to_owned(),
                serde_json::json!({"type": "string"}),
            );
        let error = validate_trailbase_continuation_contract(&openapi)
            .expect_err("internal identifier must fail the continuation contract gate");
        assert!(error.to_string().contains("unexpected properties"));
    }

    #[test]
    fn access_callback_is_documented_but_excluded_from_the_sdk() {
        let artifacts = build(workspace_root()).expect("contract generation succeeds");
        let sdk = std::str::from_utf8(
            artifacts
                .get(Path::new(SDK_GENERATED_PATH))
                .expect("SDK generated"),
        )
        .expect("SDK is UTF-8");
        assert!(sdk.contains("startTrailBaseSignIn"));
        assert!(sdk.contains("readTrailBaseContinuation"));
        assert!(sdk.contains("completeTrailBaseContinuation"));
        assert!(sdk.contains("cancelTrailBaseContinuation"));
        assert!(!sdk.contains("complete_trailbase_authentication"));
        assert!(!sdk.contains("completeTrailBaseAuthentication"));
    }

    #[test]
    fn local_operator_restore_metadata_does_not_activate_a_transport() {
        let artifacts = build(workspace_root()).expect("contract generation succeeds");
        let registry: Value = serde_json::from_slice(
            artifacts
                .get(Path::new(CAPABILITY_REGISTRY_PATH))
                .expect("public registry generated"),
        )
        .expect("registry JSON");
        let restore = array_at(&registry, "/capabilities")
            .expect("capabilities")
            .iter()
            .find(|capability| {
                string_at(capability, "/id").expect("capability ID")
                    == "portability.workspace.restore"
            })
            .expect("restore capability");
        assert_eq!(
            string_at(restore, "/authorization").expect("restore authorization"),
            "local_operator"
        );
        assert!(array_at(restore, "/scopes")
            .expect("restore scopes")
            .is_empty());
        assert_eq!(
            string_at(restore, "/lifecycle/contract_state").expect("restore contract state"),
            "reserved"
        );
        assert_eq!(
            string_at(restore, "/lifecycle/runtime_availability").expect("restore availability"),
            "guarded"
        );

        let openapi: Value = serde_json::from_slice(
            artifacts
                .get(Path::new(CONFORMANCE_OPENAPI_PATH))
                .expect("conformance OpenAPI generated"),
        )
        .expect("conformance OpenAPI JSON");
        for path_item in object_at(&openapi, "/paths")
            .expect("OpenAPI paths")
            .values()
        {
            for operation in path_item.as_object().expect("OpenAPI path item").values() {
                assert_ne!(
                    operation
                        .get("x-fasti-capability-id")
                        .and_then(Value::as_str),
                    Some("portability.workspace.restore")
                );
            }
        }

        let sdk = std::str::from_utf8(
            artifacts
                .get(Path::new(SDK_GENERATED_PATH))
                .expect("SDK generated"),
        )
        .expect("SDK is UTF-8");
        let operations = sdk
            .split_once("export const B1_CONFORMANCE_OPERATIONS = {")
            .expect("SDK operations start")
            .1
            .split_once("} as const;")
            .expect("SDK operations end")
            .0;
        assert!(!operations.contains("portability.workspace.restore"));
        assert!(!sdk.contains("restoreWorkspace("));
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
    fn production_hybrid_authorization_keeps_webhooks_credential_only() {
        for operation in PRODUCTION_RUNTIME_OPERATIONS {
            let authorization = production_operation_authorization(
                operation,
                if operation.capability_id == "observation.accept" {
                    "scoped_or_browser_session"
                } else {
                    "scoped"
                },
            )
            .expect("operation authorization is governed");
            if CREDENTIAL_ONLY_HYBRID_OPERATIONS.contains(&operation.operation_id) {
                assert_eq!(authorization, "scoped");
            } else if operation.operation_id == "submit_observation" {
                assert_eq!(authorization, "scoped_or_browser_session");
            }
        }
    }

    #[test]
    fn generated_openapi_keeps_browser_auth_on_governed_hybrid_routes() {
        let artifacts = build(workspace_root()).expect("contract generation succeeds");
        let openapi: Value = serde_json::from_slice(
            artifacts
                .get(Path::new(OPENAPI_PATH))
                .expect("production OpenAPI generated"),
        )
        .expect("production OpenAPI JSON");
        for pointer in [
            "/paths/~1api~1v1~1records/get/security",
            "/paths/~1api~1v1~1profile~1record-tracking-dispositions/get/security",
            "/paths/~1api~1v1~1profile~1nuvio-collections/get/security",
            "/paths/~1api~1v1~1search~1records/post/security",
            "/paths/~1api~1v1~1search~1candidates~1{provider_id}~1{grain}~1{candidate_receipt_id}/get/security",
        ] {
            assert_eq!(
                value_at(&openapi, pointer).expect("hybrid operation security"),
                &serde_json::json!([
                    {"credential_bearer": []},
                    {"browser_session_cookie": []}
                ]),
                "{pointer}"
            );
        }
        for pointer in [
            "/paths/~1api~1v1~1observations/post/security",
            "/paths/~1api~1v1~1records/post/security",
            "/paths/~1api~1v1~1records~1identifiers/post/security",
            "/paths/~1api~1v1~1namespaces/post/security",
            "/paths/~1api~1v1~1profile~1record-tracking-dispositions~1{record_id}/put/security",
            "/paths/~1api~1v1~1profile~1nuvio-collections/put/security",
            "/paths/~1api~1v1~1profile~1nuvio-collections/delete/security",
            "/paths/~1api~1v1~1search~1providers~1{provider_id}/post/security",
            "/paths/~1api~1v1~1search~1candidates~1{provider_id}~1{grain}~1{candidate_receipt_id}~1actions/post/security",
        ] {
            assert_eq!(
                value_at(&openapi, pointer).expect("hybrid mutation security"),
                &serde_json::json!([
                    {"credential_bearer": []},
                    {
                        "browser_session_cookie": [],
                        "csrf_cookie": [],
                        "csrf_header": []
                    }
                ]),
                "{pointer}"
            );
        }
        assert_eq!(
            value_at(
                &openapi,
                "/paths/~1api~1v1~1integrations~1nuvio~1webhook/post/security"
            )
            .expect("webhook security"),
            &serde_json::json!([{"credential_bearer": []}])
        );
        let webhook = value_at(
            &openapi,
            "/paths/~1api~1v1~1integrations~1nuvio~1webhook/post",
        )
        .expect("webhook operation");
        for problem in BROWSER_SESSION_PROBLEMS {
            assert!(!array_at(webhook, "/x-fasti-problem-codes")
                .expect("webhook problems")
                .contains(&Value::String(problem.to_owned())));
        }
        assert_eq!(
            array_at(
                &openapi,
                "/paths/~1api~1v1~1search~1candidates~1{provider_id}~1{grain}~1{candidate_receipt_id}~1actions/post/x-fasti-required-scopes"
            )
            .expect("candidate action scopes"),
            &[serde_json::json!("identity_write")]
        );
        assert_eq!(
            array_at(
                &openapi,
                "/paths/~1api~1v1~1search~1candidates~1{provider_id}~1{grain}~1{candidate_receipt_id}~1actions/post/x-fasti-conditional-required-scopes/new_operation"
            )
            .expect("new candidate action scopes"),
            &[serde_json::json!("metadata_search")]
        );
        let sdk = std::str::from_utf8(
            artifacts
                .get(Path::new(SDK_GENERATED_PATH))
                .expect("production SDK generated"),
        )
        .expect("production SDK is UTF-8");
        assert!(sdk.contains(
            "requiredScopes: [\"identity_write\"], conditionalRequiredScopes: {\"new_operation\":[\"metadata_search\"]}"
        ));

        let conformance: Value = serde_json::from_slice(
            artifacts
                .get(Path::new(CONFORMANCE_OPENAPI_PATH))
                .expect("conformance OpenAPI generated"),
        )
        .expect("conformance OpenAPI JSON");
        let acceptance = value_at(&conformance, "/paths/~1api~1v1~1observations/post")
            .expect("conformance observation acceptance");
        assert_eq!(
            string_at(acceptance, "/x-fasti-authorization").expect("authorization"),
            "scoped"
        );
        for problem in BROWSER_SESSION_PROBLEMS {
            assert!(!array_at(acceptance, "/x-fasti-problem-codes")
                .expect("conformance problems")
                .contains(&Value::String(problem.to_owned())));
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
