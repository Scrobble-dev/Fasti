// C1.2 keeps this reviewed trust boundary private until C1.3 mounts its routes.
#![allow(dead_code)]

use base64::{engine::general_purpose::URL_SAFE, engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, TimeDelta, Utc};
use fasti_application::{
    CancelAuthCeremonyCommand, CompleteTrailBaseBootstrapCommand, CompleteTrailBaseSignInCommand,
    ConfirmedTrailBaseIdentity, CreatedBrowserSession, FailAuthCeremonyCommand, HumanAccessPort,
    PreauthorizeTrailBaseBootstrapCommand, PreauthorizeTrailBaseSignInCommand, ProblemCode,
    SecretMaterial, StartAuthCeremonyCommand, StartTrailBaseBootstrapCommand,
};
use fasti_domain::{
    AuthCallbackPath, AuthCeremony, AuthCeremonyFailure, AuthCeremonyProtocol, AuthCeremonyPurpose,
    AuthCeremonySelection, AuthReturnTarget, AuthenticationMethod, AuthenticationProvenance,
    OperationId, RequestCorrelationId, Sha256Digest, TrailBaseActivationState,
    TrailBaseInstallation, TrailBaseInstanceId, TrailBaseSubject,
};
use fasti_provider_runtime::{bounded_body, pinned_client_with_timeouts};
use reqwest::{
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    Client, Response, StatusCode,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use zeroize::Zeroizing;

const PRODUCTION_TRAILBASE_ADDR: &str = "127.0.0.1:4000";
const AUTHORIZATION_UI_PATH: &str = "/_/auth/login";
const TOKEN_PATH: &str = "/api/auth/v1/token";
const STATUS_PATH: &str = "/api/auth/v1/status";
const LOGOUT_PATH: &str = "/api/auth/v1/logout";
const REFRESH_TOKEN_HEADER: &str = "Refresh-Token";
const REQUEST_LIMIT: usize = 8 * 1024;
const RESPONSE_LIMIT: usize = 16 * 1024;
const COMPACT_TOKEN_LIMIT: usize = 8 * 1024;
const TOKEN_PAYLOAD_LIMIT: usize = 4 * 1024;
const REQUEST_CONCURRENCY_LIMIT: usize = 4;
const PKCE_VAULT_LIMIT: usize = 64;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_FUTURE_SKEW: TimeDelta = TimeDelta::seconds(60);
const MAX_TOKEN_LIFETIME: TimeDelta = TimeDelta::hours(2);
const CALLBACK_PATH: &str = "/api/access/v1/trailbase/callback";
const CALLBACK_URL: &str = "http://127.0.0.1:8420/api/access/v1/trailbase/callback";

pub(super) struct TrailBaseClient {
    client: Client,
    origin: String,
    requests: Arc<Semaphore>,
}

pub(super) struct TrailBaseOrchestrator {
    client: TrailBaseClient,
    vault: PkceVault,
    access: Arc<dyn HumanAccessPort>,
    instance_id: TrailBaseInstanceId,
    activation_generation: u64,
}

pub(super) struct StartedTrailBaseCeremony {
    pub(super) operation_id: OperationId,
    pub(super) authorization_url: String,
    pub(super) expires_at: DateTime<Utc>,
    pub(super) browser_binding: SecretMaterial,
}

pub(super) struct TrailBaseCallbackOutcome {
    pub(super) created: CreatedBrowserSession,
    pub(super) return_target: AuthReturnTarget,
}

pub(super) struct TrailBaseCallbackFailure {
    pub(super) error: TrailBaseOrchestrationError,
    pub(super) return_target: Option<AuthReturnTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TrailBaseOrchestrationError {
    ApplicationProblem(ProblemCode),
    InvalidInput,
    LocalState,
    ExchangeFailed,
    ExchangeOutcomeUncertain,
    StatusRejected,
    LogoutUncertain,
    LocalAuthorizationDenied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TrailBaseFailure {
    ExchangeFailed,
    ExchangeOutcomeUncertain,
    StatusRejected,
    LogoutUncertain,
}

pub(super) struct AuthorizationCode(Zeroizing<String>);
pub(super) struct PkceVerifier(Zeroizing<String>);
struct VendorSecret(Zeroizing<String>);

#[derive(Clone, Default)]
pub(super) struct PkceVault {
    inner: Arc<Mutex<PkceVaultState>>,
}

#[derive(Default)]
struct PkceVaultState {
    reserved: usize,
    verifiers: HashMap<OperationId, PkceVerifier>,
}

pub(super) struct PkceReservation {
    vault: Arc<Mutex<PkceVaultState>>,
    active: bool,
}

pub(super) struct TrailBaseSession {
    auth_token: VendorSecret,
    refresh_token: VendorSecret,
    csrf_token: VendorSecret,
}

#[derive(Serialize)]
struct TokenRequest<'a> {
    authorization_code: &'a str,
    pkce_code_verifier: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TokenResponse {
    auth_token: String,
    refresh_token: String,
    csrf_token: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusResponse {
    auth_token: Option<String>,
    refresh_token: Option<String>,
    csrf_token: Option<String>,
}

#[derive(Serialize)]
struct LogoutRequest<'a> {
    refresh_token: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusTokenClaims {
    sub: String,
    iat: i64,
    exp: i64,
    #[serde(rename = "type")]
    token_type: u8,
    #[serde(default)]
    admin: bool,
    #[serde(default)]
    mfa: bool,
    provider: u8,
    email: Option<String>,
    username: Option<String>,
    csrf_token: String,
}

impl TrailBaseOrchestrator {
    pub(super) fn production(
        access: Arc<dyn HumanAccessPort>,
        installation: &TrailBaseInstallation,
    ) -> Result<Self, TrailBaseOrchestrationError> {
        if installation.activation_state() != TrailBaseActivationState::Active
            || installation.activation_generation() == 0
        {
            return Err(TrailBaseOrchestrationError::LocalState);
        }
        Ok(Self {
            client: TrailBaseClient::production()
                .map_err(|_| TrailBaseOrchestrationError::InvalidInput)?,
            vault: PkceVault::default(),
            access,
            instance_id: installation.id(),
            activation_generation: installation.activation_generation(),
        })
    }

    pub(super) fn start_sign_in(
        &self,
        selection: AuthCeremonySelection,
        correlation_id: RequestCorrelationId,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<StartedTrailBaseCeremony, TrailBaseOrchestrationError> {
        self.start(
            AuthCeremonyPurpose::SignIn,
            self.instance_id,
            self.activation_generation,
            selection,
            None,
            correlation_id,
            created_at,
            expires_at,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn start_bootstrap(
        &self,
        selection: AuthCeremonySelection,
        bootstrap_secret: SecretMaterial,
        correlation_id: RequestCorrelationId,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<StartedTrailBaseCeremony, TrailBaseOrchestrationError> {
        self.start(
            AuthCeremonyPurpose::FirstAdministratorBootstrap,
            self.instance_id,
            self.activation_generation,
            selection,
            Some(bootstrap_secret),
            correlation_id,
            created_at,
            expires_at,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn start(
        &self,
        purpose: AuthCeremonyPurpose,
        instance_id: TrailBaseInstanceId,
        activation_generation: u64,
        selection: AuthCeremonySelection,
        bootstrap_secret: Option<SecretMaterial>,
        correlation_id: RequestCorrelationId,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<StartedTrailBaseCeremony, TrailBaseOrchestrationError> {
        let reservation = self.vault.reserve()?;
        let verifier =
            PkceVerifier::generate().map_err(|_| TrailBaseOrchestrationError::LocalState)?;
        let pkce_challenge = verifier.challenge();
        let authorization_url = self
            .client
            .authorization_url(&pkce_challenge)
            .map_err(|_| TrailBaseOrchestrationError::InvalidInput)?;
        let mut binding_bytes = Zeroizing::new([0_u8; 32]);
        getrandom::fill(binding_bytes.as_mut())
            .map_err(|_| TrailBaseOrchestrationError::LocalState)?;
        let browser_binding = SecretMaterial::from_bytes(*binding_bytes);
        let browser_binding_digest = sha256_digest(browser_binding.expose_bytes());
        let operation_id = OperationId::new_v7();
        let ceremony = AuthCeremony::try_new(
            operation_id,
            purpose,
            AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
            instance_id,
            activation_generation,
            browser_binding_digest,
            selection,
            AuthCallbackPath::parse(CALLBACK_PATH)
                .map_err(|_| TrailBaseOrchestrationError::InvalidInput)?,
            purpose.return_target(),
            correlation_id,
            created_at,
            expires_at,
        )
        .map_err(|_| TrailBaseOrchestrationError::InvalidInput)?;
        reservation
            .commit(operation_id, verifier)
            .map_err(|_| TrailBaseOrchestrationError::LocalState)?;
        let persisted = match (purpose, bootstrap_secret) {
            (AuthCeremonyPurpose::SignIn, None) => self
                .access
                .start_auth_ceremony(StartAuthCeremonyCommand::new(ceremony)),
            (AuthCeremonyPurpose::FirstAdministratorBootstrap, Some(secret)) => self
                .access
                .start_trailbase_bootstrap(StartTrailBaseBootstrapCommand::new(ceremony, secret)),
            _ => {
                self.vault.remove(operation_id);
                return Err(TrailBaseOrchestrationError::InvalidInput);
            }
        };
        if let Err(problem) = persisted {
            self.vault.remove(operation_id);
            return Err(TrailBaseOrchestrationError::ApplicationProblem(
                problem.code(),
            ));
        }
        Ok(StartedTrailBaseCeremony {
            operation_id,
            authorization_url,
            expires_at,
            browser_binding,
        })
    }

    pub(super) fn cancel(
        &self,
        command: CancelAuthCeremonyCommand,
    ) -> Result<(), TrailBaseOrchestrationError> {
        let operation_id = command.operation_id();
        self.access
            .cancel_auth_ceremony(command)
            .map_err(|_| TrailBaseOrchestrationError::LocalState)?;
        self.vault.remove(operation_id);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn callback(
        &self,
        authorization_code: String,
        browser_binding: SecretMaterial,
        bootstrap_secret: Option<SecretMaterial>,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> Result<CreatedBrowserSession, TrailBaseOrchestrationError> {
        self.callback_for_browser(
            authorization_code,
            browser_binding,
            bootstrap_secret,
            correlation_id,
            at,
        )
        .await
        .map(|outcome| outcome.created)
        .map_err(|failure| failure.error)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn callback_for_browser(
        &self,
        authorization_code: String,
        browser_binding: SecretMaterial,
        bootstrap_secret: Option<SecretMaterial>,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> Result<TrailBaseCallbackOutcome, TrailBaseCallbackFailure> {
        let code =
            AuthorizationCode::parse(authorization_code).map_err(|_| TrailBaseCallbackFailure {
                error: TrailBaseOrchestrationError::InvalidInput,
                return_target: None,
            })?;
        let callback_path =
            AuthCallbackPath::parse(CALLBACK_PATH).map_err(|_| TrailBaseCallbackFailure {
                error: TrailBaseOrchestrationError::InvalidInput,
                return_target: None,
            })?;
        let claimed = self
            .access
            .claim_auth_ceremony(fasti_application::ClaimAuthCeremonyCommand::new(
                sha256_digest(browser_binding.expose_bytes()),
                self.instance_id,
                self.activation_generation,
                callback_path,
                correlation_id,
                at,
            ))
            .map_err(|_| TrailBaseCallbackFailure {
                error: TrailBaseOrchestrationError::LocalState,
                return_target: None,
            })?;
        let return_target = claimed.return_target();
        self.finish_claimed_callback(code, claimed, bootstrap_secret, correlation_id, at)
            .await
            .map(|created| TrailBaseCallbackOutcome {
                created,
                return_target,
            })
            .map_err(|error| TrailBaseCallbackFailure {
                error,
                return_target: Some(return_target),
            })
    }

    async fn finish_claimed_callback(
        &self,
        code: AuthorizationCode,
        claimed: AuthCeremony,
        bootstrap_secret: Option<SecretMaterial>,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> Result<CreatedBrowserSession, TrailBaseOrchestrationError> {
        let verifier = match self.vault.take(claimed.id()) {
            Some(verifier) => verifier,
            None => {
                self.record_failure(
                    claimed.id(),
                    AuthCeremonyFailure::VerifierLostOnRestart,
                    correlation_id,
                    at,
                )?;
                return Err(TrailBaseOrchestrationError::LocalState);
            }
        };
        let mut session = match self.client.exchange(&code, &verifier).await {
            Ok(session) => session,
            Err(TrailBaseFailure::ExchangeFailed) => {
                self.record_failure(
                    claimed.id(),
                    AuthCeremonyFailure::ExchangeFailed,
                    correlation_id,
                    at,
                )?;
                return Err(TrailBaseOrchestrationError::ExchangeFailed);
            }
            Err(_) => {
                self.record_failure(
                    claimed.id(),
                    AuthCeremonyFailure::ExchangeOutcomeUncertain,
                    correlation_id,
                    at,
                )?;
                return Err(TrailBaseOrchestrationError::ExchangeOutcomeUncertain);
            }
        };
        let identity = match self
            .client
            .status(
                &mut session,
                self.instance_id,
                self.activation_generation,
                at,
            )
            .await
        {
            Ok(identity) => identity,
            Err(_) => {
                return self
                    .cleanup_and_fail(
                        session,
                        claimed.id(),
                        AuthCeremonyFailure::StatusRejected,
                        TrailBaseOrchestrationError::StatusRejected,
                        correlation_id,
                        at,
                    )
                    .await;
            }
        };
        let local_authorization = match claimed.purpose() {
            AuthCeremonyPurpose::SignIn if bootstrap_secret.is_none() => self
                .access
                .preauthorize_trailbase_sign_in(PreauthorizeTrailBaseSignInCommand::new(
                    claimed.id(),
                    identity,
                    correlation_id,
                    at,
                )),
            AuthCeremonyPurpose::FirstAdministratorBootstrap if bootstrap_secret.is_some() => self
                .access
                .preauthorize_trailbase_bootstrap(PreauthorizeTrailBaseBootstrapCommand::new(
                    claimed.id(),
                    identity,
                    correlation_id,
                    at,
                )),
            _ => Err(Box::new(fasti_application::FastiProblem::forbidden(
                fasti_application::CapabilityKey::CreateBrowserSession,
                correlation_id,
            ))),
        };
        if local_authorization.is_err() {
            return self
                .cleanup_and_fail(
                    session,
                    claimed.id(),
                    AuthCeremonyFailure::LocalAuthorizationDenied,
                    TrailBaseOrchestrationError::LocalAuthorizationDenied,
                    correlation_id,
                    at,
                )
                .await;
        }
        if self.client.logout(session).await.is_err() {
            self.record_failure(
                claimed.id(),
                AuthCeremonyFailure::LogoutUncertain,
                correlation_id,
                at,
            )?;
            return Err(TrailBaseOrchestrationError::LogoutUncertain);
        }
        let completed = match claimed.purpose() {
            AuthCeremonyPurpose::SignIn => {
                self.access
                    .complete_trailbase_sign_in(CompleteTrailBaseSignInCommand::new(
                        PreauthorizeTrailBaseSignInCommand::new(
                            claimed.id(),
                            identity,
                            correlation_id,
                            at,
                        ),
                    ))
            }
            AuthCeremonyPurpose::FirstAdministratorBootstrap => self
                .access
                .complete_trailbase_bootstrap(CompleteTrailBaseBootstrapCommand::new(
                    PreauthorizeTrailBaseBootstrapCommand::new(
                        claimed.id(),
                        identity,
                        correlation_id,
                        at,
                    ),
                    bootstrap_secret.ok_or(TrailBaseOrchestrationError::LocalState)?,
                )),
            AuthCeremonyPurpose::RecentAuthentication => {
                return Err(TrailBaseOrchestrationError::LocalAuthorizationDenied);
            }
        };
        match completed {
            Ok(created) => Ok(created),
            Err(_) => {
                self.record_failure(
                    claimed.id(),
                    AuthCeremonyFailure::LocalAuthorizationDenied,
                    correlation_id,
                    at,
                )?;
                Err(TrailBaseOrchestrationError::LocalAuthorizationDenied)
            }
        }
    }

    async fn cleanup_and_fail(
        &self,
        session: TrailBaseSession,
        operation_id: OperationId,
        failure: AuthCeremonyFailure,
        result: TrailBaseOrchestrationError,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> Result<CreatedBrowserSession, TrailBaseOrchestrationError> {
        if self.client.logout(session).await.is_err() {
            self.record_failure(
                operation_id,
                AuthCeremonyFailure::LogoutUncertain,
                correlation_id,
                at,
            )?;
            return Err(TrailBaseOrchestrationError::LogoutUncertain);
        }
        self.record_failure(operation_id, failure, correlation_id, at)?;
        Err(result)
    }

    fn record_failure(
        &self,
        operation_id: OperationId,
        failure: AuthCeremonyFailure,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> Result<(), TrailBaseOrchestrationError> {
        self.access
            .fail_auth_ceremony(FailAuthCeremonyCommand::new(
                operation_id,
                failure,
                correlation_id,
                at,
            ))
            .map(|_| ())
            .map_err(|_| TrailBaseOrchestrationError::LocalState)
    }
}

impl TrailBaseClient {
    pub(super) fn production() -> Result<Self, &'static str> {
        let address = PRODUCTION_TRAILBASE_ADDR
            .parse::<SocketAddr>()
            .map_err(|_| "The fixed TrailBase address is invalid.")?;
        Self::from_loopback(address)
    }

    fn from_loopback(address: SocketAddr) -> Result<Self, &'static str> {
        if !address.ip().is_loopback() || address.port() == 0 {
            return Err("The TrailBase address must use numeric loopback.");
        }
        let host = address.ip().to_string();
        let client =
            pinned_client_with_timeouts(&host, &[address], CONNECT_TIMEOUT, TOTAL_TIMEOUT)?;
        Ok(Self {
            client,
            origin: format!("http://{address}"),
            requests: Arc::new(Semaphore::new(REQUEST_CONCURRENCY_LIMIT)),
        })
    }

    async fn acquire(&self) -> Result<OwnedSemaphorePermit, TrailBaseFailure> {
        tokio::time::timeout(TOTAL_TIMEOUT, Arc::clone(&self.requests).acquire_owned())
            .await
            .map_err(|_| TrailBaseFailure::ExchangeOutcomeUncertain)?
            .map_err(|_| TrailBaseFailure::ExchangeOutcomeUncertain)
    }

    pub(super) async fn exchange(
        &self,
        authorization_code: &AuthorizationCode,
        verifier: &PkceVerifier,
    ) -> Result<TrailBaseSession, TrailBaseFailure> {
        let _permit = self.acquire().await?;
        let body = bounded_json(&TokenRequest {
            authorization_code: authorization_code.expose(),
            pkce_code_verifier: verifier.expose(),
        })
        .map_err(|_| TrailBaseFailure::ExchangeFailed)?;
        let response = self
            .client
            .post(self.url(TOKEN_PATH))
            .header(ACCEPT, "application/json")
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|_| TrailBaseFailure::ExchangeOutcomeUncertain)?;
        if response.status() != StatusCode::OK {
            return Err(match response.status() {
                StatusCode::BAD_REQUEST
                | StatusCode::NOT_FOUND
                | StatusCode::UNSUPPORTED_MEDIA_TYPE
                | StatusCode::UNPROCESSABLE_ENTITY => TrailBaseFailure::ExchangeFailed,
                _ => TrailBaseFailure::ExchangeOutcomeUncertain,
            });
        }
        let response = decode_json::<TokenResponse>(response)
            .await
            .map_err(|_| TrailBaseFailure::ExchangeOutcomeUncertain)?;
        TrailBaseSession::try_from(response).ok_or(TrailBaseFailure::ExchangeOutcomeUncertain)
    }

    pub(super) async fn status(
        &self,
        session: &mut TrailBaseSession,
        instance_id: TrailBaseInstanceId,
        activation_generation: u64,
        now: DateTime<Utc>,
    ) -> Result<ConfirmedTrailBaseIdentity, TrailBaseFailure> {
        let _permit = self
            .acquire()
            .await
            .map_err(|_| TrailBaseFailure::StatusRejected)?;
        let response = self
            .client
            .get(self.url(STATUS_PATH))
            .header(ACCEPT, "application/json")
            .header(
                AUTHORIZATION,
                format!("Bearer {}", session.auth_token.expose()),
            )
            .header(REFRESH_TOKEN_HEADER, session.refresh_token.expose())
            .send()
            .await
            .map_err(|_| TrailBaseFailure::StatusRejected)?;
        if response.status() != StatusCode::OK {
            return Err(TrailBaseFailure::StatusRejected);
        }
        let response = decode_json::<StatusResponse>(response)
            .await
            .map_err(|_| TrailBaseFailure::StatusRejected)?;
        let (Some(auth_token), Some(refresh_token), Some(csrf_token)) = (
            response.auth_token,
            response.refresh_token,
            response.csrf_token,
        ) else {
            return Err(TrailBaseFailure::StatusRejected);
        };
        let refreshed = TokenResponse {
            auth_token,
            refresh_token,
            csrf_token,
        };
        let refreshed =
            TrailBaseSession::try_from(refreshed).ok_or(TrailBaseFailure::StatusRejected)?;
        if !session
            .refresh_token
            .constant_time_eq(&refreshed.refresh_token)
        {
            return Err(TrailBaseFailure::StatusRejected);
        }
        let claims = decode_status_claims(refreshed.auth_token.expose(), now)
            .ok_or(TrailBaseFailure::StatusRejected)?;
        if !refreshed
            .csrf_token
            .constant_time_eq_str(&claims.csrf_token)
        {
            return Err(TrailBaseFailure::StatusRejected);
        }
        let subject = decode_subject(&claims.sub).ok_or(TrailBaseFailure::StatusRejected)?;
        let method =
            authentication_method(claims.provider).ok_or(TrailBaseFailure::StatusRejected)?;
        let Some(email) = claims.email.as_deref() else {
            return Err(TrailBaseFailure::StatusRejected);
        };
        if email.trim().is_empty() {
            return Err(TrailBaseFailure::StatusRejected);
        }
        let _ignored_enrollment_metadata = (claims.admin, claims.mfa, claims.username);
        *session = refreshed;
        Ok(ConfirmedTrailBaseIdentity::new(
            instance_id,
            subject,
            AuthenticationProvenance::new(method, now, activation_generation),
        ))
    }

    pub(super) async fn logout(&self, session: TrailBaseSession) -> Result<(), TrailBaseFailure> {
        let _permit = self
            .acquire()
            .await
            .map_err(|_| TrailBaseFailure::LogoutUncertain)?;
        let body = bounded_json(&LogoutRequest {
            refresh_token: session.refresh_token.expose(),
        })
        .map_err(|_| TrailBaseFailure::LogoutUncertain)?;
        let response = self
            .client
            .post(self.url(LOGOUT_PATH))
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|_| TrailBaseFailure::LogoutUncertain)?;
        if response.status() != StatusCode::OK {
            return Err(TrailBaseFailure::LogoutUncertain);
        }
        Ok(())
    }

    fn url(&self, path: &str) -> String {
        debug_assert!(matches!(path, TOKEN_PATH | STATUS_PATH | LOGOUT_PATH));
        format!("{}{path}", self.origin)
    }

    fn authorization_url(&self, pkce_challenge: &str) -> Result<String, ()> {
        let mut url = reqwest::Url::parse(&format!("{}{AUTHORIZATION_UI_PATH}", self.origin))
            .map_err(|_| ())?;
        url.query_pairs_mut()
            .append_pair("redirect_uri", CALLBACK_URL)
            .append_pair("response_type", "code")
            .append_pair("pkce_code_challenge", pkce_challenge);
        Ok(url.into())
    }
}

impl AuthorizationCode {
    pub(super) fn parse(value: String) -> Result<Self, &'static str> {
        if value.len() != 48 || !value.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            return Err("The TrailBase authorization code is invalid.");
        }
        Ok(Self(Zeroizing::new(value)))
    }

    fn expose(&self) -> &str {
        self.0.as_str()
    }
}

impl PkceVerifier {
    pub(super) fn generate() -> Result<Self, &'static str> {
        let mut bytes = Zeroizing::new([0_u8; 32]);
        getrandom::fill(bytes.as_mut()).map_err(|_| "The operating system CSPRNG failed.")?;
        Ok(Self(Zeroizing::new(URL_SAFE_NO_PAD.encode(bytes.as_ref()))))
    }

    pub(super) fn challenge(&self) -> String {
        use sha2::{Digest, Sha256};
        URL_SAFE_NO_PAD.encode(Sha256::digest(self.expose().as_bytes()))
    }

    fn expose(&self) -> &str {
        self.0.as_str()
    }
}

impl PkceVault {
    pub(super) fn reserve(&self) -> Result<PkceReservation, TrailBaseOrchestrationError> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| TrailBaseOrchestrationError::LocalState)?;
        if state.reserved.saturating_add(state.verifiers.len()) >= PKCE_VAULT_LIMIT {
            return Err(TrailBaseOrchestrationError::ApplicationProblem(
                ProblemCode::CapacityExceeded,
            ));
        }
        state.reserved += 1;
        drop(state);
        Ok(PkceReservation {
            vault: Arc::clone(&self.inner),
            active: true,
        })
    }

    pub(super) fn take(&self, operation_id: OperationId) -> Option<PkceVerifier> {
        self.inner.lock().ok()?.verifiers.remove(&operation_id)
    }

    pub(super) fn remove(&self, operation_id: OperationId) -> bool {
        self.take(operation_id).is_some()
    }

    #[cfg(test)]
    fn live_len(&self) -> usize {
        self.inner.lock().expect("PKCE vault").verifiers.len()
    }
}

impl PkceReservation {
    pub(super) fn commit(
        mut self,
        operation_id: OperationId,
        verifier: PkceVerifier,
    ) -> Result<(), &'static str> {
        let mut state = self
            .vault
            .lock()
            .map_err(|_| "The PKCE verifier vault is unavailable.")?;
        if state.verifiers.contains_key(&operation_id) {
            return Err("The PKCE ceremony already exists.");
        }
        state.reserved = state
            .reserved
            .checked_sub(1)
            .ok_or("The PKCE verifier reservation is invalid.")?;
        state.verifiers.insert(operation_id, verifier);
        self.active = false;
        Ok(())
    }
}

impl Drop for PkceReservation {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        if let Ok(mut state) = self.vault.lock() {
            state.reserved = state.reserved.saturating_sub(1);
        }
    }
}

impl VendorSecret {
    fn parse(value: String, expected_length: Option<usize>) -> Option<Self> {
        if value.is_empty()
            || value.len() > COMPACT_TOKEN_LIMIT
            || expected_length.is_some_and(|length| value.len() != length)
            || value.bytes().any(|byte| byte.is_ascii_control())
        {
            return None;
        }
        Some(Self(Zeroizing::new(value)))
    }

    fn expose(&self) -> &str {
        self.0.as_str()
    }

    fn constant_time_eq(&self, other: &Self) -> bool {
        constant_time_eq(self.expose().as_bytes(), other.expose().as_bytes())
    }

    fn constant_time_eq_str(&self, other: &str) -> bool {
        constant_time_eq(self.expose().as_bytes(), other.as_bytes())
    }
}

impl TrailBaseSession {
    fn try_from(response: TokenResponse) -> Option<Self> {
        Some(Self {
            auth_token: VendorSecret::parse(response.auth_token, None)?,
            refresh_token: VendorSecret::parse(response.refresh_token, Some(86))?,
            csrf_token: VendorSecret::parse(response.csrf_token, Some(20))?,
        })
    }
}

fn bounded_json<T: Serialize>(value: &T) -> Result<Vec<u8>, ()> {
    let body = serde_json::to_vec(value).map_err(|_| ())?;
    (body.len() <= REQUEST_LIMIT).then_some(body).ok_or(())
}

async fn decode_json<T: DeserializeOwned>(response: Response) -> Result<T, ()> {
    if response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        != Some("application/json")
    {
        return Err(());
    }
    let body = bounded_body(response, RESPONSE_LIMIT)
        .await
        .map_err(|_| ())?;
    serde_json::from_slice(&body).map_err(|_| ())
}

fn decode_status_claims(token: &str, now: DateTime<Utc>) -> Option<StatusTokenClaims> {
    if token.len() > COMPACT_TOKEN_LIMIT {
        return None;
    }
    let mut segments = token.split('.');
    let header = segments.next()?;
    let payload = segments.next()?;
    let signature = segments.next()?;
    if header.is_empty() || payload.is_empty() || signature.is_empty() || segments.next().is_some()
    {
        return None;
    }
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    if decoded.len() > TOKEN_PAYLOAD_LIMIT || URL_SAFE_NO_PAD.encode(&decoded) != payload {
        return None;
    }
    let claims = serde_json::from_slice::<StatusTokenClaims>(&decoded).ok()?;
    if claims.token_type != 1 {
        return None;
    }
    let issued_at = DateTime::from_timestamp(claims.iat, 0)?;
    let expires_at = DateTime::from_timestamp(claims.exp, 0)?;
    if issued_at > now + MAX_FUTURE_SKEW
        || expires_at <= now
        || expires_at <= issued_at
        || expires_at - issued_at > MAX_TOKEN_LIFETIME
    {
        return None;
    }
    Some(claims)
}

fn decode_subject(value: &str) -> Option<TrailBaseSubject> {
    let decoded = URL_SAFE.decode(value).ok()?;
    let bytes = <[u8; 16]>::try_from(decoded).ok()?;
    (URL_SAFE.encode(bytes) == value).then_some(TrailBaseSubject::from_bytes(bytes))
}

fn authentication_method(provider: u8) -> Option<AuthenticationMethod> {
    match provider {
        0 => Some(AuthenticationMethod::TrailBasePassword),
        1 | 2 | 9..=17 => Some(AuthenticationMethod::TrailBaseSocial),
        _ => None,
    }
}

fn sha256_digest(value: &[u8; 32]) -> Sha256Digest {
    use sha2::{Digest, Sha256};
    let bytes: [u8; 32] = Sha256::digest(value).into();
    Sha256Digest::from_bytes(&bytes)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        extract::{Request, State},
        http::{header::HeaderName, HeaderValue},
        routing::{get, post},
        Router,
    };
    use fasti_application::{
        AccessAdministrationPort, EnrollFirstClientCommand, InitializeNodeCommand,
        VerifyTrailBaseInstallationCommand,
    };
    use fasti_store::SqliteKernel;
    use serde_json::json;
    use tokio::net::TcpListener;

    static_assertions::assert_not_impl_any!(
        AuthorizationCode: Clone, std::fmt::Debug, Serialize
    );
    static_assertions::assert_not_impl_any!(PkceVerifier: Clone, std::fmt::Debug, Serialize);
    static_assertions::assert_not_impl_any!(PkceReservation: Clone, std::fmt::Debug, Serialize);
    static_assertions::assert_not_impl_any!(VendorSecret: Clone, std::fmt::Debug, Serialize);

    #[derive(Clone, Default)]
    struct FixtureState {
        requests: Arc<Mutex<Vec<(String, String, String)>>>,
    }

    async fn spawn_fixture(router: Router) -> (TrailBaseClient, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("fixture bind");
        let address = listener.local_addr().expect("fixture address");
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.expect("fixture server");
        });
        (
            TrailBaseClient::from_loopback(address).expect("test TrailBase client"),
            task,
        )
    }

    fn jwt(provider: u8, email: Option<&str>, csrf: &str, now: DateTime<Utc>) -> String {
        compact_jwt(json!({
            "sub": URL_SAFE.encode([7_u8; 16]),
            "iat": now.timestamp(),
            "exp": (now + TimeDelta::hours(1)).timestamp(),
            "type": 1,
            "admin": true,
            "mfa": true,
            "provider": provider,
            "email": email,
            "username": "ryan",
            "csrf_token": csrf,
        }))
    }

    fn compact_jwt(claims: serde_json::Value) -> String {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"EdDSA"}"#);
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).expect("claims"));
        format!("{header}.{payload}.signature")
    }

    fn token_response(auth_token: impl Into<String>) -> serde_json::Value {
        json!({
            "auth_token": auth_token.into(),
            "refresh_token": "r".repeat(86),
            "csrf_token": "c".repeat(20),
        })
    }

    fn initialized_access_node() -> (
        tempfile::TempDir,
        Arc<SqliteKernel>,
        fasti_domain::TrailBaseInstallation,
        fasti_application::RequestAccessContext,
    ) {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = Arc::new(SqliteKernel::open(root.path()).expect("kernel"));
        let initialized = kernel
            .initialize_node(InitializeNodeCommand::new(RequestCorrelationId::new_v7()))
            .expect("initialize node");
        let enrolled = kernel
            .enroll_first_client(EnrollFirstClientCommand::new(
                RequestCorrelationId::new_v7(),
                SecretMaterial::from_bytes(*initialized.initialization_proof().expose_bytes()),
            ))
            .expect("enroll client");
        let installation = HumanAccessPort::verify_trailbase_installation(
            kernel.as_ref(),
            VerifyTrailBaseInstallationCommand::new(
                TrailBaseInstanceId::new_v7(),
                true,
                false,
                RequestCorrelationId::new_v7(),
                DateTime::parse_from_rfc3339("2026-08-30T00:00:00Z")
                    .expect("time")
                    .with_timezone(&Utc),
            ),
        )
        .expect("activate TrailBase");
        let access = *enrolled.access();
        (root, kernel, installation, access)
    }

    fn test_orchestrator(
        client: TrailBaseClient,
        kernel: &Arc<SqliteKernel>,
        installation: &TrailBaseInstallation,
    ) -> TrailBaseOrchestrator {
        TrailBaseOrchestrator {
            client,
            vault: PkceVault::default(),
            access: kernel.clone(),
            instance_id: installation.id(),
            activation_generation: installation.activation_generation(),
        }
    }

    fn start_bootstrap_ceremony(
        orchestrator: &TrailBaseOrchestrator,
        kernel: &SqliteKernel,
        access: fasti_application::RequestAccessContext,
        callback_at: DateTime<Utc>,
    ) -> StartedTrailBaseCeremony {
        orchestrator
            .start_bootstrap(
                AuthCeremonySelection::try_new(
                    AuthCeremonyPurpose::FirstAdministratorBootstrap,
                    access.workspace_id(),
                    access.grant_id(),
                    None,
                    None,
                    false,
                )
                .expect("selection"),
                kernel.ensure_bootstrap_secret().expect("bootstrap secret"),
                RequestCorrelationId::new_v7(),
                callback_at - TimeDelta::minutes(1),
                callback_at + TimeDelta::minutes(8),
            )
            .expect("start bootstrap")
    }

    #[tokio::test]
    async fn exact_exchange_status_logout_flow_returns_only_confirmed_identity() {
        let now = Utc::now();
        let state = FixtureState::default();
        let router = Router::new()
            .route(
                TOKEN_PATH,
                post(
                    |State(state): State<FixtureState>, request: Request| async move {
                        assert_eq!(
                            request.headers().get(ACCEPT),
                            Some(&HeaderValue::from_static("application/json"))
                        );
                        assert_eq!(
                            request.headers().get(CONTENT_TYPE),
                            Some(&HeaderValue::from_static("application/json"))
                        );
                        state.requests.lock().expect("requests").push((
                            "token".to_owned(),
                            request.method().to_string(),
                            request.uri().path().to_owned(),
                        ));
                        let body = axum::body::to_bytes(request.into_body(), REQUEST_LIMIT)
                            .await
                            .expect("token body");
                        let body: serde_json::Value =
                            serde_json::from_slice(&body).expect("token JSON");
                        assert_eq!(body.as_object().expect("token object").len(), 2);
                        assert_eq!(
                            body.get("authorization_code")
                                .and_then(|value| value.as_str()),
                            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                        );
                        assert_eq!(
                            body.get("pkce_code_verifier")
                                .and_then(|value| value.as_str())
                                .map(str::len),
                            Some(43)
                        );
                        axum::Json(token_response("exchange.token.signature"))
                    },
                ),
            )
            .route(
                STATUS_PATH,
                get(move |State(state): State<FixtureState>, request: Request| {
                    let claims = jwt(0, Some("person@example.test"), &"c".repeat(20), now);
                    async move {
                        assert_eq!(
                            request.headers().get(AUTHORIZATION),
                            Some(&HeaderValue::from_static("Bearer exchange.token.signature"))
                        );
                        assert_eq!(
                            request
                                .headers()
                                .get(HeaderName::from_static("refresh-token")),
                            Some(&HeaderValue::from_str(&"r".repeat(86)).expect("header"))
                        );
                        state.requests.lock().expect("requests").push((
                            "status".to_owned(),
                            request.method().to_string(),
                            request.uri().path().to_owned(),
                        ));
                        axum::Json(token_response(claims))
                    }
                }),
            )
            .route(
                LOGOUT_PATH,
                post(
                    |State(state): State<FixtureState>, request: Request| async move {
                        assert_eq!(
                            request.headers().get(CONTENT_TYPE),
                            Some(&HeaderValue::from_static("application/json"))
                        );
                        state.requests.lock().expect("requests").push((
                            "logout".to_owned(),
                            request.method().to_string(),
                            request.uri().path().to_owned(),
                        ));
                        let body = axum::body::to_bytes(request.into_body(), REQUEST_LIMIT)
                            .await
                            .expect("logout body");
                        let body: serde_json::Value =
                            serde_json::from_slice(&body).expect("logout JSON");
                        assert_eq!(body.as_object().expect("logout object").len(), 1);
                        assert_eq!(
                            body.get("refresh_token").and_then(|value| value.as_str()),
                            Some("r".repeat(86).as_str())
                        );
                        StatusCode::OK
                    },
                ),
            )
            .with_state(state.clone());
        let (client, task) = spawn_fixture(router).await;
        let code = AuthorizationCode::parse("a".repeat(48)).expect("authorization code");
        let verifier = PkceVerifier::generate().expect("verifier");
        assert_eq!(verifier.expose().len(), 43);
        assert_eq!(verifier.challenge().len(), 43);
        let mut session = client.exchange(&code, &verifier).await.expect("exchange");
        let instance_id = TrailBaseInstanceId::new_v7();
        let identity = client
            .status(&mut session, instance_id, 7, now)
            .await
            .expect("status");
        assert_eq!(identity.instance_id(), instance_id);
        assert_eq!(identity.subject(), TrailBaseSubject::from_bytes([7; 16]));
        assert_eq!(
            identity.provenance().method(),
            AuthenticationMethod::TrailBasePassword
        );
        client.logout(session).await.expect("logout");
        assert_eq!(
            state.requests.lock().expect("requests").as_slice(),
            [
                ("token".to_owned(), "POST".to_owned(), TOKEN_PATH.to_owned()),
                (
                    "status".to_owned(),
                    "GET".to_owned(),
                    STATUS_PATH.to_owned()
                ),
                (
                    "logout".to_owned(),
                    "POST".to_owned(),
                    LOGOUT_PATH.to_owned()
                ),
            ]
        );
        task.abort();
    }

    #[tokio::test]
    async fn callback_has_one_exchange_winner_and_completes_only_after_logout() {
        let (_root, kernel, installation, access) = initialized_access_node();
        let callback_at = DateTime::parse_from_rfc3339("2026-08-30T00:02:00Z")
            .expect("time")
            .with_timezone(&Utc);
        let state = FixtureState::default();
        let router = Router::new()
            .route(
                TOKEN_PATH,
                post(
                    |State(state): State<FixtureState>, request: Request| async move {
                        state.requests.lock().expect("requests").push((
                            "token".to_owned(),
                            request.method().to_string(),
                            request.uri().path().to_owned(),
                        ));
                        axum::Json(token_response("exchange.token.signature"))
                    },
                ),
            )
            .route(
                STATUS_PATH,
                get(move |State(state): State<FixtureState>| {
                    let claims = jwt(0, Some("person@example.test"), &"c".repeat(20), callback_at);
                    async move {
                        state.requests.lock().expect("requests").push((
                            "status".to_owned(),
                            "GET".to_owned(),
                            STATUS_PATH.to_owned(),
                        ));
                        axum::Json(token_response(claims))
                    }
                }),
            )
            .route(
                LOGOUT_PATH,
                post(|State(state): State<FixtureState>| async move {
                    state.requests.lock().expect("requests").push((
                        "logout".to_owned(),
                        "POST".to_owned(),
                        LOGOUT_PATH.to_owned(),
                    ));
                    StatusCode::OK
                }),
            )
            .with_state(state.clone());
        let (client, task) = spawn_fixture(router).await;
        let orchestrator = Arc::new(test_orchestrator(client, &kernel, &installation));
        let started = start_bootstrap_ceremony(&orchestrator, &kernel, access, callback_at);
        let authorization_url = reqwest::Url::parse(&started.authorization_url)
            .expect("fixed TrailBase authorization URL");
        assert_eq!(authorization_url.path(), AUTHORIZATION_UI_PATH);
        let query: HashMap<_, _> = authorization_url.query_pairs().into_owned().collect();
        assert_eq!(
            query.get("redirect_uri").map(String::as_str),
            Some(CALLBACK_URL)
        );
        assert_eq!(query.get("response_type").map(String::as_str), Some("code"));
        assert_eq!(query.get("pkce_code_challenge").map(String::len), Some(43));
        assert_eq!(started.expires_at, callback_at + TimeDelta::minutes(8));
        assert!(started.operation_id.to_string().starts_with("op_"));
        let binding = Zeroizing::new(*started.browser_binding.expose_bytes());
        let first_binding = started.browser_binding;
        let second_binding = SecretMaterial::from_bytes(*binding);
        let first = Arc::clone(&orchestrator);
        let second = Arc::clone(&orchestrator);
        let (first_result, second_result) = tokio::join!(
            first.callback(
                "a".repeat(48),
                first_binding,
                Some(kernel.ensure_bootstrap_secret().expect("bootstrap secret")),
                RequestCorrelationId::new_v7(),
                callback_at,
            ),
            second.callback(
                "a".repeat(48),
                second_binding,
                Some(kernel.ensure_bootstrap_secret().expect("bootstrap secret")),
                RequestCorrelationId::new_v7(),
                callback_at,
            ),
        );
        assert_eq!(
            usize::from(first_result.is_ok()) + usize::from(second_result.is_ok()),
            1
        );
        assert_eq!(
            state.requests.lock().expect("requests").as_slice(),
            [
                ("token".to_owned(), "POST".to_owned(), TOKEN_PATH.to_owned()),
                (
                    "status".to_owned(),
                    "GET".to_owned(),
                    STATUS_PATH.to_owned()
                ),
                (
                    "logout".to_owned(),
                    "POST".to_owned(),
                    LOGOUT_PATH.to_owned()
                ),
            ]
        );
        task.abort();
    }

    #[tokio::test]
    async fn local_authorization_denial_logs_out_once_and_replay_makes_no_remote_call() {
        let (_root, kernel, installation, access) = initialized_access_node();
        let callback_at = DateTime::parse_from_rfc3339("2026-08-30T00:02:00Z")
            .expect("time")
            .with_timezone(&Utc);
        let state = FixtureState::default();
        let router = Router::new()
            .route(
                TOKEN_PATH,
                post(|State(state): State<FixtureState>| async move {
                    state.requests.lock().expect("requests").push((
                        "token".to_owned(),
                        "POST".to_owned(),
                        TOKEN_PATH.to_owned(),
                    ));
                    axum::Json(token_response("exchange.token.signature"))
                }),
            )
            .route(
                STATUS_PATH,
                get(move |State(state): State<FixtureState>| {
                    let claims = jwt(0, Some("person@example.test"), &"c".repeat(20), callback_at);
                    async move {
                        state.requests.lock().expect("requests").push((
                            "status".to_owned(),
                            "GET".to_owned(),
                            STATUS_PATH.to_owned(),
                        ));
                        axum::Json(token_response(claims))
                    }
                }),
            )
            .route(
                LOGOUT_PATH,
                post(|State(state): State<FixtureState>| async move {
                    state.requests.lock().expect("requests").push((
                        "logout".to_owned(),
                        "POST".to_owned(),
                        LOGOUT_PATH.to_owned(),
                    ));
                    StatusCode::OK
                }),
            )
            .with_state(state.clone());
        let (client, task) = spawn_fixture(router).await;
        let access_port: Arc<dyn HumanAccessPort> = kernel.clone();
        let orchestrator = TrailBaseOrchestrator {
            client,
            vault: PkceVault::default(),
            access: access_port,
            instance_id: installation.id(),
            activation_generation: installation.activation_generation(),
        };
        let started = orchestrator
            .start_sign_in(
                AuthCeremonySelection::try_new(
                    AuthCeremonyPurpose::SignIn,
                    access.workspace_id(),
                    access.grant_id(),
                    None,
                    None,
                    false,
                )
                .expect("selection"),
                RequestCorrelationId::new_v7(),
                callback_at - TimeDelta::minutes(1),
                callback_at + TimeDelta::minutes(8),
            )
            .expect("start sign-in");
        let binding = Zeroizing::new(*started.browser_binding.expose_bytes());
        let replay_binding = SecretMaterial::from_bytes(*binding);
        let result = orchestrator
            .callback(
                "a".repeat(48),
                started.browser_binding,
                None,
                RequestCorrelationId::new_v7(),
                callback_at,
            )
            .await;
        assert!(matches!(
            result,
            Err(TrailBaseOrchestrationError::LocalAuthorizationDenied)
        ));
        let replay = orchestrator
            .callback(
                "a".repeat(48),
                replay_binding,
                None,
                RequestCorrelationId::new_v7(),
                callback_at,
            )
            .await;
        assert!(matches!(
            replay,
            Err(TrailBaseOrchestrationError::LocalState)
        ));
        assert_eq!(
            state.requests.lock().expect("requests").as_slice(),
            [
                ("token".to_owned(), "POST".to_owned(), TOKEN_PATH.to_owned()),
                (
                    "status".to_owned(),
                    "GET".to_owned(),
                    STATUS_PATH.to_owned()
                ),
                (
                    "logout".to_owned(),
                    "POST".to_owned(),
                    LOGOUT_PATH.to_owned()
                ),
            ]
        );
        task.abort();
    }

    #[tokio::test]
    async fn token_server_error_is_durably_uncertain_and_replay_makes_no_remote_call() {
        let (_root, kernel, installation, access) = initialized_access_node();
        let callback_at = DateTime::parse_from_rfc3339("2026-08-30T00:02:00Z")
            .expect("time")
            .with_timezone(&Utc);
        let state = FixtureState::default();
        let router = Router::new()
            .route(
                TOKEN_PATH,
                post(|State(state): State<FixtureState>| async move {
                    state.requests.lock().expect("requests").push((
                        "token".to_owned(),
                        "POST".to_owned(),
                        TOKEN_PATH.to_owned(),
                    ));
                    StatusCode::INTERNAL_SERVER_ERROR
                }),
            )
            .with_state(state.clone());
        let (client, task) = spawn_fixture(router).await;
        let orchestrator = test_orchestrator(client, &kernel, &installation);
        let started = start_bootstrap_ceremony(&orchestrator, &kernel, access, callback_at);
        let binding = Zeroizing::new(*started.browser_binding.expose_bytes());
        let result = orchestrator
            .callback(
                "a".repeat(48),
                started.browser_binding,
                Some(kernel.ensure_bootstrap_secret().expect("bootstrap secret")),
                RequestCorrelationId::new_v7(),
                callback_at,
            )
            .await;
        assert_eq!(
            result.err(),
            Some(TrailBaseOrchestrationError::ExchangeOutcomeUncertain)
        );
        let replay = orchestrator
            .callback(
                "a".repeat(48),
                SecretMaterial::from_bytes(*binding),
                Some(kernel.ensure_bootstrap_secret().expect("bootstrap secret")),
                RequestCorrelationId::new_v7(),
                callback_at,
            )
            .await;
        assert_eq!(replay.err(), Some(TrailBaseOrchestrationError::LocalState));
        assert_eq!(
            state.requests.lock().expect("requests").as_slice(),
            [("token".to_owned(), "POST".to_owned(), TOKEN_PATH.to_owned())]
        );
        task.abort();
    }

    #[tokio::test]
    async fn logout_failure_is_durably_uncertain_and_attempted_exactly_once() {
        let (_root, kernel, installation, access) = initialized_access_node();
        let callback_at = DateTime::parse_from_rfc3339("2026-08-30T00:02:00Z")
            .expect("time")
            .with_timezone(&Utc);
        let state = FixtureState::default();
        let router = Router::new()
            .route(
                TOKEN_PATH,
                post(|State(state): State<FixtureState>| async move {
                    state.requests.lock().expect("requests").push((
                        "token".to_owned(),
                        "POST".to_owned(),
                        TOKEN_PATH.to_owned(),
                    ));
                    axum::Json(token_response("exchange.token.signature"))
                }),
            )
            .route(
                STATUS_PATH,
                get(move |State(state): State<FixtureState>| {
                    let claims = jwt(0, Some("person@example.test"), &"c".repeat(20), callback_at);
                    async move {
                        state.requests.lock().expect("requests").push((
                            "status".to_owned(),
                            "GET".to_owned(),
                            STATUS_PATH.to_owned(),
                        ));
                        axum::Json(token_response(claims))
                    }
                }),
            )
            .route(
                LOGOUT_PATH,
                post(|State(state): State<FixtureState>| async move {
                    state.requests.lock().expect("requests").push((
                        "logout".to_owned(),
                        "POST".to_owned(),
                        LOGOUT_PATH.to_owned(),
                    ));
                    StatusCode::INTERNAL_SERVER_ERROR
                }),
            )
            .with_state(state.clone());
        let (client, task) = spawn_fixture(router).await;
        let orchestrator = test_orchestrator(client, &kernel, &installation);
        let started = start_bootstrap_ceremony(&orchestrator, &kernel, access, callback_at);
        let binding = Zeroizing::new(*started.browser_binding.expose_bytes());
        let result = orchestrator
            .callback(
                "a".repeat(48),
                started.browser_binding,
                Some(kernel.ensure_bootstrap_secret().expect("bootstrap secret")),
                RequestCorrelationId::new_v7(),
                callback_at,
            )
            .await;
        assert_eq!(
            result.err(),
            Some(TrailBaseOrchestrationError::LogoutUncertain)
        );
        let replay = orchestrator
            .callback(
                "a".repeat(48),
                SecretMaterial::from_bytes(*binding),
                Some(kernel.ensure_bootstrap_secret().expect("bootstrap secret")),
                RequestCorrelationId::new_v7(),
                callback_at,
            )
            .await;
        assert_eq!(replay.err(), Some(TrailBaseOrchestrationError::LocalState));
        assert_eq!(
            state.requests.lock().expect("requests").as_slice(),
            [
                ("token".to_owned(), "POST".to_owned(), TOKEN_PATH.to_owned()),
                (
                    "status".to_owned(),
                    "GET".to_owned(),
                    STATUS_PATH.to_owned()
                ),
                (
                    "logout".to_owned(),
                    "POST".to_owned(),
                    LOGOUT_PATH.to_owned()
                ),
            ]
        );
        task.abort();
    }

    #[tokio::test]
    async fn status_rejection_logs_out_exactly_once_before_failing() {
        let (_root, kernel, installation, access) = initialized_access_node();
        let callback_at = DateTime::parse_from_rfc3339("2026-08-30T00:02:00Z")
            .expect("time")
            .with_timezone(&Utc);
        let state = FixtureState::default();
        let router = Router::new()
            .route(
                TOKEN_PATH,
                post(|State(state): State<FixtureState>| async move {
                    state.requests.lock().expect("requests").push((
                        "token".to_owned(),
                        "POST".to_owned(),
                        TOKEN_PATH.to_owned(),
                    ));
                    axum::Json(token_response("exchange.token.signature"))
                }),
            )
            .route(
                STATUS_PATH,
                get(|State(state): State<FixtureState>| async move {
                    state.requests.lock().expect("requests").push((
                        "status".to_owned(),
                        "GET".to_owned(),
                        STATUS_PATH.to_owned(),
                    ));
                    axum::Json(json!({
                        "auth_token": null,
                        "refresh_token": null,
                        "csrf_token": null,
                    }))
                }),
            )
            .route(
                LOGOUT_PATH,
                post(|State(state): State<FixtureState>| async move {
                    state.requests.lock().expect("requests").push((
                        "logout".to_owned(),
                        "POST".to_owned(),
                        LOGOUT_PATH.to_owned(),
                    ));
                    StatusCode::OK
                }),
            )
            .with_state(state.clone());
        let (client, task) = spawn_fixture(router).await;
        let orchestrator = test_orchestrator(client, &kernel, &installation);
        let started = start_bootstrap_ceremony(&orchestrator, &kernel, access, callback_at);
        let result = orchestrator
            .callback(
                "a".repeat(48),
                started.browser_binding,
                Some(kernel.ensure_bootstrap_secret().expect("bootstrap secret")),
                RequestCorrelationId::new_v7(),
                callback_at,
            )
            .await;
        assert_eq!(
            result.err(),
            Some(TrailBaseOrchestrationError::StatusRejected)
        );
        assert_eq!(
            state.requests.lock().expect("requests").as_slice(),
            [
                ("token".to_owned(), "POST".to_owned(), TOKEN_PATH.to_owned()),
                (
                    "status".to_owned(),
                    "GET".to_owned(),
                    STATUS_PATH.to_owned()
                ),
                (
                    "logout".to_owned(),
                    "POST".to_owned(),
                    LOGOUT_PATH.to_owned()
                ),
            ]
        );
        task.abort();
    }

    #[tokio::test]
    async fn successful_exchange_requires_strict_bounded_json() {
        let bodies = [
            (None, token_response("exchange.token.signature").to_string()),
            (Some("application/json"), "{".to_owned()),
            (
                Some("application/json"),
                json!({
                    "auth_token": "exchange.token.signature",
                    "refresh_token": "r".repeat(86),
                    "csrf_token": "c".repeat(20),
                    "extra": true,
                })
                .to_string(),
            ),
            (Some("application/json"), "x".repeat(RESPONSE_LIMIT + 1)),
        ];
        for (content_type, body) in bodies {
            let router = Router::new().route(
                TOKEN_PATH,
                post(move || {
                    let body = body.clone();
                    async move {
                        let mut response =
                            axum::response::Response::builder().status(StatusCode::OK);
                        if let Some(content_type) = content_type {
                            response = response.header(CONTENT_TYPE, content_type);
                        }
                        response
                            .body(axum::body::Body::from(body))
                            .expect("response")
                    }
                }),
            );
            let (client, task) = spawn_fixture(router).await;
            let code = AuthorizationCode::parse("a".repeat(48)).expect("authorization code");
            let verifier = PkceVerifier::generate().expect("verifier");
            assert_eq!(
                client.exchange(&code, &verifier).await.err(),
                Some(TrailBaseFailure::ExchangeOutcomeUncertain)
            );
            task.abort();
        }
    }

    #[tokio::test]
    async fn oversized_chunked_exchange_and_request_timeout_are_uncertain() {
        let chunked = Router::new().route(
            TOKEN_PATH,
            post(|| async {
                let chunks = tokio_stream::iter([
                    Ok::<_, std::convert::Infallible>(vec![b'x'; RESPONSE_LIMIT]),
                    Ok(vec![b'y']),
                ]);
                axum::response::Response::builder()
                    .status(StatusCode::OK)
                    .header(CONTENT_TYPE, "application/json")
                    .body(axum::body::Body::from_stream(chunks))
                    .expect("chunked response")
            }),
        );
        let (client, task) = spawn_fixture(chunked).await;
        let code = AuthorizationCode::parse("a".repeat(48)).expect("authorization code");
        let verifier = PkceVerifier::generate().expect("verifier");
        assert_eq!(
            client.exchange(&code, &verifier).await.err(),
            Some(TrailBaseFailure::ExchangeOutcomeUncertain)
        );
        task.abort();

        let stalled = Router::new().route(
            TOKEN_PATH,
            post(|| async {
                tokio::time::sleep(TOTAL_TIMEOUT + Duration::from_millis(100)).await;
                axum::Json(token_response("exchange.token.signature"))
            }),
        );
        let (client, task) = spawn_fixture(stalled).await;
        let code = AuthorizationCode::parse("a".repeat(48)).expect("authorization code");
        let verifier = PkceVerifier::generate().expect("verifier");
        assert_eq!(
            client.exchange(&code, &verifier).await.err(),
            Some(TrailBaseFailure::ExchangeOutcomeUncertain)
        );
        task.abort();
    }

    #[tokio::test]
    async fn status_rejects_null_fields_unknown_provider_and_redirects() {
        let now = Utc::now();
        for body in [
            json!({"auth_token": null, "refresh_token": null, "csrf_token": null}),
            token_response(jwt(8, Some("person@example.test"), &"c".repeat(20), now)),
            token_response(jwt(0, None, &"c".repeat(20), now)),
            token_response(jwt(0, Some("   "), &"c".repeat(20), now)),
            token_response(jwt(0, Some("person@example.test"), &"d".repeat(20), now)),
            json!({
                "auth_token": jwt(0, Some("person@example.test"), &"c".repeat(20), now),
                "refresh_token": "x".repeat(86),
                "csrf_token": "c".repeat(20),
            }),
        ] {
            let router = Router::new()
                .route(
                    TOKEN_PATH,
                    post(|| async { axum::Json(token_response("exchange.token.signature")) }),
                )
                .route(STATUS_PATH, get(move || async move { axum::Json(body) }));
            let (client, task) = spawn_fixture(router).await;
            let code = AuthorizationCode::parse("a".repeat(48)).expect("authorization code");
            let verifier = PkceVerifier::generate().expect("verifier");
            let mut session = client.exchange(&code, &verifier).await.expect("exchange");
            assert!(matches!(
                client
                    .status(&mut session, TrailBaseInstanceId::new_v7(), 1, now)
                    .await,
                Err(TrailBaseFailure::StatusRejected)
            ));
            task.abort();
        }

        for (status, expected) in [
            (StatusCode::BAD_REQUEST, TrailBaseFailure::ExchangeFailed),
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                TrailBaseFailure::ExchangeOutcomeUncertain,
            ),
            (
                StatusCode::FOUND,
                TrailBaseFailure::ExchangeOutcomeUncertain,
            ),
        ] {
            let router = Router::new().route(TOKEN_PATH, post(move || async move { status }));
            let (client, task) = spawn_fixture(router).await;
            let code = AuthorizationCode::parse("a".repeat(48)).expect("authorization code");
            let verifier = PkceVerifier::generate().expect("verifier");
            assert_eq!(
                client.exchange(&code, &verifier).await.err(),
                Some(expected)
            );
            task.abort();
        }
    }

    #[test]
    fn authorization_code_and_status_claims_are_strictly_bounded() {
        assert!(AuthorizationCode::parse("a".repeat(47)).is_err());
        assert!(AuthorizationCode::parse(format!("{}-", "a".repeat(47))).is_err());
        let now = Utc::now();
        assert!(decode_status_claims(&jwt(17, Some("x@y.z"), &"c".repeat(20), now), now).is_some());
        assert!(decode_status_claims(&jwt(18, Some("x@y.z"), &"c".repeat(20), now), now).is_some());
        assert!(decode_subject(&URL_SAFE.encode([1_u8; 15])).is_none());
        assert_eq!(
            authentication_method(0),
            Some(AuthenticationMethod::TrailBasePassword)
        );
        for provider in [1, 2, 9, 10, 11, 12, 13, 14, 15, 16, 17] {
            assert_eq!(
                authentication_method(provider),
                Some(AuthenticationMethod::TrailBaseSocial)
            );
        }
        for provider in [3, 8, 18, u8::MAX] {
            assert_eq!(authentication_method(provider), None);
        }
        let claims = |token_type: u8, iat: DateTime<Utc>, exp: DateTime<Utc>| {
            compact_jwt(json!({
                "sub": URL_SAFE.encode([7_u8; 16]),
                "iat": iat.timestamp(),
                "exp": exp.timestamp(),
                "type": token_type,
                "provider": 0,
                "email": "person@example.test",
                "csrf_token": "c".repeat(20),
            }))
        };
        assert!(decode_status_claims(&claims(2, now, now + TimeDelta::hours(1)), now).is_none());
        assert!(decode_status_claims(
            &claims(
                1,
                now + MAX_FUTURE_SKEW + TimeDelta::seconds(1),
                now + TimeDelta::hours(1),
            ),
            now,
        )
        .is_none());
        assert!(decode_status_claims(&claims(1, now, now), now).is_none());
        assert!(decode_status_claims(
            &claims(1, now, now + MAX_TOKEN_LIFETIME + TimeDelta::seconds(1)),
            now,
        )
        .is_none());
    }

    #[test]
    fn pkce_vault_reserves_before_generation_and_never_exceeds_sixty_four() {
        let vault = PkceVault::default();
        let mut operations = Vec::new();
        for _ in 0..PKCE_VAULT_LIMIT {
            let reservation = vault.reserve().expect("capacity");
            let operation_id = OperationId::new_v7();
            reservation
                .commit(operation_id, PkceVerifier::generate().expect("verifier"))
                .expect("commit reservation");
            operations.push(operation_id);
        }
        assert_eq!(vault.live_len(), PKCE_VAULT_LIMIT);
        assert_eq!(
            vault.reserve().err(),
            Some(TrailBaseOrchestrationError::ApplicationProblem(
                ProblemCode::CapacityExceeded
            ))
        );
        assert!(vault.remove(operations[0]));
        let reservation = vault.reserve().expect("released capacity");
        drop(reservation);
        assert_eq!(vault.live_len(), PKCE_VAULT_LIMIT - 1);
    }
}
