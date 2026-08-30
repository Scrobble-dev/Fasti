use crate::{ApplicationResult, BrowserSessionMutationCommand, SecretMaterial};
use chrono::{DateTime, Utc};
use fasti_domain::{
    AuthCallbackPath, AuthCeremony, AuthSubjectId, AuthSubjectLifecycle, MembershipId,
    MembershipLifecycleAction, OperationId, RequestCorrelationId, Sha256Digest,
    TrailBaseInstallation, TrailBaseInstanceId, TrailBaseSubject, WorkspaceId, WorkspaceRole,
};

#[derive(Debug, Clone, Copy)]
pub struct VerifyTrailBaseInstallationCommand {
    instance_id: TrailBaseInstanceId,
    release_matches: bool,
    declared_restore: bool,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
}

impl VerifyTrailBaseInstallationCommand {
    pub const fn new(
        instance_id: TrailBaseInstanceId,
        release_matches: bool,
        declared_restore: bool,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            instance_id,
            release_matches,
            declared_restore,
            correlation_id,
            at,
        }
    }

    pub const fn instance_id(&self) -> TrailBaseInstanceId {
        self.instance_id
    }
    pub const fn release_matches(&self) -> bool {
        self.release_matches
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

pub struct BootstrapFirstAdministratorCommand {
    bootstrap_secret: SecretMaterial,
    operation_id: OperationId,
    trailbase_subject: TrailBaseSubject,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
}

impl BootstrapFirstAdministratorCommand {
    pub const fn new(
        bootstrap_secret: SecretMaterial,
        operation_id: OperationId,
        trailbase_subject: TrailBaseSubject,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            bootstrap_secret,
            operation_id,
            trailbase_subject,
            correlation_id,
            at,
        }
    }
    pub const fn bootstrap_secret(&self) -> &SecretMaterial {
        &self.bootstrap_secret
    }
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }
    pub const fn trailbase_subject(&self) -> TrailBaseSubject {
        self.trailbase_subject
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn at(&self) -> DateTime<Utc> {
        self.at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapFirstAdministratorOutcome {
    Created {
        subject_id: AuthSubjectId,
        membership_id: MembershipId,
        workspace_id: WorkspaceId,
    },
    AlreadyBootstrapped,
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

pub trait HumanAccessPort: Send + Sync {
    fn verify_trailbase_installation(
        &self,
        command: VerifyTrailBaseInstallationCommand,
    ) -> ApplicationResult<TrailBaseInstallation>;
    fn start_auth_ceremony(&self, command: StartAuthCeremonyCommand) -> ApplicationResult<()>;
    fn claim_auth_ceremony(
        &self,
        command: ClaimAuthCeremonyCommand,
    ) -> ApplicationResult<AuthCeremony>;
    fn cancel_auth_ceremony(
        &self,
        command: CancelAuthCeremonyCommand,
    ) -> ApplicationResult<AuthCeremony>;
    fn bootstrap_first_administrator(
        &self,
        command: BootstrapFirstAdministratorCommand,
    ) -> ApplicationResult<BootstrapFirstAdministratorOutcome>;
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
