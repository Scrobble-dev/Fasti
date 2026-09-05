//! Fasti HTTP REST API definitions and router construction.

use axum::{
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use fasti_application::{
    BrowserRequestBoundaryPolicy, LocalKernel, ProblemCode, SecretMaterial,
    C1_AUTH_CEREMONY_LIFETIME,
};
use fasti_contracts::{HealthResponse, ProblemActionDto, ProblemDetails, ViolationDto};
use fasti_domain::{AuthCeremonySelection, RequestCorrelationId, TrailBaseActivationState};
use std::collections::BTreeMap;
use std::io;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};
use utoipa::{
    openapi::{
        schema::{AdditionalProperties, Schema},
        security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme},
        RefOr,
    },
    Modify, OpenApi,
};

pub const FASTI_ACCESS_ORIGIN: &str = "http://127.0.0.1:8420";
pub const FASTI_ACCESS_HOST: &str = "127.0.0.1:8420";
pub const FASTI_ACCESS_CALLBACK_PATH: &str = "/api/access/v1/trailbase/callback";
pub const FASTI_ACCESS_CALLBACK_URL: &str =
    "http://127.0.0.1:8420/api/access/v1/trailbase/callback";
pub const FASTI_ACCESS_BINDING_COOKIE: &str = "__Secure-fasti_auth_binding";
pub const FASTI_ACCESS_CONTINUATION_PATH: &str = "/api/access/v1/trailbase/continuation";
pub const FASTI_ACCESS_CONTINUATION_COOKIE: &str = "__Secure-fasti_auth_continuation";

mod access;
mod identity_routing;
mod integrations;
mod local;
mod metadata;
mod nuvio_collections;
mod observation;
mod problem;
mod profile_state;
mod providers;
mod records;
mod search;
mod trailbase;

/// Provider-scoped gates shared by credential mutation, provider checks, and
/// metadata refreshes in one API process.
#[derive(Clone)]
pub struct ProviderOperationLocks {
    locks: Arc<BTreeMap<&'static str, Arc<tokio::sync::Mutex<()>>>>,
}

impl ProviderOperationLocks {
    pub fn new(runtime: &fasti_provider_runtime::ProviderRuntime) -> Self {
        Self {
            locks: Arc::new(
                runtime
                    .descriptors()
                    .iter()
                    .map(|provider| (provider.provider, Arc::new(tokio::sync::Mutex::new(()))))
                    .collect(),
            ),
        }
    }

    pub(crate) fn get(&self, provider_id: &str) -> Option<Arc<tokio::sync::Mutex<()>>> {
        self.locks.get(provider_id).cloned()
    }
}

#[cfg(feature = "conformance-fixture")]
mod conformance;

#[cfg(feature = "conformance-fixture")]
pub use conformance::{b1_conformance_openapi, b1_conformance_router};

#[utoipa::path(
    get,
    path = "/api/v1/health",
    tag = "system",
    responses(
        (status = 200, description = "The Fasti service is healthy", body = HealthResponse)
    )
)]
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    })
}

struct ProductionSecurityAddon;

impl Modify for ProductionSecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bootstrap_bearer",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("64-character lowercase hexadecimal secret")
                        .description(Some(
                            "One-time local data-root bootstrap secret. Never use an enrolled client credential here.",
                        ))
                        .build(),
                ),
            );
            components.add_security_scheme(
                "credential_bearer",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("64-character lowercase hexadecimal credential")
                        .description(Some(
                            "Enrolled Fasti client credential sent only in the Authorization header.",
                        ))
                        .build(),
                ),
            );
            components.add_security_scheme(
                "browser_session_cookie",
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                    "__Host-fasti_session",
                    "Opaque Fasti browser session. The browser supplies this Secure, HttpOnly, SameSite=Strict cookie.",
                ))),
            );
            components.add_security_scheme(
                "csrf_cookie",
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                    "__Host-fasti_csrf",
                    "First-party CSRF value copied by the browser SDK into X-CSRF-Token.",
                ))),
            );
            components.add_security_scheme(
                "csrf_header",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                    "X-CSRF-Token",
                    "Exact value of the __Host-fasti_csrf cookie for browser mutations.",
                ))),
            );
            components.add_security_scheme(
                "auth_binding_cookie",
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                    "__Secure-fasti_auth_binding",
                    "One-use, callback-path-scoped browser binding for a TrailBase ceremony.",
                ))),
            );
            components.add_security_scheme(
                "auth_continuation_cookie",
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                    FASTI_ACCESS_CONTINUATION_COOKIE,
                    "One-use, continuation-path-scoped binding for explicit Fasti sign-in selection.",
                ))),
            );
            let Some(RefOr::T(Schema::OneOf(override_schema))) =
                components.schemas.get_mut("MetadataOverrideMutationDto")
            else {
                panic!("metadata override OpenAPI schema must remain a tagged union");
            };
            for variant in &mut override_schema.items {
                let RefOr::T(Schema::Object(object)) = variant else {
                    panic!("metadata override variants must remain object schemas");
                };
                object.additional_properties =
                    Some(Box::new(AdditionalProperties::FreeForm(false)));
            }
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        health_check,
        access::start_trailbase_sign_in,
        access::complete_trailbase_authentication,
        access::read_trailbase_continuation,
        access::complete_trailbase_continuation,
        access::cancel_trailbase_continuation,
        access::read_access_projection,
        access::read_browser_session,
        access::end_browser_session,
        access::list_browser_sessions,
        access::revoke_browser_session,
        access::revoke_other_browser_sessions,
        access::revoke_all_browser_sessions,
        access::rotate_browser_session,
        access::select_browser_session_profile,
        local::initialize_node,
        local::enroll_first_client,
        nuvio_collections::clear_nuvio_collections,
        nuvio_collections::get_nuvio_collections,
        nuvio_collections::replace_nuvio_collections,
        observation::submit_observation,
        profile_state::list_tracking_dispositions,
        profile_state::set_tracking_disposition,
        providers::list_providers,
        providers::configure_provider_credential,
        providers::remove_provider_credential,
        providers::test_provider_credential,
        providers::read_provider_health,
        records::create_record,
        records::attach_identifier,
        records::list_records,
        search::search_provider_page,
        records::register_namespace,
        integrations::integration_status,
        integrations::nuvio_webhook,
        integrations::tautulli_webhook,
        integrations::jellyfin_webhook,
        integrations::emby_webhook,
        integrations::plex_webhook,
        metadata::refresh_metadata_claims,
        metadata::read_metadata_projection,
        metadata::configure_metadata_projection
        ,identity_routing::resolve_identity_route
        ,identity_routing::read_anime_grouping_policy
        ,identity_routing::preview_anime_grouping_policy_change
        ,identity_routing::apply_anime_grouping_policy_change
    ),
    components(schemas(
        HealthResponse,
        fasti_contracts::SearchProviderPageRequest,
        fasti_contracts::SearchProviderPageResponse,
        fasti_contracts::SearchCandidateReceiptDto,
        fasti_contracts::SearchCandidateDto,
        fasti_contracts::SearchReceiptLifetimeDto,
        fasti_contracts::SearchCacheStateDto,
        fasti_contracts::StartTrailBaseSignInRequest,
        fasti_contracts::StartTrailBaseSignInResponse,
        fasti_contracts::TrailBaseContinuationChoiceDto,
        fasti_contracts::ReadTrailBaseContinuationResponse,
        fasti_contracts::CompleteTrailBaseContinuationRequest,
        fasti_contracts::SelectBrowserSessionProfileRequest,
        fasti_contracts::BrowserSessionDto,
        fasti_contracts::ReadBrowserSessionResponse,
        fasti_contracts::ListBrowserSessionsResponse,
        fasti_contracts::RevokeBrowserSessionsResponse,
        fasti_contracts::RotateBrowserSessionResponse,
        fasti_contracts::SelectBrowserSessionProfileResponse,
        fasti_contracts::AccessEvidenceStateDto,
        fasti_contracts::AccessSubjectLifecycleDto,
        fasti_contracts::AccessMembershipLifecycleDto,
        fasti_contracts::AccessWorkspaceRoleDto,
        fasti_contracts::TrailBaseActivationStateDto,
        fasti_contracts::TrailBaseActivationBlockerDto,
        fasti_contracts::AccessAuthenticationMethodDto,
        fasti_contracts::AccessEvidenceKindDto,
        fasti_contracts::AccessCeremonyStateDto,
        fasti_contracts::AccessCeremonyFailureDto,
        fasti_contracts::AccessFirstRunStepKeyDto,
        fasti_contracts::AccessSubjectDto,
        fasti_contracts::AccessMembershipDto,
        fasti_contracts::AccessProfileGrantDto,
        fasti_contracts::BrowserSessionPolicyDto,
        fasti_contracts::RecentAuthenticationDto,
        fasti_contracts::AccessSessionAuthenticationDto,
        fasti_contracts::TrailBaseActivationDto,
        fasti_contracts::AccessFirstRunStepDto,
        fasti_contracts::AccessEvidenceDto,
        fasti_contracts::AccessProjectionResponse,
        fasti_contracts::AttachIdentifierRequest,
        fasti_contracts::AttachIdentifierResponse,
        fasti_contracts::ClientEnrollmentResponse,
        fasti_contracts::CreateRecordRequest,
        fasti_contracts::CreateRecordResponse,
        fasti_contracts::CredentialSchemeDto,
        fasti_contracts::EnrollFirstClientRequest,
        fasti_contracts::InitializeNodeRequest,
        fasti_contracts::IntegrationObservationRequest,
        fasti_contracts::IntegrationStatusDto,
        fasti_contracts::IntegrationStatusListResponse,
        fasti_contracts::ListRecordsResponse,
        fasti_contracts::ListTrackingDispositionsResponse,
        fasti_contracts::RefreshMetadataClaimsRequest,
        fasti_contracts::RefreshMetadataClaimsResponse,
        fasti_contracts::MetadataClaimDto,
        fasti_contracts::MetadataClaimProvenanceDto,
        fasti_contracts::MetadataClaimStatusDto,
        fasti_contracts::RatingClaimDto,
        fasti_contracts::RatingScaleDto,
        fasti_contracts::MetadataProjectedFieldDto,
        fasti_contracts::MetadataProjectionTierDto,
        fasti_contracts::MetadataProjectionResponse,
        fasti_contracts::MetadataProjectionQueryParameters,
        fasti_contracts::ConfigureMetadataProjectionRequest,
        fasti_contracts::MetadataProjectionConfigurationResponse,
        fasti_contracts::MetadataOverrideMutationDto,
        fasti_contracts::EnrichmentPolicyDto,
        fasti_contracts::LastKnownGoodPolicyDto,
        fasti_contracts::MetadataFieldGroupDto,
        fasti_contracts::MetadataRefreshModeDto,
        fasti_contracts::MetadataCacheEntryDto,
        fasti_contracts::MetadataCacheKeyDto,
        fasti_contracts::MetadataCachePurposeDto,
        fasti_contracts::MetadataCacheReadStateDto,
        fasti_contracts::MetadataCacheInvalidationDto,
        fasti_contracts::MetadataCacheInvalidationReasonDto,
        fasti_contracts::MetadataDataClassificationDto,
        fasti_contracts::MetadataAttributionDto,
        fasti_contracts::ResolutionIntentDto,
        fasti_contracts::IdentityRouteStatusDto,
        fasti_contracts::IdentityRouteKindDto,
        fasti_contracts::IdentityAssertionRelationDto,
        fasti_contracts::IdentityIdentifierDto,
        fasti_contracts::AcceptedIdentityRouteAssertionDto,
        fasti_contracts::IdentityRouteDto,
        fasti_contracts::ResolveIdentityRouteResponse,
        fasti_contracts::AnimeGroupingPreferenceDto,
        fasti_contracts::AnimeGroupingPolicyScopeKindDto,
        fasti_contracts::AnimeGroupingPolicySourceDto,
        fasti_contracts::AnimeGroupingPolicyScopeDto,
        fasti_contracts::AnimeGroupingPolicyChangeDto,
        fasti_contracts::AnimeGroupingPolicyDto,
        fasti_contracts::ReadAnimeGroupingPolicyResponse,
        fasti_contracts::PreviewAnimeGroupingPolicyChangeRequest,
        fasti_contracts::AnimeGroupingRecordPreviewDto,
        fasti_contracts::AnimeGroupingPolicyImpactResponse,
        fasti_contracts::ApplyAnimeGroupingPolicyChangeRequest,
        fasti_contracts::ApplyAnimeGroupingPolicyChangeResponse,
        fasti_contracts::NodeInitializationResponse,
        fasti_contracts::NuvioCatalogSourceDto,
        fasti_contracts::NuvioCollectionDto,
        fasti_contracts::NuvioCollectionFolderDto,
        fasti_contracts::NuvioCollectionSourceDto,
        fasti_contracts::NuvioCollectionsDocumentDto,
        fasti_contracts::NuvioCollectionsStateDto,
        fasti_contracts::ObservationIdentifierInput,
        fasti_contracts::ObservationIngressKind,
        fasti_contracts::ConfigureProviderCredentialRequest,
        fasti_contracts::CredentialRequirementDto,
        fasti_contracts::ListProvidersResponse,
        fasti_contracts::ProviderCapabilityDto,
        fasti_contracts::ProviderCapabilityResponse,
        fasti_contracts::ProviderCapabilityStateDto,
        fasti_contracts::ProviderCheckDto,
        fasti_contracts::ProviderCheckStateDto,
        fasti_contracts::ProviderCredentialSourceDto,
        fasti_contracts::ProviderCredentialStateDto,
        fasti_contracts::ProviderDescriptorDto,
        fasti_contracts::ProviderHealthResponse,
        fasti_contracts::ProviderKindDto,
        fasti_contracts::RecordActivityDto,
        fasti_contracts::RecordIdentifierDto,
        fasti_contracts::RecordSummaryDto,
        fasti_contracts::ListRecordsQueryParameters,
        fasti_contracts::RegisterNamespaceRequest,
        fasti_contracts::RegisterNamespaceResponse,
        fasti_contracts::ResolvedFieldDto,
        fasti_contracts::SetTrackingDispositionRequest,
        fasti_contracts::SubmitObservationRequest,
        fasti_contracts::SubmitObservationResponse,
        fasti_contracts::TrackingDispositionDto,
        fasti_contracts::TrackingDispositionStateDto,
        fasti_contracts::TrackingDispositionUpdateDto,
        ProblemActionDto,
        ProblemDetails,
        ViolationDto
    )),
    modifiers(&ProductionSecurityAddon)
)]
struct ApiDoc;

/// Builds the OpenAPI 3.1 contract for routes actually mounted by [`api_router`]
/// and the dedicated integration listener.
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

/// Constructs the public health router used alone when no data root is configured
/// and as the common base for both durable routers.
pub fn health_router() -> Router {
    Router::new().route("/api/v1/health", get(health_check))
}

/// Constructs the dedicated integration ingress surface.
///
/// It intentionally exposes only health, integration status, and authenticated
/// provider adapters. Node bootstrap, generic record mutation, and the generic
/// observation endpoint are never mounted here.
pub fn integration_router(kernel: Arc<dyn LocalKernel>) -> Router {
    let state = local::LocalApiState {
        kernel,
        browser_boundary: None,
    };
    health_router().merge(integrations::router().with_state(state))
}

/// Constructs the authenticated provider-management surface.
///
/// This is separate from [`integration_router`] so credentials and provider
/// inventory are never exposed on the dedicated webhook listener.
pub fn provider_api_router(
    kernel: Arc<dyn LocalKernel>,
    provider_state: Arc<dyn fasti_application::ProviderStatePort>,
    runtime: Arc<fasti_provider_runtime::ProviderRuntime>,
    provider_operation_locks: ProviderOperationLocks,
) -> Router {
    providers::router().with_state(providers::ProviderApiState {
        kernel,
        provider_state,
        runtime,
        provider_operation_locks,
    })
}

/// Constructs the authenticated M2 metadata surface. Provider refresh I/O and
/// durable projection policy remain behind separate application ports so this
/// HTTP adapter owns no provider or persistence policy.
pub fn metadata_api_router(
    kernel: Arc<dyn LocalKernel>,
    refresh_service: Arc<dyn fasti_application::MetadataClaimRefreshService>,
    projection_port: Arc<dyn fasti_application::MetadataProjectionPort>,
    provider_operation_locks: ProviderOperationLocks,
) -> Router {
    metadata::router().with_state(metadata::MetadataApiState {
        kernel,
        refresh_service,
        projection_port,
        provider_operation_locks,
    })
}

/// Search shares the provider runtime and mutation gate; only the exact direct
/// listener supplies a browser boundary. Other listeners remain bearer-only.
pub fn search_api_router(
    kernel: Arc<dyn LocalKernel>,
    persistence: Arc<dyn fasti_application::SearchPersistencePort>,
    service: Arc<fasti_provider_runtime::ProviderSearchService>,
    locks: ProviderOperationLocks,
    browser_boundary: Option<BrowserRequestBoundaryPolicy>,
) -> Router {
    search::router().with_state(search::SearchApiState {
        kernel,
        persistence,
        service,
        locks,
        browser_boundary,
    })
}

/// Constructs the durable local API router for fastid.
///
/// # Contract
///
/// This function merges health and durable local routes and validates that:
/// - `local_exposure_addr` is the effective loopback address clients use
///   directly or through a trusted loopback-only port forward (panics if not)
/// - `data_root` is non-empty (panics if empty)
///
/// These validations enforce the local durable route security model: the router
/// must stay on direct loopback or an explicitly declared loopback-only port
/// forward and must have an explicit data root. Intentional non-loopback durable
/// listeners use [`remote_api_router`]; missing data roots use [`health_router`].
///
/// # Panics
///
/// Panics if `local_exposure_addr` is not a loopback address, if `data_root` is
/// empty, or if the bootstrap secret cannot be prepared (durable state is
/// unavailable at startup either way; failing fast here matches every other
/// durable precondition this function already enforces).
pub fn api_router(
    kernel: Arc<dyn LocalKernel>,
    local_exposure_addr: SocketAddr,
    data_root: &Path,
) -> Router {
    assert!(
        local_exposure_addr.ip().is_loopback(),
        "api_router requires loopback client exposure, got non-loopback {local_exposure_addr}"
    );
    durable_loopback_router(kernel, data_root, None)
}

/// Constructs the only C1 browser-enabled application router.
///
/// The caller must pass the actual bound address and fallback result. A
/// requested address is not proof that the fixed origin was obtained.
pub fn direct_loopback_api_router(
    kernel: Arc<dyn LocalKernel>,
    bound_addr: SocketAddr,
    used_fallback: bool,
    data_root: &Path,
    trailbase_root: Option<&Path>,
) -> io::Result<Router> {
    DirectLoopbackAccessRuntime::new(kernel, bound_addr, used_fallback, data_root, trailbase_root)
        .map(|runtime| runtime.router())
}

/// One fixed-origin Access runtime shared by its router and packaged host.
pub struct DirectLoopbackAccessRuntime {
    router: Router,
    browser_boundary: BrowserRequestBoundaryPolicy,
    trailbase: Option<Arc<trailbase::TrailBaseOrchestrator>>,
}

/// Trusted headless host for first-administrator setup while `fastid` is stopped.
pub struct LocalOperatorAccessRuntime {
    trailbase: Arc<trailbase::TrailBaseOrchestrator>,
    access: Arc<dyn LocalKernel>,
}

/// First-administrator ceremony material kept inside trusted Rust host code.
pub struct StartedFirstAdministratorBootstrap {
    operation_id: fasti_domain::OperationId,
    authorization_url: String,
    expires_at: chrono::DateTime<chrono::Utc>,
    browser_binding: SecretMaterial,
}

impl StartedFirstAdministratorBootstrap {
    pub fn authorization_url(&self) -> &str {
        &self.authorization_url
    }

    pub const fn expires_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.expires_at
    }

    pub const fn browser_binding(&self) -> &SecretMaterial {
        &self.browser_binding
    }
}

fn verified_trailbase_orchestrator(
    kernel: &Arc<dyn LocalKernel>,
    trailbase_root: &Path,
) -> io::Result<Option<Arc<trailbase::TrailBaseOrchestrator>>> {
    let evidence = trailbase::verify_installation_receipt(trailbase_root)?;
    let installation = fasti_application::HumanAccessPort::verify_trailbase_installation(
        kernel.as_ref(),
        fasti_application::VerifyTrailBaseInstallationCommand::new(
            evidence.instance_id,
            evidence.physical_root_identity,
            evidence.release_lock_identity,
            evidence.declared_restore,
            RequestCorrelationId::new_v7(),
            chrono::Utc::now(),
        ),
    )
    .map_err(|_| io::Error::other("TrailBase activation verification failed"))?;
    Ok(
        (installation.activation_state() == TrailBaseActivationState::Active).then(|| {
            Arc::new(
                trailbase::TrailBaseOrchestrator::production(Arc::clone(kernel), &installation)
                    .expect("active TrailBase installation must satisfy the fixed C1 client"),
            )
        }),
    )
}

fn start_first_administrator_bootstrap(
    trailbase: &trailbase::TrailBaseOrchestrator,
    selection: AuthCeremonySelection,
    bootstrap_secret: SecretMaterial,
) -> Result<StartedFirstAdministratorBootstrap, ProblemCode> {
    let created_at = chrono::Utc::now();
    let expires_at = created_at
        + chrono::Duration::from_std(C1_AUTH_CEREMONY_LIFETIME)
            .expect("C1 ceremony lifetime fits chrono");
    let started = trailbase
        .start_bootstrap(
            selection,
            bootstrap_secret,
            RequestCorrelationId::new_v7(),
            created_at,
            expires_at,
        )
        .map_err(|error| match error {
            trailbase::TrailBaseOrchestrationError::ApplicationProblem(code) => code,
            trailbase::TrailBaseOrchestrationError::InvalidInput => ProblemCode::IntegrityFailed,
            trailbase::TrailBaseOrchestrationError::LocalState => ProblemCode::StorageUnavailable,
            _ => ProblemCode::TrailBaseTrustUnavailable,
        })?;
    Ok(StartedFirstAdministratorBootstrap {
        operation_id: started.operation_id,
        authorization_url: started.authorization_url,
        expires_at: started.expires_at,
        browser_binding: started.browser_binding,
    })
}

fn cancel_first_administrator_bootstrap(
    trailbase: &trailbase::TrailBaseOrchestrator,
    started: StartedFirstAdministratorBootstrap,
) -> Result<(), ProblemCode> {
    trailbase
        .cancel(fasti_application::CancelAuthCeremonyCommand::new(
            started.operation_id,
            RequestCorrelationId::new_v7(),
            chrono::Utc::now(),
        ))
        .map_err(|error| match error {
            trailbase::TrailBaseOrchestrationError::ApplicationProblem(code) => code,
            trailbase::TrailBaseOrchestrationError::LocalState => ProblemCode::StorageUnavailable,
            _ => ProblemCode::TrailBaseTrustUnavailable,
        })
}

impl DirectLoopbackAccessRuntime {
    pub fn new(
        kernel: Arc<dyn LocalKernel>,
        bound_addr: SocketAddr,
        used_fallback: bool,
        data_root: &Path,
        trailbase_root: Option<&Path>,
    ) -> io::Result<Self> {
        assert_eq!(
            bound_addr,
            FASTI_ACCESS_HOST.parse().expect("fixed C1 address"),
            "browser authentication requires the exact direct 127.0.0.1:8420 listener"
        );
        assert!(
            !used_fallback,
            "browser authentication is unavailable on a fallback listener"
        );
        let boundary =
            BrowserRequestBoundaryPolicy::try_new(FASTI_ACCESS_ORIGIN, FASTI_ACCESS_HOST)
                .expect("fixed C1 browser boundary is valid");
        let trailbase = trailbase_root
            .map(|root| verified_trailbase_orchestrator(&kernel, root))
            .transpose()?
            .flatten();
        let browser_runtime = Some((boundary.clone(), trailbase.as_ref().map(Arc::clone)));
        let router = durable_loopback_router(Arc::clone(&kernel), data_root, browser_runtime);
        Ok(Self {
            router,
            trailbase,
            browser_boundary: boundary,
        })
    }

    pub fn router(&self) -> Router {
        self.router.clone()
    }

    pub fn browser_boundary(&self) -> BrowserRequestBoundaryPolicy {
        self.browser_boundary.clone()
    }

    #[cfg(test)]
    fn from_test_orchestrator(
        kernel: Arc<dyn LocalKernel>,
        data_root: &Path,
        trailbase: Arc<trailbase::TrailBaseOrchestrator>,
    ) -> Self {
        let boundary =
            BrowserRequestBoundaryPolicy::try_new(FASTI_ACCESS_ORIGIN, FASTI_ACCESS_HOST)
                .expect("fixed C1 browser boundary is valid");
        let router = durable_loopback_router(
            Arc::clone(&kernel),
            data_root,
            Some((boundary.clone(), Some(Arc::clone(&trailbase)))),
        );
        Self {
            router,
            browser_boundary: boundary,
            trailbase: Some(trailbase),
        }
    }

    pub fn start_first_administrator_bootstrap(
        &self,
        selection: AuthCeremonySelection,
        bootstrap_secret: SecretMaterial,
    ) -> Result<StartedFirstAdministratorBootstrap, ProblemCode> {
        let trailbase = self
            .trailbase
            .as_ref()
            .ok_or(ProblemCode::TrailBaseTrustUnavailable)?;
        start_first_administrator_bootstrap(trailbase, selection, bootstrap_secret)
    }

    pub fn cancel_first_administrator_bootstrap(
        &self,
        started: StartedFirstAdministratorBootstrap,
    ) -> Result<(), ProblemCode> {
        let trailbase = self
            .trailbase
            .as_ref()
            .ok_or(ProblemCode::TrailBaseTrustUnavailable)?;
        cancel_first_administrator_bootstrap(trailbase, started)
    }
}

impl LocalOperatorAccessRuntime {
    pub fn new(kernel: Arc<dyn LocalKernel>, trailbase_root: &Path) -> io::Result<Self> {
        let trailbase = verified_trailbase_orchestrator(&kernel, trailbase_root)?
            .ok_or_else(|| io::Error::other("TrailBase installation is not active"))?;
        Ok(Self {
            trailbase,
            access: kernel,
        })
    }

    #[cfg(test)]
    fn from_test_orchestrator(
        kernel: Arc<dyn LocalKernel>,
        trailbase: Arc<trailbase::TrailBaseOrchestrator>,
    ) -> Self {
        Self {
            trailbase,
            access: kernel,
        }
    }

    pub fn start_first_administrator_bootstrap(
        &self,
    ) -> Result<StartedFirstAdministratorBootstrap, ProblemCode> {
        let correlation_id = RequestCorrelationId::new_v7();
        let bootstrap_secret = self
            .access
            .ensure_bootstrap_secret()
            .map_err(|problem| problem.code())?;
        let selection = self
            .access
            .prepare_trailbase_bootstrap(fasti_application::PrepareTrailBaseBootstrapQuery::new(
                SecretMaterial::from_bytes(*bootstrap_secret.expose_bytes()),
                correlation_id,
            ))
            .map_err(|problem| problem.code())?;
        start_first_administrator_bootstrap(&self.trailbase, selection, bootstrap_secret)
    }

    pub async fn complete_first_administrator_bootstrap(
        &self,
        started: &StartedFirstAdministratorBootstrap,
        callback_url: &str,
    ) -> Result<(), ProblemCode> {
        let code = access::exact_callback_url_code(callback_url)
            .ok_or(ProblemCode::TrailBaseProofInvalid)?;
        self.trailbase
            .callback_for_operator(
                code,
                &started.browser_binding,
                RequestCorrelationId::new_v7(),
                chrono::Utc::now(),
            )
            .await
            .map_err(|error| match error {
                trailbase::TrailBaseOrchestrationError::ApplicationProblem(code) => code,
                trailbase::TrailBaseOrchestrationError::InvalidInput => {
                    ProblemCode::TrailBaseProofInvalid
                }
                trailbase::TrailBaseOrchestrationError::LocalState => {
                    ProblemCode::StorageUnavailable
                }
                trailbase::TrailBaseOrchestrationError::LogoutUncertain => {
                    ProblemCode::TrailBaseSessionCleanupFailed
                }
                _ => ProblemCode::IdentityServiceUnavailable,
            })
    }

    pub fn cancel_first_administrator_bootstrap(
        &self,
        started: StartedFirstAdministratorBootstrap,
    ) -> Result<(), ProblemCode> {
        cancel_first_administrator_bootstrap(&self.trailbase, started)
    }
}

fn durable_loopback_router(
    kernel: Arc<dyn LocalKernel>,
    data_root: &Path,
    browser_runtime: Option<(
        BrowserRequestBoundaryPolicy,
        Option<Arc<trailbase::TrailBaseOrchestrator>>,
    )>,
) -> Router {
    assert!(
        !data_root.as_os_str().is_empty(),
        "api_router requires non-empty data_root"
    );
    // Primed here, before the router serves anything, so a legitimate first
    // client can read <data_root>/bootstrap.secret and present it back to
    // /api/v1/node/initialization -- see
    // AccessAdministrationPort::ensure_bootstrap_secret.
    kernel
        .ensure_bootstrap_secret()
        .expect("bootstrap secret must be preparable before serving any route");
    let active_browser_boundary = browser_runtime
        .as_ref()
        .map(|(boundary, _)| boundary.clone());
    let access = browser_runtime.map_or_else(Router::new, |(boundary, trailbase)| {
        access::router(Arc::clone(&kernel), boundary, trailbase)
    });
    let integration_state = local::LocalApiState {
        kernel: Arc::clone(&kernel),
        browser_boundary: None,
    };
    health_router()
        .merge(local::router(kernel, true, active_browser_boundary))
        .merge(access)
        .merge(integrations::router().with_state(integration_state))
}

/// Constructs the authenticated durable router for a non-loopback listener.
/// The daemon enables this only behind an explicitly configured HTTPS proxy.
pub fn remote_api_router(
    kernel: Arc<dyn LocalKernel>,
    bind_addr: SocketAddr,
    data_root: &Path,
) -> Router {
    assert!(
        !bind_addr.ip().is_loopback(),
        "remote_api_router requires a non-loopback bind address"
    );
    assert!(
        !data_root.as_os_str().is_empty(),
        "remote_api_router requires non-empty data_root"
    );
    let integration_state = local::LocalApiState {
        kernel: Arc::clone(&kernel),
        browser_boundary: None,
    };
    health_router()
        .merge(local::router(kernel, false, None))
        .merge(integrations::router().with_state(integration_state))
}

/// Adds a static-file fallback to `router`, serving a pre-built single-page
/// app from `static_dir` for any request that doesn't match an `/api/*`
/// route. A missing file (including client-side routes like `/status`)
/// falls back to `static_dir/index.html`, so the SPA's own router handles
/// the path. When `static_dir` is `None`, `router` is returned unchanged --
/// existing callers that never pass a static dir see no behavior change.
///
/// This is applied once, after the durable/remote/health router is chosen,
/// rather than duplicated into each of those three constructors above.
pub fn with_static_fallback(router: Router, static_dir: Option<&Path>) -> Router {
    let Some(static_dir) = static_dir else {
        return router;
    };
    // `.fallback()`, not `.not_found_service()` -- the latter forces the
    // response status to 404, which is right for a custom error page but
    // wrong for SPA client-side routing: `/status` is a real page in the
    // app, so it must come back 200 with index.html for the SPA's own
    // router to take over. That SPA behavior must not extend to `/api/*`,
    // though: an unmatched API path (a typo, a removed endpoint) would
    // otherwise silently come back 200 with the HTML shell instead of a 404,
    // which every API client -- browser fetch, SDK, curl -- expects to see.
    let index_html = static_dir.join("index.html");
    let serve_dir = ServeDir::new(static_dir).fallback(ServeFile::new(index_html));
    router.fallback_service(tower::service_fn(move |request: Request| {
        let serve_dir = serve_dir.clone();
        async move {
            if request.uri().path().starts_with("/api/") {
                return Ok::<Response, std::convert::Infallible>(
                    StatusCode::NOT_FOUND.into_response(),
                );
            }
            let response = tower::ServiceExt::oneshot(serve_dir, request)
                .await
                .into_response();
            Ok(response)
        }
    }))
}

#[cfg(test)]
mod tests {
    include!("search_http_tests.rs");
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{header, Method, Request, StatusCode},
    };
    #[cfg(target_os = "linux")]
    use fasti_application::{
        AccessAdministrationPort, AuthenticateCredentialQuery, CapabilityKey,
        ClaimAuthCeremonyCommand, CompleteTrailBaseBootstrapCommand, ConfirmedTrailBaseIdentity,
        HumanAccessPort, PreauthorizeTrailBaseBootstrapCommand, SecretMaterial,
        StartTrailBaseBootstrapCommand, VerifyTrailBaseInstallationCommand,
    };
    #[cfg(target_os = "linux")]
    use fasti_domain::{
        AuthCallbackPath, AuthCeremony, AuthCeremonyProtocol, AuthCeremonyPurpose,
        AuthCeremonySelection, AuthenticationMethod, AuthenticationProvenance, OperationId,
        RequestCorrelationId, Sha256Digest, TrailBaseInstanceId, TrailBaseSubject,
    };
    #[cfg(target_os = "linux")]
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tower::ServiceExt;
    use utoipa::openapi::OpenApiVersion;

    #[cfg(target_os = "linux")]
    fn test_kernel() -> (tempfile::TempDir, Arc<fasti_store::SqliteKernel>) {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = Arc::new(fasti_store::SqliteKernel::open(root.path()).expect("SQLite kernel"));
        (root, kernel)
    }

    #[cfg(target_os = "linux")]
    fn test_bind_addr() -> SocketAddr {
        "127.0.0.1:8420".parse().expect("loopback address")
    }

    #[cfg(target_os = "linux")]
    fn test_trailbase_root() -> tempfile::TempDir {
        fn digest(bytes: &[u8]) -> String {
            trailbase::sha256_digest(bytes).to_string()
        }

        let root = tempfile::tempdir().expect("temporary TrailBase root");
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
            .expect("private root");
        let nonce = [7_u8; 32];
        let lock = root.path().join("runtime.lock");
        std::fs::write(&lock, nonce).expect("runtime nonce");
        std::fs::set_permissions(&lock, std::fs::Permissions::from_mode(0o600))
            .expect("private nonce");
        let metadata = std::fs::metadata(root.path()).expect("root metadata");
        let mut identity = Vec::with_capacity(48);
        identity.extend_from_slice(&metadata.dev().to_be_bytes());
        identity.extend_from_slice(&metadata.ino().to_be_bytes());
        identity.extend_from_slice(&nonce);
        let (runtime_target, artifact_identity) = if cfg!(target_arch = "aarch64") {
            (
                "linux-aarch64",
                "sha256:e8d86d361682e697d78fa159fb9c706f30ebdcc886a3015daa78d75eb9d7c199",
            )
        } else {
            (
                "linux-x86_64",
                "sha256:550c053355bdc68222c94fe84ecc0e23ef983cfb7232863a7c51ff9b84bce18e",
            )
        };
        let receipt = serde_json::json!({
            "schema_version": "fasti.trailbase-installation.v1",
            "instance_id": fasti_domain::TrailBaseInstanceId::new_v7(),
            "physical_root_identity": digest(&identity),
            "release_lock_identity": digest(include_bytes!("../../../third_party/trailbase/release.json")),
            "runtime": "native",
            "runtime_target": runtime_target,
            "artifact_identity": artifact_identity,
            "declared_restore": false,
            "created_at": "2026-08-30T00:00:00Z",
            "verified_at": "2026-08-30T00:00:01Z"
        });
        let receipt_path = root.path().join(".fasti-installation.json");
        std::fs::write(
            &receipt_path,
            serde_json::to_vec(&receipt).expect("receipt JSON"),
        )
        .expect("installation receipt");
        std::fs::set_permissions(&receipt_path, std::fs::Permissions::from_mode(0o600))
            .expect("private receipt");
        root
    }

    #[tokio::test]
    async fn static_fallback_is_a_no_op_when_no_dir_is_configured() {
        let router = with_static_fallback(health_router(), None);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/nonexistent")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        // No static dir configured -> unmatched routes still 404, exactly as
        // health_router() alone behaves. This is the regression guard: adding
        // FASTI_STATIC_DIR support must not change behavior when it's unset.
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn static_fallback_serves_index_html_for_unmatched_spa_routes() {
        let static_dir = tempfile::tempdir().expect("temporary static dir");
        std::fs::write(
            static_dir.path().join("index.html"),
            "<html>fasti workbench</html>",
        )
        .expect("write index.html");

        // The real API route still wins over the static fallback.
        let response = with_static_fallback(health_router(), Some(static_dir.path()))
            .oneshot(
                Request::builder()
                    .uri("/api/v1/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);

        // A client-side route (not a real file, not an API route) falls
        // back to index.html so the SPA's own router can handle it.
        let response = with_static_fallback(health_router(), Some(static_dir.path()))
            .oneshot(
                Request::builder()
                    .uri("/status")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        assert_eq!(body, "<html>fasti workbench</html>".as_bytes());
    }

    #[tokio::test]
    async fn static_fallback_serves_a_real_asset_file_directly() {
        let static_dir = tempfile::tempdir().expect("temporary static dir");
        std::fs::write(static_dir.path().join("index.html"), "shell").expect("write index.html");
        std::fs::write(static_dir.path().join("app.js"), "console.log(1)").expect("write app.js");

        let router = with_static_fallback(health_router(), Some(static_dir.path()));

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/app.js")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        assert_eq!(body, "console.log(1)".as_bytes());
    }

    #[tokio::test]
    async fn static_fallback_leaves_unmatched_api_paths_as_a_plain_404() {
        let static_dir = tempfile::tempdir().expect("temporary static dir");
        std::fs::write(
            static_dir.path().join("index.html"),
            "<html>fasti workbench</html>",
        )
        .expect("write index.html");

        // A path under /api/* that no route matches must not fall back to
        // the SPA shell with a 200 -- every API client expects a 404 there,
        // not an HTML document.
        let response = with_static_fallback(health_router(), Some(static_dir.path()))
            .oneshot(
                Request::get("/api/v1/not-a-real-route")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        assert!(
            !String::from_utf8_lossy(&body).contains("fasti workbench"),
            "an unmatched API path must not receive the SPA shell"
        );
    }

    #[tokio::test]
    async fn static_fallback_rejects_non_get_methods_with_not_found_not_method_not_allowed() {
        let static_dir = tempfile::tempdir().expect("temporary static dir");
        std::fs::write(static_dir.path().join("index.html"), "shell").expect("write index.html");

        let router = with_static_fallback(health_router(), Some(static_dir.path()));

        // A route this server never registers must stay a uniform 404 for
        // every method -- not 405, which would leak "a route matched this
        // path" and contradict SECURITY.md's absent-route guarantee.
        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/events")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn openapi_is_3_1_and_documents_the_real_routes() {
        let document = openapi();

        assert!(matches!(document.openapi, OpenApiVersion::Version31));
        for path in [
            "/api/v1/health",
            "/api/v1/node/initialization",
            "/api/v1/client-enrollments",
            "/api/v1/observations",
            "/api/v1/records",
            "/api/v1/records/identifiers",
            "/api/v1/namespaces",
            "/api/v1/integrations",
            "/api/v1/integrations/nuvio/webhook",
            "/api/v1/integrations/tautulli/webhook",
            "/api/v1/integrations/jellyfin/webhook",
            "/api/v1/integrations/emby/webhook",
            "/api/v1/integrations/plex/webhook",
            "/api/v1/profile/record-tracking-dispositions",
            "/api/v1/profile/record-tracking-dispositions/{record_id}",
            "/api/v1/profile/nuvio-collections",
            "/api/v1/providers",
            "/api/v1/providers/{provider_id}/credentials/{capability_id}",
            "/api/v1/providers/{provider_id}/credentials/{capability_id}/tests",
            "/api/v1/providers/{provider_id}/health",
            "/api/v1/metadata/claims/refresh",
            "/api/v1/records/{record_id}/metadata-projection",
            "/api/v1/profile/metadata-projection",
            "/api/v1/records/{record_id}/identity-route",
            "/api/v1/profile/anime-grouping-policy",
            "/api/v1/profile/anime-grouping-policy/preview",
            "/api/access/v1/trailbase/sign-in",
            "/api/access/v1/trailbase/callback",
            "/api/access/v1/trailbase/continuation",
            "/api/access/v1/projection",
            "/api/access/v1/browser-session",
            "/api/access/v1/browser-sessions",
            "/api/access/v1/browser-sessions/others",
            "/api/access/v1/browser-sessions/{browser_session_id}",
            "/api/access/v1/browser-session/rotation",
            "/api/access/v1/browser-session/profile",
        ] {
            assert!(document.paths.paths.contains_key(path), "missing {path}");
        }
        assert_eq!(document.paths.paths.len(), 37);

        let serialized = serde_json::to_string(&document).expect("serializable OpenAPI document");
        assert!(serialized.contains("#/components/schemas/HealthResponse"));
        let value = serde_json::to_value(&document).expect("OpenAPI JSON value");
        assert_eq!(
            value.pointer("/components/securitySchemes/credential_bearer/scheme"),
            Some(&serde_json::json!("bearer"))
        );
        assert_eq!(
            value.pointer("/components/securitySchemes/bootstrap_bearer/scheme"),
            Some(&serde_json::json!("bearer"))
        );
        assert_eq!(
            value.pointer("/components/securitySchemes/browser_session_cookie"),
            Some(&serde_json::json!({
                "type": "apiKey",
                "in": "cookie",
                "name": "__Host-fasti_session",
                "description": "Opaque Fasti browser session. The browser supplies this Secure, HttpOnly, SameSite=Strict cookie."
            }))
        );
        assert_eq!(
            value.pointer("/components/securitySchemes/csrf_cookie/name"),
            Some(&serde_json::json!("__Host-fasti_csrf"))
        );
        assert_eq!(
            value.pointer("/components/securitySchemes/csrf_header/name"),
            Some(&serde_json::json!("X-CSRF-Token"))
        );
        assert_eq!(
            value.pointer("/components/securitySchemes/auth_binding_cookie/name"),
            Some(&serde_json::json!("__Secure-fasti_auth_binding"))
        );
        assert_eq!(
            value.pointer("/components/securitySchemes/auth_continuation_cookie/name"),
            Some(&serde_json::json!("__Secure-fasti_auth_continuation"))
        );
        assert_eq!(
            value.pointer("/paths/~1api~1access~1v1~1projection/get/security"),
            Some(&serde_json::json!([{"browser_session_cookie": []}]))
        );
        assert_eq!(
            value.pointer("/paths/~1api~1access~1v1~1browser-session/delete/security"),
            Some(&serde_json::json!([{
                "browser_session_cookie": [],
                "csrf_cookie": [],
                "csrf_header": []
            }]))
        );
        assert_eq!(
            value.pointer("/paths/~1api~1access~1v1~1trailbase~1callback/get/security"),
            Some(&serde_json::json!([{"auth_binding_cookie": []}]))
        );
        for method in ["get", "post", "delete"] {
            assert_eq!(
                value.pointer(&format!(
                    "/paths/~1api~1access~1v1~1trailbase~1continuation/{method}/security"
                )),
                Some(&serde_json::json!([{"auth_continuation_cookie": []}]))
            );
        }
        assert_eq!(
            value
                .pointer("/paths/~1api~1v1~1records/get/security")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(2)
        );
        assert_eq!(
            value.pointer("/paths/~1api~1v1~1records/get/security"),
            Some(&serde_json::json!([
                {"credential_bearer": []},
                {"browser_session_cookie": []}
            ]))
        );
        assert!(value
            .pointer("/paths/~1api~1v1~1health/get/security")
            .is_none());
        assert!(value
            .pointer("/paths/~1api~1v1~1client-enrollments/post/security")
            .is_none());
        for (pointer, expected) in [
            ("/paths/~1api~1v1~1providers/get/operationId", "list_providers"),
            (
                "/paths/~1api~1v1~1providers~1{provider_id}~1credentials~1{capability_id}/put/operationId",
                "configure_provider_credential",
            ),
            (
                "/paths/~1api~1v1~1providers~1{provider_id}~1credentials~1{capability_id}/delete/operationId",
                "remove_provider_credential",
            ),
            (
                "/paths/~1api~1v1~1providers~1{provider_id}~1credentials~1{capability_id}~1tests/post/operationId",
                "test_provider_credential",
            ),
            (
                "/paths/~1api~1v1~1providers~1{provider_id}~1health/get/operationId",
                "read_provider_health",
            ),
        ] {
            assert_eq!(value.pointer(pointer), Some(&serde_json::json!(expected)));
        }

        let schemas = &document.components.expect("OpenAPI components").schemas;
        for schema in [
            "HealthResponse",
            "StartTrailBaseSignInRequest",
            "StartTrailBaseSignInResponse",
            "TrailBaseContinuationChoiceDto",
            "ReadTrailBaseContinuationResponse",
            "CompleteTrailBaseContinuationRequest",
            "BrowserSessionDto",
            "AccessProjectionResponse",
            "ListBrowserSessionsResponse",
            "RevokeBrowserSessionsResponse",
            "NodeInitializationResponse",
            "NuvioCatalogSourceDto",
            "NuvioCollectionDto",
            "NuvioCollectionFolderDto",
            "NuvioCollectionSourceDto",
            "NuvioCollectionsDocumentDto",
            "NuvioCollectionsStateDto",
            "ClientEnrollmentResponse",
            "ObservationIdentifierInput",
            "ObservationIngressKind",
            "ConfigureProviderCredentialRequest",
            "CredentialRequirementDto",
            "ListProvidersResponse",
            "ProviderCapabilityDto",
            "ProviderCapabilityResponse",
            "ProviderCapabilityStateDto",
            "ProviderCheckDto",
            "ProviderCheckStateDto",
            "ProviderCredentialSourceDto",
            "ProviderCredentialStateDto",
            "ProviderDescriptorDto",
            "ProviderHealthResponse",
            "ProviderKindDto",
            "SubmitObservationRequest",
            "SubmitObservationResponse",
            "IntegrationObservationRequest",
            "IntegrationStatusDto",
            "IntegrationStatusListResponse",
            "AttachIdentifierRequest",
            "AttachIdentifierResponse",
            "CreateRecordRequest",
            "CreateRecordResponse",
            "ListRecordsResponse",
            "ListTrackingDispositionsResponse",
            "RecordActivityDto",
            "RecordIdentifierDto",
            "RecordSummaryDto",
            "RegisterNamespaceRequest",
            "RegisterNamespaceResponse",
            "ResolvedFieldDto",
            "SetTrackingDispositionRequest",
            "TrackingDispositionDto",
            "TrackingDispositionStateDto",
            "TrackingDispositionUpdateDto",
            "ProblemActionDto",
            "ProblemDetails",
            "ViolationDto",
        ] {
            assert!(
                schemas.contains_key(schema),
                "missing shared schema {schema}"
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn documented_health_route_is_mounted() {
        let (root, kernel) = test_kernel();
        let response = api_router(kernel, test_bind_addr(), root.path())
            .oneshot(
                Request::get("/api/v1/health")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn nuvio_collections_replace_get_and_clear_use_the_authenticated_profile() {
        let (root, kernel) = test_kernel();
        let app = api_router(kernel, test_bind_addr(), root.path());
        let credential = enroll_admin(&app, root.path()).await.credential;
        let document = r#"[{"id":"collection","title":"Collection","folders":[{"id":"folder","title":"Folder","sources":[{"provider":"tmdb","tmdbSourceType":"discover","mediaType":"movie","filters":{"voteCountGte":10,"vote_count.gte":10},"id":"source"}]}]}]"#;

        let replaced = app
            .clone()
            .oneshot(
                Request::put("/api/v1/profile/nuvio-collections")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .body(Body::from(document))
                    .expect("replace request"),
            )
            .await
            .expect("replace response");
        assert_eq!(replaced.status(), StatusCode::OK);
        let replaced: fasti_contracts::NuvioCollectionsStateDto = serde_json::from_slice(
            &to_bytes(replaced.into_body(), 64 * 1024)
                .await
                .expect("bounded replace body"),
        )
        .expect("replace state");
        let replaced = serde_json::to_value(replaced.document.expect("stored document"))
            .expect("document JSON");
        assert_eq!(
            replaced[0]["folders"][0]["sources"][0]["mediaType"],
            "MOVIE"
        );
        assert_eq!(
            replaced[0]["folders"][0]["sources"][0]["filters"]["vote_count.gte"],
            10
        );

        let read = app
            .clone()
            .oneshot(
                Request::get("/api/v1/profile/nuvio-collections")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .body(Body::empty())
                    .expect("get request"),
            )
            .await
            .expect("get response");
        assert_eq!(read.status(), StatusCode::OK);
        let read: fasti_contracts::NuvioCollectionsStateDto = serde_json::from_slice(
            &to_bytes(read.into_body(), 64 * 1024)
                .await
                .expect("bounded get body"),
        )
        .expect("get state");
        assert!(read.document.is_some());

        let cleared = app
            .oneshot(
                Request::delete("/api/v1/profile/nuvio-collections")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .body(Body::empty())
                    .expect("clear request"),
            )
            .await
            .expect("clear response");
        assert_eq!(cleared.status(), StatusCode::OK);
        let cleared: fasti_contracts::NuvioCollectionsStateDto = serde_json::from_slice(
            &to_bytes(cleared.into_body(), 4096)
                .await
                .expect("bounded clear body"),
        )
        .expect("clear state");
        assert!(cleared.document.is_none());
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn identity_routing_http_routes_authorize_preview_apply_and_replay() {
        let (root, kernel) = test_kernel();
        let app = api_router(kernel, test_bind_addr(), root.path());
        let missing_record = fasti_domain::RecordId::new_v7();
        let unauthorized = app
            .clone()
            .oneshot(
                Request::get(format!(
                    "/api/v1/records/{missing_record}/identity-route?intent=metadata_enrichment&target_provider=tmdb"
                ))
                .body(Body::empty())
                .expect("identity route request"),
            )
            .await
            .expect("identity route response");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let credential = enroll_admin(&app, root.path()).await.credential;
        let auth = |builder: axum::http::request::Builder| {
            builder.header(header::AUTHORIZATION, format!("Bearer {credential}"))
        };
        let created = app
            .clone()
            .oneshot(
                auth(Request::post("/api/v1/records"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"grain":"release"}"#))
                    .expect("create record request"),
            )
            .await
            .expect("create record response");
        assert_eq!(created.status(), StatusCode::OK);
        let created: fasti_contracts::CreateRecordResponse = serde_json::from_slice(
            &to_bytes(created.into_body(), 4096)
                .await
                .expect("bounded record body"),
        )
        .expect("record response");

        let route = app
            .clone()
            .oneshot(
                auth(Request::get(format!(
                    "/api/v1/records/{}/identity-route?intent=metadata_enrichment&target_provider=tmdb",
                    created.record_id
                )))
                .body(Body::empty())
                .expect("route request"),
            )
            .await
            .expect("route response");
        assert_eq!(route.status(), StatusCode::OK);
        let route: fasti_contracts::ResolveIdentityRouteResponse = serde_json::from_slice(
            &to_bytes(route.into_body(), 16 * 1024)
                .await
                .expect("bounded route body"),
        )
        .expect("route payload");
        assert_eq!(route.record_id, created.record_id);
        assert_eq!(
            route.status,
            fasti_contracts::IdentityRouteStatusDto::Missing
        );

        let read = app
            .clone()
            .oneshot(
                auth(Request::get(
                    "/api/v1/profile/anime-grouping-policy?scope=profile",
                ))
                .body(Body::empty())
                .expect("read policy request"),
            )
            .await
            .expect("read policy response");
        assert_eq!(read.status(), StatusCode::OK);
        let read: fasti_contracts::ReadAnimeGroupingPolicyResponse = serde_json::from_slice(
            &to_bytes(read.into_body(), 4096)
                .await
                .expect("bounded policy body"),
        )
        .expect("policy payload");
        assert_eq!(read.policy.revision, 0);

        let other_client = fasti_domain::ClientId::new_v7();
        let forbidden = app
            .clone()
            .oneshot(
                auth(Request::get(format!(
                    "/api/v1/profile/anime-grouping-policy?scope=client&client_id={other_client}"
                )))
                .body(Body::empty())
                .expect("cross-client request"),
            )
            .await
            .expect("cross-client response");
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

        let preview_body = serde_json::json!({
            "scope": {"kind": "profile", "client_id": null},
            "change": {"kind": "set", "preference": "group_by_tv_work"},
            "after_record_id": null,
            "limit": 10,
        });
        let preview = app
            .clone()
            .oneshot(
                auth(Request::post(
                    "/api/v1/profile/anime-grouping-policy/preview",
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(preview_body.to_string()))
                .expect("preview request"),
            )
            .await
            .expect("preview response");
        assert_eq!(preview.status(), StatusCode::OK);
        let preview: fasti_contracts::AnimeGroupingPolicyImpactResponse = serde_json::from_slice(
            &to_bytes(preview.into_body(), 16 * 1024)
                .await
                .expect("bounded preview body"),
        )
        .expect("preview payload");
        assert_eq!(preview.total_records, 1);

        let operation_id = fasti_domain::OperationId::new_v7();
        let apply_body = serde_json::json!({
            "operation_id": operation_id,
            "scope": {"kind": "profile", "client_id": null},
            "expected_revision": 0,
            "change": {"kind": "set", "preference": "group_by_tv_work"},
        });
        let send_apply = || {
            auth(Request::put("/api/v1/profile/anime-grouping-policy"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(apply_body.to_string()))
                .expect("apply request")
        };
        let applied = app
            .clone()
            .oneshot(send_apply())
            .await
            .expect("apply response");
        assert_eq!(applied.status(), StatusCode::OK);
        let applied: fasti_contracts::ApplyAnimeGroupingPolicyChangeResponse =
            serde_json::from_slice(
                &to_bytes(applied.into_body(), 16 * 1024)
                    .await
                    .expect("bounded apply body"),
            )
            .expect("apply payload");
        assert_eq!(applied.operation_id, operation_id.to_string());
        assert_eq!(applied.policy.revision, 1);

        let replayed = app
            .clone()
            .oneshot(send_apply())
            .await
            .expect("replay response");
        assert_eq!(replayed.status(), StatusCode::OK);
        let replayed: fasti_contracts::ApplyAnimeGroupingPolicyChangeResponse =
            serde_json::from_slice(
                &to_bytes(replayed.into_body(), 16 * 1024)
                    .await
                    .expect("bounded replay body"),
            )
            .expect("replay payload");
        assert_eq!(replayed, applied);

        let rollback_operation_id = fasti_domain::OperationId::new_v7();
        let rollback_body = serde_json::json!({
            "operation_id": rollback_operation_id,
            "scope": {"kind": "profile", "client_id": null},
            "expected_revision": 1,
            "change": {
                "kind": "rollback",
                "applied_operation_id": operation_id,
            },
        });
        let send_rollback = || {
            auth(Request::put("/api/v1/profile/anime-grouping-policy"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(rollback_body.to_string()))
                .expect("rollback request")
        };
        let rolled_back = app
            .clone()
            .oneshot(send_rollback())
            .await
            .expect("rollback response");
        assert_eq!(rolled_back.status(), StatusCode::OK);
        let rolled_back: fasti_contracts::ApplyAnimeGroupingPolicyChangeResponse =
            serde_json::from_slice(
                &to_bytes(rolled_back.into_body(), 16 * 1024)
                    .await
                    .expect("bounded rollback body"),
            )
            .expect("rollback payload");
        assert_eq!(rolled_back.operation_id, rollback_operation_id.to_string());
        assert_eq!(rolled_back.policy.revision, 2);
        assert_eq!(
            rolled_back.policy.preference,
            fasti_contracts::AnimeGroupingPreferenceDto::Automatic
        );
        assert_eq!(
            rolled_back.rolled_back_operation_id,
            Some(operation_id.to_string())
        );

        let replayed_rollback = app
            .oneshot(send_rollback())
            .await
            .expect("rollback replay response");
        assert_eq!(replayed_rollback.status(), StatusCode::OK);
        let replayed_rollback: fasti_contracts::ApplyAnimeGroupingPolicyChangeResponse =
            serde_json::from_slice(
                &to_bytes(replayed_rollback.into_body(), 16 * 1024)
                    .await
                    .expect("bounded rollback replay body"),
            )
            .expect("rollback replay payload");
        assert_eq!(replayed_rollback, rolled_back);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn identity_routing_browser_session_uses_direct_loopback_cookies_and_csrf() {
        let (root, kernel) = test_kernel();
        let bootstrap_app = api_router(kernel.clone(), test_bind_addr(), root.path());
        let enrolled = enroll_admin(&bootstrap_app, root.path()).await;
        let access = kernel
            .authenticate_credential(AuthenticateCredentialQuery::new(
                RequestCorrelationId::new_v7(),
                CapabilityKey::ReadAnimeGroupingPolicy,
                SecretMaterial::try_from_hex(&enrolled.credential).expect("issued credential"),
            ))
            .expect("enrolled access");

        let now = chrono::Utc::now();
        let installation = kernel
            .verify_trailbase_installation(VerifyTrailBaseInstallationCommand::new(
                TrailBaseInstanceId::new_v7(),
                Sha256Digest::from_bytes(&[31; 32]),
                Sha256Digest::from_bytes(&[32; 32]),
                false,
                RequestCorrelationId::new_v7(),
                now,
            ))
            .expect("active TrailBase installation");
        let purpose = AuthCeremonyPurpose::FirstAdministratorBootstrap;
        let ceremony = AuthCeremony::try_new(
            OperationId::new_v7(),
            purpose,
            AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
            installation.id(),
            installation.activation_generation(),
            Sha256Digest::from_bytes(&[33; 32]),
            Some(
                AuthCeremonySelection::try_new(
                    purpose,
                    access.workspace_id(),
                    access.grant_id(),
                    None,
                    None,
                )
                .expect("bootstrap selection"),
            ),
            false,
            AuthCallbackPath::parse("/api/access/v1/trailbase/callback").expect("callback path"),
            purpose.return_target(),
            RequestCorrelationId::new_v7(),
            now,
            now + chrono::TimeDelta::minutes(10),
        )
        .expect("bootstrap ceremony");
        kernel
            .start_trailbase_bootstrap(StartTrailBaseBootstrapCommand::new(
                ceremony.clone(),
                kernel.ensure_bootstrap_secret().expect("bootstrap secret"),
            ))
            .expect("start bootstrap");
        kernel
            .claim_auth_ceremony(ClaimAuthCeremonyCommand::new(
                ceremony.browser_binding_digest().clone(),
                installation.id(),
                installation.activation_generation(),
                ceremony.callback_path().clone(),
                RequestCorrelationId::new_v7(),
                now + chrono::TimeDelta::seconds(1),
            ))
            .expect("claim bootstrap");
        let authorization = PreauthorizeTrailBaseBootstrapCommand::new(
            ceremony.id(),
            ConfirmedTrailBaseIdentity::new(
                installation.id(),
                TrailBaseSubject::from_bytes([34; 16]),
                AuthenticationProvenance::new(
                    AuthenticationMethod::TrailBasePassword,
                    now + chrono::TimeDelta::seconds(2),
                    installation.activation_generation(),
                ),
            ),
            RequestCorrelationId::new_v7(),
            now + chrono::TimeDelta::seconds(2),
        );
        kernel
            .preauthorize_trailbase_bootstrap(authorization)
            .expect("preauthorize bootstrap");
        let session = kernel
            .complete_trailbase_bootstrap(CompleteTrailBaseBootstrapCommand::new(
                authorization,
                kernel.ensure_bootstrap_secret().expect("bootstrap secret"),
            ))
            .expect("browser session");

        let session_secret = session.session_secret().expose_hex();
        let csrf = session.csrf_secret().expose_hex();
        let cookie = format!(
            "{}={session_secret}; {}={csrf}",
            local::SESSION_COOKIE,
            local::CSRF_COOKIE
        );
        let browser_read = |builder: axum::http::request::Builder| {
            builder
                .header(header::HOST, FASTI_ACCESS_HOST)
                .header(header::COOKIE, &cookie)
        };
        let browser_mutation = |builder: axum::http::request::Builder| {
            browser_read(builder)
                .header(header::ORIGIN, FASTI_ACCESS_ORIGIN)
                .header(local::CSRF_HEADER, &csrf)
        };
        let app = direct_loopback_api_router(kernel, test_bind_addr(), false, root.path(), None)
            .expect("direct loopback router");

        let created = app
            .clone()
            .oneshot(
                browser_mutation(Request::post("/api/v1/records"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"grain":"release"}"#))
                    .expect("create record request"),
            )
            .await
            .expect("create record response");
        assert_eq!(created.status(), StatusCode::OK);

        let read = app
            .clone()
            .oneshot(
                browser_read(Request::get(
                    "/api/v1/profile/anime-grouping-policy?scope=profile",
                ))
                .body(Body::empty())
                .expect("read policy request"),
            )
            .await
            .expect("read policy response");
        assert_eq!(read.status(), StatusCode::OK);

        let preview = app
            .clone()
            .oneshot(
                browser_mutation(Request::post(
                    "/api/v1/profile/anime-grouping-policy/preview",
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "scope": {"kind": "profile", "client_id": null},
                        "change": {"kind": "set", "preference": "group_by_tv_work"},
                        "after_record_id": null,
                        "limit": 10,
                    })
                    .to_string(),
                ))
                .expect("preview request"),
            )
            .await
            .expect("preview response");
        assert_eq!(preview.status(), StatusCode::OK);

        let operation_id = OperationId::new_v7();
        let apply_body = serde_json::json!({
            "operation_id": operation_id,
            "scope": {"kind": "profile", "client_id": null},
            "expected_revision": 0,
            "change": {"kind": "set", "preference": "group_by_tv_work"},
        });
        let send_apply = || {
            browser_mutation(Request::put("/api/v1/profile/anime-grouping-policy"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(apply_body.to_string()))
                .expect("apply request")
        };
        let applied = app
            .clone()
            .oneshot(send_apply())
            .await
            .expect("apply response");
        assert_eq!(applied.status(), StatusCode::OK);
        let applied: fasti_contracts::ApplyAnimeGroupingPolicyChangeResponse =
            serde_json::from_slice(
                &to_bytes(applied.into_body(), 16 * 1024)
                    .await
                    .expect("bounded apply body"),
            )
            .expect("apply payload");
        let replayed = app
            .clone()
            .oneshot(send_apply())
            .await
            .expect("apply replay response");
        assert_eq!(replayed.status(), StatusCode::OK);
        let replayed: fasti_contracts::ApplyAnimeGroupingPolicyChangeResponse =
            serde_json::from_slice(
                &to_bytes(replayed.into_body(), 16 * 1024)
                    .await
                    .expect("bounded replay body"),
            )
            .expect("replay payload");
        assert_eq!(replayed, applied);

        let rollback_operation_id = OperationId::new_v7();
        let rollback_body = serde_json::json!({
            "operation_id": rollback_operation_id,
            "scope": {"kind": "profile", "client_id": null},
            "expected_revision": 1,
            "change": {"kind": "rollback", "applied_operation_id": operation_id},
        });
        let send_rollback = || {
            browser_mutation(Request::put("/api/v1/profile/anime-grouping-policy"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(rollback_body.to_string()))
                .expect("rollback request")
        };
        let rolled_back = app
            .clone()
            .oneshot(send_rollback())
            .await
            .expect("rollback response");
        assert_eq!(rolled_back.status(), StatusCode::OK);
        let rolled_back: fasti_contracts::ApplyAnimeGroupingPolicyChangeResponse =
            serde_json::from_slice(
                &to_bytes(rolled_back.into_body(), 16 * 1024)
                    .await
                    .expect("bounded rollback body"),
            )
            .expect("rollback payload");
        assert_eq!(
            rolled_back.policy.preference,
            fasti_contracts::AnimeGroupingPreferenceDto::Automatic
        );
        let replayed_rollback = app
            .oneshot(send_rollback())
            .await
            .expect("rollback replay response");
        assert_eq!(replayed_rollback.status(), StatusCode::OK);
        let replayed_rollback: fasti_contracts::ApplyAnimeGroupingPolicyChangeResponse =
            serde_json::from_slice(
                &to_bytes(replayed_rollback.into_body(), 16 * 1024)
                    .await
                    .expect("bounded rollback replay body"),
            )
            .expect("rollback replay payload");
        assert_eq!(replayed_rollback, rolled_back);
    }

    #[cfg(target_os = "linux")]
    async fn enroll_admin(
        app: &Router,
        data_root: &std::path::Path,
    ) -> fasti_contracts::ClientEnrollmentResponse {
        // api_router primes this file at construction time -- read it the
        // same way a legitimate first client would, proving local
        // filesystem access to this data root.
        let bootstrap_secret = std::fs::read_to_string(data_root.join("bootstrap.secret"))
            .expect("bootstrap secret readable after api_router primes it");
        let initialized = app
            .clone()
            .oneshot(
                Request::post("/api/v1/node/initialization")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", bootstrap_secret.trim()),
                    )
                    .body(Body::from("{}"))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(initialized.status(), StatusCode::OK);
        let initialized: fasti_contracts::NodeInitializationResponse = serde_json::from_slice(
            &to_bytes(initialized.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("initialization response");

        let enrollment_request = serde_json::json!({
            "initialization_proof": initialized.initialization_proof
        });
        let enrolled = app
            .clone()
            .oneshot(
                Request::post("/api/v1/client-enrollments")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(enrollment_request.to_string()))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(enrolled.status(), StatusCode::OK);
        serde_json::from_slice(
            &to_bytes(enrolled.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("enrollment response")
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn browser_authentication_routes_are_absent_until_c1() {
        let (root, kernel) = test_kernel();
        let app = api_router(kernel, test_bind_addr(), root.path());

        for (method, path) in [
            (Method::POST, "/api/v1/browser/session"),
            (Method::GET, "/api/v1/browser/sessions"),
            (Method::GET, "/api/v1/browser/users"),
            (Method::GET, "/api/v1/browser/auth/passkeys"),
            (Method::POST, "/api/v1/browser/auth/totp/enroll/begin"),
            (Method::POST, "/api/v1/browser/auth/oidc/discover"),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .expect("browser authentication request"),
                )
                .await
                .expect("router response");
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        }
    }

    #[cfg(target_os = "linux")]
    async fn assert_access_routes_are_absent(router: Router) {
        for path in [
            "/api/access/v1/projection",
            "/api/access/v1/trailbase/callback?code=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::get(path)
                        .body(Body::empty())
                        .expect("Access request"),
                )
                .await
                .expect("router response");
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn access_routes_exist_only_on_the_exact_direct_listener() {
        let (root, kernel) = test_kernel();
        let inactive = direct_loopback_api_router(
            kernel.clone(),
            "127.0.0.1:8420".parse().expect("fixed listener"),
            false,
            root.path(),
            None,
        )
        .expect("inactive router");
        let inactive_callback = inactive
            .oneshot(
                Request::get(format!(
                    "/api/access/v1/trailbase/callback?code={}",
                    "a".repeat(48)
                ))
                .header(header::HOST, "127.0.0.1:8420")
                .body(Body::empty())
                .expect("callback request"),
            )
            .await
            .expect("inactive callback response");
        assert_eq!(inactive_callback.status(), StatusCode::SEE_OTHER);

        let trailbase_root = test_trailbase_root();
        let direct = direct_loopback_api_router(
            kernel.clone(),
            "127.0.0.1:8420".parse().expect("fixed listener"),
            false,
            root.path(),
            Some(trailbase_root.path()),
        )
        .expect("verified direct router");
        let generic = api_router(kernel.clone(), test_bind_addr(), root.path());
        let integration = integration_router(kernel.clone());
        let remote = remote_api_router(
            kernel,
            "0.0.0.0:8420".parse().expect("remote listener"),
            root.path(),
        );

        let direct_projection = direct
            .clone()
            .oneshot(
                Request::get("/api/access/v1/projection")
                    .header(header::HOST, "127.0.0.1:8420")
                    .body(Body::empty())
                    .expect("Access request"),
            )
            .await
            .expect("direct response");
        assert_eq!(direct_projection.status(), StatusCode::UNAUTHORIZED);

        let callback = direct
            .oneshot(
                Request::get(format!(
                    "/api/access/v1/trailbase/callback?code={}",
                    "a".repeat(48)
                ))
                .header(header::HOST, "127.0.0.1:8420")
                .body(Body::empty())
                .expect("callback request"),
            )
            .await
            .expect("callback response");
        assert_eq!(callback.status(), StatusCode::SEE_OTHER);
        assert!(callback
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .any(|value| value
                .to_str()
                .expect("cookie")
                .starts_with("__Secure-fasti_auth_binding=;")));

        for router in [generic, integration, remote] {
            assert_access_routes_are_absent(router).await;
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn copied_trailbase_receipt_cannot_activate_a_different_root() {
        let (data_root, kernel) = test_kernel();
        let source = test_trailbase_root();
        let copy = tempfile::tempdir().expect("copied TrailBase root");
        std::fs::set_permissions(copy.path(), std::fs::Permissions::from_mode(0o700))
            .expect("private copied root");
        for name in ["runtime.lock", ".fasti-installation.json"] {
            std::fs::copy(source.path().join(name), copy.path().join(name))
                .expect("copy installation evidence");
            std::fs::set_permissions(
                copy.path().join(name),
                std::fs::Permissions::from_mode(0o600),
            )
            .expect("private copied evidence");
        }

        let result = DirectLoopbackAccessRuntime::new(
            kernel,
            "127.0.0.1:8420".parse().expect("fixed listener"),
            false,
            data_root.path(),
            Some(copy.path()),
        );
        assert!(result.is_err());
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn node_initialization_refuses_a_missing_or_wrong_bootstrap_secret() {
        let (root, kernel) = test_kernel();
        let app = api_router(kernel, test_bind_addr(), root.path());

        let missing_header = app
            .clone()
            .oneshot(
                Request::post("/api/v1/node/initialization")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{}"))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(missing_header.status(), StatusCode::FORBIDDEN);

        let wrong_secret = SecretMaterial::from_bytes([7_u8; 32]).expose_hex();
        let wrong_header = app
            .clone()
            .oneshot(
                Request::post("/api/v1/node/initialization")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {wrong_secret}"))
                    .body(Body::from("{}"))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(wrong_header.status(), StatusCode::FORBIDDEN);

        // A second process that can read the same data root -- exactly the
        // legitimate-first-client scenario this whole mechanism exists for --
        // is not blocked by a wrong attempt that came before it.
        let bootstrap_secret = std::fs::read_to_string(root.path().join("bootstrap.secret"))
            .expect("bootstrap secret readable after api_router primes it");
        let correct_header = app
            .oneshot(
                Request::post("/api/v1/node/initialization")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", bootstrap_secret.trim()),
                    )
                    .body(Body::from("{}"))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(correct_header.status(), StatusCode::OK);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn bootstrap_secret_survives_a_router_rebuild_and_has_owner_only_permissions() {
        let (root, kernel) = test_kernel();
        let _first_router = api_router(kernel.clone(), test_bind_addr(), root.path());
        let first_read = std::fs::read_to_string(root.path().join("bootstrap.secret"))
            .expect("bootstrap secret readable after first priming");

        // Simulates a daemon restart against the same data root: a second
        // api_router build must not regenerate (and thereby invalidate) the
        // secret a legitimate client may have already read.
        let _second_router = api_router(kernel, test_bind_addr(), root.path());
        let second_read = std::fs::read_to_string(root.path().join("bootstrap.secret"))
            .expect("bootstrap secret readable after second priming");
        assert_eq!(first_read, second_read);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(root.path().join("bootstrap.secret"))
                .expect("bootstrap secret metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(
                mode, 0o600,
                "bootstrap secret must be owner-read-write only"
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn durable_bootstrap_issues_one_credential_and_closes_initialization() {
        let (root, kernel) = test_kernel();
        let app = api_router(kernel.clone(), test_bind_addr(), root.path());
        let bootstrap_secret = std::fs::read_to_string(root.path().join("bootstrap.secret"))
            .expect("bootstrap secret readable after api_router primes it");

        let initialized = app
            .clone()
            .oneshot(
                Request::post("/api/v1/node/initialization")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", bootstrap_secret.trim()),
                    )
                    .body(Body::from("{}"))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(initialized.status(), StatusCode::OK);
        let initialized: fasti_contracts::NodeInitializationResponse = serde_json::from_slice(
            &to_bytes(initialized.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("initialization response");

        let invalid_secret = "not-a-secret";
        let denied = app
            .clone()
            .oneshot(
                Request::post("/api/v1/client-enrollments")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({ "initialization_proof": invalid_secret }).to_string(),
                    ))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(denied.status(), StatusCode::CONFLICT);
        let denied_body = to_bytes(denied.into_body(), 4096)
            .await
            .expect("bounded body");
        assert!(!String::from_utf8_lossy(&denied_body).contains(invalid_secret));
        let denied: ProblemDetails =
            serde_json::from_slice(&denied_body).expect("problem response");
        assert_eq!(denied.code, "bootstrap_closed");

        let enrollment_request = serde_json::json!({
            "initialization_proof": initialized.initialization_proof
        });
        let enrolled = app
            .clone()
            .oneshot(
                Request::post("/api/v1/client-enrollments")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(enrollment_request.to_string()))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(enrolled.status(), StatusCode::OK);
        let enrolled: fasti_contracts::ClientEnrollmentResponse = serde_json::from_slice(
            &to_bytes(enrolled.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("enrollment response");
        assert_eq!(
            enrolled.credential_scheme,
            fasti_contracts::CredentialSchemeDto::Bearer
        );
        kernel
            .authenticate_credential(AuthenticateCredentialQuery::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                CapabilityKey::InspectReview,
                SecretMaterial::try_from_hex(&enrolled.credential).expect("issued credential"),
            ))
            .expect("durable credential authenticates");

        let repeated = app
            .oneshot(
                Request::post("/api/v1/node/initialization")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", bootstrap_secret.trim()),
                    )
                    .body(Body::from("{}"))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(repeated.status(), StatusCode::CONFLICT);
        let problem: ProblemDetails = serde_json::from_slice(
            &to_bytes(repeated.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("problem response");
        assert_eq!(problem.code, "already_initialized");
        assert_eq!(problem.safe_state, "prior_state_retained");
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn observation_requires_bearer_and_replays_one_source_event_exactly_once() {
        let (root, kernel) = test_kernel();
        let app = api_router(kernel.clone(), test_bind_addr(), root.path());
        let request = serde_json::json!({
            "kind": "consumption_occurrence",
            "source": "nuvio",
            "source_event_id": "session-42:stop:episode-7",
            "observed_at": "2026-08-26T18:10:00Z",
            "occurred_at": "2026-08-26T18:09:58Z",
            "target_grain": "episode",
            "identifiers": [
                {"namespace":"imdb.title","grain":"series","value":"tt1234567"},
                {"namespace":"kitsu.anime","grain":"release","value":"7442"}
            ],
            "title": "Example episode",
            "progress_percent": 100.0,
            "position_seconds": 1440,
            "duration_seconds": 1440
        });

        let unauthorized = app
            .clone()
            .oneshot(
                Request::post("/api/v1/observations")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(request.to_string()))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let credential = enroll_admin(&app, root.path()).await.credential;
        let send = |body: serde_json::Value| {
            Request::post("/api/v1/observations")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                .body(Body::from(body.to_string()))
                .expect("valid request")
        };

        let committed = app
            .clone()
            .oneshot(send(request.clone()))
            .await
            .expect("router response");
        assert_eq!(committed.status(), StatusCode::OK);
        let committed: fasti_contracts::SubmitObservationResponse = serde_json::from_slice(
            &to_bytes(committed.into_body(), 16 * 1024)
                .await
                .expect("bounded body"),
        )
        .expect("observation response");
        assert_eq!(committed.disposition, "committed");

        let replayed = app
            .clone()
            .oneshot(send(request.clone()))
            .await
            .expect("router response");
        assert_eq!(replayed.status(), StatusCode::OK);
        let replayed: fasti_contracts::SubmitObservationResponse = serde_json::from_slice(
            &to_bytes(replayed.into_body(), 16 * 1024)
                .await
                .expect("bounded body"),
        )
        .expect("replayed response");
        assert_eq!(replayed.disposition, "replayed");
        assert_eq!(replayed.receipt_id, committed.receipt_id);
        assert_eq!(replayed.observation_id, committed.observation_id);

        let mut changed = request;
        changed["title"] = serde_json::json!("Changed evidence for the same source event");
        let conflict = app.oneshot(send(changed)).await.expect("router response");
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        let problem: ProblemDetails = serde_json::from_slice(
            &to_bytes(conflict.into_body(), 16 * 1024)
                .await
                .expect("bounded body"),
        )
        .expect("problem response");
        assert_eq!(problem.code, "idempotency_conflict");
        assert_eq!(problem.safe_state, "prior_state_retained");
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn partial_progress_is_rejected_without_creating_false_history() {
        let (root, kernel) = test_kernel();
        let app = api_router(kernel, test_bind_addr(), root.path());
        let credential = enroll_admin(&app, root.path()).await.credential;
        let request = serde_json::json!({
            "kind": "consumption_occurrence",
            "source": "nuvio",
            "source_event_id": "session-42:progress:episode-7",
            "observed_at": "2026-08-26T18:10:00Z",
            "target_grain": "episode",
            "identifiers": [],
            "progress_percent": 72.5,
            "position_seconds": 1044,
            "duration_seconds": 1440
        });
        let response = app
            .oneshot(
                Request::post("/api/v1/observations")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .body(Body::from(request.to_string()))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let problem: ProblemDetails = serde_json::from_slice(
            &to_bytes(response.into_body(), 16 * 1024)
                .await
                .expect("bounded body"),
        )
        .expect("problem response");
        assert_eq!(problem.code, "invalid_observation");
        assert_eq!(problem.safe_state, "no_mutation");
    }

    #[tokio::test]
    async fn remote_health_router_exposes_no_local_capability_route() {
        let response = health_router()
            .oneshot(
                Request::post("/api/v1/observations")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn integration_router_exposes_adapters_but_not_bootstrap_or_generic_mutation() {
        let (_root, kernel) = test_kernel();
        let app = integration_router(kernel);

        let status = app
            .clone()
            .oneshot(
                Request::get("/api/v1/integrations")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(status.status(), StatusCode::OK);

        for path in [
            "/api/v1/node/initialization",
            "/api/v1/records",
            "/api/v1/observations",
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::post(path)
                        .body(Body::empty())
                        .expect("valid request"),
                )
                .await
                .expect("router response");
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn event_submission_alias_is_absent() {
        let (root, kernel) = test_kernel();
        let response = api_router(kernel, test_bind_addr(), root.path())
            .oneshot(
                Request::post("/api/v1/events")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn other_b1_fixture_routes_remain_absent_from_production() {
        let (root, kernel) = test_kernel();
        for (method, path) in [
            ("GET", "/api/v1/capabilities"),
            ("GET", "/api/v1/receipts/stream"),
            ("GET", "/api/v1/receipts/rcp_not-a-real-id"),
            ("PUT", "/api/v1/profile-selection"),
            ("POST", "/api/v1/credential-rotations"),
            ("POST", "/api/v1/credential-revocations"),
            ("PUT", "/api/v1/listener-configuration"),
        ] {
            let response = api_router(kernel.clone(), test_bind_addr(), root.path())
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .expect("valid request"),
                )
                .await
                .expect("router response");
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {path}");
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn records_require_bearer_and_support_create_list_attach_and_namespace_registration() {
        let (root, kernel) = test_kernel();
        let app = api_router(kernel.clone(), test_bind_addr(), root.path());

        let unauthorized = app
            .clone()
            .oneshot(
                Request::get("/api/v1/records")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let credential = enroll_admin(&app, root.path()).await.credential;
        let auth = |builder: axum::http::request::Builder| {
            builder.header(header::AUTHORIZATION, format!("Bearer {credential}"))
        };

        let empty_list = app
            .clone()
            .oneshot(
                auth(Request::get("/api/v1/records"))
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(empty_list.status(), StatusCode::OK);
        let empty_list: fasti_contracts::ListRecordsResponse = serde_json::from_slice(
            &to_bytes(empty_list.into_body(), 16 * 1024)
                .await
                .expect("bounded body"),
        )
        .expect("list response");
        assert!(empty_list.records.is_empty());

        let created = app
            .clone()
            .oneshot(
                auth(Request::post("/api/v1/records"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"grain":"work"}"#))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(created.status(), StatusCode::OK);
        let created: fasti_contracts::CreateRecordResponse = serde_json::from_slice(
            &to_bytes(created.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("create-record response");
        assert_eq!(created.grain, "work");

        let namespace = app
            .clone()
            .oneshot(
                auth(Request::post("/api/v1/namespaces"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "namespace": "google-books",
                            "label": "Google Books",
                            "grains": ["work"],
                            "id_pattern": ".+",
                            "normalization": "identity",
                            "licence_posture": "identifiers_only",
                        })
                        .to_string(),
                    ))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(namespace.status(), StatusCode::OK);
        let namespace: fasti_contracts::RegisterNamespaceResponse = serde_json::from_slice(
            &to_bytes(namespace.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("register-namespace response");
        assert!(namespace.created);

        let attached = app
            .clone()
            .oneshot(
                auth(Request::post("/api/v1/records/identifiers"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "record_id": created.record_id,
                            "namespace": "google-books",
                            "grain": "work",
                            "value": "abc123",
                        })
                        .to_string(),
                    ))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(attached.status(), StatusCode::OK);
        let attached: fasti_contracts::AttachIdentifierResponse = serde_json::from_slice(
            &to_bytes(attached.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("attach-identifier response");
        assert!(attached.created);

        let populated_list = app
            .clone()
            .oneshot(
                auth(Request::get("/api/v1/records"))
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(populated_list.status(), StatusCode::OK);
        let populated_list: fasti_contracts::ListRecordsResponse = serde_json::from_slice(
            &to_bytes(populated_list.into_body(), 16 * 1024)
                .await
                .expect("bounded body"),
        )
        .expect("list response");
        assert_eq!(populated_list.records.len(), 1);
        assert_eq!(populated_list.records[0].record_id, created.record_id);
        assert_eq!(populated_list.records[0].identifiers.len(), 1);
        assert_eq!(populated_list.records[0].identifiers[0].value, "abc123");
        for (record_id, expected_count) in [
            (created.record_id.clone(), 1),
            (fasti_domain::RecordId::new_v7().to_string(), 0),
        ] {
            let response = app
                .clone()
                .oneshot(
                    auth(Request::get(format!(
                        "/api/v1/records?record_id={record_id}"
                    )))
                    .body(Body::empty())
                    .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let selected: fasti_contracts::ListRecordsResponse =
                serde_json::from_slice(&to_bytes(response.into_body(), 16 * 1024).await.unwrap())
                    .unwrap();
            assert!(!selected.truncated);
            assert_eq!(selected.records.len(), expected_count);
            if expected_count == 1 {
                assert_eq!(selected, populated_list);
            }
        }
        for query in [
            "record_id=invalid".to_owned(),
            "record_id=".to_owned(),
            "unknown=value".to_owned(),
            format!("record_id={0}&record_id={0}", created.record_id),
        ] {
            let path = format!("/api/v1/records?{query}");
            let unauthorized = app
                .clone()
                .oneshot(
                    Request::get(&path)
                        .header(header::AUTHORIZATION, "Bearer invalid")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
            let invalid = app
                .clone()
                .oneshot(auth(Request::get(&path)).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let problem: fasti_contracts::ProblemDetails =
                serde_json::from_slice(&to_bytes(invalid.into_body(), 16 * 1024).await.unwrap())
                    .unwrap();
            assert_eq!(problem.code, "validation_failed");
        }
        assert_eq!(
            populated_list.records[0]
                .overview
                .as_ref()
                .and_then(|field| field.value.as_deref()),
            None,
        );
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn profile_tracking_disposition_is_authenticated_set_list_and_unset() {
        let (root, kernel) = test_kernel();
        let app = api_router(kernel, test_bind_addr(), root.path());
        let unauthorized = app
            .clone()
            .oneshot(
                Request::get("/api/v1/profile/record-tracking-dispositions")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        let credential = enroll_admin(&app, root.path()).await.credential;
        let auth = |builder: axum::http::request::Builder| {
            builder.header(header::AUTHORIZATION, format!("Bearer {credential}"))
        };

        let created = app
            .clone()
            .oneshot(
                auth(Request::post("/api/v1/records"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"grain":"work"}"#))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(created.status(), StatusCode::OK);
        let created: fasti_contracts::CreateRecordResponse = serde_json::from_slice(
            &to_bytes(created.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("create-record response");
        let state_path = format!(
            "/api/v1/profile/record-tracking-dispositions/{}",
            created.record_id
        );

        let set = app
            .clone()
            .oneshot(
                auth(Request::put(&state_path))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"disposition":"watching"}"#))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(set.status(), StatusCode::OK);
        let set: fasti_contracts::TrackingDispositionStateDto =
            serde_json::from_slice(&to_bytes(set.into_body(), 4096).await.expect("bounded body"))
                .expect("set tracking response");
        assert_eq!(set.record_id, created.record_id);
        assert_eq!(
            set.disposition,
            Some(fasti_contracts::TrackingDispositionDto::Watching)
        );

        let listed = app
            .clone()
            .oneshot(
                auth(Request::get("/api/v1/profile/record-tracking-dispositions"))
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(listed.status(), StatusCode::OK);
        let listed: fasti_contracts::ListTrackingDispositionsResponse = serde_json::from_slice(
            &to_bytes(listed.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("list tracking response");
        assert_eq!(listed.states, vec![set]);

        let unset = app
            .clone()
            .oneshot(
                auth(Request::put(&state_path))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"disposition":"unset"}"#))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(unset.status(), StatusCode::OK);
        let unset: fasti_contracts::TrackingDispositionStateDto = serde_json::from_slice(
            &to_bytes(unset.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("unset tracking response");
        assert_eq!(unset.record_id, created.record_id);
        assert_eq!(unset.disposition, None);
    }
}
