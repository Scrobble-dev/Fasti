use crate::{ApplicationResult, BrowserSessionMutationCommand, SecretMaterial};
use chrono::{DateTime, Utc};
use fasti_domain::{
    AuthCallbackPath, AuthCeremony, AuthCeremonyFailure, AuthSubjectId, AuthSubjectLifecycle,
    AuthenticationProvenance, MembershipId, MembershipLifecycleAction, OperationId,
    RequestCorrelationId, Sha256Digest, TrailBaseInstallation, TrailBaseInstanceId,
    TrailBaseSubject, WorkspaceRole,
};

#[derive(Debug, Clone)]
pub struct VerifyTrailBaseInstallationCommand {
    instance_id: TrailBaseInstanceId,
    observed_root_identity: Sha256Digest,
    release_lock_identity: Sha256Digest,
    declared_restore: bool,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy)]
pub struct ReadTrailBaseInstallationQuery {
    correlation_id: RequestCorrelationId,
}

impl ReadTrailBaseInstallationQuery {
    pub const fn new(correlation_id: RequestCorrelationId) -> Self {
        Self { correlation_id }
    }

    pub const fn correlation_id(self) -> RequestCorrelationId {
        self.correlation_id
    }
}

impl VerifyTrailBaseInstallationCommand {
    pub const fn new(
        instance_id: TrailBaseInstanceId,
        observed_root_identity: Sha256Digest,
        release_lock_identity: Sha256Digest,
        declared_restore: bool,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            instance_id,
            observed_root_identity,
            release_lock_identity,
            declared_restore,
            correlation_id,
            at,
        }
    }

    pub const fn instance_id(&self) -> TrailBaseInstanceId {
        self.instance_id
    }
    pub const fn observed_root_identity(&self) -> &Sha256Digest {
        &self.observed_root_identity
    }
    pub const fn release_lock_identity(&self) -> &Sha256Digest {
        &self.release_lock_identity
    }
    pub const fn declared_restore(&self) -> bool {
        self.declared_restore
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn at(&self) -> DateTime<Utc> {
        self.at
    }
}

#[derive(Debug, Clone)]
pub struct StartAuthCeremonyCommand {
    ceremony: AuthCeremony,
}

pub struct StartTrailBaseBootstrapCommand {
    ceremony: AuthCeremony,
    bootstrap_secret: SecretMaterial,
}

impl StartTrailBaseBootstrapCommand {
    pub const fn new(ceremony: AuthCeremony, bootstrap_secret: SecretMaterial) -> Self {
        Self {
            ceremony,
            bootstrap_secret,
        }
    }
    pub const fn ceremony(&self) -> &AuthCeremony {
        &self.ceremony
    }
    pub const fn bootstrap_secret(&self) -> &SecretMaterial {
        &self.bootstrap_secret
    }
}

impl StartAuthCeremonyCommand {
    pub const fn new(ceremony: AuthCeremony) -> Self {
        Self { ceremony }
    }
    pub const fn ceremony(&self) -> &AuthCeremony {
        &self.ceremony
    }
}

#[derive(Debug, Clone)]
pub struct ClaimAuthCeremonyCommand {
    browser_binding_digest: Sha256Digest,
    instance_id: TrailBaseInstanceId,
    activation_generation: u64,
    callback_path: AuthCallbackPath,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
}

impl ClaimAuthCeremonyCommand {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        browser_binding_digest: Sha256Digest,
        instance_id: TrailBaseInstanceId,
        activation_generation: u64,
        callback_path: AuthCallbackPath,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            browser_binding_digest,
            instance_id,
            activation_generation,
            callback_path,
            correlation_id,
            at,
        }
    }
    pub const fn browser_binding_digest(&self) -> &Sha256Digest {
        &self.browser_binding_digest
    }
    pub const fn instance_id(&self) -> TrailBaseInstanceId {
        self.instance_id
    }
    pub const fn activation_generation(&self) -> u64 {
        self.activation_generation
    }
    pub const fn callback_path(&self) -> &AuthCallbackPath {
        &self.callback_path
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn at(&self) -> DateTime<Utc> {
        self.at
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CancelAuthCeremonyCommand {
    operation_id: OperationId,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
}

pub struct ChangeMembershipLifecycleCommand {
    proof: BrowserSessionMutationCommand,
    membership_id: MembershipId,
    action: AdministratorMembershipAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdministratorMembershipAction {
    Approve,
    Suspend,
    Resume,
    Remove,
}

impl AdministratorMembershipAction {
    pub const fn lifecycle_action(self) -> MembershipLifecycleAction {
        match self {
            Self::Approve => MembershipLifecycleAction::Approve,
            Self::Suspend => MembershipLifecycleAction::Suspend,
            Self::Resume => MembershipLifecycleAction::Resume,
            Self::Remove => MembershipLifecycleAction::Remove,
        }
    }
}

impl ChangeMembershipLifecycleCommand {
    pub const fn new(
        proof: BrowserSessionMutationCommand,
        membership_id: MembershipId,
        action: AdministratorMembershipAction,
    ) -> Self {
        Self {
            proof,
            membership_id,
            action,
        }
    }
    pub const fn proof(&self) -> &BrowserSessionMutationCommand {
        &self.proof
    }
    pub const fn membership_id(&self) -> MembershipId {
        self.membership_id
    }
    pub const fn action(&self) -> AdministratorMembershipAction {
        self.action
    }
}

pub struct ChangeMembershipRoleCommand {
    proof: BrowserSessionMutationCommand,
    membership_id: MembershipId,
    role: WorkspaceRole,
}

impl ChangeMembershipRoleCommand {
    pub const fn new(
        proof: BrowserSessionMutationCommand,
        membership_id: MembershipId,
        role: WorkspaceRole,
    ) -> Self {
        Self {
            proof,
            membership_id,
            role,
        }
    }
    pub const fn proof(&self) -> &BrowserSessionMutationCommand {
        &self.proof
    }
    pub const fn membership_id(&self) -> MembershipId {
        self.membership_id
    }
    pub const fn role(&self) -> WorkspaceRole {
        self.role
    }
}

pub struct ChangeAuthSubjectLifecycleCommand {
    proof: BrowserSessionMutationCommand,
    subject_id: AuthSubjectId,
    action: AdministratorSubjectAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdministratorSubjectAction {
    Disable,
    BeginRecovery,
    Reactivate,
}

impl AdministratorSubjectAction {
    pub const fn lifecycle(self) -> AuthSubjectLifecycle {
        match self {
            Self::Disable => AuthSubjectLifecycle::Disabled,
            Self::BeginRecovery => AuthSubjectLifecycle::RecoveryPending,
            Self::Reactivate => AuthSubjectLifecycle::Active,
        }
    }
}

impl ChangeAuthSubjectLifecycleCommand {
    pub const fn new(
        proof: BrowserSessionMutationCommand,
        subject_id: AuthSubjectId,
        action: AdministratorSubjectAction,
    ) -> Self {
        Self {
            proof,
            subject_id,
            action,
        }
    }
    pub const fn proof(&self) -> &BrowserSessionMutationCommand {
        &self.proof
    }
    pub const fn subject_id(&self) -> AuthSubjectId {
        self.subject_id
    }
    pub const fn action(&self) -> AdministratorSubjectAction {
        self.action
    }
}

impl CancelAuthCeremonyCommand {
    pub const fn new(
        operation_id: OperationId,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            operation_id,
            correlation_id,
            at,
        }
    }
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn at(&self) -> DateTime<Utc> {
        self.at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfirmedTrailBaseIdentity {
    instance_id: TrailBaseInstanceId,
    subject: TrailBaseSubject,
    provenance: AuthenticationProvenance,
}

impl ConfirmedTrailBaseIdentity {
    pub const fn new(
        instance_id: TrailBaseInstanceId,
        subject: TrailBaseSubject,
        provenance: AuthenticationProvenance,
    ) -> Self {
        Self {
            instance_id,
            subject,
            provenance,
        }
    }
    pub const fn instance_id(&self) -> TrailBaseInstanceId {
        self.instance_id
    }
    pub const fn subject(&self) -> TrailBaseSubject {
        self.subject
    }
    pub const fn provenance(&self) -> AuthenticationProvenance {
        self.provenance
    }
}

pub const AUTH_SELECTION_CHOICE_LIMIT: usize = 64;

#[derive(Debug, Clone, Copy)]
pub struct ConfirmTrailBaseSignInCommand(PreauthorizeTrailBaseSignInCommand);

impl ConfirmTrailBaseSignInCommand {
    pub const fn new(authorization: PreauthorizeTrailBaseSignInCommand) -> Self {
        Self(authorization)
    }
    pub const fn authorization(&self) -> PreauthorizeTrailBaseSignInCommand {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct ReadTrailBaseSignInContinuationQuery {
    browser_binding_digest: Sha256Digest,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
}

impl ReadTrailBaseSignInContinuationQuery {
    pub const fn new(
        browser_binding_digest: Sha256Digest,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            browser_binding_digest,
            correlation_id,
            at,
        }
    }
    pub const fn browser_binding_digest(&self) -> &Sha256Digest {
        &self.browser_binding_digest
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn at(&self) -> DateTime<Utc> {
        self.at
    }
}

#[derive(Debug, Clone)]
pub struct CompleteTrailBaseSignInContinuationCommand {
    browser_binding_digest: Sha256Digest,
    choice_ordinal: u8,
    candidate_revision: Sha256Digest,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
}

impl CompleteTrailBaseSignInContinuationCommand {
    pub const fn new(
        browser_binding_digest: Sha256Digest,
        choice_ordinal: u8,
        candidate_revision: Sha256Digest,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            browser_binding_digest,
            choice_ordinal,
            candidate_revision,
            correlation_id,
            at,
        }
    }
    pub const fn browser_binding_digest(&self) -> &Sha256Digest {
        &self.browser_binding_digest
    }
    pub const fn choice_ordinal(&self) -> u8 {
        self.choice_ordinal
    }
    pub const fn candidate_revision(&self) -> &Sha256Digest {
        &self.candidate_revision
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn at(&self) -> DateTime<Utc> {
        self.at
    }
}

#[derive(Debug, Clone)]
pub struct CancelTrailBaseSignInContinuationCommand {
    browser_binding_digest: Sha256Digest,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
}

impl CancelTrailBaseSignInContinuationCommand {
    pub const fn new(
        browser_binding_digest: Sha256Digest,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            browser_binding_digest,
            correlation_id,
            at,
        }
    }
    pub const fn browser_binding_digest(&self) -> &Sha256Digest {
        &self.browser_binding_digest
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn at(&self) -> DateTime<Utc> {
        self.at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthSelectionChoice {
    ordinal: u8,
    workspace_ordinal: u8,
    profile_ordinal: u8,
    workspace_created_at: DateTime<Utc>,
    profile_created_at: DateTime<Utc>,
    membership_state: fasti_domain::MembershipLifecycle,
    role: WorkspaceRole,
}

impl AuthSelectionChoice {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        ordinal: u8,
        workspace_ordinal: u8,
        profile_ordinal: u8,
        workspace_created_at: DateTime<Utc>,
        profile_created_at: DateTime<Utc>,
        membership_state: fasti_domain::MembershipLifecycle,
        role: WorkspaceRole,
    ) -> Self {
        Self {
            ordinal,
            workspace_ordinal,
            profile_ordinal,
            workspace_created_at,
            profile_created_at,
            membership_state,
            role,
        }
    }
    pub const fn ordinal(self) -> u8 {
        self.ordinal
    }
    pub const fn workspace_ordinal(self) -> u8 {
        self.workspace_ordinal
    }
    pub const fn profile_ordinal(self) -> u8 {
        self.profile_ordinal
    }
    pub const fn workspace_created_at(self) -> DateTime<Utc> {
        self.workspace_created_at
    }
    pub const fn profile_created_at(self) -> DateTime<Utc> {
        self.profile_created_at
    }
    pub const fn membership_state(self) -> fasti_domain::MembershipLifecycle {
        self.membership_state
    }
    pub const fn role(self) -> WorkspaceRole {
        self.role
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSelectionProjection {
    expires_at: DateTime<Utc>,
    remembered: bool,
    candidate_revision: Sha256Digest,
    choices: Vec<AuthSelectionChoice>,
}

impl AuthSelectionProjection {
    pub fn new(
        expires_at: DateTime<Utc>,
        remembered: bool,
        candidate_revision: Sha256Digest,
        choices: Vec<AuthSelectionChoice>,
    ) -> Self {
        assert!(!choices.is_empty() && choices.len() <= AUTH_SELECTION_CHOICE_LIMIT);
        Self {
            expires_at,
            remembered,
            candidate_revision,
            choices,
        }
    }
    pub const fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
    pub const fn remembered(&self) -> bool {
        self.remembered
    }
    pub const fn candidate_revision(&self) -> &Sha256Digest {
        &self.candidate_revision
    }
    pub fn choices(&self) -> &[AuthSelectionChoice] {
        &self.choices
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PreauthorizeTrailBaseSignInCommand {
    operation_id: OperationId,
    identity: ConfirmedTrailBaseIdentity,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
}

impl PreauthorizeTrailBaseSignInCommand {
    pub const fn new(
        operation_id: OperationId,
        identity: ConfirmedTrailBaseIdentity,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            operation_id,
            identity,
            correlation_id,
            at,
        }
    }
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }
    pub const fn identity(&self) -> ConfirmedTrailBaseIdentity {
        self.identity
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn at(&self) -> DateTime<Utc> {
        self.at
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PreauthorizeTrailBaseBootstrapCommand {
    operation_id: OperationId,
    identity: ConfirmedTrailBaseIdentity,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
}

impl PreauthorizeTrailBaseBootstrapCommand {
    pub const fn new(
        operation_id: OperationId,
        identity: ConfirmedTrailBaseIdentity,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            operation_id,
            identity,
            correlation_id,
            at,
        }
    }
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }
    pub const fn identity(&self) -> ConfirmedTrailBaseIdentity {
        self.identity
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn at(&self) -> DateTime<Utc> {
        self.at
    }
}

pub struct CompleteTrailBaseBootstrapCommand {
    authorization: PreauthorizeTrailBaseBootstrapCommand,
    bootstrap_secret: SecretMaterial,
}

impl CompleteTrailBaseBootstrapCommand {
    pub const fn new(
        authorization: PreauthorizeTrailBaseBootstrapCommand,
        bootstrap_secret: SecretMaterial,
    ) -> Self {
        Self {
            authorization,
            bootstrap_secret,
        }
    }
    pub const fn authorization(&self) -> &PreauthorizeTrailBaseBootstrapCommand {
        &self.authorization
    }
    pub const fn bootstrap_secret(&self) -> &SecretMaterial {
        &self.bootstrap_secret
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FailAuthCeremonyCommand {
    operation_id: OperationId,
    failure: AuthCeremonyFailure,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
}

impl FailAuthCeremonyCommand {
    pub const fn new(
        operation_id: OperationId,
        failure: AuthCeremonyFailure,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            operation_id,
            failure,
            correlation_id,
            at,
        }
    }
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }
    pub const fn failure(&self) -> AuthCeremonyFailure {
        self.failure
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn at(&self) -> DateTime<Utc> {
        self.at
    }
}

pub trait HumanAccessPort: Send + Sync {
    fn read_trailbase_installation(
        &self,
        query: ReadTrailBaseInstallationQuery,
    ) -> ApplicationResult<Option<TrailBaseInstallation>>;
    fn verify_trailbase_installation(
        &self,
        command: VerifyTrailBaseInstallationCommand,
    ) -> ApplicationResult<TrailBaseInstallation>;
    fn start_auth_ceremony(&self, command: StartAuthCeremonyCommand) -> ApplicationResult<()>;
    fn start_trailbase_bootstrap(
        &self,
        command: StartTrailBaseBootstrapCommand,
    ) -> ApplicationResult<()>;
    fn claim_auth_ceremony(
        &self,
        command: ClaimAuthCeremonyCommand,
    ) -> ApplicationResult<AuthCeremony>;
    fn cancel_auth_ceremony(
        &self,
        command: CancelAuthCeremonyCommand,
    ) -> ApplicationResult<AuthCeremony>;
    fn preauthorize_trailbase_sign_in(
        &self,
        command: PreauthorizeTrailBaseSignInCommand,
    ) -> ApplicationResult<()>;
    fn confirm_trailbase_sign_in(
        &self,
        command: ConfirmTrailBaseSignInCommand,
    ) -> ApplicationResult<AuthCeremony>;
    fn read_trailbase_sign_in_continuation(
        &self,
        query: ReadTrailBaseSignInContinuationQuery,
    ) -> ApplicationResult<AuthSelectionProjection>;
    fn complete_trailbase_sign_in_continuation(
        &self,
        command: CompleteTrailBaseSignInContinuationCommand,
    ) -> ApplicationResult<crate::CreatedBrowserSession>;
    fn cancel_trailbase_sign_in_continuation(
        &self,
        command: CancelTrailBaseSignInContinuationCommand,
    ) -> ApplicationResult<AuthCeremony>;
    fn preauthorize_trailbase_bootstrap(
        &self,
        command: PreauthorizeTrailBaseBootstrapCommand,
    ) -> ApplicationResult<()>;
    fn fail_auth_ceremony(
        &self,
        command: FailAuthCeremonyCommand,
    ) -> ApplicationResult<AuthCeremony>;
    fn complete_trailbase_bootstrap(
        &self,
        command: CompleteTrailBaseBootstrapCommand,
    ) -> ApplicationResult<crate::CreatedBrowserSession>;
    fn change_membership_lifecycle(
        &self,
        command: ChangeMembershipLifecycleCommand,
    ) -> ApplicationResult<bool>;
    fn change_membership_role(
        &self,
        command: ChangeMembershipRoleCommand,
    ) -> ApplicationResult<bool>;
    fn change_auth_subject_lifecycle(
        &self,
        command: ChangeAuthSubjectLifecycleCommand,
    ) -> ApplicationResult<bool>;
}
