use crate::{ApplicationResult, CapabilityKey, RequestAccessContext, SecretMaterial};
use chrono::{DateTime, Utc};
use fasti_domain::{BrowserUserId, RequestCorrelationId};

const MIN_PASSWORD_BYTES: usize = 8;
const MAX_PASSWORD_BYTES: usize = 128;
const MIN_SESSION_MINUTES: u32 = 5;
/// The default, general-purpose maximum browser session lifetime (24h).
/// `CreateBrowserSessionCommand::try_new` also accepts a caller-supplied
/// ceiling above the floor for opt-in, non-default deployments (currently
/// only the loopback-gated `FASTI_DEVELOPMENT_AUTO_LOGIN` dev convenience);
/// production callers should pass this constant.
pub const MAX_SESSION_MINUTES: u32 = 24 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrowserAuthInputError;

impl std::fmt::Display for BrowserAuthInputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("browser authentication input is invalid")
    }
}

impl std::error::Error for BrowserAuthInputError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserUsername(String);

impl BrowserUsername {
    pub fn try_new(value: impl Into<String>) -> Result<Self, BrowserAuthInputError> {
        let value = value.into();
        let valid = (3..=64).contains(&value.len())
            && value
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'-' | b'_')
            });
        valid.then_some(Self(value)).ok_or(BrowserAuthInputError)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A bounded password that cannot be logged through `Debug` or cloned.
pub struct BrowserPassword(Vec<u8>);

impl BrowserPassword {
    pub fn try_new(value: impl Into<String>) -> Result<Self, BrowserAuthInputError> {
        let bytes = value.into().into_bytes();
        let valid = (MIN_PASSWORD_BYTES..=MAX_PASSWORD_BYTES).contains(&bytes.len())
            && !bytes.iter().any(u8::is_ascii_control);
        valid.then_some(Self(bytes)).ok_or(BrowserAuthInputError)
    }

    pub fn expose_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for BrowserPassword {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserUserView {
    user_id: BrowserUserId,
    username: String,
    is_admin: bool,
    is_test_account: bool,
    active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl BrowserUserView {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        user_id: BrowserUserId,
        username: String,
        is_admin: bool,
        is_test_account: bool,
        active: bool,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            user_id,
            username,
            is_admin,
            is_test_account,
            active,
            created_at,
            updated_at,
        }
    }

    pub const fn user_id(&self) -> BrowserUserId {
        self.user_id
    }
    pub fn username(&self) -> &str {
        &self.username
    }
    pub const fn is_admin(&self) -> bool {
        self.is_admin
    }
    pub const fn is_test_account(&self) -> bool {
        self.is_test_account
    }
    pub const fn active(&self) -> bool {
        self.active
    }
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub const fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }
}

pub struct CreateBrowserSessionCommand {
    correlation_id: RequestCorrelationId,
    username: BrowserUsername,
    password: BrowserPassword,
    lifetime_minutes: u32,
}

impl CreateBrowserSessionCommand {
    /// `max_lifetime_minutes` is the caller's ceiling for this request; pass
    /// [`MAX_SESSION_MINUTES`] unless a deliberately relaxed, non-default
    /// deployment ceiling applies (see its doc comment).
    pub fn try_new(
        correlation_id: RequestCorrelationId,
        username: BrowserUsername,
        password: BrowserPassword,
        lifetime_minutes: u32,
        max_lifetime_minutes: u32,
    ) -> Result<Self, BrowserAuthInputError> {
        if max_lifetime_minutes < MIN_SESSION_MINUTES
            || !(MIN_SESSION_MINUTES..=max_lifetime_minutes).contains(&lifetime_minutes)
        {
            return Err(BrowserAuthInputError);
        }
        Ok(Self {
            correlation_id,
            username,
            password,
            lifetime_minutes,
        })
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn username(&self) -> &BrowserUsername {
        &self.username
    }
    pub const fn password(&self) -> &BrowserPassword {
        &self.password
    }
    pub const fn lifetime_minutes(&self) -> u32 {
        self.lifetime_minutes
    }
}

pub struct CreatedBrowserSession {
    user: BrowserUserView,
    session: SecretMaterial,
    csrf: SecretMaterial,
    expires_at: DateTime<Utc>,
}

impl CreatedBrowserSession {
    pub fn new(
        user: BrowserUserView,
        session: SecretMaterial,
        csrf: SecretMaterial,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self {
            user,
            session,
            csrf,
            expires_at,
        }
    }
    pub const fn user(&self) -> &BrowserUserView {
        &self.user
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub const fn csrf(&self) -> &SecretMaterial {
        &self.csrf
    }
    pub const fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
}

pub struct AuthenticateBrowserSessionQuery {
    correlation_id: RequestCorrelationId,
    capability: CapabilityKey,
    session: SecretMaterial,
    csrf: Option<SecretMaterial>,
    require_csrf: bool,
}

impl AuthenticateBrowserSessionQuery {
    pub fn new(
        correlation_id: RequestCorrelationId,
        capability: CapabilityKey,
        session: SecretMaterial,
        csrf: Option<SecretMaterial>,
        require_csrf: bool,
    ) -> Self {
        assert!(capability
            .allowed_problem_codes()
            .contains(&crate::ProblemCode::AuthenticationFailed));
        Self {
            correlation_id,
            capability,
            session,
            csrf,
            require_csrf,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn capability(&self) -> CapabilityKey {
        self.capability
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub const fn csrf(&self) -> Option<&SecretMaterial> {
        self.csrf.as_ref()
    }
    pub const fn require_csrf(&self) -> bool {
        self.require_csrf
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedBrowserSession {
    user: BrowserUserView,
    access: RequestAccessContext,
    expires_at: DateTime<Utc>,
}

impl AuthenticatedBrowserSession {
    pub fn new(
        user: BrowserUserView,
        access: RequestAccessContext,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self {
            user,
            access,
            expires_at,
        }
    }
    pub const fn user(&self) -> &BrowserUserView {
        &self.user
    }
    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }
    pub const fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
}

pub struct EndBrowserSessionCommand {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
    csrf: SecretMaterial,
}

impl EndBrowserSessionCommand {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        session: SecretMaterial,
        csrf: SecretMaterial,
    ) -> Self {
        Self {
            correlation_id,
            session,
            csrf,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub const fn csrf(&self) -> &SecretMaterial {
        &self.csrf
    }
    pub fn into_parts(self) -> (RequestCorrelationId, SecretMaterial, SecretMaterial) {
        (self.correlation_id, self.session, self.csrf)
    }
}

pub struct ListBrowserUsersQuery {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
}

impl ListBrowserUsersQuery {
    pub const fn new(correlation_id: RequestCorrelationId, session: SecretMaterial) -> Self {
        Self {
            correlation_id,
            session,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub fn into_parts(self) -> (RequestCorrelationId, SecretMaterial) {
        (self.correlation_id, self.session)
    }
}

pub struct UpdateBrowserUserCommand {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
    csrf: SecretMaterial,
    target_user_id: BrowserUserId,
    current_password: BrowserPassword,
    username: Option<BrowserUsername>,
    password: Option<BrowserPassword>,
    active: Option<bool>,
}

impl UpdateBrowserUserCommand {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        correlation_id: RequestCorrelationId,
        session: SecretMaterial,
        csrf: SecretMaterial,
        target_user_id: BrowserUserId,
        current_password: BrowserPassword,
        username: Option<BrowserUsername>,
        password: Option<BrowserPassword>,
        active: Option<bool>,
    ) -> Result<Self, BrowserAuthInputError> {
        if username.is_none() && password.is_none() && active.is_none() {
            return Err(BrowserAuthInputError);
        }
        Ok(Self {
            correlation_id,
            session,
            csrf,
            target_user_id,
            current_password,
            username,
            password,
            active,
        })
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub const fn csrf(&self) -> &SecretMaterial {
        &self.csrf
    }
    pub const fn target_user_id(&self) -> BrowserUserId {
        self.target_user_id
    }
    pub const fn current_password(&self) -> &BrowserPassword {
        &self.current_password
    }
    pub const fn username(&self) -> Option<&BrowserUsername> {
        self.username.as_ref()
    }
    pub const fn password(&self) -> Option<&BrowserPassword> {
        self.password.as_ref()
    }
    pub const fn active(&self) -> Option<bool> {
        self.active
    }
    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        RequestCorrelationId,
        SecretMaterial,
        SecretMaterial,
        BrowserUserId,
        BrowserPassword,
        Option<BrowserUsername>,
        Option<BrowserPassword>,
        Option<bool>,
    ) {
        (
            self.correlation_id,
            self.session,
            self.csrf,
            self.target_user_id,
            self.current_password,
            self.username,
            self.password,
            self.active,
        )
    }
}

pub struct DeleteBrowserUserCommand {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
    csrf: SecretMaterial,
    target_user_id: BrowserUserId,
    current_password: BrowserPassword,
}

impl DeleteBrowserUserCommand {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        session: SecretMaterial,
        csrf: SecretMaterial,
        target_user_id: BrowserUserId,
        current_password: BrowserPassword,
    ) -> Self {
        Self {
            correlation_id,
            session,
            csrf,
            target_user_id,
            current_password,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub const fn csrf(&self) -> &SecretMaterial {
        &self.csrf
    }
    pub const fn target_user_id(&self) -> BrowserUserId {
        self.target_user_id
    }
    pub const fn current_password(&self) -> &BrowserPassword {
        &self.current_password
    }
    pub fn into_parts(
        self,
    ) -> (
        RequestCorrelationId,
        SecretMaterial,
        SecretMaterial,
        BrowserUserId,
        BrowserPassword,
    ) {
        (
            self.correlation_id,
            self.session,
            self.csrf,
            self.target_user_id,
            self.current_password,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserSessionSummary {
    session_id: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    location: String,
    device_type: String,
    is_current: bool,
}

impl BrowserSessionSummary {
    pub fn new(
        session_id: String,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        last_seen_at: DateTime<Utc>,
        location: String,
        device_type: String,
        is_current: bool,
    ) -> Self {
        Self {
            session_id,
            created_at,
            expires_at,
            last_seen_at,
            location,
            device_type,
            is_current,
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub const fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
    pub const fn last_seen_at(&self) -> DateTime<Utc> {
        self.last_seen_at
    }
    pub fn location(&self) -> &str {
        &self.location
    }
    pub fn device_type(&self) -> &str {
        &self.device_type
    }
    pub const fn is_current(&self) -> bool {
        self.is_current
    }
}

pub struct ListBrowserSessionsQuery {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
}

impl ListBrowserSessionsQuery {
    pub const fn new(correlation_id: RequestCorrelationId, session: SecretMaterial) -> Self {
        Self {
            correlation_id,
            session,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub fn into_parts(self) -> (RequestCorrelationId, SecretMaterial) {
        (self.correlation_id, self.session)
    }
}

pub struct EndSpecificBrowserSessionCommand {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
    csrf: SecretMaterial,
    target_session_id: String,
}

impl EndSpecificBrowserSessionCommand {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        session: SecretMaterial,
        csrf: SecretMaterial,
        target_session_id: String,
    ) -> Self {
        Self {
            correlation_id,
            session,
            csrf,
            target_session_id,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub const fn csrf(&self) -> &SecretMaterial {
        &self.csrf
    }
    pub fn target_session_id(&self) -> &str {
        &self.target_session_id
    }
    pub fn into_parts(self) -> (RequestCorrelationId, SecretMaterial, SecretMaterial, String) {
        (
            self.correlation_id,
            self.session,
            self.csrf,
            self.target_session_id,
        )
    }
}

pub struct EndAllOtherBrowserSessionsCommand {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
    csrf: SecretMaterial,
}

impl EndAllOtherBrowserSessionsCommand {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        session: SecretMaterial,
        csrf: SecretMaterial,
    ) -> Self {
        Self {
            correlation_id,
            session,
            csrf,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub const fn csrf(&self) -> &SecretMaterial {
        &self.csrf
    }
    pub fn into_parts(self) -> (RequestCorrelationId, SecretMaterial, SecretMaterial) {
        (self.correlation_id, self.session, self.csrf)
    }
}

pub struct SwitchBrowserSessionProfileCommand {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
    csrf: SecretMaterial,
    target_profile_id: fasti_domain::ProfileId,
}

impl SwitchBrowserSessionProfileCommand {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        session: SecretMaterial,
        csrf: SecretMaterial,
        target_profile_id: fasti_domain::ProfileId,
    ) -> Self {
        Self {
            correlation_id,
            session,
            csrf,
            target_profile_id,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub const fn csrf(&self) -> &SecretMaterial {
        &self.csrf
    }
    pub const fn target_profile_id(&self) -> fasti_domain::ProfileId {
        self.target_profile_id
    }
    pub fn into_parts(
        self,
    ) -> (
        RequestCorrelationId,
        SecretMaterial,
        SecretMaterial,
        fasti_domain::ProfileId,
    ) {
        (
            self.correlation_id,
            self.session,
            self.csrf,
            self.target_profile_id,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasskeySummary {
    passkey_id: String,
    name: String,
    created_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
}

impl PasskeySummary {
    pub fn new(
        passkey_id: String,
        name: String,
        created_at: DateTime<Utc>,
        last_used_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            passkey_id,
            name,
            created_at,
            last_used_at,
        }
    }
    pub fn passkey_id(&self) -> &str {
        &self.passkey_id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub const fn last_used_at(&self) -> Option<DateTime<Utc>> {
        self.last_used_at
    }
}

pub struct ListPasskeysQuery {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
}

impl ListPasskeysQuery {
    pub const fn new(correlation_id: RequestCorrelationId, session: SecretMaterial) -> Self {
        Self {
            correlation_id,
            session,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub fn into_parts(self) -> (RequestCorrelationId, SecretMaterial) {
        (self.correlation_id, self.session)
    }
}

pub struct DeletePasskeyCommand {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
    csrf: SecretMaterial,
    passkey_id: String,
}

impl DeletePasskeyCommand {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        session: SecretMaterial,
        csrf: SecretMaterial,
        passkey_id: String,
    ) -> Self {
        Self {
            correlation_id,
            session,
            csrf,
            passkey_id,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub const fn csrf(&self) -> &SecretMaterial {
        &self.csrf
    }
    pub fn passkey_id(&self) -> &str {
        &self.passkey_id
    }
    pub fn into_parts(self) -> (RequestCorrelationId, SecretMaterial, SecretMaterial, String) {
        (
            self.correlation_id,
            self.session,
            self.csrf,
            self.passkey_id,
        )
    }
}

pub struct BeginPasskeyRegistrationQuery {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
}

impl BeginPasskeyRegistrationQuery {
    pub const fn new(correlation_id: RequestCorrelationId, session: SecretMaterial) -> Self {
        Self {
            correlation_id,
            session,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub fn into_parts(self) -> (RequestCorrelationId, SecretMaterial) {
        (self.correlation_id, self.session)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasskeyRegistrationChallengeView {
    challenge: String,
    rp_name: String,
    rp_id: String,
    user_id: String,
    user_name: String,
}

impl PasskeyRegistrationChallengeView {
    pub fn new(
        challenge: String,
        rp_name: String,
        rp_id: String,
        user_id: String,
        user_name: String,
    ) -> Self {
        Self {
            challenge,
            rp_name,
            rp_id,
            user_id,
            user_name,
        }
    }
    pub fn challenge(&self) -> &str {
        &self.challenge
    }
    pub fn rp_name(&self) -> &str {
        &self.rp_name
    }
    pub fn rp_id(&self) -> &str {
        &self.rp_id
    }
    pub fn user_id(&self) -> &str {
        &self.user_id
    }
    pub fn user_name(&self) -> &str {
        &self.user_name
    }
}

pub struct CompletePasskeyRegistrationCommand {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
    csrf: SecretMaterial,
    name: String,
    credential_id: String,
    client_data_json: String,
    attestation_object: String,
}

impl CompletePasskeyRegistrationCommand {
    pub fn new(
        correlation_id: RequestCorrelationId,
        session: SecretMaterial,
        csrf: SecretMaterial,
        name: String,
        credential_id: String,
        client_data_json: String,
        attestation_object: String,
    ) -> Self {
        Self {
            correlation_id,
            session,
            csrf,
            name,
            credential_id,
            client_data_json,
            attestation_object,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub const fn csrf(&self) -> &SecretMaterial {
        &self.csrf
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn credential_id(&self) -> &str {
        &self.credential_id
    }
    pub fn client_data_json(&self) -> &str {
        &self.client_data_json
    }
    pub fn attestation_object(&self) -> &str {
        &self.attestation_object
    }
    pub fn into_parts(
        self,
    ) -> (
        RequestCorrelationId,
        SecretMaterial,
        SecretMaterial,
        String,
        String,
        String,
        String,
    ) {
        (
            self.correlation_id,
            self.session,
            self.csrf,
            self.name,
            self.credential_id,
            self.client_data_json,
            self.attestation_object,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TotpEnrollmentView {
    secret: String,
    otpauth_uri: String,
    backup_codes: Vec<String>,
}

impl TotpEnrollmentView {
    pub fn new(secret: String, otpauth_uri: String, backup_codes: Vec<String>) -> Self {
        Self {
            secret,
            otpauth_uri,
            backup_codes,
        }
    }
    pub fn secret(&self) -> &str {
        &self.secret
    }
    pub fn otpauth_uri(&self) -> &str {
        &self.otpauth_uri
    }
    pub fn backup_codes(&self) -> &[String] {
        &self.backup_codes
    }
}

pub struct EnrollTotpBeginCommand {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
    csrf: SecretMaterial,
}

impl EnrollTotpBeginCommand {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        session: SecretMaterial,
        csrf: SecretMaterial,
    ) -> Self {
        Self {
            correlation_id,
            session,
            csrf,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub const fn csrf(&self) -> &SecretMaterial {
        &self.csrf
    }
    pub fn into_parts(self) -> (RequestCorrelationId, SecretMaterial, SecretMaterial) {
        (self.correlation_id, self.session, self.csrf)
    }
}

pub struct EnrollTotpConfirmCommand {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
    csrf: SecretMaterial,
    code: String,
}

impl EnrollTotpConfirmCommand {
    pub fn new(
        correlation_id: RequestCorrelationId,
        session: SecretMaterial,
        csrf: SecretMaterial,
        code: String,
    ) -> Self {
        Self {
            correlation_id,
            session,
            csrf,
            code,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub const fn csrf(&self) -> &SecretMaterial {
        &self.csrf
    }
    pub fn code(&self) -> &str {
        &self.code
    }
    pub fn into_parts(self) -> (RequestCorrelationId, SecretMaterial, SecretMaterial, String) {
        (self.correlation_id, self.session, self.csrf, self.code)
    }
}

pub struct DisableTotpCommand {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
    csrf: SecretMaterial,
    current_password: BrowserPassword,
}

impl DisableTotpCommand {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        session: SecretMaterial,
        csrf: SecretMaterial,
        current_password: BrowserPassword,
    ) -> Self {
        Self {
            correlation_id,
            session,
            csrf,
            current_password,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub const fn csrf(&self) -> &SecretMaterial {
        &self.csrf
    }
    pub const fn current_password(&self) -> &BrowserPassword {
        &self.current_password
    }
    pub fn into_parts(
        self,
    ) -> (
        RequestCorrelationId,
        SecretMaterial,
        SecretMaterial,
        BrowserPassword,
    ) {
        (
            self.correlation_id,
            self.session,
            self.csrf,
            self.current_password,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcConfigView {
    issuer_url: String,
    client_id: String,
    pkce_enabled: bool,
    scopes: Vec<String>,
    enabled: bool,
}

impl OidcConfigView {
    pub fn new(
        issuer_url: String,
        client_id: String,
        pkce_enabled: bool,
        scopes: Vec<String>,
        enabled: bool,
    ) -> Self {
        Self {
            issuer_url,
            client_id,
            pkce_enabled,
            scopes,
            enabled,
        }
    }
    pub fn issuer_url(&self) -> &str {
        &self.issuer_url
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub const fn pkce_enabled(&self) -> bool {
        self.pkce_enabled
    }
    pub fn scopes(&self) -> &[String] {
        &self.scopes
    }
    pub const fn enabled(&self) -> bool {
        self.enabled
    }
}

pub struct SaveOidcConfigCommand {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
    csrf: SecretMaterial,
    issuer_url: String,
    client_id: String,
    client_secret: Option<String>,
    pkce_enabled: bool,
    scopes: Vec<String>,
    enabled: bool,
}

impl SaveOidcConfigCommand {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        correlation_id: RequestCorrelationId,
        session: SecretMaterial,
        csrf: SecretMaterial,
        issuer_url: String,
        client_id: String,
        client_secret: Option<String>,
        pkce_enabled: bool,
        scopes: Vec<String>,
        enabled: bool,
    ) -> Self {
        Self {
            correlation_id,
            session,
            csrf,
            issuer_url,
            client_id,
            client_secret,
            pkce_enabled,
            scopes,
            enabled,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub const fn csrf(&self) -> &SecretMaterial {
        &self.csrf
    }
    pub fn issuer_url(&self) -> &str {
        &self.issuer_url
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub fn client_secret(&self) -> Option<&str> {
        self.client_secret.as_deref()
    }
    pub const fn pkce_enabled(&self) -> bool {
        self.pkce_enabled
    }
    pub fn scopes(&self) -> &[String] {
        &self.scopes
    }
    pub const fn enabled(&self) -> bool {
        self.enabled
    }
    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        RequestCorrelationId,
        SecretMaterial,
        SecretMaterial,
        String,
        String,
        Option<String>,
        bool,
        Vec<String>,
        bool,
    ) {
        (
            self.correlation_id,
            self.session,
            self.csrf,
            self.issuer_url,
            self.client_id,
            self.client_secret,
            self.pkce_enabled,
            self.scopes,
            self.enabled,
        )
    }
}

pub struct GetOidcConfigQuery {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
}

impl GetOidcConfigQuery {
    pub const fn new(correlation_id: RequestCorrelationId, session: SecretMaterial) -> Self {
        Self {
            correlation_id,
            session,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub fn into_parts(self) -> (RequestCorrelationId, SecretMaterial) {
        (self.correlation_id, self.session)
    }
}

pub struct DeleteOidcConfigCommand {
    correlation_id: RequestCorrelationId,
    session: SecretMaterial,
    csrf: SecretMaterial,
}

impl DeleteOidcConfigCommand {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        session: SecretMaterial,
        csrf: SecretMaterial,
    ) -> Self {
        Self {
            correlation_id,
            session,
            csrf,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session(&self) -> &SecretMaterial {
        &self.session
    }
    pub const fn csrf(&self) -> &SecretMaterial {
        &self.csrf
    }
    pub fn into_parts(self) -> (RequestCorrelationId, SecretMaterial, SecretMaterial) {
        (self.correlation_id, self.session, self.csrf)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcDiscoveryView {
    authorization_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: Option<String>,
    jwks_uri: String,
}

impl OidcDiscoveryView {
    pub fn new(
        authorization_endpoint: String,
        token_endpoint: String,
        userinfo_endpoint: Option<String>,
        jwks_uri: String,
    ) -> Self {
        Self {
            authorization_endpoint,
            token_endpoint,
            userinfo_endpoint,
            jwks_uri,
        }
    }
    pub fn authorization_endpoint(&self) -> &str {
        &self.authorization_endpoint
    }
    pub fn token_endpoint(&self) -> &str {
        &self.token_endpoint
    }
    pub fn userinfo_endpoint(&self) -> Option<&str> {
        self.userinfo_endpoint.as_deref()
    }
    pub fn jwks_uri(&self) -> &str {
        &self.jwks_uri
    }
}

pub struct DiscoverOidcQuery {
    correlation_id: RequestCorrelationId,
    issuer_url: String,
}

impl DiscoverOidcQuery {
    pub fn new(correlation_id: RequestCorrelationId, issuer_url: String) -> Self {
        Self {
            correlation_id,
            issuer_url,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub fn issuer_url(&self) -> &str {
        &self.issuer_url
    }
    pub fn into_parts(self) -> (RequestCorrelationId, String) {
        (self.correlation_id, self.issuer_url)
    }
}

pub trait BrowserAccountPort: Send + Sync {
    /// Seeds the one-time development browser account if it does not exist
    /// yet. Returns `true` when this call actually created it, `false` when
    /// an account was already seeded (the given credential is then unused).
    fn ensure_development_browser_user(
        &self,
        username: BrowserUsername,
        password: BrowserPassword,
    ) -> ApplicationResult<bool>;
    fn create_browser_session(
        &self,
        command: CreateBrowserSessionCommand,
    ) -> ApplicationResult<CreatedBrowserSession>;
    fn authenticate_browser_session(
        &self,
        query: AuthenticateBrowserSessionQuery,
    ) -> ApplicationResult<AuthenticatedBrowserSession>;
    fn end_browser_session(&self, command: EndBrowserSessionCommand) -> ApplicationResult<()>;
    fn list_browser_sessions(
        &self,
        query: ListBrowserSessionsQuery,
    ) -> ApplicationResult<Vec<BrowserSessionSummary>>;
    fn end_specific_browser_session(
        &self,
        command: EndSpecificBrowserSessionCommand,
    ) -> ApplicationResult<bool>;
    fn end_all_other_browser_sessions(
        &self,
        command: EndAllOtherBrowserSessionsCommand,
    ) -> ApplicationResult<u64>;
    fn switch_browser_session_profile(
        &self,
        command: SwitchBrowserSessionProfileCommand,
    ) -> ApplicationResult<AuthenticatedBrowserSession>;
    fn list_browser_users(
        &self,
        query: ListBrowserUsersQuery,
    ) -> ApplicationResult<Vec<BrowserUserView>>;
    fn update_browser_user(
        &self,
        command: UpdateBrowserUserCommand,
    ) -> ApplicationResult<BrowserUserView>;
    fn delete_browser_user(&self, command: DeleteBrowserUserCommand) -> ApplicationResult<bool>;

    // Passkeys (WebAuthn)
    fn list_passkeys(&self, query: ListPasskeysQuery) -> ApplicationResult<Vec<PasskeySummary>>;
    fn delete_passkey(&self, command: DeletePasskeyCommand) -> ApplicationResult<bool>;
    fn begin_passkey_registration(
        &self,
        query: BeginPasskeyRegistrationQuery,
    ) -> ApplicationResult<PasskeyRegistrationChallengeView>;
    fn complete_passkey_registration(
        &self,
        command: CompletePasskeyRegistrationCommand,
    ) -> ApplicationResult<PasskeySummary>;

    // Authenticator 2FA (RFC 6238 TOTP)
    fn enroll_totp_begin(
        &self,
        command: EnrollTotpBeginCommand,
    ) -> ApplicationResult<TotpEnrollmentView>;
    fn enroll_totp_confirm(&self, command: EnrollTotpConfirmCommand) -> ApplicationResult<bool>;
    fn disable_totp(&self, command: DisableTotpCommand) -> ApplicationResult<bool>;

    // OpenID Connect (OIDC) SSO
    fn get_oidc_config(
        &self,
        query: GetOidcConfigQuery,
    ) -> ApplicationResult<Option<OidcConfigView>>;
    fn save_oidc_config(&self, command: SaveOidcConfigCommand)
        -> ApplicationResult<OidcConfigView>;
    fn delete_oidc_config(&self, command: DeleteOidcConfigCommand) -> ApplicationResult<bool>;
    fn discover_oidc(&self, query: DiscoverOidcQuery) -> ApplicationResult<OidcDiscoveryView>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_credentials_are_bounded() {
        assert!(BrowserUsername::try_new("testadmin").is_ok());
        assert!(BrowserUsername::try_new("Test Admin").is_err());
        assert!(BrowserPassword::try_new("testadmin").is_ok());
        assert!(BrowserPassword::try_new("short").is_err());
        assert!(CreateBrowserSessionCommand::try_new(
            RequestCorrelationId::new_v7(),
            BrowserUsername::try_new("testadmin").expect("username"),
            BrowserPassword::try_new("testadmin").expect("password"),
            0,
            MAX_SESSION_MINUTES,
        )
        .is_err());
    }

    #[test]
    fn session_lifetime_is_bounded_by_the_caller_supplied_ceiling() {
        let session = |lifetime, max| {
            CreateBrowserSessionCommand::try_new(
                RequestCorrelationId::new_v7(),
                BrowserUsername::try_new("testadmin").expect("username"),
                BrowserPassword::try_new("testadmin").expect("password"),
                lifetime,
                max,
            )
        };
        assert!(session(MAX_SESSION_MINUTES, MAX_SESSION_MINUTES).is_ok());
        assert!(session(MAX_SESSION_MINUTES + 1, MAX_SESSION_MINUTES).is_err());
        assert!(session(MAX_SESSION_MINUTES + 1, MAX_SESSION_MINUTES * 100).is_ok());
        assert!(session(MIN_SESSION_MINUTES - 1, MAX_SESSION_MINUTES * 100).is_err());
    }
}
