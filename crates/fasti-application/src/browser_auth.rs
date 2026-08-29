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
    fn list_browser_users(
        &self,
        query: ListBrowserUsersQuery,
    ) -> ApplicationResult<Vec<BrowserUserView>>;
    fn update_browser_user(
        &self,
        command: UpdateBrowserUserCommand,
    ) -> ApplicationResult<BrowserUserView>;
    fn delete_browser_user(&self, command: DeleteBrowserUserCommand) -> ApplicationResult<bool>;
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
