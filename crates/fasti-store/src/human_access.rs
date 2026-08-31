use crate::crypto::sha256_bytes;
use crate::kernel::timestamp;
use crate::SqliteKernel;
use chrono::{DateTime, Duration, Utc};
use fasti_application::{
    AccessAdministrationPort, ApplicationResult, AuthSelectionChoice, AuthSelectionProjection,
    BrowserSessionMutationCommand, CancelAuthCeremonyCommand,
    CancelTrailBaseSignInContinuationCommand, CapabilityKey, ChangeAuthSubjectLifecycleCommand,
    ChangeMembershipLifecycleCommand, ChangeMembershipRoleCommand, ClaimAuthCeremonyCommand,
    CompleteTrailBaseBootstrapCommand, CompleteTrailBaseSignInContinuationCommand,
    ConfirmTrailBaseSignInCommand, ConfirmedTrailBaseIdentity, CreatedBrowserSession,
    FailAuthCeremonyCommand, FastiProblem, HumanAccessPort, PreauthorizeTrailBaseBootstrapCommand,
    PreauthorizeTrailBaseSignInCommand, PrepareTrailBaseBootstrapQuery, ProblemCode,
    ReadTrailBaseInstallationQuery, ReadTrailBaseSignInContinuationQuery, SessionPolicy,
    StartAuthCeremonyCommand, StartTrailBaseBootstrapCommand, VerifyTrailBaseInstallationCommand,
    AUTH_SELECTION_CHOICE_LIMIT,
};
use fasti_domain::{
    AccessAuditEventKind, AccessInvalidationEffect, AccessInvariantError, AdministratorContinuity,
    AuthCallbackPath, AuthCeremony, AuthCeremonyConfirmation, AuthCeremonyFailure,
    AuthCeremonyProtocol, AuthCeremonyPurpose, AuthCeremonySelection, AuthCeremonyState,
    AuthReturnTarget, AuthSubject, AuthSubjectId, AuthSubjectLifecycle, AuthenticationAssurance,
    AuthenticationMethod, AuthenticationProvenance, BrowserSessionId, ClientId, MembershipId,
    MembershipLifecycle, MembershipLifecycleAction, OperationId, ProfileGrantId, ProfileId,
    RecentAuthentication, RequestCorrelationId, Sha256Digest, TrailBaseActivationState,
    TrailBaseExternalAnchor, TrailBaseInstallation, TrailBaseInstanceId, WorkspaceId,
    WorkspaceMembership, WorkspaceRole,
};
use rusqlite::{
    params, Connection, ErrorCode, OptionalExtension, Transaction, TransactionBehavior,
};
use thiserror::Error;

const CEREMONY_CAPACITY: i64 = 10_000;
const CEREMONY_RETENTION_HOURS: i64 = 24;
const AUDIT_CAPACITY: i64 = 10_000;
const AUDIT_RETENTION_DAYS: i64 = 90;

#[derive(Debug, Error)]
pub(crate) enum HumanAccessStoreError {
    #[error("human Access storage failed")]
    Storage(#[from] rusqlite::Error),
    #[error("human Access storage is unavailable")]
    StorageUnavailable,
    #[error("human Access persisted state is invalid")]
    Integrity,
    #[error("human Access capacity is exhausted")]
    CapacityExceeded,
    #[error("human Access record was not found")]
    NotFound,
    #[error("human Access record conflicts with existing state")]
    Conflict,
    #[error("human Access subject has no selectable affiliation")]
    Unaffiliated,
    #[error("human Access selection changed")]
    SelectionChanged,
    #[error("human Access continuation binding is invalid")]
    BindingInvalid,
    #[error("human Access proof expired")]
    ProofExpired,
    #[error("human Access trust is unavailable")]
    TrustUnavailable,
    #[error("human Access ceremony ended with attributable failure {0:?}")]
    AttributableFailure(AuthCeremonyFailure),
    #[error(transparent)]
    Invariant(#[from] AccessInvariantError),
}

type StoreResult<T> = Result<T, HumanAccessStoreError>;

struct CeremonyRow {
    operation_id: String,
    purpose: String,
    protocol: String,
    trailbase_instance_id: String,
    activation_generation: i64,
    browser_binding_digest: String,
    workspace_id: Option<String>,
    selected_profile_grant_id: Option<String>,
    bound_browser_session_id: Option<String>,
    invited_membership_id: Option<String>,
    remembered: i64,
    confirmed_auth_subject_id: Option<String>,
    authentication_method: Option<String>,
    authentication_verified_at: Option<String>,
    confirmed_auth_epoch: Option<i64>,
    confirmed_authorization_epoch: Option<i64>,
    callback_path: String,
    return_target: String,
    correlation_id: String,
    state: String,
    failure: Option<String>,
    created_at: String,
    expires_at: String,
    claimed_at: Option<String>,
    terminal_at: Option<String>,
}

const CEREMONY_COLUMNS: &str = "operation_id, purpose, protocol, trailbase_instance_id, activation_generation, browser_binding_digest, workspace_id, selected_profile_grant_id, bound_browser_session_id, invited_membership_id, remembered, confirmed_auth_subject_id, authentication_method, authentication_verified_at, confirmed_auth_epoch, confirmed_authorization_epoch, callback_path, return_target, correlation_id, state, failure, created_at, expires_at, claimed_at, terminal_at";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SignInCandidate {
    membership_id: MembershipId,
    workspace_id: WorkspaceId,
    workspace_created_at: DateTime<Utc>,
    membership_state: MembershipLifecycle,
    role: WorkspaceRole,
    profile_grant_id: ProfileGrantId,
    profile_id: ProfileId,
    profile_created_at: DateTime<Utc>,
    client_id: ClientId,
    workspace_ordinal: u8,
    profile_ordinal: u8,
}

impl SignInCandidate {
    fn selection(self) -> StoreResult<AuthCeremonySelection> {
        AuthCeremonySelection::try_new(
            AuthCeremonyPurpose::SignIn,
            self.workspace_id,
            self.profile_grant_id,
            None,
            matches!(self.membership_state, MembershipLifecycle::Invited)
                .then_some(self.membership_id),
        )
        .map_err(HumanAccessStoreError::Invariant)
    }

    const fn projection(self, ordinal: u8) -> AuthSelectionChoice {
        AuthSelectionChoice::new(
            ordinal,
            self.workspace_ordinal,
            self.profile_ordinal,
            self.workspace_created_at,
            self.profile_created_at,
            self.membership_state,
            self.role,
        )
    }
}

impl SqliteKernel {
    pub(crate) fn prepare_trailbase_bootstrap_selection(
        &self,
    ) -> StoreResult<AuthCeremonySelection> {
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let (workspace_id, grant_id) = connection
            .query_row(
                r#"
                SELECT node.workspace_id, grant.grant_id
                FROM node_state node
                JOIN profile_grants grant
                  ON grant.workspace_id = node.workspace_id
                 AND grant.profile_id = node.profile_id
                 AND grant.client_id = node.client_id
                JOIN clients client
                  ON client.workspace_id = grant.workspace_id
                 AND client.client_id = grant.client_id
                WHERE node.singleton = 1
                  AND node.initialized = 1
                  AND node.initialization_consumed_at IS NOT NULL
                  AND node.recovery_restore_attempt_id IS NULL
                  AND grant.status = 'active'
                  AND client.status = 'active'
                  AND NOT EXISTS(SELECT 1 FROM workspace_memberships)
                  AND NOT EXISTS(SELECT 1 FROM trailbase_auth_anchors)
                "#,
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or(HumanAccessStoreError::Conflict)?;
        AuthCeremonySelection::try_new(
            AuthCeremonyPurpose::FirstAdministratorBootstrap,
            workspace_id
                .parse()
                .map_err(|_| HumanAccessStoreError::Integrity)?,
            grant_id
                .parse()
                .map_err(|_| HumanAccessStoreError::Integrity)?,
            None,
            None,
        )
        .map_err(HumanAccessStoreError::Invariant)
    }

    pub(crate) fn verify_trailbase_installation(
        &self,
        instance_id: TrailBaseInstanceId,
        observed_root: Sha256Digest,
        release_lock_identity: Sha256Digest,
        declared_restore: bool,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> StoreResult<TrailBaseInstallation> {
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        let current = load_installation(&transaction)?;
        let is_new = current.is_none();
        let mut installation = match current {
            Some(current) if current.id() == instance_id => current,
            Some(_) => return Err(HumanAccessStoreError::Integrity),
            None => TrailBaseInstallation::new(
                instance_id,
                observed_root.clone(),
                release_lock_identity.clone(),
                at,
            ),
        };

        let changed = if declared_restore {
            installation.declare_restore(at)?
        } else {
            installation.verify(&observed_root, &release_lock_identity, at)?
        };
        persist_installation(&transaction, &installation, is_new)?;
        if changed {
            let event = match installation.activation_state() {
                TrailBaseActivationState::Active => AccessAuditEventKind::TrailBaseActivated,
                TrailBaseActivationState::Blocked(_) => AccessAuditEventKind::TrailBaseBlocked,
                TrailBaseActivationState::Inactive => return Err(HumanAccessStoreError::Integrity),
            };
            insert_installation_audit(&transaction, event, installation.id(), correlation_id, at)?;
        }
        transaction.commit()?;
        Ok(installation)
    }

    pub(crate) fn maintain_auth_ceremonies(
        &self,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> StoreResult<Vec<OperationId>> {
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        let expired = maintain_ceremonies(&transaction, correlation_id, at)?;
        transaction.commit()?;
        Ok(expired)
    }

    pub(crate) fn insert_auth_ceremony(&self, ceremony: &AuthCeremony) -> StoreResult<()> {
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        maintain_ceremonies(
            &transaction,
            ceremony.correlation_id(),
            ceremony.created_at(),
        )?;
        let count: i64 =
            transaction.query_row("SELECT COUNT(*) FROM auth_ceremonies", [], |row| row.get(0))?;
        if count >= CEREMONY_CAPACITY {
            transaction.commit()?;
            return Err(HumanAccessStoreError::CapacityExceeded);
        }
        require_active_installation(
            &transaction,
            ceremony.trailbase_instance_id(),
            ceremony.activation_generation(),
        )?;
        let result = transaction.execute(
            r#"
            INSERT INTO auth_ceremonies(
                operation_id, purpose, protocol, trailbase_instance_id,
                activation_generation, browser_binding_digest, workspace_id,
                selected_profile_grant_id, bound_browser_session_id,
                invited_membership_id, remembered, callback_path, return_target,
                correlation_id, state, failure, created_at, expires_at,
                claimed_at, terminal_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                      ?13, ?14, ?15, NULL, ?16, ?17, NULL, NULL)
            "#,
            params![
                ceremony.id().to_string(),
                ceremony.purpose().as_str(),
                ceremony.protocol().as_str(),
                ceremony.trailbase_instance_id().to_string(),
                i64::try_from(ceremony.activation_generation())
                    .map_err(|_| HumanAccessStoreError::Integrity)?,
                ceremony.browser_binding_digest().as_str(),
                ceremony
                    .selection()
                    .map(|selection| selection.workspace_id().to_string()),
                ceremony
                    .selection()
                    .map(|selection| selection.selected_profile_grant_id().to_string()),
                ceremony
                    .selection()
                    .and_then(AuthCeremonySelection::bound_browser_session_id)
                    .map(|id| id.to_string()),
                ceremony
                    .selection()
                    .and_then(AuthCeremonySelection::invited_membership_id)
                    .map(|id| id.to_string()),
                i64::from(ceremony.remembered()),
                ceremony.callback_path().as_str(),
                ceremony.return_target().as_str(),
                ceremony.correlation_id().to_string(),
                ceremony.state().as_str(),
                timestamp(ceremony.created_at()),
                timestamp(ceremony.expires_at()),
            ],
        );
        map_insert(result)?;
        transaction.commit()?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn claim_auth_ceremony(
        &self,
        browser_binding_digest: &Sha256Digest,
        instance_id: TrailBaseInstanceId,
        activation_generation: u64,
        callback_path: &AuthCallbackPath,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> StoreResult<AuthCeremony> {
        self.maintain_auth_ceremonies(correlation_id, at)?;
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        let mut ceremony = load_ceremony_by_binding(&transaction, browser_binding_digest)?
            .ok_or(HumanAccessStoreError::NotFound)?;
        let prior_state = ceremony.state();
        ceremony.claim(
            browser_binding_digest,
            instance_id,
            activation_generation,
            callback_path,
            at,
        )?;
        match require_active_installation(&transaction, instance_id, activation_generation) {
            Ok(()) => {}
            Err(HumanAccessStoreError::TrustUnavailable) => {
                ceremony.fail(AuthCeremonyFailure::TrustUnavailable, at)?;
                persist_ceremony_transition(&transaction, &ceremony, prior_state)?;
                insert_ceremony_audit(
                    &transaction,
                    AccessAuditEventKind::CeremonyFailed,
                    &ceremony,
                    correlation_id,
                    at,
                )?;
                transaction.commit()?;
                return Ok(ceremony);
            }
            Err(error) => return Err(error),
        }
        persist_ceremony_transition(&transaction, &ceremony, prior_state)?;
        insert_ceremony_audit(
            &transaction,
            AccessAuditEventKind::CeremonyClaimed,
            &ceremony,
            correlation_id,
            at,
        )?;
        transaction.commit()?;
        Ok(ceremony)
    }

    pub(crate) fn cancel_auth_ceremony(
        &self,
        operation_id: OperationId,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> StoreResult<AuthCeremony> {
        self.maintain_auth_ceremonies(correlation_id, at)?;
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        let mut ceremony =
            load_ceremony(&transaction, operation_id)?.ok_or(HumanAccessStoreError::NotFound)?;
        let prior_state = ceremony.state();
        ceremony.cancel(at)?;
        persist_ceremony_transition(&transaction, &ceremony, prior_state)?;
        insert_ceremony_audit(
            &transaction,
            AccessAuditEventKind::CeremonyCancelled,
            &ceremony,
            correlation_id,
            at,
        )?;
        transaction.commit()?;
        Ok(ceremony)
    }

    pub(crate) fn preauthorize_trailbase_sign_in(
        &self,
        command: PreauthorizeTrailBaseSignInCommand,
    ) -> StoreResult<()> {
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Deferred)?;
        preauthorize_sign_in_identity(&transaction, command).map_err(|error| {
            if matches!(error, HumanAccessStoreError::NotFound) {
                HumanAccessStoreError::Unaffiliated
            } else {
                error
            }
        })?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn confirm_trailbase_sign_in(
        &self,
        command: ConfirmTrailBaseSignInCommand,
    ) -> StoreResult<AuthCeremony> {
        let authorization = command.authorization();
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        let mut ceremony = load_ceremony(&transaction, authorization.operation_id())?
            .ok_or(HumanAccessStoreError::BindingInvalid)?;
        if authorization.at() >= ceremony.expires_at() {
            return Err(HumanAccessStoreError::ProofExpired);
        }
        if !confirmed_ceremony_matches(
            &ceremony,
            authorization.identity(),
            AuthCeremonyPurpose::SignIn,
            authorization.at(),
        ) {
            return Err(HumanAccessStoreError::BindingInvalid);
        }
        match require_active_installation(
            &transaction,
            authorization.identity().instance_id(),
            authorization
                .identity()
                .provenance()
                .activation_generation(),
        ) {
            Ok(()) => {}
            Err(HumanAccessStoreError::TrustUnavailable) => {
                fail_claimed_after_logout(
                    &transaction,
                    &mut ceremony,
                    AuthCeremonyFailure::TrustUnavailable,
                    authorization.correlation_id(),
                    authorization.at(),
                )?;
                transaction.commit()?;
                return Err(HumanAccessStoreError::TrustUnavailable);
            }
            Err(error) => return Err(error),
        }
        let subject = match load_anchored_subject(&transaction, authorization.identity()) {
            Ok(subject) => subject,
            Err(HumanAccessStoreError::NotFound) => {
                fail_claimed_after_logout(
                    &transaction,
                    &mut ceremony,
                    AuthCeremonyFailure::LocalAuthorizationDenied,
                    authorization.correlation_id(),
                    authorization.at(),
                )?;
                transaction.commit()?;
                return Err(HumanAccessStoreError::Unaffiliated);
            }
            Err(error) => return Err(error),
        };
        let prior_state = ceremony.state();
        ceremony.require_selection(
            AuthCeremonyConfirmation::new(subject, authorization.identity().provenance()),
            authorization.at(),
        )?;
        persist_ceremony_transition(&transaction, &ceremony, prior_state)?;
        insert_ceremony_audit(
            &transaction,
            AccessAuditEventKind::CeremonySelectionRequired,
            &ceremony,
            authorization.correlation_id(),
            authorization.at(),
        )?;
        transaction.commit()?;
        Ok(ceremony)
    }

    pub(crate) fn read_trailbase_sign_in_continuation(
        &self,
        query: ReadTrailBaseSignInContinuationQuery,
    ) -> StoreResult<AuthSelectionProjection> {
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        maintain_ceremonies(&transaction, query.correlation_id(), query.at())?;
        let ceremony = load_ceremony_by_binding(&transaction, query.browser_binding_digest())?
            .ok_or(HumanAccessStoreError::BindingInvalid)?;
        match ceremony.state() {
            AuthCeremonyState::Expired => {
                transaction.commit()?;
                return Err(HumanAccessStoreError::ProofExpired);
            }
            AuthCeremonyState::Failed | AuthCeremonyState::CleanupUncertain => {
                let failure = ceremony.failure().ok_or(HumanAccessStoreError::Integrity)?;
                transaction.commit()?;
                return Err(HumanAccessStoreError::AttributableFailure(failure));
            }
            AuthCeremonyState::Claimed => return Err(HumanAccessStoreError::Integrity),
            AuthCeremonyState::SelectionRequired => {}
            AuthCeremonyState::Pending
            | AuthCeremonyState::Completed
            | AuthCeremonyState::Cancelled => return Err(HumanAccessStoreError::BindingInvalid),
        }
        let confirmation = ceremony
            .confirmation()
            .ok_or(HumanAccessStoreError::Integrity)?;
        let subject = load_subject(&transaction, confirmation.subject_id())?;
        if !matches!(subject.lifecycle(), AuthSubjectLifecycle::Active) {
            return Err(HumanAccessStoreError::Unaffiliated);
        }
        let candidates = load_sign_in_candidates(&transaction, subject.id())?;
        let projection = project_auth_selection(&ceremony, &candidates)?;
        transaction.commit()?;
        Ok(projection)
    }

    pub(crate) fn complete_trailbase_sign_in_continuation(
        &self,
        command: CompleteTrailBaseSignInContinuationCommand,
    ) -> StoreResult<CreatedBrowserSession> {
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        maintain_ceremonies(&transaction, command.correlation_id(), command.at())?;
        let mut ceremony =
            load_ceremony_by_binding(&transaction, command.browser_binding_digest())?
                .ok_or(HumanAccessStoreError::BindingInvalid)?;
        match ceremony.state() {
            AuthCeremonyState::SelectionRequired => {}
            AuthCeremonyState::Expired => {
                transaction.commit()?;
                return Err(HumanAccessStoreError::ProofExpired);
            }
            AuthCeremonyState::Failed | AuthCeremonyState::CleanupUncertain => {
                let failure = ceremony.failure().ok_or(HumanAccessStoreError::Integrity)?;
                transaction.commit()?;
                return Err(HumanAccessStoreError::AttributableFailure(failure));
            }
            AuthCeremonyState::Pending
            | AuthCeremonyState::Claimed
            | AuthCeremonyState::Completed
            | AuthCeremonyState::Cancelled => return Err(HumanAccessStoreError::BindingInvalid),
        }
        let confirmation = ceremony
            .confirmation()
            .ok_or(HumanAccessStoreError::Integrity)?;
        require_active_installation(
            &transaction,
            ceremony.trailbase_instance_id(),
            ceremony.activation_generation(),
        )?;
        let mut subject = load_subject(&transaction, confirmation.subject_id())?;
        if !matches!(subject.lifecycle(), AuthSubjectLifecycle::Active)
            || subject.auth_epoch() != confirmation.auth_epoch()
            || subject.authorization_epoch() != confirmation.authorization_epoch()
        {
            return Err(HumanAccessStoreError::SelectionChanged);
        }
        let candidates = load_sign_in_candidates(&transaction, subject.id()).map_err(|error| {
            if matches!(error, HumanAccessStoreError::Unaffiliated) {
                HumanAccessStoreError::SelectionChanged
            } else {
                error
            }
        })?;
        if &auth_selection_revision(&ceremony, &candidates)? != command.candidate_revision() {
            return Err(HumanAccessStoreError::SelectionChanged);
        }
        let candidate = candidates
            .get(usize::from(command.choice_ordinal()))
            .copied()
            .ok_or(HumanAccessStoreError::Invariant(
                AccessInvariantError::InvalidCeremonySelectionBinding,
            ))?;
        let selection = candidate.selection()?;
        let mut membership = load_membership(&transaction, candidate.membership_id)?
            .ok_or(HumanAccessStoreError::SelectionChanged)?;
        if membership.subject_id() != subject.id()
            || membership.workspace_id() != candidate.workspace_id
            || membership.lifecycle() != candidate.membership_state
            || membership.role() != candidate.role
        {
            return Err(HumanAccessStoreError::SelectionChanged);
        }
        if matches!(membership.lifecycle(), MembershipLifecycle::Invited) {
            membership.apply_lifecycle_action(
                &mut subject,
                MembershipLifecycleAction::AcceptInvitation,
                0,
                command.at(),
            )?;
            persist_membership_subject(&transaction, &membership, &subject)?;
            insert_membership_audit(
                &transaction,
                AccessAuditEventKind::MembershipInvitationAccepted,
                &membership,
                subject.id(),
                command.correlation_id(),
                command.at(),
            )?;
        }
        let grants =
            load_active_subject_grants(&transaction, subject.id(), candidate.workspace_id)?;
        if !grants.contains(&candidate.profile_grant_id) {
            return Err(HumanAccessStoreError::SelectionChanged);
        }
        let created = crate::browser_auth::insert_session(
            &transaction,
            subject,
            candidate.workspace_id,
            &grants,
            candidate.profile_grant_id,
            SessionPolicy::C1,
            ceremony.remembered(),
            command.at(),
            0,
            CapabilityKey::CreateBrowserSession,
            command.correlation_id(),
        )
        .map_err(|problem| session_creation_error(problem.code()))?;
        insert_session_authentication(
            &transaction,
            &created,
            ceremony.trailbase_instance_id(),
            confirmation.provenance(),
        )?;
        insert_browser_session_issued_audit(
            &transaction,
            &created,
            &ceremony,
            subject.id(),
            command.correlation_id(),
            command.at(),
        )?;
        let prior_state = ceremony.state();
        ceremony.complete_selection(selection, command.at())?;
        persist_ceremony_transition(&transaction, &ceremony, prior_state)?;
        insert_ceremony_audit(
            &transaction,
            AccessAuditEventKind::CeremonyCompleted,
            &ceremony,
            command.correlation_id(),
            command.at(),
        )?;
        transaction.commit()?;
        Ok(created)
    }

    pub(crate) fn cancel_trailbase_sign_in_continuation(
        &self,
        command: CancelTrailBaseSignInContinuationCommand,
    ) -> StoreResult<AuthCeremony> {
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        maintain_ceremonies(&transaction, command.correlation_id(), command.at())?;
        let mut ceremony =
            load_ceremony_by_binding(&transaction, command.browser_binding_digest())?
                .ok_or(HumanAccessStoreError::BindingInvalid)?;
        match ceremony.state() {
            AuthCeremonyState::SelectionRequired => {}
            AuthCeremonyState::Failed
            | AuthCeremonyState::CleanupUncertain
            | AuthCeremonyState::Expired
            | AuthCeremonyState::Completed
            | AuthCeremonyState::Cancelled => {
                transaction.commit()?;
                return Ok(ceremony);
            }
            AuthCeremonyState::Pending | AuthCeremonyState::Claimed => {
                return Err(HumanAccessStoreError::BindingInvalid)
            }
        }
        let prior_state = ceremony.state();
        ceremony.cancel(command.at())?;
        persist_ceremony_transition(&transaction, &ceremony, prior_state)?;
        insert_ceremony_audit(
            &transaction,
            AccessAuditEventKind::CeremonyCancelled,
            &ceremony,
            command.correlation_id(),
            command.at(),
        )?;
        transaction.commit()?;
        Ok(ceremony)
    }

    pub(crate) fn preauthorize_trailbase_bootstrap(
        &self,
        command: &PreauthorizeTrailBaseBootstrapCommand,
    ) -> StoreResult<()> {
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Deferred)?;
        authorize_bootstrap(&transaction, command)?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn fail_auth_ceremony(
        &self,
        command: FailAuthCeremonyCommand,
    ) -> StoreResult<AuthCeremony> {
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        let mut ceremony = load_ceremony(&transaction, command.operation_id())?
            .ok_or(HumanAccessStoreError::NotFound)?;
        let prior_state = ceremony.state();
        if matches!(
            command.failure(),
            AuthCeremonyFailure::ExchangeOutcomeUncertain | AuthCeremonyFailure::LogoutUncertain
        ) {
            ceremony.mark_cleanup_uncertain(command.failure(), command.at())?;
        } else {
            ceremony.fail(command.failure(), command.at())?;
        }
        persist_ceremony_transition(&transaction, &ceremony, prior_state)?;
        insert_ceremony_audit(
            &transaction,
            if matches!(ceremony.state(), AuthCeremonyState::CleanupUncertain) {
                AccessAuditEventKind::CeremonyCleanupUncertain
            } else {
                AccessAuditEventKind::CeremonyFailed
            },
            &ceremony,
            command.correlation_id(),
            command.at(),
        )?;
        transaction.commit()?;
        Ok(ceremony)
    }

    pub(crate) fn complete_trailbase_bootstrap(
        &self,
        command: &CompleteTrailBaseBootstrapCommand,
    ) -> StoreResult<CreatedBrowserSession> {
        self.complete_trailbase_bootstrap_transaction(command, true)?
            .ok_or(HumanAccessStoreError::Integrity)
    }

    pub(crate) fn complete_trailbase_identity_bootstrap(
        &self,
        command: &CompleteTrailBaseBootstrapCommand,
    ) -> StoreResult<()> {
        if self
            .complete_trailbase_bootstrap_transaction(command, false)?
            .is_some()
        {
            return Err(HumanAccessStoreError::Integrity);
        }
        Ok(())
    }

    fn complete_trailbase_bootstrap_transaction(
        &self,
        command: &CompleteTrailBaseBootstrapCommand,
        issue_browser_session: bool,
    ) -> StoreResult<Option<CreatedBrowserSession>> {
        let authorization = command.authorization();
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        let mut ceremony = authorize_bootstrap(&transaction, authorization)?;
        let selection = ceremony
            .selection()
            .ok_or(HumanAccessStoreError::Integrity)?;
        let subject = AuthSubject::try_new(
            AuthSubjectId::new_v7(),
            AuthSubjectLifecycle::Active,
            0,
            0,
            authorization.at(),
            authorization.at(),
        )?;
        let anchor = TrailBaseExternalAnchor::new(
            authorization.identity().instance_id(),
            authorization.identity().subject(),
            subject.id(),
            authorization.at(),
        );
        let membership = WorkspaceMembership::try_new(
            MembershipId::new_v7(),
            subject.id(),
            selection.workspace_id(),
            MembershipLifecycle::Active,
            WorkspaceRole::Administrator,
            authorization.at(),
            authorization.at(),
        )?;
        insert_subject(&transaction, &subject)?;
        insert_anchor(&transaction, &anchor)?;
        insert_membership(&transaction, &membership)?;
        transaction.execute(
            "INSERT INTO auth_subject_profile_grants(auth_subject_id, profile_grant_id) VALUES (?1, ?2)",
            params![
                subject.id().to_string(),
                selection.selected_profile_grant_id().to_string(),
            ],
        )?;
        let grants = [selection.selected_profile_grant_id()];
        let created = crate::browser_auth::insert_session(
            &transaction,
            subject,
            selection.workspace_id(),
            &grants,
            grants[0],
            SessionPolicy::C1,
            false,
            authorization.at(),
            0,
            CapabilityKey::CreateBrowserSession,
            authorization.correlation_id(),
        )
        .map_err(|problem| session_creation_error(problem.code()))?;
        insert_session_authentication(
            &transaction,
            &created,
            authorization.identity().instance_id(),
            authorization.identity().provenance(),
        )?;
        insert_anchor_audit(
            &transaction,
            &anchor,
            authorization.correlation_id(),
            authorization.at(),
        )?;
        let installation =
            load_installation(&transaction)?.ok_or(HumanAccessStoreError::Integrity)?;
        insert_first_administrator_audit(
            &transaction,
            &installation,
            &subject,
            &membership,
            ceremony.id(),
            authorization.correlation_id(),
            authorization.at(),
        )?;
        insert_browser_session_issued_audit(
            &transaction,
            &created,
            &ceremony,
            subject.id(),
            authorization.correlation_id(),
            authorization.at(),
        )?;
        if !issue_browser_session {
            revoke_subject_sessions(
                &transaction,
                subject.id(),
                Some(selection.workspace_id()),
                subject.id(),
                authorization.correlation_id(),
                authorization.at(),
            )?;
        }
        let prior_state = ceremony.state();
        ceremony.complete(authorization.at())?;
        persist_ceremony_transition(&transaction, &ceremony, prior_state)?;
        insert_ceremony_audit(
            &transaction,
            AccessAuditEventKind::CeremonyCompleted,
            &ceremony,
            authorization.correlation_id(),
            authorization.at(),
        )?;
        transaction.commit()?;
        Ok(issue_browser_session.then_some(created))
    }

    fn persist_membership_lifecycle_change(
        &self,
        command: &ChangeMembershipLifecycleCommand,
    ) -> ApplicationResult<bool> {
        let capability = CapabilityKey::CreateBrowserSession;
        let correlation_id = command.proof().correlation_id();
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| access_problem(HumanAccessStoreError::Integrity, correlation_id))?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)
            .map_err(|error| {
            access_problem(HumanAccessStoreError::Storage(error), correlation_id)
        })?;
        let actor = authenticate_administrator_session(&transaction, command.proof())?;
        require_recent_authentication(&transaction, &actor, command.proof().now())
            .map_err(|error| access_problem(error, correlation_id))?;
        let mut membership = load_membership(&transaction, command.membership_id())
            .map_err(|error| access_problem(error, correlation_id))?
            .ok_or_else(|| access_problem(HumanAccessStoreError::NotFound, correlation_id))?;
        authorize_workspace_administrator(&transaction, &actor, membership.workspace_id())
            .map_err(|error| access_problem(error, correlation_id))?;
        let mut subject = load_subject(&transaction, membership.subject_id())
            .map_err(|error| access_problem(error, correlation_id))?;
        let viable = crate::browser_auth::viable_administrator_count(
            &transaction,
            &membership.workspace_id().to_string(),
            capability,
            correlation_id,
        )?;
        let effect = membership
            .apply_lifecycle_action(
                &mut subject,
                command.action().lifecycle_action(),
                u64::try_from(viable).map_err(|_| {
                    access_problem(HumanAccessStoreError::Integrity, correlation_id)
                })?,
                command.proof().now(),
            )
            .map_err(|error| {
                access_problem(HumanAccessStoreError::Invariant(error), correlation_id)
            })?
            .ok_or_else(|| access_problem(HumanAccessStoreError::Integrity, correlation_id))?;
        persist_membership_subject(&transaction, &membership, &subject)
            .map_err(|error| access_problem(error, correlation_id))?;
        if effect.revoke_browser_sessions() {
            revoke_subject_sessions(
                &transaction,
                subject.id(),
                Some(membership.workspace_id()),
                actor.subject().id(),
                correlation_id,
                command.proof().now(),
            )
            .map_err(|error| access_problem(error, correlation_id))?;
        }
        insert_membership_audit(
            &transaction,
            command.action().lifecycle_action().audit_event(),
            &membership,
            actor.subject().id(),
            correlation_id,
            command.proof().now(),
        )
        .map_err(|error| access_problem(error, correlation_id))?;
        transaction.commit().map_err(|error| {
            access_problem(HumanAccessStoreError::Storage(error), correlation_id)
        })?;
        Ok(true)
    }

    fn persist_membership_role_change(
        &self,
        command: &ChangeMembershipRoleCommand,
    ) -> ApplicationResult<bool> {
        let capability = CapabilityKey::CreateBrowserSession;
        let correlation_id = command.proof().correlation_id();
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| access_problem(HumanAccessStoreError::Integrity, correlation_id))?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)
            .map_err(|error| {
            access_problem(HumanAccessStoreError::Storage(error), correlation_id)
        })?;
        let actor = authenticate_administrator_session(&transaction, command.proof())?;
        require_recent_authentication(&transaction, &actor, command.proof().now())
            .map_err(|error| access_problem(error, correlation_id))?;
        let mut membership = load_membership(&transaction, command.membership_id())
            .map_err(|error| access_problem(error, correlation_id))?
            .ok_or_else(|| access_problem(HumanAccessStoreError::NotFound, correlation_id))?;
        authorize_workspace_administrator(&transaction, &actor, membership.workspace_id())
            .map_err(|error| access_problem(error, correlation_id))?;
        let mut subject = load_subject(&transaction, membership.subject_id())
            .map_err(|error| access_problem(error, correlation_id))?;
        let viable = crate::browser_auth::viable_administrator_count(
            &transaction,
            &membership.workspace_id().to_string(),
            capability,
            correlation_id,
        )?;
        let Some(_) = membership
            .change_role(
                &mut subject,
                command.role(),
                u64::try_from(viable).map_err(|_| {
                    access_problem(HumanAccessStoreError::Integrity, correlation_id)
                })?,
                command.proof().now(),
            )
            .map_err(|error| {
                access_problem(HumanAccessStoreError::Invariant(error), correlation_id)
            })?
        else {
            transaction.commit().map_err(|error| {
                access_problem(HumanAccessStoreError::Storage(error), correlation_id)
            })?;
            return Ok(false);
        };
        persist_membership_subject(&transaction, &membership, &subject)
            .map_err(|error| access_problem(error, correlation_id))?;
        insert_membership_audit(
            &transaction,
            membership.role().audit_event(),
            &membership,
            actor.subject().id(),
            correlation_id,
            command.proof().now(),
        )
        .map_err(|error| access_problem(error, correlation_id))?;
        transaction.commit().map_err(|error| {
            access_problem(HumanAccessStoreError::Storage(error), correlation_id)
        })?;
        Ok(true)
    }

    fn persist_subject_lifecycle_change(
        &self,
        command: &ChangeAuthSubjectLifecycleCommand,
    ) -> ApplicationResult<bool> {
        let correlation_id = command.proof().correlation_id();
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| access_problem(HumanAccessStoreError::Integrity, correlation_id))?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)
            .map_err(|error| {
            access_problem(HumanAccessStoreError::Storage(error), correlation_id)
        })?;
        let actor = authenticate_administrator_session(&transaction, command.proof())?;
        require_recent_authentication(&transaction, &actor, command.proof().now())
            .map_err(|error| access_problem(error, correlation_id))?;
        authorize_subject_administrator(&transaction, &actor, command.subject_id())
            .map_err(|error| access_problem(error, correlation_id))?;
        let mut subject = load_subject(&transaction, command.subject_id())
            .map_err(|error| access_problem(error, correlation_id))?;
        let leaving_active = matches!(subject.lifecycle(), AuthSubjectLifecycle::Active)
            && !matches!(command.action().lifecycle(), AuthSubjectLifecycle::Active);
        let mut memberships = if leaving_active {
            load_active_memberships(&transaction, subject.id())
                .map_err(|error| access_problem(error, correlation_id))?
        } else {
            Vec::new()
        };
        let continuity = AdministratorContinuity::for_subject_deactivation(
            sole_administrator_workspace_count(&transaction, subject.id())
                .map_err(|error| access_problem(error, correlation_id))?,
        );
        let Some(effect) = subject
            .transition_lifecycle(
                command.action().lifecycle(),
                continuity,
                command.proof().now(),
            )
            .map_err(|error| {
                access_problem(HumanAccessStoreError::Invariant(error), correlation_id)
            })?
        else {
            transaction.commit().map_err(|error| {
                access_problem(HumanAccessStoreError::Storage(error), correlation_id)
            })?;
            return Ok(false);
        };
        if !matches!(
            effect,
            AccessInvalidationEffect::SubjectAuthenticationChanged
        ) {
            return Err(access_problem(
                HumanAccessStoreError::Integrity,
                correlation_id,
            ));
        }
        for membership in &mut memberships {
            membership
                .apply_lifecycle_action(
                    &mut subject,
                    MembershipLifecycleAction::Suspend,
                    0,
                    command.proof().now(),
                )
                .map_err(|error| {
                    access_problem(HumanAccessStoreError::Invariant(error), correlation_id)
                })?;
            persist_membership(&transaction, membership)
                .map_err(|error| access_problem(error, correlation_id))?;
            insert_membership_audit(
                &transaction,
                AccessAuditEventKind::MembershipSuspended,
                membership,
                actor.subject().id(),
                correlation_id,
                command.proof().now(),
            )
            .map_err(|error| access_problem(error, correlation_id))?;
        }
        persist_subject(&transaction, &subject)
            .map_err(|error| access_problem(error, correlation_id))?;
        revoke_subject_sessions(
            &transaction,
            subject.id(),
            None,
            actor.subject().id(),
            correlation_id,
            command.proof().now(),
        )
        .map_err(|error| access_problem(error, correlation_id))?;
        insert_subject_audit(
            &transaction,
            &subject,
            actor.subject().id(),
            correlation_id,
            command.proof().now(),
        )
        .map_err(|error| access_problem(error, correlation_id))?;
        transaction.commit().map_err(|error| {
            access_problem(HumanAccessStoreError::Storage(error), correlation_id)
        })?;
        Ok(true)
    }
}

pub(crate) fn recover_auth_ceremonies_on_open(
    connection: &Connection,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<u64> {
    recover_auth_ceremonies(connection, correlation_id, at)
}

fn recover_auth_ceremonies(
    connection: &Connection,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<u64> {
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    let ceremonies = load_live_ceremonies(&transaction)?;
    let mut recovered = 0_u64;
    for mut ceremony in ceremonies {
        let prior_state = ceremony.state();
        if ceremony.recover_after_restart(at)? {
            persist_ceremony_transition(&transaction, &ceremony, prior_state)?;
            insert_ceremony_audit(
                &transaction,
                match ceremony.state() {
                    AuthCeremonyState::Failed => AccessAuditEventKind::CeremonyFailed,
                    AuthCeremonyState::CleanupUncertain => {
                        AccessAuditEventKind::CeremonyCleanupUncertain
                    }
                    _ => return Err(HumanAccessStoreError::Integrity),
                },
                &ceremony,
                correlation_id,
                at,
            )?;
            recovered = recovered
                .checked_add(1)
                .ok_or(HumanAccessStoreError::Integrity)?;
        }
    }
    prune_terminal_ceremonies(&transaction, at)?;
    prune_audit_age(&transaction, at)?;
    prune_audit_overflow(&transaction)?;
    transaction.commit()?;
    Ok(recovered)
}

fn maintain_ceremonies(
    transaction: &Transaction<'_>,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<Vec<OperationId>> {
    let ceremonies = load_expired_ceremonies(transaction, at)?;
    let mut expired = Vec::with_capacity(ceremonies.len());
    for mut ceremony in ceremonies {
        let prior_state = ceremony.state();
        if ceremony.expire(at)? {
            persist_ceremony_transition(transaction, &ceremony, prior_state)?;
            insert_ceremony_audit(
                transaction,
                AccessAuditEventKind::CeremonyExpired,
                &ceremony,
                correlation_id,
                at,
            )?;
            expired.push(ceremony.id());
        }
    }
    prune_terminal_ceremonies(transaction, at)?;
    prune_audit_age(transaction, at)?;
    prune_audit_overflow(transaction)?;
    Ok(expired)
}

fn load_installation(connection: &Connection) -> StoreResult<Option<TrailBaseInstallation>> {
    let row = connection
        .query_row(
            r#"
            SELECT trailbase_instance_id, physical_root_identity, release_lock_identity,
                   activation_state, activation_blocker, activation_generation, created_at,
                   updated_at
            FROM trailbase_installation WHERE singleton = 1
            "#,
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            },
        )
        .optional()?;
    row.map(
        |(id, root, release_lock, state, blocker, generation, created, updated)| {
            TrailBaseInstallation::try_from_persisted(
                id.parse().map_err(|_| HumanAccessStoreError::Integrity)?,
                root.parse().map_err(|_| HumanAccessStoreError::Integrity)?,
                release_lock
                    .map(|value| value.parse().map_err(|_| HumanAccessStoreError::Integrity))
                    .transpose()?,
                TrailBaseActivationState::from_storage(&state, blocker.as_deref())
                    .ok_or(HumanAccessStoreError::Integrity)?,
                u64::try_from(generation).map_err(|_| HumanAccessStoreError::Integrity)?,
                parse_time(&created)?,
                parse_time(&updated)?,
            )
            .map_err(|_| HumanAccessStoreError::Integrity)
        },
    )
    .transpose()
}

fn load_initialized_workspace(transaction: &Transaction<'_>) -> StoreResult<WorkspaceId> {
    transaction
        .query_row(
            r#"
            SELECT node.workspace_id
            FROM node_state node
            JOIN workspaces workspace ON workspace.workspace_id = node.workspace_id
            WHERE node.singleton = 1 AND node.initialized = 1
              AND node.initialization_consumed_at IS NOT NULL
              AND node.recovery_restore_attempt_id IS NULL
            "#,
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(HumanAccessStoreError::NotFound)?
        .parse()
        .map_err(|_| HumanAccessStoreError::Integrity)
}

fn load_subject(
    transaction: &Transaction<'_>,
    subject_id: AuthSubjectId,
) -> StoreResult<AuthSubject> {
    let row = transaction
        .query_row(
            r#"
            SELECT lifecycle, auth_epoch, authorization_epoch, created_at, updated_at
            FROM auth_subjects WHERE auth_subject_id = ?1
            "#,
            [subject_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or(HumanAccessStoreError::NotFound)?;
    AuthSubject::try_new(
        subject_id,
        AuthSubjectLifecycle::from_storage(&row.0).ok_or(HumanAccessStoreError::Integrity)?,
        u64::try_from(row.1).map_err(|_| HumanAccessStoreError::Integrity)?,
        u64::try_from(row.2).map_err(|_| HumanAccessStoreError::Integrity)?,
        parse_time(&row.3)?,
        parse_time(&row.4)?,
    )
    .map_err(HumanAccessStoreError::Invariant)
}

fn load_membership(
    transaction: &Transaction<'_>,
    membership_id: MembershipId,
) -> StoreResult<Option<WorkspaceMembership>> {
    let row = transaction
        .query_row(
            r#"
            SELECT auth_subject_id, workspace_id, lifecycle, role, created_at, updated_at
            FROM workspace_memberships WHERE membership_id = ?1
            "#,
            [membership_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()?;
    row.map(|row| {
        WorkspaceMembership::try_new(
            membership_id,
            row.0
                .parse()
                .map_err(|_| HumanAccessStoreError::Integrity)?,
            row.1
                .parse()
                .map_err(|_| HumanAccessStoreError::Integrity)?,
            MembershipLifecycle::from_storage(&row.2).ok_or(HumanAccessStoreError::Integrity)?,
            WorkspaceRole::from_storage(&row.3).ok_or(HumanAccessStoreError::Integrity)?,
            parse_time(&row.4)?,
            parse_time(&row.5)?,
        )
        .map_err(HumanAccessStoreError::Invariant)
    })
    .transpose()
}

fn validate_confirmed_ceremony(
    transaction: &Transaction<'_>,
    operation_id: OperationId,
    identity: ConfirmedTrailBaseIdentity,
    expected_purpose: AuthCeremonyPurpose,
    at: DateTime<Utc>,
) -> StoreResult<AuthCeremony> {
    let ceremony =
        load_ceremony(transaction, operation_id)?.ok_or(HumanAccessStoreError::NotFound)?;
    if !confirmed_ceremony_matches(&ceremony, identity, expected_purpose, at)
        || at >= ceremony.expires_at()
    {
        return Err(HumanAccessStoreError::NotFound);
    }
    require_active_installation(
        transaction,
        identity.instance_id(),
        identity.provenance().activation_generation(),
    )?;
    Ok(ceremony)
}

fn confirmed_ceremony_matches(
    ceremony: &AuthCeremony,
    identity: ConfirmedTrailBaseIdentity,
    expected_purpose: AuthCeremonyPurpose,
    at: DateTime<Utc>,
) -> bool {
    matches!(ceremony.state(), AuthCeremonyState::Claimed)
        && ceremony.purpose() == expected_purpose
        && matches!(
            ceremony.protocol(),
            AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce
        )
        && ceremony.trailbase_instance_id() == identity.instance_id()
        && ceremony.activation_generation() == identity.provenance().activation_generation()
        && identity.provenance().verified_at()
            >= ceremony.claimed_at().unwrap_or(ceremony.created_at())
        && identity.provenance().verified_at() <= at
}

fn fail_claimed_after_logout(
    transaction: &Transaction<'_>,
    ceremony: &mut AuthCeremony,
    failure: AuthCeremonyFailure,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<()> {
    let prior_state = ceremony.state();
    ceremony.fail(failure, at)?;
    persist_ceremony_transition(transaction, ceremony, prior_state)?;
    insert_ceremony_audit(
        transaction,
        AccessAuditEventKind::CeremonyFailed,
        ceremony,
        correlation_id,
        at,
    )
}

fn load_anchored_subject(
    transaction: &Transaction<'_>,
    identity: ConfirmedTrailBaseIdentity,
) -> StoreResult<AuthSubject> {
    let subject_id = transaction
        .query_row(
            r#"
            SELECT auth_subject_id
            FROM trailbase_auth_anchors
            WHERE trailbase_instance_id = ?1 AND trailbase_subject = ?2
            "#,
            params![
                identity.instance_id().to_string(),
                identity.subject().as_bytes().as_slice(),
            ],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(HumanAccessStoreError::NotFound)?
        .parse::<AuthSubjectId>()
        .map_err(|_| HumanAccessStoreError::Integrity)?;
    let subject = load_subject(transaction, subject_id)?;
    if !matches!(subject.lifecycle(), AuthSubjectLifecycle::Active) {
        return Err(HumanAccessStoreError::NotFound);
    }
    Ok(subject)
}

fn load_active_subject_grants(
    transaction: &Transaction<'_>,
    subject_id: AuthSubjectId,
    workspace_id: WorkspaceId,
) -> StoreResult<Vec<ProfileGrantId>> {
    let mut statement = transaction.prepare(
        r#"
        SELECT grant.grant_id
        FROM auth_subject_profile_grants subject_grant
        JOIN profile_grants grant ON grant.grant_id = subject_grant.profile_grant_id
        JOIN clients client ON client.client_id = grant.client_id
        WHERE subject_grant.auth_subject_id = ?1
          AND grant.workspace_id = ?2
          AND grant.status = 'active'
          AND client.status = 'active'
        ORDER BY grant.grant_id
        "#,
    )?;
    let rows = statement
        .query_map(
            params![subject_id.to_string(), workspace_id.to_string()],
            |row| row.get::<_, String>(0),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|row| {
            row.parse::<ProfileGrantId>()
                .map_err(|_| HumanAccessStoreError::Integrity)
        })
        .collect()
}

fn preauthorize_sign_in_identity(
    transaction: &Transaction<'_>,
    command: PreauthorizeTrailBaseSignInCommand,
) -> StoreResult<(AuthCeremony, AuthSubject)> {
    let ceremony = validate_confirmed_ceremony(
        transaction,
        command.operation_id(),
        command.identity(),
        AuthCeremonyPurpose::SignIn,
        command.at(),
    )?;
    let subject = load_anchored_subject(transaction, command.identity())?;
    Ok((ceremony, subject))
}

fn load_sign_in_candidates(
    transaction: &Transaction<'_>,
    subject_id: AuthSubjectId,
) -> StoreResult<Vec<SignInCandidate>> {
    let mut statement = transaction.prepare(
        r#"
        SELECT
            membership.membership_id,
            membership.workspace_id,
            workspace.created_at,
            membership.lifecycle,
            membership.role,
            grant.grant_id,
            grant.profile_id,
            profile.created_at,
            grant.client_id
        FROM workspace_memberships membership
        JOIN workspaces workspace
          ON workspace.workspace_id = membership.workspace_id
        JOIN auth_subject_profile_grants subject_grant
          ON subject_grant.auth_subject_id = membership.auth_subject_id
        JOIN profile_grants grant
          ON grant.grant_id = subject_grant.profile_grant_id
         AND grant.workspace_id = membership.workspace_id
        JOIN profiles profile
          ON profile.profile_id = grant.profile_id
         AND profile.workspace_id = grant.workspace_id
        JOIN clients client
          ON client.client_id = grant.client_id
         AND client.workspace_id = grant.workspace_id
        WHERE membership.auth_subject_id = ?1
          AND membership.lifecycle IN ('active', 'invited')
          AND grant.status = 'active'
          AND client.status = 'active'
        ORDER BY
            membership.workspace_id COLLATE BINARY,
            grant.profile_id COLLATE BINARY,
            grant.grant_id COLLATE BINARY,
            membership.membership_id COLLATE BINARY,
            grant.client_id COLLATE BINARY
        LIMIT ?2
        "#,
    )?;
    let rows = statement
        .query_map(
            params![
                subject_id.to_string(),
                i64::try_from(AUTH_SELECTION_CHOICE_LIMIT + 1)
                    .map_err(|_| HumanAccessStoreError::Integrity)?
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    if rows.len() > AUTH_SELECTION_CHOICE_LIMIT {
        return Err(HumanAccessStoreError::CapacityExceeded);
    }

    let mut candidates = Vec::with_capacity(rows.len());
    let mut current_workspace = None::<String>;
    let mut current_profile = None::<String>;
    let mut workspace_ordinal = 0_u8;
    let mut profile_ordinal = 0_u8;
    for row in rows {
        if current_workspace.as_deref() != Some(row.1.as_str()) {
            workspace_ordinal = workspace_ordinal
                .checked_add(1)
                .ok_or(HumanAccessStoreError::CapacityExceeded)?;
            profile_ordinal = 0;
            current_workspace = Some(row.1.clone());
            current_profile = None;
        }
        if current_profile.as_deref() != Some(row.6.as_str()) {
            profile_ordinal = profile_ordinal
                .checked_add(1)
                .ok_or(HumanAccessStoreError::CapacityExceeded)?;
            current_profile = Some(row.6.clone());
        }
        candidates.push(SignInCandidate {
            membership_id: row
                .0
                .parse()
                .map_err(|_| HumanAccessStoreError::Integrity)?,
            workspace_id: row
                .1
                .parse()
                .map_err(|_| HumanAccessStoreError::Integrity)?,
            workspace_created_at: parse_time(&row.2)?,
            membership_state: MembershipLifecycle::from_storage(&row.3)
                .ok_or(HumanAccessStoreError::Integrity)?,
            role: WorkspaceRole::from_storage(&row.4).ok_or(HumanAccessStoreError::Integrity)?,
            profile_grant_id: row
                .5
                .parse()
                .map_err(|_| HumanAccessStoreError::Integrity)?,
            profile_id: row
                .6
                .parse()
                .map_err(|_| HumanAccessStoreError::Integrity)?,
            profile_created_at: parse_time(&row.7)?,
            client_id: row
                .8
                .parse()
                .map_err(|_| HumanAccessStoreError::Integrity)?,
            workspace_ordinal,
            profile_ordinal,
        });
    }
    if candidates.is_empty() {
        Err(HumanAccessStoreError::Unaffiliated)
    } else {
        Ok(candidates)
    }
}

fn append_auth_selection_field(buffer: &mut Vec<u8>, value: &str) -> StoreResult<()> {
    let length = u32::try_from(value.len()).map_err(|_| HumanAccessStoreError::Integrity)?;
    buffer.extend_from_slice(&length.to_be_bytes());
    buffer.extend_from_slice(value.as_bytes());
    Ok(())
}

fn auth_selection_revision(
    ceremony: &AuthCeremony,
    candidates: &[SignInCandidate],
) -> StoreResult<Sha256Digest> {
    let confirmation = ceremony
        .confirmation()
        .ok_or(HumanAccessStoreError::Integrity)?;
    let mut bytes = Vec::with_capacity(256 * candidates.len().max(1));
    bytes.extend_from_slice(b"fasti.auth.selection.v1\0");
    bytes.extend_from_slice(&ceremony.activation_generation().to_be_bytes());
    bytes.extend_from_slice(&confirmation.auth_epoch().to_be_bytes());
    bytes.extend_from_slice(&confirmation.authorization_epoch().to_be_bytes());
    bytes.extend_from_slice(
        &u32::try_from(candidates.len())
            .map_err(|_| HumanAccessStoreError::CapacityExceeded)?
            .to_be_bytes(),
    );
    for (index, candidate) in candidates.iter().copied().enumerate() {
        bytes.push(u8::try_from(index).map_err(|_| HumanAccessStoreError::CapacityExceeded)?);
        bytes.push(candidate.workspace_ordinal);
        bytes.push(candidate.profile_ordinal);
        for value in [
            candidate.membership_id.to_string(),
            candidate.workspace_id.to_string(),
            timestamp(candidate.workspace_created_at),
            candidate.membership_state.as_str().to_owned(),
            candidate.role.as_str().to_owned(),
            candidate.profile_grant_id.to_string(),
            candidate.profile_id.to_string(),
            timestamp(candidate.profile_created_at),
            candidate.client_id.to_string(),
        ] {
            append_auth_selection_field(&mut bytes, &value)?;
        }
    }
    Ok(Sha256Digest::from_bytes(&sha256_bytes(&bytes)))
}

fn project_auth_selection(
    ceremony: &AuthCeremony,
    candidates: &[SignInCandidate],
) -> StoreResult<AuthSelectionProjection> {
    let choices = candidates
        .iter()
        .copied()
        .enumerate()
        .map(|(index, candidate)| {
            u8::try_from(index)
                .map(|ordinal| candidate.projection(ordinal))
                .map_err(|_| HumanAccessStoreError::CapacityExceeded)
        })
        .collect::<StoreResult<Vec<_>>>()?;
    Ok(AuthSelectionProjection::new(
        ceremony.expires_at(),
        ceremony.remembered(),
        auth_selection_revision(ceremony, candidates)?,
        choices,
    ))
}

fn authorize_bootstrap(
    transaction: &Transaction<'_>,
    command: &PreauthorizeTrailBaseBootstrapCommand,
) -> StoreResult<AuthCeremony> {
    let ceremony = validate_confirmed_ceremony(
        transaction,
        command.operation_id(),
        command.identity(),
        AuthCeremonyPurpose::FirstAdministratorBootstrap,
        command.at(),
    )?;
    let selection = ceremony
        .selection()
        .ok_or(HumanAccessStoreError::NotFound)?;
    if load_initialized_workspace(transaction)? != selection.workspace_id() {
        return Err(HumanAccessStoreError::NotFound);
    }
    let membership_count: i64 =
        transaction.query_row("SELECT COUNT(*) FROM workspace_memberships", [], |row| {
            row.get(0)
        })?;
    let anchor_exists: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM trailbase_auth_anchors)",
        [],
        |row| row.get(0),
    )?;
    let grant_active: bool = transaction.query_row(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM profile_grants grant
            JOIN clients client ON client.client_id = grant.client_id
            WHERE grant.grant_id = ?1 AND grant.workspace_id = ?2
              AND grant.status = 'active' AND client.status = 'active'
        )
        "#,
        params![
            selection.selected_profile_grant_id().to_string(),
            selection.workspace_id().to_string(),
        ],
        |row| row.get(0),
    )?;
    if membership_count != 0 || anchor_exists || !grant_active {
        return Err(HumanAccessStoreError::Conflict);
    }
    Ok(ceremony)
}

fn load_active_memberships(
    transaction: &Transaction<'_>,
    subject_id: AuthSubjectId,
) -> StoreResult<Vec<WorkspaceMembership>> {
    let mut statement = transaction.prepare(
        r#"
        SELECT membership_id FROM workspace_memberships
        WHERE auth_subject_id = ?1 AND lifecycle = 'active'
        ORDER BY membership_id
        "#,
    )?;
    let ids = statement
        .query_map([subject_id.to_string()], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    ids.into_iter()
        .map(|id| {
            let id = id.parse().map_err(|_| HumanAccessStoreError::Integrity)?;
            load_membership(transaction, id)?.ok_or(HumanAccessStoreError::Integrity)
        })
        .collect()
}

fn authorize_workspace_administrator(
    transaction: &Transaction<'_>,
    actor: &fasti_application::AuthenticatedBrowserSession,
    workspace_id: WorkspaceId,
) -> StoreResult<()> {
    if actor.session().workspace_id() != workspace_id {
        return Err(HumanAccessStoreError::NotFound);
    }
    let authorized: bool = transaction.query_row(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM workspace_memberships
            WHERE auth_subject_id = ?1 AND workspace_id = ?2
              AND lifecycle = 'active' AND role = 'administrator'
        )
        "#,
        params![actor.subject().id().to_string(), workspace_id.to_string()],
        |row| row.get(0),
    )?;
    if authorized {
        Ok(())
    } else {
        Err(HumanAccessStoreError::NotFound)
    }
}

fn authenticate_administrator_session(
    transaction: &Transaction<'_>,
    proof: &BrowserSessionMutationCommand,
) -> ApplicationResult<fasti_application::AuthenticatedBrowserSession> {
    crate::browser_auth::authenticate_session(
        transaction,
        proof.session_secret(),
        Some(proof.csrf_secret()),
        proof.now(),
        CapabilityKey::EndBrowserSession,
        proof.correlation_id(),
    )
    .map_err(|problem| {
        let code = match problem.code() {
            ProblemCode::CapabilityUnavailable => ProblemCode::CapabilityUnavailable,
            ProblemCode::IntegrityFailed => ProblemCode::IntegrityFailed,
            ProblemCode::StorageUnavailable => ProblemCode::StorageUnavailable,
            _ => ProblemCode::Forbidden,
        };
        Box::new(FastiProblem::from_code(
            code,
            CapabilityKey::CreateBrowserSession,
            proof.correlation_id(),
        ))
    })
}

fn require_recent_authentication(
    transaction: &Transaction<'_>,
    actor: &fasti_application::AuthenticatedBrowserSession,
    at: DateTime<Utc>,
) -> StoreResult<()> {
    let row = transaction
        .query_row(
            r#"
        SELECT authentication.method, authentication.verified_at,
               authentication.recent_authentication_expires_at,
               authentication.activation_generation,
               installation.activation_state, installation.activation_generation
        FROM fasti_browser_session_authentication authentication
        JOIN trailbase_installation installation
          ON installation.trailbase_instance_id = authentication.trailbase_instance_id
        WHERE authentication.browser_session_id = ?1
        "#,
            [actor.session().id().to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )
        .optional()?
        .ok_or(HumanAccessStoreError::NotFound)?;
    let provenance = AuthenticationProvenance::new(
        AuthenticationMethod::from_storage(&row.0).ok_or(HumanAccessStoreError::Integrity)?,
        parse_time(&row.1)?,
        u64::try_from(row.3).map_err(|_| HumanAccessStoreError::Integrity)?,
    );
    let recent = RecentAuthentication::try_new(
        actor.subject().id(),
        provenance,
        actor.subject().auth_epoch(),
        parse_time(row.2.as_deref().ok_or(HumanAccessStoreError::NotFound)?)?,
        Duration::minutes(10),
    )?;
    let installation_generation =
        u64::try_from(row.5).map_err(|_| HumanAccessStoreError::Integrity)?;
    if row.4 == "active"
        && recent.satisfies(
            &actor.subject(),
            installation_generation,
            AuthenticationAssurance::SingleFactor,
            at,
        )
    {
        Ok(())
    } else {
        Err(HumanAccessStoreError::NotFound)
    }
}

fn authorize_subject_administrator(
    transaction: &Transaction<'_>,
    actor: &fasti_application::AuthenticatedBrowserSession,
    subject_id: AuthSubjectId,
) -> StoreResult<()> {
    let (affected, authorized): (i64, i64) = transaction.query_row(
        r#"
        SELECT
            COUNT(*),
            COALESCE(SUM(CASE WHEN EXISTS(
                SELECT 1 FROM workspace_memberships actor
                WHERE actor.auth_subject_id = ?1
                  AND actor.workspace_id = target.workspace_id
                  AND actor.lifecycle = 'active'
                  AND actor.role = 'administrator'
            ) THEN 1 ELSE 0 END), 0)
        FROM workspace_memberships target
        WHERE target.auth_subject_id = ?2 AND target.lifecycle <> 'removed'
        "#,
        params![actor.subject().id().to_string(), subject_id.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if affected > 0 && affected == authorized {
        Ok(())
    } else {
        Err(HumanAccessStoreError::NotFound)
    }
}

fn sole_administrator_workspace_count(
    transaction: &Transaction<'_>,
    subject_id: AuthSubjectId,
) -> StoreResult<u64> {
    let count: i64 = transaction.query_row(
        r#"
        SELECT COUNT(*)
        FROM workspace_memberships target
        JOIN auth_subjects target_subject
          ON target_subject.auth_subject_id = target.auth_subject_id
        WHERE target.auth_subject_id = ?1
          AND target.lifecycle = 'active'
          AND target.role = 'administrator'
          AND target_subject.lifecycle = 'active'
          AND 1 = (
              SELECT COUNT(*)
              FROM workspace_memberships viable
              JOIN auth_subjects viable_subject
                ON viable_subject.auth_subject_id = viable.auth_subject_id
              WHERE viable.workspace_id = target.workspace_id
                AND viable.lifecycle = 'active'
                AND viable.role = 'administrator'
                AND viable_subject.lifecycle = 'active'
          )
        "#,
        [subject_id.to_string()],
        |row| row.get(0),
    )?;
    u64::try_from(count).map_err(|_| HumanAccessStoreError::Integrity)
}

fn persist_membership_subject(
    transaction: &Transaction<'_>,
    membership: &WorkspaceMembership,
    subject: &AuthSubject,
) -> StoreResult<()> {
    persist_subject(transaction, subject)?;
    persist_membership(transaction, membership)
}

fn insert_subject(transaction: &Transaction<'_>, subject: &AuthSubject) -> StoreResult<()> {
    map_insert(transaction.execute(
        r#"
        INSERT INTO auth_subjects(
            auth_subject_id, lifecycle, auth_epoch, authorization_epoch,
            created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
        params![
            subject.id().to_string(),
            subject.lifecycle().as_str(),
            i64::try_from(subject.auth_epoch()).map_err(|_| HumanAccessStoreError::Integrity)?,
            i64::try_from(subject.authorization_epoch())
                .map_err(|_| HumanAccessStoreError::Integrity)?,
            timestamp(subject.created_at()),
            timestamp(subject.updated_at()),
        ],
    ))?;
    Ok(())
}

fn insert_anchor(
    transaction: &Transaction<'_>,
    anchor: &TrailBaseExternalAnchor,
) -> StoreResult<()> {
    map_insert(transaction.execute(
        r#"
        INSERT INTO trailbase_auth_anchors(
            trailbase_instance_id, trailbase_subject, auth_subject_id, linked_at
        ) VALUES (?1, ?2, ?3, ?4)
        "#,
        params![
            anchor.trailbase_instance_id().to_string(),
            anchor.trailbase_subject().as_bytes().as_slice(),
            anchor.auth_subject_id().to_string(),
            timestamp(anchor.linked_at()),
        ],
    ))?;
    Ok(())
}

fn insert_membership(
    transaction: &Transaction<'_>,
    membership: &WorkspaceMembership,
) -> StoreResult<()> {
    map_insert(transaction.execute(
        r#"
        INSERT INTO workspace_memberships(
            membership_id, auth_subject_id, workspace_id, lifecycle,
            role, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        params![
            membership.id().to_string(),
            membership.subject_id().to_string(),
            membership.workspace_id().to_string(),
            membership.lifecycle().as_str(),
            membership.role().as_str(),
            timestamp(membership.created_at()),
            timestamp(membership.updated_at()),
        ],
    ))?;
    Ok(())
}

fn persist_membership(
    transaction: &Transaction<'_>,
    membership: &WorkspaceMembership,
) -> StoreResult<()> {
    let changed = transaction.execute(
        r#"
        UPDATE workspace_memberships
        SET lifecycle = ?1, role = ?2, updated_at = ?3
        WHERE membership_id = ?4 AND auth_subject_id = ?5 AND workspace_id = ?6
        "#,
        params![
            membership.lifecycle().as_str(),
            membership.role().as_str(),
            timestamp(membership.updated_at()),
            membership.id().to_string(),
            membership.subject_id().to_string(),
            membership.workspace_id().to_string(),
        ],
    )?;
    if changed == 1 {
        Ok(())
    } else {
        Err(HumanAccessStoreError::Conflict)
    }
}

fn persist_subject(transaction: &Transaction<'_>, subject: &AuthSubject) -> StoreResult<()> {
    let changed = transaction.execute(
        r#"
        UPDATE auth_subjects
        SET lifecycle = ?1, auth_epoch = ?2, authorization_epoch = ?3, updated_at = ?4
        WHERE auth_subject_id = ?5
        "#,
        params![
            subject.lifecycle().as_str(),
            i64::try_from(subject.auth_epoch()).map_err(|_| HumanAccessStoreError::Integrity)?,
            i64::try_from(subject.authorization_epoch())
                .map_err(|_| HumanAccessStoreError::Integrity)?,
            timestamp(subject.updated_at()),
            subject.id().to_string(),
        ],
    )?;
    if changed == 1 {
        Ok(())
    } else {
        Err(HumanAccessStoreError::Conflict)
    }
}

fn revoke_subject_sessions(
    transaction: &Transaction<'_>,
    subject_id: AuthSubjectId,
    workspace_id: Option<WorkspaceId>,
    actor_id: AuthSubjectId,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<u64> {
    let rows = {
        let (sql, workspace) = match workspace_id {
            Some(workspace_id) => (
                r#"
                SELECT session.browser_session_id, session.workspace_id,
                       anchor.trailbase_instance_id
                FROM fasti_browser_sessions session
                JOIN trailbase_auth_anchors anchor
                  ON anchor.auth_subject_id = session.auth_subject_id
                WHERE session.auth_subject_id = ?1 AND session.workspace_id = ?2
                  AND session.revoked_at IS NULL
                ORDER BY session.browser_session_id
                "#,
                Some(workspace_id.to_string()),
            ),
            None => (
                r#"
                SELECT session.browser_session_id, session.workspace_id,
                       anchor.trailbase_instance_id
                FROM fasti_browser_sessions session
                JOIN trailbase_auth_anchors anchor
                  ON anchor.auth_subject_id = session.auth_subject_id
                WHERE session.auth_subject_id = ?1 AND session.revoked_at IS NULL
                ORDER BY session.browser_session_id
                "#,
                None,
            ),
        };
        let mut statement = transaction.prepare(sql)?;
        let read = |row: &rusqlite::Row<'_>| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        };
        match workspace {
            Some(workspace) => statement
                .query_map(params![subject_id.to_string(), workspace], read)?
                .collect::<Result<Vec<_>, _>>()?,
            None => statement
                .query_map([subject_id.to_string()], read)?
                .collect::<Result<Vec<_>, _>>()?,
        }
    };
    for (session_id, workspace_id, instance_id) in &rows {
        let changed = transaction.execute(
            "UPDATE fasti_browser_sessions SET revoked_at = ?1 WHERE browser_session_id = ?2 AND revoked_at IS NULL",
            params![timestamp(at), session_id],
        )?;
        if changed != 1 {
            return Err(HumanAccessStoreError::Conflict);
        }
        insert_browser_session_revocation_audit(
            transaction,
            instance_id,
            subject_id,
            actor_id,
            workspace_id,
            session_id,
            correlation_id,
            at,
        )?;
    }
    u64::try_from(rows.len()).map_err(|_| HumanAccessStoreError::Integrity)
}

fn persist_installation(
    transaction: &Transaction<'_>,
    installation: &TrailBaseInstallation,
    insert: bool,
) -> StoreResult<()> {
    let generation = i64::try_from(installation.activation_generation())
        .map_err(|_| HumanAccessStoreError::Integrity)?;
    let (state, blocker) = match installation.activation_state() {
        TrailBaseActivationState::Inactive => ("inactive", None),
        TrailBaseActivationState::Active => ("active", None),
        TrailBaseActivationState::Blocked(blocker) => ("blocked", Some(blocker.as_str())),
    };
    let changed = if insert {
        transaction.execute(
            r#"
            INSERT INTO trailbase_installation(
                singleton, trailbase_instance_id, physical_root_identity, release_lock_identity,
                activation_state, activation_blocker, activation_generation,
                created_at, updated_at
            ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                installation.id().to_string(),
                installation.physical_root_identity().as_str(),
                installation
                    .release_lock_identity()
                    .map(Sha256Digest::as_str),
                state,
                blocker,
                generation,
                timestamp(installation.created_at()),
                timestamp(installation.updated_at()),
            ],
        )?
    } else {
        transaction.execute(
            r#"
            UPDATE trailbase_installation
            SET release_lock_identity = ?1, activation_state = ?2, activation_blocker = ?3,
                activation_generation = ?4, updated_at = ?5
            WHERE singleton = 1 AND trailbase_instance_id = ?6
            "#,
            params![
                installation
                    .release_lock_identity()
                    .map(Sha256Digest::as_str),
                state,
                blocker,
                generation,
                timestamp(installation.updated_at()),
                installation.id().to_string(),
            ],
        )?
    };
    if changed == 1 {
        Ok(())
    } else {
        Err(HumanAccessStoreError::Integrity)
    }
}

fn require_active_installation(
    transaction: &Transaction<'_>,
    instance_id: TrailBaseInstanceId,
    generation: u64,
) -> StoreResult<()> {
    let generation = i64::try_from(generation).map_err(|_| HumanAccessStoreError::Integrity)?;
    let active: bool = transaction.query_row(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM trailbase_installation
            WHERE singleton = 1 AND trailbase_instance_id = ?1
              AND activation_state = 'active' AND activation_generation = ?2
        )
        "#,
        params![instance_id.to_string(), generation],
        |row| row.get(0),
    )?;
    if active {
        Ok(())
    } else {
        Err(HumanAccessStoreError::TrustUnavailable)
    }
}

fn load_ceremony(
    transaction: &Transaction<'_>,
    operation_id: OperationId,
) -> StoreResult<Option<AuthCeremony>> {
    load_ceremony_where(transaction, "operation_id = ?1", operation_id.to_string())
}

fn load_ceremony_by_binding(
    transaction: &Transaction<'_>,
    digest: &Sha256Digest,
) -> StoreResult<Option<AuthCeremony>> {
    load_ceremony_where(
        transaction,
        "browser_binding_digest = ?1",
        digest.as_str().to_owned(),
    )
}

fn load_ceremony_where(
    transaction: &Transaction<'_>,
    predicate: &str,
    value: String,
) -> StoreResult<Option<AuthCeremony>> {
    let sql = format!("SELECT {CEREMONY_COLUMNS} FROM auth_ceremonies WHERE {predicate}");
    transaction
        .query_row(&sql, [value], read_ceremony_row)
        .optional()?
        .map(parse_ceremony)
        .transpose()
}

fn load_live_ceremonies(transaction: &Transaction<'_>) -> StoreResult<Vec<AuthCeremony>> {
    load_ceremonies(
        transaction,
        "WHERE state IN ('pending', 'claimed') ORDER BY operation_id",
        [],
    )
}

fn load_expired_ceremonies(
    transaction: &Transaction<'_>,
    at: DateTime<Utc>,
) -> StoreResult<Vec<AuthCeremony>> {
    load_ceremonies(
        transaction,
        "WHERE state IN ('pending', 'selection_required') AND expires_at <= ?1 ORDER BY expires_at, operation_id",
        [timestamp(at)],
    )
}

fn load_ceremonies<P>(
    transaction: &Transaction<'_>,
    suffix: &str,
    params: P,
) -> StoreResult<Vec<AuthCeremony>>
where
    P: rusqlite::Params,
{
    let sql = format!("SELECT {CEREMONY_COLUMNS} FROM auth_ceremonies {suffix}");
    let mut statement = transaction.prepare(&sql)?;
    let rows = statement
        .query_map(params, read_ceremony_row)?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(parse_ceremony).collect()
}

fn read_ceremony_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CeremonyRow> {
    Ok(CeremonyRow {
        operation_id: row.get(0)?,
        purpose: row.get(1)?,
        protocol: row.get(2)?,
        trailbase_instance_id: row.get(3)?,
        activation_generation: row.get(4)?,
        browser_binding_digest: row.get(5)?,
        workspace_id: row.get(6)?,
        selected_profile_grant_id: row.get(7)?,
        bound_browser_session_id: row.get(8)?,
        invited_membership_id: row.get(9)?,
        remembered: row.get(10)?,
        confirmed_auth_subject_id: row.get(11)?,
        authentication_method: row.get(12)?,
        authentication_verified_at: row.get(13)?,
        confirmed_auth_epoch: row.get(14)?,
        confirmed_authorization_epoch: row.get(15)?,
        callback_path: row.get(16)?,
        return_target: row.get(17)?,
        correlation_id: row.get(18)?,
        state: row.get(19)?,
        failure: row.get(20)?,
        created_at: row.get(21)?,
        expires_at: row.get(22)?,
        claimed_at: row.get(23)?,
        terminal_at: row.get(24)?,
    })
}

fn parse_ceremony(row: CeremonyRow) -> StoreResult<AuthCeremony> {
    let purpose =
        AuthCeremonyPurpose::from_storage(&row.purpose).ok_or(HumanAccessStoreError::Integrity)?;
    let activation_generation =
        u64::try_from(row.activation_generation).map_err(|_| HumanAccessStoreError::Integrity)?;
    let selection = match (
        row.workspace_id.as_deref(),
        row.selected_profile_grant_id.as_deref(),
    ) {
        (Some(workspace_id), Some(profile_grant_id)) => Some(
            AuthCeremonySelection::try_new(
                purpose,
                workspace_id
                    .parse::<WorkspaceId>()
                    .map_err(|_| HumanAccessStoreError::Integrity)?,
                profile_grant_id
                    .parse::<ProfileGrantId>()
                    .map_err(|_| HumanAccessStoreError::Integrity)?,
                row.bound_browser_session_id
                    .as_deref()
                    .map(str::parse::<BrowserSessionId>)
                    .transpose()
                    .map_err(|_| HumanAccessStoreError::Integrity)?,
                row.invited_membership_id
                    .as_deref()
                    .map(str::parse::<MembershipId>)
                    .transpose()
                    .map_err(|_| HumanAccessStoreError::Integrity)?,
            )
            .map_err(|_| HumanAccessStoreError::Integrity)?,
        ),
        (None, None)
            if row.bound_browser_session_id.is_none() && row.invited_membership_id.is_none() =>
        {
            None
        }
        _ => return Err(HumanAccessStoreError::Integrity),
    };
    let confirmation = match (
        row.confirmed_auth_subject_id.as_deref(),
        row.authentication_method.as_deref(),
        row.authentication_verified_at.as_deref(),
        row.confirmed_auth_epoch,
        row.confirmed_authorization_epoch,
    ) {
        (
            Some(subject_id),
            Some(method),
            Some(verified_at),
            Some(auth_epoch),
            Some(authorization_epoch),
        ) => Some(AuthCeremonyConfirmation::try_from_persisted(
            subject_id
                .parse::<AuthSubjectId>()
                .map_err(|_| HumanAccessStoreError::Integrity)?,
            u64::try_from(auth_epoch).map_err(|_| HumanAccessStoreError::Integrity)?,
            u64::try_from(authorization_epoch).map_err(|_| HumanAccessStoreError::Integrity)?,
            AuthenticationProvenance::new(
                AuthenticationMethod::from_storage(method)
                    .ok_or(HumanAccessStoreError::Integrity)?,
                parse_time(verified_at)?,
                activation_generation,
            ),
        )),
        (None, None, None, None, None) => None,
        _ => return Err(HumanAccessStoreError::Integrity),
    };
    let failure = row
        .failure
        .as_deref()
        .map(|value| {
            AuthCeremonyFailure::from_storage(value).ok_or(HumanAccessStoreError::Integrity)
        })
        .transpose()?;
    AuthCeremony::try_from_persisted(
        row.operation_id
            .parse()
            .map_err(|_| HumanAccessStoreError::Integrity)?,
        purpose,
        AuthCeremonyProtocol::from_storage(&row.protocol)
            .ok_or(HumanAccessStoreError::Integrity)?,
        row.trailbase_instance_id
            .parse()
            .map_err(|_| HumanAccessStoreError::Integrity)?,
        activation_generation,
        row.browser_binding_digest
            .parse()
            .map_err(|_| HumanAccessStoreError::Integrity)?,
        selection,
        match row.remembered {
            0 => false,
            1 => true,
            _ => return Err(HumanAccessStoreError::Integrity),
        },
        confirmation,
        AuthCallbackPath::parse(row.callback_path).map_err(|_| HumanAccessStoreError::Integrity)?,
        AuthReturnTarget::from_storage(&row.return_target)
            .ok_or(HumanAccessStoreError::Integrity)?,
        row.correlation_id
            .parse()
            .map_err(|_| HumanAccessStoreError::Integrity)?,
        AuthCeremonyState::from_storage(&row.state).ok_or(HumanAccessStoreError::Integrity)?,
        failure,
        parse_time(&row.created_at)?,
        parse_time(&row.expires_at)?,
        row.claimed_at.as_deref().map(parse_time).transpose()?,
        row.terminal_at.as_deref().map(parse_time).transpose()?,
    )
    .map_err(|_| HumanAccessStoreError::Integrity)
}

fn persist_ceremony_transition(
    transaction: &Transaction<'_>,
    ceremony: &AuthCeremony,
    prior_state: AuthCeremonyState,
) -> StoreResult<()> {
    let changed = transaction.execute(
        r#"
        UPDATE auth_ceremonies
        SET workspace_id = ?1, selected_profile_grant_id = ?2,
            bound_browser_session_id = ?3, invited_membership_id = ?4,
            confirmed_auth_subject_id = ?5, authentication_method = ?6,
            authentication_verified_at = ?7, confirmed_auth_epoch = ?8,
            confirmed_authorization_epoch = ?9, state = ?10, failure = ?11,
            claimed_at = ?12, terminal_at = ?13
        WHERE operation_id = ?14 AND state = ?15
        "#,
        params![
            ceremony
                .selection()
                .map(|selection| selection.workspace_id().to_string()),
            ceremony
                .selection()
                .map(|selection| selection.selected_profile_grant_id().to_string()),
            ceremony
                .selection()
                .and_then(AuthCeremonySelection::bound_browser_session_id)
                .map(|id| id.to_string()),
            ceremony
                .selection()
                .and_then(AuthCeremonySelection::invited_membership_id)
                .map(|id| id.to_string()),
            ceremony
                .confirmation()
                .map(|confirmation| confirmation.subject_id().to_string()),
            ceremony
                .confirmation()
                .map(|confirmation| confirmation.provenance().method().as_str()),
            ceremony
                .confirmation()
                .map(|confirmation| { timestamp(confirmation.provenance().verified_at()) }),
            ceremony
                .confirmation()
                .map(|confirmation| i64::try_from(confirmation.auth_epoch()))
                .transpose()
                .map_err(|_| HumanAccessStoreError::Integrity)?,
            ceremony
                .confirmation()
                .map(|confirmation| i64::try_from(confirmation.authorization_epoch()))
                .transpose()
                .map_err(|_| HumanAccessStoreError::Integrity)?,
            ceremony.state().as_str(),
            ceremony.failure().map(AuthCeremonyFailure::as_str),
            ceremony.claimed_at().map(timestamp),
            ceremony.terminal_at().map(timestamp),
            ceremony.id().to_string(),
            prior_state.as_str(),
        ],
    )?;
    if changed == 1 {
        Ok(())
    } else {
        Err(HumanAccessStoreError::Conflict)
    }
}

fn insert_installation_audit(
    transaction: &Transaction<'_>,
    event: AccessAuditEventKind,
    instance_id: TrailBaseInstanceId,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<()> {
    prune_audit_age(transaction, at)?;
    transaction.execute(
        r#"
        INSERT INTO access_audit_events(
            event_kind, trailbase_instance_id, correlation_id, occurred_at
        ) VALUES (?1, ?2, ?3, ?4)
        "#,
        params![
            event.as_str(),
            instance_id.to_string(),
            correlation_id.to_string(),
            timestamp(at),
        ],
    )?;
    prune_audit_overflow(transaction)?;
    Ok(())
}

fn insert_anchor_audit(
    transaction: &Transaction<'_>,
    anchor: &TrailBaseExternalAnchor,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<()> {
    prune_audit_age(transaction, at)?;
    transaction.execute(
        r#"
        INSERT INTO access_audit_events(
            event_kind, trailbase_instance_id, auth_subject_id,
            correlation_id, occurred_at
        ) VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        params![
            AccessAuditEventKind::AnchorLinked.as_str(),
            anchor.trailbase_instance_id().to_string(),
            anchor.auth_subject_id().to_string(),
            correlation_id.to_string(),
            timestamp(at),
        ],
    )?;
    prune_audit_overflow(transaction)?;
    Ok(())
}

fn insert_membership_audit(
    transaction: &Transaction<'_>,
    event: AccessAuditEventKind,
    membership: &WorkspaceMembership,
    actor_id: AuthSubjectId,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<()> {
    prune_audit_age(transaction, at)?;
    transaction.execute(
        r#"
        INSERT INTO access_audit_events(
            event_kind, auth_subject_id, actor_auth_subject_id, workspace_id,
            membership_id, correlation_id, occurred_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        params![
            event.as_str(),
            membership.subject_id().to_string(),
            actor_id.to_string(),
            membership.workspace_id().to_string(),
            membership.id().to_string(),
            correlation_id.to_string(),
            timestamp(at),
        ],
    )?;
    prune_audit_overflow(transaction)?;
    Ok(())
}

fn insert_subject_audit(
    transaction: &Transaction<'_>,
    subject: &AuthSubject,
    actor_id: AuthSubjectId,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<()> {
    prune_audit_age(transaction, at)?;
    transaction.execute(
        r#"
        INSERT INTO access_audit_events(
            event_kind, auth_subject_id, actor_auth_subject_id,
            correlation_id, occurred_at
        ) VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        params![
            subject.lifecycle().audit_event().as_str(),
            subject.id().to_string(),
            actor_id.to_string(),
            correlation_id.to_string(),
            timestamp(at),
        ],
    )?;
    prune_audit_overflow(transaction)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_browser_session_revocation_audit(
    transaction: &Transaction<'_>,
    instance_id: &str,
    subject_id: AuthSubjectId,
    actor_id: AuthSubjectId,
    workspace_id: &str,
    session_id: &str,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<()> {
    prune_audit_age(transaction, at)?;
    transaction.execute(
        r#"
        INSERT INTO access_audit_events(
            event_kind, trailbase_instance_id, auth_subject_id,
            actor_auth_subject_id, workspace_id, browser_session_id,
            correlation_id, occurred_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
        params![
            AccessAuditEventKind::BrowserSessionRevoked.as_str(),
            instance_id,
            subject_id.to_string(),
            actor_id.to_string(),
            workspace_id,
            session_id,
            correlation_id.to_string(),
            timestamp(at),
        ],
    )?;
    prune_audit_overflow(transaction)?;
    Ok(())
}

fn insert_session_authentication(
    transaction: &Transaction<'_>,
    created: &CreatedBrowserSession,
    trailbase_instance_id: TrailBaseInstanceId,
    provenance: AuthenticationProvenance,
) -> StoreResult<()> {
    map_insert(transaction.execute(
        r#"
        INSERT INTO fasti_browser_session_authentication(
            browser_session_id, trailbase_instance_id, activation_generation,
            method, verified_at, recent_authentication_expires_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, NULL)
        "#,
        params![
            created.session().id().to_string(),
            trailbase_instance_id.to_string(),
            i64::try_from(provenance.activation_generation())
                .map_err(|_| HumanAccessStoreError::Integrity)?,
            provenance.method().as_str(),
            timestamp(provenance.verified_at()),
        ],
    ))?;
    Ok(())
}

fn insert_browser_session_issued_audit(
    transaction: &Transaction<'_>,
    created: &CreatedBrowserSession,
    ceremony: &AuthCeremony,
    subject_id: AuthSubjectId,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<()> {
    prune_audit_age(transaction, at)?;
    transaction.execute(
        r#"
        INSERT INTO access_audit_events(
            event_kind, trailbase_instance_id, auth_subject_id, workspace_id,
            operation_id, browser_session_id, correlation_id, occurred_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
        params![
            AccessAuditEventKind::BrowserSessionIssued.as_str(),
            ceremony.trailbase_instance_id().to_string(),
            subject_id.to_string(),
            created.session().workspace_id().to_string(),
            ceremony.id().to_string(),
            created.session().id().to_string(),
            correlation_id.to_string(),
            timestamp(at),
        ],
    )?;
    prune_audit_overflow(transaction)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_first_administrator_audit(
    transaction: &Transaction<'_>,
    installation: &TrailBaseInstallation,
    subject: &AuthSubject,
    membership: &WorkspaceMembership,
    operation_id: OperationId,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<()> {
    prune_audit_age(transaction, at)?;
    transaction.execute(
        r#"
        INSERT INTO access_audit_events(
            event_kind, trailbase_instance_id, auth_subject_id, workspace_id,
            membership_id, operation_id, correlation_id, occurred_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
        params![
            AccessAuditEventKind::FirstAdministratorBootstrapped.as_str(),
            installation.id().to_string(),
            subject.id().to_string(),
            membership.workspace_id().to_string(),
            membership.id().to_string(),
            operation_id.to_string(),
            correlation_id.to_string(),
            timestamp(at),
        ],
    )?;
    prune_audit_overflow(transaction)?;
    Ok(())
}

fn insert_ceremony_audit(
    transaction: &Transaction<'_>,
    event: AccessAuditEventKind,
    ceremony: &AuthCeremony,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<()> {
    prune_audit_age(transaction, at)?;
    transaction.execute(
        r#"
        INSERT INTO access_audit_events(
            event_kind, trailbase_instance_id, operation_id,
            correlation_id, occurred_at
        ) VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        params![
            event.as_str(),
            ceremony.trailbase_instance_id().to_string(),
            ceremony.id().to_string(),
            correlation_id.to_string(),
            timestamp(at),
        ],
    )?;
    prune_audit_overflow(transaction)?;
    Ok(())
}

fn prune_terminal_ceremonies(transaction: &Transaction<'_>, at: DateTime<Utc>) -> StoreResult<()> {
    let cutoff = at
        .checked_sub_signed(Duration::hours(CEREMONY_RETENTION_HOURS))
        .ok_or(HumanAccessStoreError::Integrity)?;
    transaction.execute(
        "DELETE FROM auth_ceremonies WHERE terminal_at IS NOT NULL AND terminal_at <= ?1",
        [timestamp(cutoff)],
    )?;
    Ok(())
}

fn prune_audit_age(transaction: &Transaction<'_>, at: DateTime<Utc>) -> StoreResult<()> {
    let cutoff = at
        .checked_sub_signed(Duration::days(AUDIT_RETENTION_DAYS))
        .ok_or(HumanAccessStoreError::Integrity)?;
    transaction.execute(
        "DELETE FROM access_audit_events WHERE occurred_at <= ?1",
        [timestamp(cutoff)],
    )?;
    Ok(())
}

fn prune_audit_overflow(transaction: &Transaction<'_>) -> StoreResult<()> {
    transaction.execute(
        r#"
        DELETE FROM access_audit_events
        WHERE audit_event_id IN (
            SELECT audit_event_id FROM access_audit_events
            ORDER BY occurred_at DESC, audit_event_id DESC
            LIMIT -1 OFFSET ?1
        )
        "#,
        [AUDIT_CAPACITY],
    )?;
    Ok(())
}

fn map_insert(result: rusqlite::Result<usize>) -> StoreResult<usize> {
    result.map_err(|error| match &error {
        rusqlite::Error::SqliteFailure(detail, _)
            if detail.code == ErrorCode::ConstraintViolation =>
        {
            HumanAccessStoreError::Conflict
        }
        _ => HumanAccessStoreError::Storage(error),
    })
}

fn parse_time(value: &str) -> StoreResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| HumanAccessStoreError::Integrity)
}

impl HumanAccessPort for SqliteKernel {
    fn read_trailbase_installation(
        &self,
        query: ReadTrailBaseInstallationQuery,
    ) -> ApplicationResult<Option<TrailBaseInstallation>> {
        let correlation_id = query.correlation_id();
        let connection =
            self.lock_connection(CapabilityKey::ReadAccessProjection, correlation_id)?;
        load_installation(&connection).map_err(|error| access_problem(error, correlation_id))
    }

    fn verify_trailbase_installation(
        &self,
        command: VerifyTrailBaseInstallationCommand,
    ) -> ApplicationResult<TrailBaseInstallation> {
        SqliteKernel::verify_trailbase_installation(
            self,
            command.instance_id(),
            command.observed_root_identity().clone(),
            command.release_lock_identity().clone(),
            command.declared_restore(),
            command.correlation_id(),
            command.at(),
        )
        .map_err(|error| access_problem(error, command.correlation_id()))
    }

    fn start_auth_ceremony(&self, command: StartAuthCeremonyCommand) -> ApplicationResult<()> {
        self.insert_auth_ceremony(command.ceremony())
            .map_err(|error| access_problem(error, command.ceremony().correlation_id()))
    }

    fn start_trailbase_bootstrap(
        &self,
        command: StartTrailBaseBootstrapCommand,
    ) -> ApplicationResult<()> {
        verify_bootstrap_secret(
            self,
            command.bootstrap_secret(),
            command.ceremony().correlation_id(),
        )?;
        self.insert_auth_ceremony(command.ceremony())
            .map_err(|error| bootstrap_problem(error, command.ceremony().correlation_id()))
    }

    fn prepare_trailbase_bootstrap(
        &self,
        query: PrepareTrailBaseBootstrapQuery,
    ) -> ApplicationResult<AuthCeremonySelection> {
        verify_bootstrap_secret(self, query.bootstrap_secret(), query.correlation_id())?;
        self.prepare_trailbase_bootstrap_selection()
            .map_err(|error| bootstrap_problem(error, query.correlation_id()))
    }

    fn claim_auth_ceremony(
        &self,
        command: ClaimAuthCeremonyCommand,
    ) -> ApplicationResult<AuthCeremony> {
        SqliteKernel::claim_auth_ceremony(
            self,
            command.browser_binding_digest(),
            command.instance_id(),
            command.activation_generation(),
            command.callback_path(),
            command.correlation_id(),
            command.at(),
        )
        .map_err(|error| access_problem(error, command.correlation_id()))
    }

    fn cancel_auth_ceremony(
        &self,
        command: CancelAuthCeremonyCommand,
    ) -> ApplicationResult<AuthCeremony> {
        SqliteKernel::cancel_auth_ceremony(
            self,
            command.operation_id(),
            command.correlation_id(),
            command.at(),
        )
        .map_err(|error| access_problem(error, command.correlation_id()))
    }

    fn preauthorize_trailbase_sign_in(
        &self,
        command: PreauthorizeTrailBaseSignInCommand,
    ) -> ApplicationResult<()> {
        SqliteKernel::preauthorize_trailbase_sign_in(self, command)
            .map_err(|error| access_problem(error, command.correlation_id()))
    }

    fn confirm_trailbase_sign_in(
        &self,
        command: ConfirmTrailBaseSignInCommand,
    ) -> ApplicationResult<AuthCeremony> {
        let correlation_id = command.authorization().correlation_id();
        SqliteKernel::confirm_trailbase_sign_in(self, command)
            .map_err(|error| access_problem(error, correlation_id))
    }

    fn read_trailbase_sign_in_continuation(
        &self,
        query: ReadTrailBaseSignInContinuationQuery,
    ) -> ApplicationResult<AuthSelectionProjection> {
        let correlation_id = query.correlation_id();
        SqliteKernel::read_trailbase_sign_in_continuation(self, query)
            .map_err(|error| access_problem(error, correlation_id))
    }

    fn complete_trailbase_sign_in_continuation(
        &self,
        command: CompleteTrailBaseSignInContinuationCommand,
    ) -> ApplicationResult<CreatedBrowserSession> {
        let correlation_id = command.correlation_id();
        SqliteKernel::complete_trailbase_sign_in_continuation(self, command)
            .map_err(|error| access_problem(error, correlation_id))
    }

    fn cancel_trailbase_sign_in_continuation(
        &self,
        command: CancelTrailBaseSignInContinuationCommand,
    ) -> ApplicationResult<AuthCeremony> {
        let correlation_id = command.correlation_id();
        SqliteKernel::cancel_trailbase_sign_in_continuation(self, command)
            .map_err(|error| access_problem(error, correlation_id))
    }

    fn preauthorize_trailbase_bootstrap(
        &self,
        command: PreauthorizeTrailBaseBootstrapCommand,
    ) -> ApplicationResult<()> {
        SqliteKernel::preauthorize_trailbase_bootstrap(self, &command)
            .map_err(|error| bootstrap_problem(error, command.correlation_id()))
    }

    fn fail_auth_ceremony(
        &self,
        command: FailAuthCeremonyCommand,
    ) -> ApplicationResult<AuthCeremony> {
        SqliteKernel::fail_auth_ceremony(self, command)
            .map_err(|error| access_problem(error, command.correlation_id()))
    }

    fn complete_trailbase_bootstrap(
        &self,
        command: CompleteTrailBaseBootstrapCommand,
    ) -> ApplicationResult<CreatedBrowserSession> {
        let authorization = command.authorization();
        verify_bootstrap_secret(
            self,
            command.bootstrap_secret(),
            authorization.correlation_id(),
        )?;
        let correlation_id = authorization.correlation_id();
        SqliteKernel::complete_trailbase_bootstrap(self, &command)
            .map_err(|error| bootstrap_problem(error, correlation_id))
    }

    fn complete_trailbase_identity_bootstrap(
        &self,
        command: CompleteTrailBaseBootstrapCommand,
    ) -> ApplicationResult<()> {
        let authorization = command.authorization();
        verify_bootstrap_secret(
            self,
            command.bootstrap_secret(),
            authorization.correlation_id(),
        )?;
        let correlation_id = authorization.correlation_id();
        SqliteKernel::complete_trailbase_identity_bootstrap(self, &command)
            .map_err(|error| bootstrap_problem(error, correlation_id))
    }

    fn change_membership_lifecycle(
        &self,
        command: ChangeMembershipLifecycleCommand,
    ) -> ApplicationResult<bool> {
        self.persist_membership_lifecycle_change(&command)
    }

    fn change_membership_role(
        &self,
        command: ChangeMembershipRoleCommand,
    ) -> ApplicationResult<bool> {
        self.persist_membership_role_change(&command)
    }

    fn change_auth_subject_lifecycle(
        &self,
        command: ChangeAuthSubjectLifecycleCommand,
    ) -> ApplicationResult<bool> {
        self.persist_subject_lifecycle_change(&command)
    }
}

fn verify_bootstrap_secret(
    kernel: &SqliteKernel,
    supplied: &fasti_application::SecretMaterial,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    let expected = AccessAdministrationPort::ensure_bootstrap_secret(kernel).map_err(|_| {
        Box::new(FastiProblem::storage_unavailable(
            CapabilityKey::AccessIdentityBootstrap,
            correlation_id,
        ))
    })?;
    if supplied.constant_time_eq(&expected) {
        Ok(())
    } else {
        Err(Box::new(FastiProblem::forbidden(
            CapabilityKey::AccessIdentityBootstrap,
            correlation_id,
        )))
    }
}

fn session_creation_error(code: ProblemCode) -> HumanAccessStoreError {
    if code == ProblemCode::StorageUnavailable {
        HumanAccessStoreError::StorageUnavailable
    } else {
        HumanAccessStoreError::Integrity
    }
}

fn access_problem(
    error: HumanAccessStoreError,
    correlation_id: RequestCorrelationId,
) -> Box<FastiProblem> {
    human_access_problem(error, CapabilityKey::CreateBrowserSession, correlation_id)
}

fn bootstrap_problem(
    error: HumanAccessStoreError,
    correlation_id: RequestCorrelationId,
) -> Box<FastiProblem> {
    human_access_problem(
        error,
        CapabilityKey::AccessIdentityBootstrap,
        correlation_id,
    )
}

fn human_access_problem(
    error: HumanAccessStoreError,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Box<FastiProblem> {
    let code = match error {
        HumanAccessStoreError::Storage(_) | HumanAccessStoreError::StorageUnavailable => {
            ProblemCode::StorageUnavailable
        }
        HumanAccessStoreError::Integrity
        | HumanAccessStoreError::Invariant(
            AccessInvariantError::TrailBaseInstallationBlocked
            | AccessInvariantError::ActivationGenerationOverflow
            | AccessInvariantError::EpochOverflow,
        ) => ProblemCode::IntegrityFailed,
        HumanAccessStoreError::CapacityExceeded => ProblemCode::CapacityExceeded,
        HumanAccessStoreError::Unaffiliated => ProblemCode::AuthSubjectUnaffiliated,
        HumanAccessStoreError::SelectionChanged => ProblemCode::AuthSelectionChanged,
        HumanAccessStoreError::BindingInvalid => ProblemCode::AuthBrowserBindingInvalid,
        HumanAccessStoreError::ProofExpired => ProblemCode::TrailBaseProofInvalid,
        HumanAccessStoreError::TrustUnavailable => ProblemCode::TrailBaseTrustUnavailable,
        HumanAccessStoreError::AttributableFailure(failure) => match failure {
            AuthCeremonyFailure::VerifierLostOnRestart
            | AuthCeremonyFailure::ExchangeOutcomeUncertain
            | AuthCeremonyFailure::ExchangeFailed
            | AuthCeremonyFailure::StatusRejected => ProblemCode::IdentityServiceUnavailable,
            AuthCeremonyFailure::LogoutUncertain => ProblemCode::TrailBaseSessionCleanupFailed,
            AuthCeremonyFailure::LocalAuthorizationDenied => ProblemCode::AuthSubjectUnaffiliated,
            AuthCeremonyFailure::LocalPersistenceFailed => {
                ProblemCode::AuthContinuationPersistenceFailed
            }
            AuthCeremonyFailure::TrustUnavailable => ProblemCode::TrailBaseTrustUnavailable,
        },
        HumanAccessStoreError::NotFound
        | HumanAccessStoreError::Conflict
        | HumanAccessStoreError::Invariant(
            AccessInvariantError::CeremonyBindingMismatch
            | AccessInvariantError::CeremonyInstallationMismatch
            | AccessInvariantError::CeremonyGenerationMismatch
            | AccessInvariantError::CeremonyCallbackMismatch,
        ) => ProblemCode::Forbidden,
        HumanAccessStoreError::Invariant(_) => ProblemCode::ValidationFailed,
    };
    Box::new(FastiProblem::from_code(code, capability, correlation_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestNode;
    use fasti_application::{
        BrowserRequestBoundaryPolicy, BrowserSessionPort, CreateBrowserSessionCommand,
        CreatedBrowserSession, SecretMaterial, SessionPolicy,
    };
    use fasti_domain::{ProfileGrantId, TrailBaseActivationBlocker, TrailBaseSubject};
    use std::sync::{Arc, Barrier};

    fn at(minutes: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-08-30T00:00:00Z")
            .expect("fixed time")
            .with_timezone(&Utc)
            + Duration::minutes(minutes)
    }

    fn fixture_root_identity() -> Sha256Digest {
        Sha256Digest::from_bytes(&[0x11; 32])
    }

    fn fixture_release_lock_identity() -> Sha256Digest {
        Sha256Digest::from_bytes(&[0x22; 32])
    }

    fn other_release_lock_identity() -> Sha256Digest {
        Sha256Digest::from_bytes(&[0x33; 32])
    }

    fn active_kernel() -> (tempfile::TempDir, SqliteKernel, TrailBaseInstallation) {
        let root = tempfile::tempdir().expect("temporary root");
        let kernel = SqliteKernel::open(root.path()).expect("kernel");
        let installation = kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("active installation");
        (root, kernel, installation)
    }

    fn ceremony(installation: &TrailBaseInstallation, created: i64, expires: i64) -> AuthCeremony {
        AuthCeremony::try_new(
            OperationId::new_v7(),
            AuthCeremonyPurpose::SignIn,
            AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
            installation.id(),
            installation.activation_generation(),
            Sha256Digest::from_bytes(&sha256_bytes(OperationId::new_v7().to_string().as_bytes())),
            None,
            false,
            AuthCallbackPath::parse("/auth/trailbase/callback").expect("callback"),
            AuthReturnTarget::ApplicationHome,
            RequestCorrelationId::new_v7(),
            at(created),
            at(expires),
        )
        .expect("ceremony")
    }

    fn selected_ceremony(
        installation: &TrailBaseInstallation,
        purpose: AuthCeremonyPurpose,
        workspace_id: WorkspaceId,
        grant_id: ProfileGrantId,
        invitation: Option<MembershipId>,
        binding_byte: u8,
    ) -> AuthCeremony {
        AuthCeremony::try_new(
            OperationId::new_v7(),
            purpose,
            AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
            installation.id(),
            installation.activation_generation(),
            Sha256Digest::from_bytes(&[binding_byte; 32]),
            Some(
                AuthCeremonySelection::try_new(purpose, workspace_id, grant_id, None, invitation)
                    .expect("selected ceremony"),
            ),
            false,
            AuthCallbackPath::parse("/api/access/v1/trailbase/callback").expect("callback"),
            purpose.return_target(),
            RequestCorrelationId::new_v7(),
            at(1),
            at(10),
        )
        .expect("selected ceremony")
    }

    fn claim_ceremony(
        kernel: &SqliteKernel,
        installation: &TrailBaseInstallation,
        ceremony: &AuthCeremony,
    ) {
        kernel
            .insert_auth_ceremony(ceremony)
            .expect("insert ceremony");
        kernel
            .claim_auth_ceremony(
                ceremony.browser_binding_digest(),
                installation.id(),
                installation.activation_generation(),
                ceremony.callback_path(),
                RequestCorrelationId::new_v7(),
                at(2),
            )
            .expect("claim ceremony");
    }

    fn confirmed_identity(
        installation: &TrailBaseInstallation,
        subject_byte: u8,
    ) -> ConfirmedTrailBaseIdentity {
        ConfirmedTrailBaseIdentity::new(
            installation.id(),
            TrailBaseSubject::from_bytes([subject_byte; 16]),
            AuthenticationProvenance::new(
                AuthenticationMethod::TrailBasePassword,
                at(3),
                installation.activation_generation(),
            ),
        )
    }

    fn bootstrap_ceremony(
        node: &TestNode,
        installation: &TrailBaseInstallation,
        binding_byte: u8,
    ) -> AuthCeremony {
        selected_ceremony(
            installation,
            AuthCeremonyPurpose::FirstAdministratorBootstrap,
            node.access.workspace_id(),
            node.access.grant_id(),
            None,
            binding_byte,
        )
    }

    fn claim_bootstrap(
        kernel: &SqliteKernel,
        installation: &TrailBaseInstallation,
        ceremony: &AuthCeremony,
    ) {
        claim_ceremony(kernel, installation, ceremony);
    }

    fn copy_secret(secret: &SecretMaterial) -> SecretMaterial {
        SecretMaterial::try_from_hex(&secret.expose_hex()).expect("copy test secret")
    }

    fn mutation_proof(
        session: &CreatedBrowserSession,
        now: DateTime<Utc>,
    ) -> BrowserSessionMutationCommand {
        let boundary =
            BrowserRequestBoundaryPolicy::try_new("https://fasti.example", "fasti.example")
                .expect("boundary policy")
                .validate(Some("https://fasti.example"), Some("fasti.example"))
                .expect("request boundary");
        BrowserSessionMutationCommand::new(
            RequestCorrelationId::new_v7(),
            copy_secret(session.session_secret()),
            copy_secret(session.csrf_secret()),
            boundary,
            now,
        )
    }

    fn invalid_csrf_proof(
        session: &CreatedBrowserSession,
        now: DateTime<Utc>,
    ) -> BrowserSessionMutationCommand {
        let boundary =
            BrowserRequestBoundaryPolicy::try_new("https://fasti.example", "fasti.example")
                .expect("boundary policy")
                .validate(Some("https://fasti.example"), Some("fasti.example"))
                .expect("request boundary");
        BrowserSessionMutationCommand::new(
            RequestCorrelationId::new_v7(),
            copy_secret(session.session_secret()),
            SecretMaterial::from_bytes([99; 32]),
            boundary,
            now,
        )
    }

    fn bootstrap_administrator(
        node: &TestNode,
        installation: &TrailBaseInstallation,
    ) -> (AuthSubjectId, MembershipId, WorkspaceId) {
        let ceremony = bootstrap_ceremony(node, installation, 71);
        claim_bootstrap(&node.kernel, installation, &ceremony);
        let authorization = PreauthorizeTrailBaseBootstrapCommand::new(
            ceremony.id(),
            confirmed_identity(installation, 71),
            RequestCorrelationId::new_v7(),
            at(3),
        );
        HumanAccessPort::preauthorize_trailbase_bootstrap(&node.kernel, authorization)
            .expect("preauthorize bootstrap");
        HumanAccessPort::complete_trailbase_bootstrap(
            &node.kernel,
            CompleteTrailBaseBootstrapCommand::new(
                authorization,
                node.kernel
                    .ensure_bootstrap_secret()
                    .expect("bootstrap secret"),
            ),
        )
        .expect("bootstrap administrator");
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let (subject_id, membership_id): (String, String) = connection
            .query_row(
                r#"
                SELECT anchor.auth_subject_id, membership.membership_id
                FROM trailbase_auth_anchors anchor
                JOIN workspace_memberships membership
                  ON membership.auth_subject_id = anchor.auth_subject_id
                WHERE anchor.trailbase_instance_id = ?1
                  AND anchor.trailbase_subject = ?2
                  AND membership.workspace_id = ?3
                "#,
                params![
                    installation.id().to_string(),
                    TrailBaseSubject::from_bytes([71; 16]).as_bytes().as_slice(),
                    node.access.workspace_id().to_string(),
                ],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("bootstrap identities");
        let subject_id = subject_id.parse::<AuthSubjectId>().expect("subject id");
        let membership_id = membership_id
            .parse::<MembershipId>()
            .expect("membership id");
        let workspace_id = node.access.workspace_id();
        (subject_id, membership_id, workspace_id)
    }

    fn add_subject(
        node: &TestNode,
        installation: &TrailBaseInstallation,
        workspace_id: WorkspaceId,
        lifecycle: MembershipLifecycle,
        role: WorkspaceRole,
        subject_byte: u8,
    ) -> (AuthSubjectId, MembershipId) {
        let subject_id = AuthSubjectId::new_v7();
        let membership_id = MembershipId::new_v7();
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let transaction = connection.unchecked_transaction().expect("transaction");
        transaction
            .execute(
                "INSERT INTO auth_subjects(auth_subject_id, lifecycle, auth_epoch, authorization_epoch, created_at, updated_at) VALUES (?1, 'active', 0, 0, ?2, ?2)",
                params![subject_id.to_string(), timestamp(at(3))],
            )
            .expect("subject");
        transaction
            .execute(
                "INSERT INTO trailbase_auth_anchors(trailbase_instance_id, trailbase_subject, auth_subject_id, linked_at) VALUES (?1, ?2, ?3, ?4)",
                params![
                    installation.id().to_string(),
                    vec![subject_byte; 16],
                    subject_id.to_string(),
                    timestamp(at(3)),
                ],
            )
            .expect("anchor");
        transaction
            .execute(
                "INSERT INTO workspace_memberships(membership_id, auth_subject_id, workspace_id, lifecycle, role, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![
                    membership_id.to_string(),
                    subject_id.to_string(),
                    workspace_id.to_string(),
                    lifecycle.as_str(),
                    role.as_str(),
                    timestamp(at(3)),
                ],
            )
            .expect("membership");
        transaction.commit().expect("commit subject");
        (subject_id, membership_id)
    }

    fn create_test_session(
        node: &TestNode,
        installation: &TrailBaseInstallation,
        subject_id: AuthSubjectId,
        workspace_id: WorkspaceId,
        grant_id: ProfileGrantId,
        now: DateTime<Utc>,
    ) -> CreatedBrowserSession {
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute(
                    "INSERT OR IGNORE INTO auth_subject_profile_grants(auth_subject_id, profile_grant_id) VALUES (?1, ?2)",
                    params![subject_id.to_string(), grant_id.to_string()],
                )
                .expect("subject grant");
        }
        let session = BrowserSessionPort::create_browser_session(
            &node.kernel,
            CreateBrowserSessionCommand::try_new(
                RequestCorrelationId::new_v7(),
                subject_id,
                workspace_id,
                vec![grant_id],
                grant_id,
                SessionPolicy::C1,
                false,
                now,
            )
            .expect("session command"),
        )
        .expect("browser session");
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute(
                    r#"
                    INSERT INTO fasti_browser_session_authentication(
                        browser_session_id, trailbase_instance_id, activation_generation,
                        method, verified_at, recent_authentication_expires_at
                    ) VALUES (?1, ?2, ?3, 'trailbase_password', ?4, ?5)
                    "#,
                    params![
                        session.session().id().to_string(),
                        installation.id().to_string(),
                        i64::try_from(installation.activation_generation())
                            .expect("generation fits"),
                        timestamp(now),
                        timestamp(now + Duration::minutes(10)),
                    ],
                )
                .expect("recent authentication");
        }
        session
    }

    fn selection_required_sign_in(
        node: &TestNode,
        installation: &TrailBaseInstallation,
        subject_byte: u8,
    ) -> (AuthCeremony, AuthSubjectId, MembershipId) {
        let (subject_id, membership_id) = add_subject(
            node,
            installation,
            node.access.workspace_id(),
            MembershipLifecycle::Active,
            WorkspaceRole::Member,
            subject_byte,
        );
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute(
                    "INSERT INTO auth_subject_profile_grants(auth_subject_id, profile_grant_id) VALUES (?1, ?2)",
                    params![subject_id.to_string(), node.access.grant_id().to_string()],
                )
                .expect("subject grant");
        }
        let ceremony = ceremony(installation, 1, 10);
        claim_ceremony(&node.kernel, installation, &ceremony);
        let authorization = PreauthorizeTrailBaseSignInCommand::new(
            ceremony.id(),
            confirmed_identity(installation, subject_byte),
            RequestCorrelationId::new_v7(),
            at(4),
        );
        HumanAccessPort::preauthorize_trailbase_sign_in(&node.kernel, authorization)
            .expect("preauthorization");
        HumanAccessPort::confirm_trailbase_sign_in(
            &node.kernel,
            ConfirmTrailBaseSignInCommand::new(authorization),
        )
        .expect("confirm after logout");
        (ceremony, subject_id, membership_id)
    }

    fn insert_pending_rows(
        kernel: &SqliteKernel,
        installation: &TrailBaseInstallation,
        count: usize,
    ) {
        let connection = kernel.inner.connection.lock().expect("connection");
        let transaction = connection.unchecked_transaction().expect("transaction");
        {
            let mut statement = transaction
                .prepare(
                    r#"
                    INSERT INTO auth_ceremonies(
                        operation_id, purpose, protocol, trailbase_instance_id,
                        activation_generation, browser_binding_digest, workspace_id,
                        selected_profile_grant_id, bound_browser_session_id,
                        invited_membership_id, remembered, callback_path, return_target,
                        correlation_id, state, failure, created_at, expires_at,
                        claimed_at, terminal_at
                    ) VALUES (?1, 'sign_in', 'trailbase_authorization_code_pkce', ?2, ?3, ?4,
                              NULL, NULL, NULL, NULL, 0, '/auth/trailbase/callback',
                              'application_home', ?5, 'pending', NULL, ?6, ?7, NULL, NULL)
                    "#,
                )
                .expect("prepare insert");
            for sequence in 0..count {
                statement
                    .execute(params![
                        OperationId::new_v7().to_string(),
                        installation.id().to_string(),
                        i64::try_from(installation.activation_generation())
                            .expect("generation fits"),
                        Sha256Digest::from_bytes(&sha256_bytes(&sequence.to_be_bytes())).as_str(),
                        RequestCorrelationId::new_v7().to_string(),
                        timestamp(at(1)),
                        timestamp(at(10)),
                    ])
                    .expect("insert pending row");
            }
        }
        transaction.commit().expect("commit pending rows");
    }

    #[test]
    fn post_cleanup_sign_in_accepts_the_exact_invitation_and_issues_one_session_atomically() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("active installation");
        let (subject_id, membership_id) = add_subject(
            &node,
            &installation,
            node.access.workspace_id(),
            MembershipLifecycle::Invited,
            WorkspaceRole::Member,
            81,
        );
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute(
                    "INSERT INTO auth_subject_profile_grants(auth_subject_id, profile_grant_id) VALUES (?1, ?2)",
                    params![subject_id.to_string(), node.access.grant_id().to_string()],
                )
                .expect("subject grant");
        }
        let ceremony = ceremony(&installation, 1, 10);
        claim_ceremony(&node.kernel, &installation, &ceremony);
        let authorization = PreauthorizeTrailBaseSignInCommand::new(
            ceremony.id(),
            confirmed_identity(&installation, 81),
            RequestCorrelationId::new_v7(),
            at(4),
        );
        HumanAccessPort::preauthorize_trailbase_sign_in(&node.kernel, authorization)
            .expect("preauthorization");
        HumanAccessPort::confirm_trailbase_sign_in(
            &node.kernel,
            ConfirmTrailBaseSignInCommand::new(authorization),
        )
        .expect("confirm after logout");
        let projection = HumanAccessPort::read_trailbase_sign_in_continuation(
            &node.kernel,
            ReadTrailBaseSignInContinuationQuery::new(
                ceremony.browser_binding_digest().clone(),
                RequestCorrelationId::new_v7(),
                at(5),
            ),
        )
        .expect("read choices");
        assert_eq!(projection.choices().len(), 1);
        let created = HumanAccessPort::complete_trailbase_sign_in_continuation(
            &node.kernel,
            CompleteTrailBaseSignInContinuationCommand::new(
                ceremony.browser_binding_digest().clone(),
                0,
                projection.candidate_revision().clone(),
                RequestCorrelationId::new_v7(),
                at(6),
            ),
        )
        .expect("complete sign-in");

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let state: (String, i64, String, Option<String>, i64) = connection
            .query_row(
                r#"
                SELECT membership.lifecycle, subject.authorization_epoch, ceremony.state,
                       authentication.recent_authentication_expires_at,
                       (SELECT COUNT(*) FROM access_audit_events audit
                        WHERE (audit.operation_id = ceremony.operation_id
                               AND audit.event_kind IN ('browser_session_issued',
                                                        'ceremony_completed'))
                           OR (audit.membership_id = membership.membership_id
                               AND audit.event_kind = 'membership_invitation_accepted'))
                FROM workspace_memberships membership
                JOIN auth_subjects subject ON subject.auth_subject_id = membership.auth_subject_id
                JOIN auth_ceremonies ceremony ON ceremony.operation_id = ?1
                JOIN fasti_browser_session_authentication authentication
                  ON authentication.browser_session_id = ?2
                WHERE membership.membership_id = ?3
                "#,
                params![
                    ceremony.id().to_string(),
                    created.session().id().to_string(),
                    membership_id.to_string(),
                ],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("final state");
        assert_eq!(
            state,
            ("active".to_owned(), 1, "completed".to_owned(), None, 3)
        );
    }

    #[test]
    fn invalid_in_range_choice_retains_selection_without_creating_a_session() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("active installation");
        let (ceremony, _, _) = selection_required_sign_in(&node, &installation, 82);
        let projection = HumanAccessPort::read_trailbase_sign_in_continuation(
            &node.kernel,
            ReadTrailBaseSignInContinuationQuery::new(
                ceremony.browser_binding_digest().clone(),
                RequestCorrelationId::new_v7(),
                at(5),
            ),
        )
        .expect("one sign-in choice");

        let problem = match HumanAccessPort::complete_trailbase_sign_in_continuation(
            &node.kernel,
            CompleteTrailBaseSignInContinuationCommand::new(
                ceremony.browser_binding_digest().clone(),
                1,
                projection.candidate_revision().clone(),
                RequestCorrelationId::new_v7(),
                at(6),
            ),
        ) {
            Err(problem) => problem,
            Ok(_) => panic!("ordinal one is invalid for one choice"),
        };
        assert_eq!(problem.code(), ProblemCode::ValidationFailed);

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let retained: (String, i64, i64) = connection
            .query_row(
                r#"
                SELECT state,
                       (SELECT COUNT(*) FROM fasti_browser_sessions),
                       (SELECT COUNT(*) FROM fasti_browser_session_authentication)
                FROM auth_ceremonies WHERE operation_id = ?1
                "#,
                [ceremony.id().to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("retained selection state");
        assert_eq!(retained, ("selection_required".to_owned(), 0, 0));
        drop(connection);

        HumanAccessPort::complete_trailbase_sign_in_continuation(
            &node.kernel,
            CompleteTrailBaseSignInContinuationCommand::new(
                ceremony.browser_binding_digest().clone(),
                0,
                projection.candidate_revision().clone(),
                RequestCorrelationId::new_v7(),
                at(7),
            ),
        )
        .expect("valid ordinal remains usable");
    }

    #[test]
    fn sign_in_audit_failure_rolls_back_invitation_session_provenance_and_ceremony() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("active installation");
        let (subject_id, membership_id) = add_subject(
            &node,
            &installation,
            node.access.workspace_id(),
            MembershipLifecycle::Invited,
            WorkspaceRole::Member,
            82,
        );
        let ceremony = ceremony(&installation, 1, 10);
        claim_ceremony(&node.kernel, &installation, &ceremony);
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute(
                    "INSERT INTO auth_subject_profile_grants(auth_subject_id, profile_grant_id) VALUES (?1, ?2)",
                    params![subject_id.to_string(), node.access.grant_id().to_string()],
                )
                .expect("subject grant");
            connection
                .execute_batch(
                    "CREATE TRIGGER fail_session_issue_audit BEFORE INSERT ON access_audit_events WHEN NEW.event_kind = 'browser_session_issued' BEGIN SELECT RAISE(ABORT, 'injected audit failure'); END;",
                )
                .expect("failure trigger");
        }
        let authorization = PreauthorizeTrailBaseSignInCommand::new(
            ceremony.id(),
            confirmed_identity(&installation, 82),
            RequestCorrelationId::new_v7(),
            at(4),
        );
        HumanAccessPort::preauthorize_trailbase_sign_in(&node.kernel, authorization)
            .expect("preauthorization");
        HumanAccessPort::confirm_trailbase_sign_in(
            &node.kernel,
            ConfirmTrailBaseSignInCommand::new(authorization),
        )
        .expect("confirm after logout");
        let projection = HumanAccessPort::read_trailbase_sign_in_continuation(
            &node.kernel,
            ReadTrailBaseSignInContinuationQuery::new(
                ceremony.browser_binding_digest().clone(),
                RequestCorrelationId::new_v7(),
                at(5),
            ),
        )
        .expect("read choices");
        assert!(HumanAccessPort::complete_trailbase_sign_in_continuation(
            &node.kernel,
            CompleteTrailBaseSignInContinuationCommand::new(
                ceremony.browser_binding_digest().clone(),
                0,
                projection.candidate_revision().clone(),
                RequestCorrelationId::new_v7(),
                at(6),
            ),
        )
        .is_err());

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let state: (String, i64, String, i64, i64) = connection
            .query_row(
                r#"
                SELECT membership.lifecycle, subject.authorization_epoch, ceremony.state,
                       (SELECT COUNT(*) FROM fasti_browser_sessions),
                       (SELECT COUNT(*) FROM fasti_browser_session_authentication)
                FROM workspace_memberships membership
                JOIN auth_subjects subject ON subject.auth_subject_id = membership.auth_subject_id
                JOIN auth_ceremonies ceremony ON ceremony.operation_id = ?1
                WHERE membership.membership_id = ?2
                "#,
                params![ceremony.id().to_string(), membership_id.to_string()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("rolled-back state");
        assert_eq!(
            state,
            (
                "invited".to_owned(),
                0,
                "selection_required".to_owned(),
                0,
                0
            )
        );
    }

    #[test]
    fn sign_in_candidate_dependency_races_reject_the_stale_revision_without_a_session() {
        for dependency in [
            "auth_epoch",
            "authorization_epoch",
            "membership_role",
            "grant",
            "client",
            "activation_generation",
        ] {
            let node = TestNode::new();
            let installation = node
                .kernel
                .verify_trailbase_installation(
                    TrailBaseInstanceId::new_v7(),
                    fixture_root_identity(),
                    fixture_release_lock_identity(),
                    false,
                    RequestCorrelationId::new_v7(),
                    at(0),
                )
                .expect("active installation");
            let (ceremony, subject_id, membership_id) =
                selection_required_sign_in(&node, &installation, 90);
            let before = HumanAccessPort::read_trailbase_sign_in_continuation(
                &node.kernel,
                ReadTrailBaseSignInContinuationQuery::new(
                    ceremony.browser_binding_digest().clone(),
                    RequestCorrelationId::new_v7(),
                    at(5),
                ),
            )
            .expect("initial choices");
            let repeated = HumanAccessPort::read_trailbase_sign_in_continuation(
                &node.kernel,
                ReadTrailBaseSignInContinuationQuery::new(
                    ceremony.browser_binding_digest().clone(),
                    RequestCorrelationId::new_v7(),
                    at(5),
                ),
            )
            .expect("repeat choices");
            assert_eq!(before, repeated, "{dependency}");
            if dependency == "activation_generation" {
                node.kernel
                    .verify_trailbase_installation(
                        installation.id(),
                        fixture_root_identity(),
                        fixture_release_lock_identity(),
                        true,
                        RequestCorrelationId::new_v7(),
                        at(6),
                    )
                    .expect("advance active installation generation");
            } else {
                let connection = node.kernel.inner.connection.lock().expect("connection");
                match dependency {
                    "auth_epoch" => connection.execute(
                        "UPDATE auth_subjects SET auth_epoch = auth_epoch + 1 WHERE auth_subject_id = ?1",
                        [subject_id.to_string()],
                    ),
                    "authorization_epoch" => connection.execute(
                        "UPDATE auth_subjects SET authorization_epoch = authorization_epoch + 1 WHERE auth_subject_id = ?1",
                        [subject_id.to_string()],
                    ),
                    "membership_role" => connection.execute(
                        "UPDATE workspace_memberships SET role = 'administrator' WHERE membership_id = ?1",
                        [membership_id.to_string()],
                    ),
                    "grant" => connection.execute(
                        "UPDATE profile_grants SET status = 'revoked' WHERE grant_id = ?1",
                        [node.access.grant_id().to_string()],
                    ),
                    "client" => connection.execute(
                        "UPDATE clients SET status = 'revoked' WHERE client_id = ?1",
                        [node.access.client_id().to_string()],
                    ),
                    _ => unreachable!(),
                }
                .expect("mutate candidate dependency");
            }
            let error = HumanAccessPort::complete_trailbase_sign_in_continuation(
                &node.kernel,
                CompleteTrailBaseSignInContinuationCommand::new(
                    ceremony.browser_binding_digest().clone(),
                    0,
                    before.candidate_revision().clone(),
                    RequestCorrelationId::new_v7(),
                    at(6),
                ),
            )
            .err()
            .expect("stale dependency must fail closed");
            assert_eq!(
                error.code(),
                if dependency == "activation_generation" {
                    ProblemCode::TrailBaseTrustUnavailable
                } else {
                    ProblemCode::AuthSelectionChanged
                },
                "{dependency}"
            );
            let connection = node.kernel.inner.connection.lock().expect("connection");
            let state: (String, i64, i64) = connection
                .query_row(
                    "SELECT state, (SELECT COUNT(*) FROM fasti_browser_sessions), (SELECT COUNT(*) FROM fasti_browser_session_authentication) FROM auth_ceremonies WHERE operation_id = ?1",
                    [ceremony.id().to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("stale completion state");
            assert_eq!(
                state,
                ("selection_required".to_owned(), 0, 0),
                "{dependency}"
            );
        }
    }

    #[test]
    fn concurrent_continuation_completions_issue_exactly_one_session() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("active installation");
        let (ceremony, _, _) = selection_required_sign_in(&node, &installation, 93);
        let projection = HumanAccessPort::read_trailbase_sign_in_continuation(
            &node.kernel,
            ReadTrailBaseSignInContinuationQuery::new(
                ceremony.browser_binding_digest().clone(),
                RequestCorrelationId::new_v7(),
                at(5),
            ),
        )
        .expect("sign-in choices");
        let barrier = Arc::new(Barrier::new(3));
        let workers = [0, 1].map(|_| {
            let kernel = node.kernel.clone();
            let barrier = Arc::clone(&barrier);
            let binding = ceremony.browser_binding_digest().clone();
            let revision = projection.candidate_revision().clone();
            std::thread::spawn(move || {
                barrier.wait();
                HumanAccessPort::complete_trailbase_sign_in_continuation(
                    &kernel,
                    CompleteTrailBaseSignInContinuationCommand::new(
                        binding,
                        0,
                        revision,
                        RequestCorrelationId::new_v7(),
                        at(6),
                    ),
                )
            })
        });
        barrier.wait();
        let results = workers.map(|worker| worker.join().expect("completion worker"));
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| {
                    result.as_ref().is_err_and(|problem| {
                        problem.code() == ProblemCode::AuthBrowserBindingInvalid
                    })
                })
                .count(),
            1
        );

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let state: (String, i64, i64, i64, i64) = connection
            .query_row(
                r#"
                SELECT state,
                       (SELECT COUNT(*) FROM fasti_browser_sessions),
                       (SELECT COUNT(*) FROM fasti_browser_session_authentication),
                       (SELECT COUNT(*) FROM access_audit_events audit
                        WHERE audit.operation_id = ceremony.operation_id
                          AND audit.event_kind = 'browser_session_issued'),
                       (SELECT COUNT(*) FROM access_audit_events audit
                        WHERE audit.operation_id = ceremony.operation_id
                          AND audit.event_kind = 'ceremony_completed')
                FROM auth_ceremonies ceremony
                WHERE operation_id = ?1
                "#,
                [ceremony.id().to_string()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("atomic completion state");
        assert_eq!(state, ("completed".to_owned(), 1, 1, 1, 1));
    }

    #[test]
    fn selection_required_survives_restart_and_retains_confirmation_at_terminal_boundaries() {
        type ConfirmationRow = (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<i64>,
        );
        let read_row = |kernel: &SqliteKernel, operation_id: OperationId| -> ConfirmationRow {
            let connection = kernel.inner.connection.lock().expect("connection");
            connection
                .query_row(
                    r#"
                    SELECT state, confirmed_auth_subject_id, authentication_method,
                           authentication_verified_at, confirmed_auth_epoch,
                           confirmed_authorization_epoch
                    FROM auth_ceremonies WHERE operation_id = ?1
                    "#,
                    [operation_id.to_string()],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                        ))
                    },
                )
                .expect("ceremony row")
        };

        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("active installation");
        let (expiring, _, _) = selection_required_sign_in(&node, &installation, 94);
        let (cancelled, _, _) = selection_required_sign_in(&node, &installation, 95);
        let expiring_projection = HumanAccessPort::read_trailbase_sign_in_continuation(
            &node.kernel,
            ReadTrailBaseSignInContinuationQuery::new(
                expiring.browser_binding_digest().clone(),
                RequestCorrelationId::new_v7(),
                at(5),
            ),
        )
        .expect("expiring projection");
        let cancelled_projection = HumanAccessPort::read_trailbase_sign_in_continuation(
            &node.kernel,
            ReadTrailBaseSignInContinuationQuery::new(
                cancelled.browser_binding_digest().clone(),
                RequestCorrelationId::new_v7(),
                at(5),
            ),
        )
        .expect("cancel projection");
        let expiring_before = read_row(&node.kernel, expiring.id());
        let cancelled_before = read_row(&node.kernel, cancelled.id());

        let (root, _) = node.into_stopped();
        let reopened = SqliteKernel::open(root.path()).expect("reopen data root");
        assert_eq!(
            HumanAccessPort::read_trailbase_sign_in_continuation(
                &reopened,
                ReadTrailBaseSignInContinuationQuery::new(
                    expiring.browser_binding_digest().clone(),
                    RequestCorrelationId::new_v7(),
                    at(5),
                ),
            )
            .expect("restarted expiring projection"),
            expiring_projection
        );
        assert_eq!(
            HumanAccessPort::read_trailbase_sign_in_continuation(
                &reopened,
                ReadTrailBaseSignInContinuationQuery::new(
                    cancelled.browser_binding_digest().clone(),
                    RequestCorrelationId::new_v7(),
                    at(5),
                ),
            )
            .expect("restarted cancel projection"),
            cancelled_projection
        );

        let cancelled_state = HumanAccessPort::cancel_trailbase_sign_in_continuation(
            &reopened,
            CancelTrailBaseSignInContinuationCommand::new(
                cancelled.browser_binding_digest().clone(),
                RequestCorrelationId::new_v7(),
                at(6),
            ),
        )
        .expect("cancel persisted continuation");
        assert_eq!(cancelled_state.state(), AuthCeremonyState::Cancelled);
        let cancelled_after = read_row(&reopened, cancelled.id());
        assert_eq!(cancelled_after.0, "cancelled");
        assert_eq!(
            (
                &cancelled_after.1,
                &cancelled_after.2,
                &cancelled_after.3,
                cancelled_after.4,
                cancelled_after.5,
            ),
            (
                &cancelled_before.1,
                &cancelled_before.2,
                &cancelled_before.3,
                cancelled_before.4,
                cancelled_before.5,
            )
        );

        assert!(HumanAccessPort::read_trailbase_sign_in_continuation(
            &reopened,
            ReadTrailBaseSignInContinuationQuery::new(
                expiring.browser_binding_digest().clone(),
                RequestCorrelationId::new_v7(),
                at(9),
            ),
        )
        .is_ok());
        for minute in [10, 11] {
            let error = HumanAccessPort::read_trailbase_sign_in_continuation(
                &reopened,
                ReadTrailBaseSignInContinuationQuery::new(
                    expiring.browser_binding_digest().clone(),
                    RequestCorrelationId::new_v7(),
                    at(minute),
                ),
            )
            .expect_err("expired proof");
            assert_eq!(error.code(), ProblemCode::TrailBaseProofInvalid);
        }
        let expiring_after = read_row(&reopened, expiring.id());
        assert_eq!(expiring_after.0, "expired");
        assert_eq!(
            (
                &expiring_after.1,
                &expiring_after.2,
                &expiring_after.3,
                expiring_after.4,
                expiring_after.5,
            ),
            (
                &expiring_before.1,
                &expiring_before.2,
                &expiring_before.3,
                expiring_before.4,
                expiring_before.5,
            )
        );

        let connection = reopened.inner.connection.lock().expect("connection");
        let terminal_state: (Option<String>, Option<String>, i64, i64) = connection
            .query_row(
                r#"
                SELECT (SELECT terminal_at FROM auth_ceremonies WHERE operation_id = ?1),
                       (SELECT terminal_at FROM auth_ceremonies WHERE operation_id = ?2),
                       (SELECT COUNT(*) FROM access_audit_events WHERE operation_id = ?1 AND event_kind = 'ceremony_expired'),
                       (SELECT COUNT(*) FROM access_audit_events WHERE operation_id = ?2 AND event_kind = 'ceremony_cancelled')
                "#,
                params![expiring.id().to_string(), cancelled.id().to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("terminal evidence");
        assert_eq!(
            terminal_state,
            (Some(timestamp(at(10))), Some(timestamp(at(6))), 1, 1)
        );
    }

    #[test]
    fn attributable_post_claim_failures_remain_bound_until_dismissed() {
        for (failure, expected) in [
            (
                AuthCeremonyFailure::ExchangeFailed,
                ProblemCode::IdentityServiceUnavailable,
            ),
            (
                AuthCeremonyFailure::ExchangeOutcomeUncertain,
                ProblemCode::IdentityServiceUnavailable,
            ),
            (
                AuthCeremonyFailure::StatusRejected,
                ProblemCode::IdentityServiceUnavailable,
            ),
            (
                AuthCeremonyFailure::LogoutUncertain,
                ProblemCode::TrailBaseSessionCleanupFailed,
            ),
            (
                AuthCeremonyFailure::LocalAuthorizationDenied,
                ProblemCode::AuthSubjectUnaffiliated,
            ),
            (
                AuthCeremonyFailure::LocalPersistenceFailed,
                ProblemCode::AuthContinuationPersistenceFailed,
            ),
            (
                AuthCeremonyFailure::TrustUnavailable,
                ProblemCode::TrailBaseTrustUnavailable,
            ),
        ] {
            let (_root, kernel, installation) = active_kernel();
            let ceremony = ceremony(&installation, 1, 10);
            claim_ceremony(&kernel, &installation, &ceremony);
            HumanAccessPort::fail_auth_ceremony(
                &kernel,
                FailAuthCeremonyCommand::new(
                    ceremony.id(),
                    failure,
                    RequestCorrelationId::new_v7(),
                    at(4),
                ),
            )
            .expect("persist attributable failure");
            let error = HumanAccessPort::read_trailbase_sign_in_continuation(
                &kernel,
                ReadTrailBaseSignInContinuationQuery::new(
                    ceremony.browser_binding_digest().clone(),
                    RequestCorrelationId::new_v7(),
                    at(5),
                ),
            )
            .expect_err("terminal continuation returns governed evidence");
            assert_eq!(error.code(), expected, "{failure:?}");
            HumanAccessPort::cancel_trailbase_sign_in_continuation(
                &kernel,
                CancelTrailBaseSignInContinuationCommand::new(
                    ceremony.browser_binding_digest().clone(),
                    RequestCorrelationId::new_v7(),
                    at(6),
                ),
            )
            .expect("bound terminal continuation can be dismissed");
        }
    }

    #[test]
    fn sign_in_selection_enforces_zero_sixty_four_and_sixty_five_choice_bounds() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("active installation");
        let (selected, subject_id, _) = selection_required_sign_in(&node, &installation, 91);
        for _ in 1..AUTH_SELECTION_CHOICE_LIMIT {
            let access = node.add_profile_with_scopes(&[]);
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute(
                    "INSERT INTO auth_subject_profile_grants(auth_subject_id, profile_grant_id) VALUES (?1, ?2)",
                    params![subject_id.to_string(), access.grant_id().to_string()],
                )
                .expect("subject grant");
        }
        let projection = HumanAccessPort::read_trailbase_sign_in_continuation(
            &node.kernel,
            ReadTrailBaseSignInContinuationQuery::new(
                selected.browser_binding_digest().clone(),
                RequestCorrelationId::new_v7(),
                at(5),
            ),
        )
        .expect("exactly 64 choices");
        assert_eq!(projection.choices().len(), AUTH_SELECTION_CHOICE_LIMIT);
        for (index, choice) in projection.choices().iter().copied().enumerate() {
            assert_eq!(usize::from(choice.ordinal()), index);
            assert_eq!(choice.workspace_ordinal(), 1);
            assert_eq!(usize::from(choice.profile_ordinal()), index + 1);
        }
        let extra = node.add_profile_with_scopes(&[]);
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute(
                    "INSERT INTO auth_subject_profile_grants(auth_subject_id, profile_grant_id) VALUES (?1, ?2)",
                    params![subject_id.to_string(), extra.grant_id().to_string()],
                )
                .expect("sixty-fifth subject grant");
        }
        let capacity = HumanAccessPort::read_trailbase_sign_in_continuation(
            &node.kernel,
            ReadTrailBaseSignInContinuationQuery::new(
                selected.browser_binding_digest().clone(),
                RequestCorrelationId::new_v7(),
                at(5),
            ),
        )
        .expect_err("sixty-five choices must fail closed");
        assert_eq!(capacity.code(), ProblemCode::CapacityExceeded);

        let empty = TestNode::new();
        let empty_installation = empty
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("empty active installation");
        let (_, _) = add_subject(
            &empty,
            &empty_installation,
            empty.access.workspace_id(),
            MembershipLifecycle::Active,
            WorkspaceRole::Member,
            92,
        );
        let empty_ceremony = ceremony(&empty_installation, 1, 10);
        claim_ceremony(&empty.kernel, &empty_installation, &empty_ceremony);
        let authorization = PreauthorizeTrailBaseSignInCommand::new(
            empty_ceremony.id(),
            confirmed_identity(&empty_installation, 92),
            RequestCorrelationId::new_v7(),
            at(4),
        );
        HumanAccessPort::confirm_trailbase_sign_in(
            &empty.kernel,
            ConfirmTrailBaseSignInCommand::new(authorization),
        )
        .expect("confirm unaffiliated identity");
        let unaffiliated = HumanAccessPort::read_trailbase_sign_in_continuation(
            &empty.kernel,
            ReadTrailBaseSignInContinuationQuery::new(
                empty_ceremony.browser_binding_digest().clone(),
                RequestCorrelationId::new_v7(),
                at(5),
            ),
        )
        .expect_err("zero choices must not invent a selection");
        assert_eq!(unaffiliated.code(), ProblemCode::AuthSubjectUnaffiliated);
    }

    #[test]
    fn post_cleanup_bootstrap_creates_no_identity_before_final_transaction() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("active installation");
        let ceremony = selected_ceremony(
            &installation,
            AuthCeremonyPurpose::FirstAdministratorBootstrap,
            node.access.workspace_id(),
            node.access.grant_id(),
            None,
            83,
        );
        claim_ceremony(&node.kernel, &installation, &ceremony);
        let identity = confirmed_identity(&installation, 83);
        let preauthorization = PreauthorizeTrailBaseBootstrapCommand::new(
            ceremony.id(),
            identity,
            RequestCorrelationId::new_v7(),
            at(4),
        );
        HumanAccessPort::preauthorize_trailbase_bootstrap(&node.kernel, preauthorization)
            .expect("bootstrap preauthorization");
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            let counts: (i64, i64, i64) = connection
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM auth_subjects), (SELECT COUNT(*) FROM workspace_memberships), (SELECT COUNT(*) FROM fasti_browser_sessions)",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("pre-cleanup counts");
            assert_eq!(counts, (0, 0, 0));
        }
        let completion = CompleteTrailBaseBootstrapCommand::new(
            PreauthorizeTrailBaseBootstrapCommand::new(
                ceremony.id(),
                identity,
                RequestCorrelationId::new_v7(),
                at(5),
            ),
            node.kernel
                .ensure_bootstrap_secret()
                .expect("bootstrap secret"),
        );
        let created = HumanAccessPort::complete_trailbase_bootstrap(&node.kernel, completion)
            .expect("complete bootstrap");
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let state: (i64, i64, i64, i64, String, Option<String>) = connection
            .query_row(
                r#"
                SELECT (SELECT COUNT(*) FROM auth_subjects),
                       (SELECT COUNT(*) FROM trailbase_auth_anchors),
                       (SELECT COUNT(*) FROM workspace_memberships),
                       (SELECT COUNT(*) FROM auth_subject_profile_grants),
                       ceremony.state, authentication.recent_authentication_expires_at
                FROM auth_ceremonies ceremony
                JOIN fasti_browser_session_authentication authentication
                  ON authentication.browser_session_id = ?2
                WHERE ceremony.operation_id = ?1
                "#,
                params![
                    ceremony.id().to_string(),
                    created.session().id().to_string()
                ],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("bootstrap state");
        assert_eq!(state, (1, 1, 1, 1, "completed".to_owned(), None));
    }

    #[test]
    fn operator_bootstrap_requires_consumed_node_enrollment() {
        let node = TestNode::new();
        let secret = node
            .kernel
            .ensure_bootstrap_secret()
            .expect("bootstrap secret");
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute(
                    "UPDATE node_state SET initialization_consumed_at = NULL WHERE singleton = 1",
                    [],
                )
                .expect("restore provisional state");
        }
        let denied = HumanAccessPort::prepare_trailbase_bootstrap(
            &node.kernel,
            PrepareTrailBaseBootstrapQuery::new(
                SecretMaterial::from_bytes(*secret.expose_bytes()),
                RequestCorrelationId::new_v7(),
            ),
        )
        .expect_err("provisional enrollment must not authorize human bootstrap");
        assert_eq!(denied.code(), ProblemCode::Forbidden);
        assert_eq!(denied.capability(), CapabilityKey::AccessIdentityBootstrap);
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute(
                    "UPDATE node_state SET initialization_consumed_at = ?1 WHERE singleton = 1",
                    [timestamp(at(1))],
                )
                .expect("complete enrollment");
            let ceremony_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM auth_ceremonies", [], |row| row.get(0))
                .expect("ceremony count");
            assert_eq!(ceremony_count, 0);
        }
        let selection = HumanAccessPort::prepare_trailbase_bootstrap(
            &node.kernel,
            PrepareTrailBaseBootstrapQuery::new(secret, RequestCorrelationId::new_v7()),
        )
        .expect("completed enrollment selection");
        assert_eq!(selection.workspace_id(), node.access.workspace_id());
        assert_eq!(
            selection.selected_profile_grant_id(),
            node.access.grant_id()
        );
    }

    #[test]
    fn operator_bootstrap_preserves_provenance_without_an_active_session() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("active installation");
        let ceremony = bootstrap_ceremony(&node, &installation, 86);
        claim_bootstrap(&node.kernel, &installation, &ceremony);
        let identity = confirmed_identity(&installation, 86);
        let authorization = PreauthorizeTrailBaseBootstrapCommand::new(
            ceremony.id(),
            identity,
            RequestCorrelationId::new_v7(),
            at(4),
        );
        HumanAccessPort::preauthorize_trailbase_bootstrap(&node.kernel, authorization)
            .expect("bootstrap preauthorization");
        HumanAccessPort::complete_trailbase_identity_bootstrap(
            &node.kernel,
            CompleteTrailBaseBootstrapCommand::new(
                authorization,
                node.kernel
                    .ensure_bootstrap_secret()
                    .expect("bootstrap secret"),
            ),
        )
        .expect("identity-only completion");
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let evidence: (i64, i64, i64, i64, i64, i64, String) = connection
            .query_row(
                r#"
                SELECT (SELECT COUNT(*) FROM auth_subjects),
                       (SELECT COUNT(*) FROM workspace_memberships),
                       (SELECT COUNT(*) FROM fasti_browser_sessions),
                       (SELECT COUNT(*) FROM fasti_browser_sessions WHERE revoked_at IS NULL),
                       (SELECT COUNT(*) FROM fasti_browser_session_authentication),
                       (SELECT COUNT(*) FROM access_audit_events WHERE event_kind = 'browser_session_revoked'),
                       state
                FROM auth_ceremonies
                WHERE operation_id = ?1
                "#,
                [ceremony.id().to_string()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .expect("operator bootstrap evidence");
        assert_eq!(evidence, (1, 1, 1, 0, 1, 1, "completed".to_owned()));
    }

    #[test]
    fn operator_bootstrap_rechecks_enrollment_and_global_anchor_state() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("active installation");
        let ceremony = bootstrap_ceremony(&node, &installation, 87);
        claim_bootstrap(&node.kernel, &installation, &ceremony);
        let authorization = PreauthorizeTrailBaseBootstrapCommand::new(
            ceremony.id(),
            confirmed_identity(&installation, 87),
            RequestCorrelationId::new_v7(),
            at(4),
        );
        HumanAccessPort::preauthorize_trailbase_bootstrap(&node.kernel, authorization)
            .expect("bootstrap preauthorization");
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute(
                    "UPDATE node_state SET initialization_consumed_at = NULL WHERE singleton = 1",
                    [],
                )
                .expect("restore provisional enrollment");
        }
        let denied = HumanAccessPort::complete_trailbase_identity_bootstrap(
            &node.kernel,
            CompleteTrailBaseBootstrapCommand::new(
                authorization,
                node.kernel
                    .ensure_bootstrap_secret()
                    .expect("bootstrap secret"),
            ),
        )
        .expect_err("final transaction must recheck enrollment");
        assert_eq!(denied.code(), ProblemCode::Forbidden);
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            let state: (i64, i64, i64, String) = connection
                .query_row(
                    r#"
                    SELECT (SELECT COUNT(*) FROM auth_subjects),
                           (SELECT COUNT(*) FROM workspace_memberships),
                           (SELECT COUNT(*) FROM fasti_browser_sessions),
                           state
                    FROM auth_ceremonies WHERE operation_id = ?1
                    "#,
                    [ceremony.id().to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .expect("unchanged provisional bootstrap");
            assert_eq!(state, (0, 0, 0, "claimed".to_owned()));
        }

        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("active installation");
        let ceremony = bootstrap_ceremony(&node, &installation, 88);
        claim_bootstrap(&node.kernel, &installation, &ceremony);
        let authorization = PreauthorizeTrailBaseBootstrapCommand::new(
            ceremony.id(),
            confirmed_identity(&installation, 88),
            RequestCorrelationId::new_v7(),
            at(4),
        );
        HumanAccessPort::preauthorize_trailbase_bootstrap(&node.kernel, authorization)
            .expect("bootstrap preauthorization");
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            let transaction = connection.unchecked_transaction().expect("transaction");
            let subject = AuthSubject::try_new(
                AuthSubjectId::new_v7(),
                AuthSubjectLifecycle::Active,
                0,
                0,
                at(5),
                at(5),
            )
            .expect("unrelated subject");
            insert_subject(&transaction, &subject).expect("insert unrelated subject");
            insert_anchor(
                &transaction,
                &TrailBaseExternalAnchor::new(
                    installation.id(),
                    TrailBaseSubject::from_bytes([99; 16]),
                    subject.id(),
                    at(5),
                ),
            )
            .expect("insert unrelated anchor");
            transaction.commit().expect("commit unrelated anchor");
        }
        let denied = HumanAccessPort::complete_trailbase_identity_bootstrap(
            &node.kernel,
            CompleteTrailBaseBootstrapCommand::new(
                authorization,
                node.kernel
                    .ensure_bootstrap_secret()
                    .expect("bootstrap secret"),
            ),
        )
        .expect_err("any existing anchor must close bootstrap");
        assert_eq!(denied.code(), ProblemCode::Forbidden);
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let state: (i64, i64, i64, String) = connection
            .query_row(
                r#"
                SELECT (SELECT COUNT(*) FROM auth_subjects),
                       (SELECT COUNT(*) FROM trailbase_auth_anchors),
                       (SELECT COUNT(*) FROM fasti_browser_sessions),
                       state
                FROM auth_ceremonies WHERE operation_id = ?1
                "#,
                [ceremony.id().to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("unchanged anchored bootstrap");
        assert_eq!(state, (1, 1, 0, "claimed".to_owned()));
    }

    #[test]
    fn operator_bootstrap_revocation_failure_rolls_back_identity() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("active installation");
        let ceremony = bootstrap_ceremony(&node, &installation, 89);
        claim_bootstrap(&node.kernel, &installation, &ceremony);
        let authorization = PreauthorizeTrailBaseBootstrapCommand::new(
            ceremony.id(),
            confirmed_identity(&installation, 89),
            RequestCorrelationId::new_v7(),
            at(4),
        );
        HumanAccessPort::preauthorize_trailbase_bootstrap(&node.kernel, authorization)
            .expect("bootstrap preauthorization");
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute_batch(
                    "CREATE TRIGGER fail_operator_revocation_audit BEFORE INSERT ON access_audit_events WHEN NEW.event_kind = 'browser_session_revoked' BEGIN SELECT RAISE(ABORT, 'injected revocation audit failure'); END;",
                )
                .expect("failure trigger");
        }
        HumanAccessPort::complete_trailbase_identity_bootstrap(
            &node.kernel,
            CompleteTrailBaseBootstrapCommand::new(
                authorization,
                node.kernel
                    .ensure_bootstrap_secret()
                    .expect("bootstrap secret"),
            ),
        )
        .expect_err("revocation audit failure must abort completion");
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let state: (i64, i64, i64, i64, i64, String) = connection
            .query_row(
                r#"
                SELECT (SELECT COUNT(*) FROM auth_subjects),
                       (SELECT COUNT(*) FROM trailbase_auth_anchors),
                       (SELECT COUNT(*) FROM workspace_memberships),
                       (SELECT COUNT(*) FROM auth_subject_profile_grants),
                       (SELECT COUNT(*) FROM fasti_browser_sessions),
                       state
                FROM auth_ceremonies WHERE operation_id = ?1
                "#,
                [ceremony.id().to_string()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("rolled-back operator bootstrap");
        assert_eq!(state, (0, 0, 0, 0, 0, "claimed".to_owned()));
    }

    #[test]
    fn post_cleanup_bootstrap_secret_and_audit_failures_leave_no_identity_or_session() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("active installation");
        let ceremony = selected_ceremony(
            &installation,
            AuthCeremonyPurpose::FirstAdministratorBootstrap,
            node.access.workspace_id(),
            node.access.grant_id(),
            None,
            84,
        );
        claim_ceremony(&node.kernel, &installation, &ceremony);
        let authorization = PreauthorizeTrailBaseBootstrapCommand::new(
            ceremony.id(),
            confirmed_identity(&installation, 84),
            RequestCorrelationId::new_v7(),
            at(4),
        );
        HumanAccessPort::preauthorize_trailbase_bootstrap(&node.kernel, authorization)
            .expect("bootstrap preauthorization");
        let wrong = HumanAccessPort::complete_trailbase_bootstrap(
            &node.kernel,
            CompleteTrailBaseBootstrapCommand::new(
                authorization,
                SecretMaterial::from_bytes([0; 32]),
            ),
        )
        .err()
        .expect("wrong bootstrap secret");
        assert_eq!(wrong.code(), ProblemCode::Forbidden);
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute_batch(
                    "CREATE TRIGGER fail_bootstrap_session_audit BEFORE INSERT ON access_audit_events WHEN NEW.event_kind = 'browser_session_issued' BEGIN SELECT RAISE(ABORT, 'injected bootstrap audit failure'); END;",
                )
                .expect("failure trigger");
        }
        let failed = HumanAccessPort::complete_trailbase_bootstrap(
            &node.kernel,
            CompleteTrailBaseBootstrapCommand::new(
                authorization,
                node.kernel
                    .ensure_bootstrap_secret()
                    .expect("bootstrap secret"),
            ),
        );
        assert!(failed.is_err());
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let state: (i64, i64, i64, i64, i64, String) = connection
            .query_row(
                r#"
                SELECT (SELECT COUNT(*) FROM auth_subjects),
                       (SELECT COUNT(*) FROM trailbase_auth_anchors),
                       (SELECT COUNT(*) FROM workspace_memberships),
                       (SELECT COUNT(*) FROM auth_subject_profile_grants),
                       (SELECT COUNT(*) FROM fasti_browser_sessions),
                       ceremony.state
                FROM auth_ceremonies ceremony
                WHERE ceremony.operation_id = ?1
                "#,
                [ceremony.id().to_string()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("rolled-back bootstrap");
        assert_eq!(state, (0, 0, 0, 0, 0, "claimed".to_owned()));
    }

    #[test]
    fn concurrent_post_cleanup_bootstrap_replay_has_one_atomic_winner() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("active installation");
        let ceremony = selected_ceremony(
            &installation,
            AuthCeremonyPurpose::FirstAdministratorBootstrap,
            node.access.workspace_id(),
            node.access.grant_id(),
            None,
            85,
        );
        claim_ceremony(&node.kernel, &installation, &ceremony);
        let authorization = PreauthorizeTrailBaseBootstrapCommand::new(
            ceremony.id(),
            confirmed_identity(&installation, 85),
            RequestCorrelationId::new_v7(),
            at(4),
        );
        HumanAccessPort::preauthorize_trailbase_bootstrap(&node.kernel, authorization)
            .expect("bootstrap preauthorization");
        let barrier = Arc::new(Barrier::new(3));
        let workers = [authorization; 2].map(|authorization| {
            let kernel = node.kernel.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let secret = kernel.ensure_bootstrap_secret().expect("bootstrap secret");
                barrier.wait();
                HumanAccessPort::complete_trailbase_bootstrap(
                    &kernel,
                    CompleteTrailBaseBootstrapCommand::new(authorization, secret),
                )
            })
        });
        barrier.wait();
        let results = workers.map(|worker| worker.join().expect("bootstrap worker"));
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let counts: (i64, i64, i64, i64, i64, i64) = connection
            .query_row(
                r#"
                SELECT (SELECT COUNT(*) FROM auth_subjects),
                       (SELECT COUNT(*) FROM trailbase_auth_anchors),
                       (SELECT COUNT(*) FROM workspace_memberships),
                       (SELECT COUNT(*) FROM auth_subject_profile_grants),
                       (SELECT COUNT(*) FROM fasti_browser_sessions),
                       (SELECT COUNT(*) FROM auth_ceremonies WHERE state = 'completed')
                "#,
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("bootstrap winner counts");
        assert_eq!(counts, (1, 1, 1, 1, 1, 1));
    }

    #[test]
    fn installation_persists_exact_root_and_release_identities() {
        let (_root, kernel, active) = active_kernel();
        assert_eq!(active.physical_root_identity(), &fixture_root_identity());
        assert_eq!(
            active.release_lock_identity(),
            Some(&fixture_release_lock_identity())
        );

        let repeated = kernel
            .verify_trailbase_installation(
                active.id(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("same installation identity is idempotent");
        assert_eq!(repeated.activation_generation(), 1);

        let release_blocked = kernel
            .verify_trailbase_installation(
                active.id(),
                fixture_root_identity(),
                other_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(1),
            )
            .expect("exact release mismatch blocks");
        assert_eq!(
            release_blocked.activation_state(),
            TrailBaseActivationState::Blocked(TrailBaseActivationBlocker::ReleaseMismatch)
        );
        assert_eq!(release_blocked.activation_generation(), 2);
        assert_eq!(
            release_blocked.release_lock_identity(),
            Some(&fixture_release_lock_identity())
        );

        let repaired = kernel
            .verify_trailbase_installation(
                active.id(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(2),
            )
            .expect("exact release repairs the recoverable blocker");
        assert_eq!(
            repaired.activation_state(),
            TrailBaseActivationState::Active
        );
        assert_eq!(repaired.activation_generation(), 2);

        let root_blocked = kernel
            .verify_trailbase_installation(
                active.id(),
                Sha256Digest::from_bytes(&[0x44; 32]),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(3),
            )
            .expect("exact root mismatch blocks");
        assert_eq!(
            root_blocked.activation_state(),
            TrailBaseActivationState::Blocked(
                TrailBaseActivationBlocker::PhysicalRootIdentityMismatch
            )
        );
        assert_eq!(root_blocked.activation_generation(), 3);
    }

    #[test]
    fn installation_is_bound_to_physical_root_and_declared_restore_is_terminal_in_c1() {
        let (_root, kernel, active) = active_kernel();
        assert_eq!(active.activation_state(), TrailBaseActivationState::Active);
        assert_eq!(active.activation_generation(), 1);

        let blocked = kernel
            .verify_trailbase_installation(
                active.id(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                true,
                RequestCorrelationId::new_v7(),
                at(1),
            )
            .expect("declare restore");
        assert!(matches!(
            blocked.activation_state(),
            TrailBaseActivationState::Blocked(_)
        ));
        assert_eq!(blocked.activation_generation(), 2);
        assert!(matches!(
            kernel.verify_trailbase_installation(
                active.id(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(2),
            ),
            Err(HumanAccessStoreError::Invariant(
                AccessInvariantError::TrailBaseInstallationBlocked
            ))
        ));
    }

    #[test]
    fn ceremony_claim_has_one_winner_and_cancellation_rejects_claimed_rows() {
        let (_root, kernel, installation) = active_kernel();
        let ceremony = ceremony(&installation, 1, 10);
        kernel
            .insert_auth_ceremony(&ceremony)
            .expect("insert ceremony");
        let claimed = kernel
            .claim_auth_ceremony(
                ceremony.browser_binding_digest(),
                installation.id(),
                installation.activation_generation(),
                ceremony.callback_path(),
                RequestCorrelationId::new_v7(),
                at(2),
            )
            .expect("claim ceremony");
        assert_eq!(claimed.state(), AuthCeremonyState::Claimed);
        assert!(matches!(
            kernel.claim_auth_ceremony(
                ceremony.browser_binding_digest(),
                installation.id(),
                installation.activation_generation(),
                ceremony.callback_path(),
                RequestCorrelationId::new_v7(),
                at(3),
            ),
            Err(HumanAccessStoreError::Invariant(
                AccessInvariantError::InvalidCeremonyTransition
            ))
        ));
        assert!(matches!(
            kernel.cancel_auth_ceremony(ceremony.id(), RequestCorrelationId::new_v7(), at(3),),
            Err(HumanAccessStoreError::Invariant(
                AccessInvariantError::InvalidCeremonyTransition
            ))
        ));
    }

    #[test]
    fn concurrent_claims_grant_exactly_one_exchange_permission() {
        let (_root, kernel, installation) = active_kernel();
        let ceremony = ceremony(&installation, 1, 10);
        kernel
            .insert_auth_ceremony(&ceremony)
            .expect("insert ceremony");
        let barrier = Arc::new(Barrier::new(3));
        let instance_id = installation.id();
        let activation_generation = installation.activation_generation();
        let mut workers = Vec::new();
        for _ in 0..2 {
            let kernel = kernel.clone();
            let barrier = Arc::clone(&barrier);
            let digest = ceremony.browser_binding_digest().clone();
            let callback = ceremony.callback_path().clone();
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                kernel.claim_auth_ceremony(
                    &digest,
                    instance_id,
                    activation_generation,
                    &callback,
                    RequestCorrelationId::new_v7(),
                    at(2),
                )
            }));
        }
        barrier.wait();
        let results: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().expect("claim worker"))
            .collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| {
                    matches!(
                        result,
                        Err(HumanAccessStoreError::Invariant(
                            AccessInvariantError::InvalidCeremonyTransition
                        ))
                    )
                })
                .count(),
            1
        );
    }

    #[test]
    fn ceremony_capacity_rejects_the_row_after_the_exact_ten_thousand_bound() {
        let (_root, kernel, installation) = active_kernel();
        insert_pending_rows(&kernel, &installation, 9_999);
        kernel
            .insert_auth_ceremony(&ceremony(&installation, 1, 10))
            .expect("ten-thousandth row");
        assert!(matches!(
            kernel.insert_auth_ceremony(&ceremony(&installation, 1, 10)),
            Err(HumanAccessStoreError::CapacityExceeded)
        ));
        let connection = kernel.inner.connection.lock().expect("connection");
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM auth_ceremonies", [], |row| row.get(0))
            .expect("ceremony count");
        assert_eq!(count, CEREMONY_CAPACITY);
    }

    #[test]
    fn terminal_and_audit_retention_use_the_exact_frozen_boundaries() {
        let (_root, kernel, installation) = active_kernel();
        let terminal = ceremony(&installation, 1, 3);
        kernel
            .insert_auth_ceremony(&terminal)
            .expect("insert ceremony");
        kernel
            .cancel_auth_ceremony(terminal.id(), RequestCorrelationId::new_v7(), at(2))
            .expect("cancel ceremony");

        kernel
            .maintain_auth_ceremonies(RequestCorrelationId::new_v7(), at(2 + 24 * 60 - 1))
            .expect("retain before boundary");
        {
            let connection = kernel.inner.connection.lock().expect("connection");
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM auth_ceremonies WHERE operation_id = ?1",
                    [terminal.id().to_string()],
                    |row| row.get(0),
                )
                .expect("terminal count");
            assert_eq!(count, 1);
        }
        kernel
            .maintain_auth_ceremonies(RequestCorrelationId::new_v7(), at(2 + 24 * 60))
            .expect("prune at boundary");
        {
            let connection = kernel.inner.connection.lock().expect("connection");
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM auth_ceremonies WHERE operation_id = ?1",
                    [terminal.id().to_string()],
                    |row| row.get(0),
                )
                .expect("terminal count");
            assert_eq!(count, 0);
        }

        kernel
            .maintain_auth_ceremonies(
                RequestCorrelationId::new_v7(),
                at(AUDIT_RETENTION_DAYS * 24 * 60 - 1),
            )
            .expect("retain audit before boundary");
        {
            let connection = kernel.inner.connection.lock().expect("connection");
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM access_audit_events WHERE occurred_at = ?1",
                    [timestamp(at(0))],
                    |row| row.get(0),
                )
                .expect("old audit count");
            assert_eq!(count, 1);
        }
        kernel
            .maintain_auth_ceremonies(
                RequestCorrelationId::new_v7(),
                at(AUDIT_RETENTION_DAYS * 24 * 60),
            )
            .expect("prune audit at boundary");
        let connection = kernel.inner.connection.lock().expect("connection");
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM access_audit_events WHERE occurred_at = ?1",
                [timestamp(at(0))],
                |row| row.get(0),
            )
            .expect("old audit count");
        assert_eq!(count, 0);
    }

    #[test]
    fn audit_overflow_keeps_the_newest_ten_thousand_events() {
        let (_root, kernel, installation) = active_kernel();
        {
            let connection = kernel.inner.connection.lock().expect("connection");
            let transaction = connection.unchecked_transaction().expect("transaction");
            {
                let mut statement = transaction
                    .prepare(
                        r#"
                        INSERT INTO access_audit_events(
                            event_kind, trailbase_instance_id, correlation_id, occurred_at
                        ) VALUES ('trailbase_activated', ?1, ?2, ?3)
                        "#,
                    )
                    .expect("prepare audit insert");
                for _ in 1..AUDIT_CAPACITY {
                    statement
                        .execute(params![
                            installation.id().to_string(),
                            RequestCorrelationId::new_v7().to_string(),
                            timestamp(at(0)),
                        ])
                        .expect("insert audit");
                }
            }
            transaction.commit().expect("commit audits");
        }
        kernel
            .verify_trailbase_installation(
                installation.id(),
                fixture_root_identity(),
                other_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(1),
            )
            .expect("block mismatched release");
        let connection = kernel.inner.connection.lock().expect("connection");
        let (count, newest_kind, oldest_id): (i64, String, i64) = connection
            .query_row(
                r#"
                SELECT COUNT(*),
                       (SELECT event_kind FROM access_audit_events
                        ORDER BY occurred_at DESC, audit_event_id DESC LIMIT 1),
                       (SELECT MIN(audit_event_id) FROM access_audit_events)
                FROM access_audit_events
                "#,
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("audit inventory");
        assert_eq!(count, AUDIT_CAPACITY);
        assert_eq!(newest_kind, AccessAuditEventKind::TrailBaseBlocked.as_str());
        assert_eq!(oldest_id, 2);
    }

    #[test]
    fn membership_mutations_recheck_admin_continuity_and_revoke_suspended_sessions() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("installation");
        let (actor_id, actor_membership_id, workspace_id) =
            bootstrap_administrator(&node, &installation);
        let (target_id, target_membership_id) = add_subject(
            &node,
            &installation,
            workspace_id,
            MembershipLifecycle::Active,
            WorkspaceRole::Administrator,
            72,
        );
        let actor_session = create_test_session(
            &node,
            &installation,
            actor_id,
            workspace_id,
            node.access.grant_id(),
            at(3),
        );
        let target_session = create_test_session(
            &node,
            &installation,
            target_id,
            workspace_id,
            node.access.grant_id(),
            at(3),
        );

        assert!(HumanAccessPort::change_membership_role(
            &node.kernel,
            ChangeMembershipRoleCommand::new(
                mutation_proof(&actor_session, at(4)),
                target_membership_id,
                WorkspaceRole::Member,
            ),
        )
        .expect("demote second administrator"));
        assert!(HumanAccessPort::change_membership_lifecycle(
            &node.kernel,
            ChangeMembershipLifecycleCommand::new(
                mutation_proof(&actor_session, at(5)),
                target_membership_id,
                fasti_application::AdministratorMembershipAction::Suspend,
            ),
        )
        .expect("suspend member"));

        let problem = HumanAccessPort::change_membership_role(
            &node.kernel,
            ChangeMembershipRoleCommand::new(
                mutation_proof(&actor_session, at(6)),
                actor_membership_id,
                WorkspaceRole::Member,
            ),
        )
        .expect_err("sole viable administrator must remain");
        assert_eq!(problem.code(), ProblemCode::ValidationFailed);

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let target: (String, String, i64, Option<String>) = connection
            .query_row(
                r#"
                SELECT membership.lifecycle, membership.role,
                       subject.authorization_epoch, session.revoked_at
                FROM workspace_memberships membership
                JOIN auth_subjects subject
                  ON subject.auth_subject_id = membership.auth_subject_id
                JOIN fasti_browser_sessions session
                  ON session.auth_subject_id = membership.auth_subject_id
                WHERE membership.membership_id = ?1
                  AND session.browser_session_id = ?2
                "#,
                params![
                    target_membership_id.to_string(),
                    target_session.session().id().to_string(),
                ],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("target state");
        assert_eq!(target.0, "suspended");
        assert_eq!(target.1, "member");
        assert_eq!(target.2, 2);
        assert_eq!(target.3, Some(timestamp(at(5))));
        let actor: (String, i64) = connection
            .query_row(
                r#"
                SELECT membership.role, subject.authorization_epoch
                FROM workspace_memberships membership
                JOIN auth_subjects subject
                  ON subject.auth_subject_id = membership.auth_subject_id
                WHERE membership.membership_id = ?1
                "#,
                [actor_membership_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("actor state");
        assert_eq!(actor, ("administrator".to_owned(), 0));
        let audits: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM access_audit_events WHERE event_kind IN ('membership_demoted', 'membership_suspended') AND auth_subject_id = ?1",
                [target_id.to_string()],
                |row| row.get(0),
            )
            .expect("membership audits");
        assert_eq!(audits, 2);
        let revocation_audits: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM access_audit_events WHERE event_kind = 'browser_session_revoked' AND auth_subject_id = ?1 AND browser_session_id = ?2",
                params![
                    target_id.to_string(),
                    target_session.session().id().to_string(),
                ],
                |row| row.get(0),
            )
            .expect("revocation audit");
        assert_eq!(revocation_audits, 1);
    }

    #[test]
    fn membership_lifecycle_actions_persist_exact_state_and_audit_sequence() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("installation");
        let (actor_id, _, workspace_id) = bootstrap_administrator(&node, &installation);
        let (target_id, target_membership_id) = add_subject(
            &node,
            &installation,
            workspace_id,
            MembershipLifecycle::PendingApproval,
            WorkspaceRole::Member,
            77,
        );
        let actor_session = create_test_session(
            &node,
            &installation,
            actor_id,
            workspace_id,
            node.access.grant_id(),
            at(3),
        );

        for (minute, action) in [
            (4, fasti_application::AdministratorMembershipAction::Approve),
            (5, fasti_application::AdministratorMembershipAction::Suspend),
            (6, fasti_application::AdministratorMembershipAction::Resume),
            (7, fasti_application::AdministratorMembershipAction::Remove),
        ] {
            assert!(HumanAccessPort::change_membership_lifecycle(
                &node.kernel,
                ChangeMembershipLifecycleCommand::new(
                    mutation_proof(&actor_session, at(minute)),
                    target_membership_id,
                    action,
                ),
            )
            .expect("valid lifecycle action"));
        }

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let persisted: (String, i64) = connection
            .query_row(
                r#"
                SELECT membership.lifecycle, subject.authorization_epoch
                FROM workspace_memberships membership
                JOIN auth_subjects subject
                  ON subject.auth_subject_id = membership.auth_subject_id
                WHERE membership.membership_id = ?1
                "#,
                [target_membership_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("lifecycle state");
        assert_eq!(persisted, ("removed".to_owned(), 4));
        let audit_sequence = {
            let mut statement = connection
                .prepare(
                    r#"
                    SELECT event_kind FROM access_audit_events
                    WHERE membership_id = ?1 AND actor_auth_subject_id = ?2
                    ORDER BY occurred_at, audit_event_id
                    "#,
                )
                .expect("prepare lifecycle audits");
            statement
                .query_map(
                    params![target_membership_id.to_string(), actor_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .expect("query lifecycle audits")
                .collect::<Result<Vec<_>, _>>()
                .expect("collect lifecycle audits")
        };
        assert_eq!(
            audit_sequence,
            [
                "membership_approved",
                "membership_suspended",
                "membership_resumed",
                "membership_removed",
            ]
        );
        let target_lifecycle: String = connection
            .query_row(
                "SELECT lifecycle FROM auth_subjects WHERE auth_subject_id = ?1",
                [target_id.to_string()],
                |row| row.get(0),
            )
            .expect("target subject");
        assert_eq!(target_lifecycle, "active");
    }

    #[test]
    fn concurrent_admin_demotion_preserves_one_viable_administrator() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("installation");
        let (first_id, first_membership_id, workspace_id) =
            bootstrap_administrator(&node, &installation);
        let (second_id, second_membership_id) = add_subject(
            &node,
            &installation,
            workspace_id,
            MembershipLifecycle::Active,
            WorkspaceRole::Administrator,
            78,
        );
        let first_session = create_test_session(
            &node,
            &installation,
            first_id,
            workspace_id,
            node.access.grant_id(),
            at(3),
        );
        let second_session = create_test_session(
            &node,
            &installation,
            second_id,
            workspace_id,
            node.access.grant_id(),
            at(3),
        );
        let barrier = Arc::new(Barrier::new(3));
        let mut workers = Vec::new();
        for (kernel, barrier, command) in [
            (
                node.kernel.clone(),
                Arc::clone(&barrier),
                ChangeMembershipRoleCommand::new(
                    mutation_proof(&first_session, at(4)),
                    second_membership_id,
                    WorkspaceRole::Member,
                ),
            ),
            (
                node.kernel.clone(),
                Arc::clone(&barrier),
                ChangeMembershipRoleCommand::new(
                    mutation_proof(&second_session, at(4)),
                    first_membership_id,
                    WorkspaceRole::Member,
                ),
            ),
        ] {
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                HumanAccessPort::change_membership_role(&kernel, command)
            }));
        }
        barrier.wait();
        let outcomes: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().expect("demotion worker"))
            .collect();
        assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
        assert_eq!(
            outcomes.iter().filter(|outcome| outcome.is_err()).count(),
            1
        );

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let persisted: (i64, i64) = connection
            .query_row(
                r#"
                SELECT
                    (SELECT COUNT(*) FROM workspace_memberships membership
                     JOIN auth_subjects subject
                       ON subject.auth_subject_id = membership.auth_subject_id
                     WHERE membership.workspace_id = ?1
                       AND membership.lifecycle = 'active'
                       AND membership.role = 'administrator'
                       AND subject.lifecycle = 'active'),
                    (SELECT COUNT(*) FROM access_audit_events
                     WHERE workspace_id = ?1 AND event_kind = 'membership_demoted')
                "#,
                [workspace_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("administrator continuity");
        assert_eq!(persisted, (1, 1));
    }

    #[test]
    fn subject_lifecycle_change_is_authorized_atomic_and_revokes_all_sessions() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("installation");
        let (actor_id, _, workspace_id) = bootstrap_administrator(&node, &installation);
        let (target_id, _) = add_subject(
            &node,
            &installation,
            workspace_id,
            MembershipLifecycle::Active,
            WorkspaceRole::Member,
            73,
        );
        let actor_session = create_test_session(
            &node,
            &installation,
            actor_id,
            workspace_id,
            node.access.grant_id(),
            at(3),
        );
        let target_session = create_test_session(
            &node,
            &installation,
            target_id,
            workspace_id,
            node.access.grant_id(),
            at(3),
        );

        assert!(HumanAccessPort::change_auth_subject_lifecycle(
            &node.kernel,
            ChangeAuthSubjectLifecycleCommand::new(
                mutation_proof(&actor_session, at(4)),
                target_id,
                fasti_application::AdministratorSubjectAction::Disable,
            ),
        )
        .expect("disable subject"));

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let persisted: (String, i64, i64, String, Option<String>, i64, i64, i64) = connection
            .query_row(
                r#"
                SELECT subject.lifecycle, subject.auth_epoch,
                       subject.authorization_epoch, membership.lifecycle,
                       session.revoked_at,
                       (SELECT COUNT(*) FROM access_audit_events
                        WHERE event_kind = 'subject_disabled'
                          AND auth_subject_id = ?1
                          AND actor_auth_subject_id = ?3),
                       (SELECT COUNT(*) FROM access_audit_events
                        WHERE event_kind = 'membership_suspended'
                          AND membership_id = membership.membership_id
                          AND actor_auth_subject_id = ?3),
                       (SELECT COUNT(*) FROM access_audit_events
                        WHERE event_kind = 'browser_session_revoked'
                          AND auth_subject_id = ?1
                          AND actor_auth_subject_id = ?3)
                FROM auth_subjects subject
                JOIN workspace_memberships membership
                  ON membership.auth_subject_id = subject.auth_subject_id
                JOIN fasti_browser_sessions session
                  ON session.auth_subject_id = subject.auth_subject_id
                WHERE subject.auth_subject_id = ?1
                  AND session.browser_session_id = ?2
                "#,
                params![
                    target_id.to_string(),
                    target_session.session().id().to_string(),
                    actor_id.to_string(),
                ],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .expect("subject state");
        assert_eq!(persisted.0, "disabled");
        assert_eq!(persisted.1, 1);
        assert_eq!(persisted.2, 1);
        assert_eq!(persisted.3, "suspended");
        assert_eq!(persisted.4, Some(timestamp(at(4))));
        assert_eq!(persisted.5, 1);
        assert_eq!(persisted.6, 1);
        assert_eq!(persisted.7, 1);
    }

    #[test]
    fn subject_lifecycle_requires_admin_authority_in_every_current_workspace() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("installation");
        let (actor_id, _, workspace_id) = bootstrap_administrator(&node, &installation);
        let (target_id, target_membership_id) = add_subject(
            &node,
            &installation,
            workspace_id,
            MembershipLifecycle::Active,
            WorkspaceRole::Member,
            79,
        );
        let other_workspace_id = WorkspaceId::new_v7();
        let other_membership_id = MembershipId::new_v7();
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            let transaction = connection.unchecked_transaction().expect("transaction");
            transaction
                .execute(
                    "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                    params![other_workspace_id.to_string(), timestamp(at(3))],
                )
                .expect("other workspace");
            transaction
                .execute(
                    "INSERT INTO workspace_memberships(membership_id, auth_subject_id, workspace_id, lifecycle, role, created_at, updated_at) VALUES (?1, ?2, ?3, 'active', 'member', ?4, ?4)",
                    params![
                        other_membership_id.to_string(),
                        target_id.to_string(),
                        other_workspace_id.to_string(),
                        timestamp(at(3)),
                    ],
                )
                .expect("other membership");
            transaction.commit().expect("other workspace membership");
        }
        let actor_session = create_test_session(
            &node,
            &installation,
            actor_id,
            workspace_id,
            node.access.grant_id(),
            at(3),
        );
        let target_session = create_test_session(
            &node,
            &installation,
            target_id,
            workspace_id,
            node.access.grant_id(),
            at(3),
        );

        let problem = HumanAccessPort::change_auth_subject_lifecycle(
            &node.kernel,
            ChangeAuthSubjectLifecycleCommand::new(
                mutation_proof(&actor_session, at(4)),
                target_id,
                fasti_application::AdministratorSubjectAction::Disable,
            ),
        )
        .expect_err("partial cross-workspace authority must fail");
        assert_eq!(problem.code(), ProblemCode::Forbidden);

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let persisted: (String, i64, i64, Option<String>, i64) = connection
            .query_row(
                r#"
                SELECT subject.lifecycle, subject.auth_epoch,
                       (SELECT COUNT(*) FROM workspace_memberships
                        WHERE auth_subject_id = ?1 AND lifecycle = 'active'),
                       session.revoked_at,
                       (SELECT COUNT(*) FROM access_audit_events
                        WHERE auth_subject_id = ?1
                          AND event_kind IN (
                              'subject_disabled', 'membership_suspended',
                              'browser_session_revoked'
                          ))
                FROM auth_subjects subject
                JOIN fasti_browser_sessions session
                  ON session.auth_subject_id = subject.auth_subject_id
                WHERE subject.auth_subject_id = ?1
                  AND session.browser_session_id = ?2
                "#,
                params![
                    target_id.to_string(),
                    target_session.session().id().to_string(),
                ],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("unchanged cross-workspace target");
        assert_eq!(persisted, ("active".to_owned(), 0, 2, None, 0));
        let original_membership: String = connection
            .query_row(
                "SELECT lifecycle FROM workspace_memberships WHERE membership_id = ?1",
                [target_membership_id.to_string()],
                |row| row.get(0),
            )
            .expect("original membership");
        assert_eq!(original_membership, "active");
    }

    #[test]
    fn subject_audit_failure_rolls_back_membership_epoch_and_revocation() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("installation");
        let (actor_id, _, workspace_id) = bootstrap_administrator(&node, &installation);
        let (target_id, target_membership_id) = add_subject(
            &node,
            &installation,
            workspace_id,
            MembershipLifecycle::Active,
            WorkspaceRole::Member,
            80,
        );
        let actor_session = create_test_session(
            &node,
            &installation,
            actor_id,
            workspace_id,
            node.access.grant_id(),
            at(3),
        );
        let target_session = create_test_session(
            &node,
            &installation,
            target_id,
            workspace_id,
            node.access.grant_id(),
            at(3),
        );
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute_batch(
                    r#"
                    CREATE TRIGGER reject_subject_disabled_audit
                    BEFORE INSERT ON access_audit_events
                    WHEN NEW.event_kind = 'subject_disabled'
                    BEGIN
                        SELECT RAISE(ABORT, 'injected subject audit failure');
                    END;
                    "#,
                )
                .expect("subject audit fault");
        }

        let problem = HumanAccessPort::change_auth_subject_lifecycle(
            &node.kernel,
            ChangeAuthSubjectLifecycleCommand::new(
                mutation_proof(&actor_session, at(4)),
                target_id,
                fasti_application::AdministratorSubjectAction::Disable,
            ),
        )
        .expect_err("subject audit failure must roll back");
        assert_eq!(problem.code(), ProblemCode::StorageUnavailable);

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let persisted: (String, i64, i64, String, Option<String>, i64) = connection
            .query_row(
                r#"
                SELECT subject.lifecycle, subject.auth_epoch,
                       subject.authorization_epoch, membership.lifecycle,
                       session.revoked_at,
                       (SELECT COUNT(*) FROM access_audit_events
                        WHERE auth_subject_id = ?1
                          AND event_kind IN (
                              'subject_disabled', 'membership_suspended',
                              'browser_session_revoked'
                          ))
                FROM auth_subjects subject
                JOIN workspace_memberships membership
                  ON membership.auth_subject_id = subject.auth_subject_id
                 AND membership.membership_id = ?2
                JOIN fasti_browser_sessions session
                  ON session.auth_subject_id = subject.auth_subject_id
                 AND session.browser_session_id = ?3
                WHERE subject.auth_subject_id = ?1
                "#,
                params![
                    target_id.to_string(),
                    target_membership_id.to_string(),
                    target_session.session().id().to_string(),
                ],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("rolled back subject state");
        assert_eq!(
            persisted,
            ("active".to_owned(), 0, 0, "active".to_owned(), None, 0)
        );
    }

    #[test]
    fn store_epoch_boundary_fails_before_partial_role_change() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("installation");
        let (actor_id, _, workspace_id) = bootstrap_administrator(&node, &installation);
        let (target_id, target_membership_id) = add_subject(
            &node,
            &installation,
            workspace_id,
            MembershipLifecycle::Active,
            WorkspaceRole::Member,
            81,
        );
        let actor_session = create_test_session(
            &node,
            &installation,
            actor_id,
            workspace_id,
            node.access.grant_id(),
            at(3),
        );
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute(
                    "UPDATE auth_subjects SET authorization_epoch = ?1 WHERE auth_subject_id = ?2",
                    params![i64::MAX, target_id.to_string()],
                )
                .expect("set persisted epoch boundary");
        }

        let problem = HumanAccessPort::change_membership_role(
            &node.kernel,
            ChangeMembershipRoleCommand::new(
                mutation_proof(&actor_session, at(4)),
                target_membership_id,
                WorkspaceRole::Administrator,
            ),
        )
        .expect_err("next epoch cannot fit the persisted representation");
        assert_eq!(problem.code(), ProblemCode::IntegrityFailed);

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let persisted: (String, i64, i64) = connection
            .query_row(
                r#"
                SELECT membership.role, subject.authorization_epoch,
                       (SELECT COUNT(*) FROM access_audit_events
                        WHERE membership_id = ?1
                          AND event_kind = 'membership_promoted')
                FROM workspace_memberships membership
                JOIN auth_subjects subject
                  ON subject.auth_subject_id = membership.auth_subject_id
                WHERE membership.membership_id = ?1
                "#,
                [target_membership_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("unchanged epoch boundary");
        assert_eq!(persisted, ("member".to_owned(), i64::MAX, 0));
    }

    #[test]
    fn membership_audit_failure_rolls_back_epoch_revocation_and_membership() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("installation");
        let (actor_id, _, workspace_id) = bootstrap_administrator(&node, &installation);
        let (target_id, target_membership_id) = add_subject(
            &node,
            &installation,
            workspace_id,
            MembershipLifecycle::Active,
            WorkspaceRole::Member,
            74,
        );
        let actor_session = create_test_session(
            &node,
            &installation,
            actor_id,
            workspace_id,
            node.access.grant_id(),
            at(3),
        );
        let target_session = create_test_session(
            &node,
            &installation,
            target_id,
            workspace_id,
            node.access.grant_id(),
            at(3),
        );
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute_batch(
                    r#"
                    CREATE TRIGGER reject_membership_suspended_audit
                    BEFORE INSERT ON access_audit_events
                    WHEN NEW.event_kind = 'membership_suspended'
                    BEGIN
                        SELECT RAISE(ABORT, 'injected audit failure');
                    END;
                    "#,
                )
                .expect("audit fault");
        }

        let problem = HumanAccessPort::change_membership_lifecycle(
            &node.kernel,
            ChangeMembershipLifecycleCommand::new(
                mutation_proof(&actor_session, at(4)),
                target_membership_id,
                fasti_application::AdministratorMembershipAction::Suspend,
            ),
        )
        .expect_err("audit failure must roll back");
        assert_eq!(problem.code(), ProblemCode::StorageUnavailable);

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let state: (String, i64, Option<String>, i64) = connection
            .query_row(
                r#"
                SELECT membership.lifecycle, subject.authorization_epoch,
                       session.revoked_at,
                       (SELECT COUNT(*) FROM access_audit_events
                        WHERE auth_subject_id = ?1
                          AND event_kind IN ('membership_suspended', 'browser_session_revoked'))
                FROM workspace_memberships membership
                JOIN auth_subjects subject
                  ON subject.auth_subject_id = membership.auth_subject_id
                JOIN fasti_browser_sessions session
                  ON session.auth_subject_id = subject.auth_subject_id
                WHERE membership.membership_id = ?2
                  AND session.browser_session_id = ?3
                "#,
                params![
                    target_id.to_string(),
                    target_membership_id.to_string(),
                    target_session.session().id().to_string(),
                ],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("rolled back state");
        assert_eq!(state, ("active".to_owned(), 0, None, 0));
    }

    #[test]
    fn membership_mutation_denials_leave_target_and_audit_unchanged() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("installation");
        let (actor_id, _, workspace_id) = bootstrap_administrator(&node, &installation);
        let (target_id, target_membership_id) = add_subject(
            &node,
            &installation,
            workspace_id,
            MembershipLifecycle::Active,
            WorkspaceRole::Member,
            75,
        );
        let actor_session = create_test_session(
            &node,
            &installation,
            actor_id,
            workspace_id,
            node.access.grant_id(),
            at(3),
        );
        let target_session = create_test_session(
            &node,
            &installation,
            target_id,
            workspace_id,
            node.access.grant_id(),
            at(3),
        );

        for proof in [
            invalid_csrf_proof(&actor_session, at(4)),
            mutation_proof(&target_session, at(4)),
        ] {
            let problem = HumanAccessPort::change_membership_role(
                &node.kernel,
                ChangeMembershipRoleCommand::new(
                    proof,
                    target_membership_id,
                    WorkspaceRole::Administrator,
                ),
            )
            .expect_err("invalid actor proof must fail");
            assert_eq!(problem.code(), ProblemCode::Forbidden);
        }
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute(
                    "UPDATE fasti_browser_session_authentication SET recent_authentication_expires_at = NULL WHERE browser_session_id = ?1",
                    [actor_session.session().id().to_string()],
                )
                .expect("remove recent assertion while preserving provenance");
        }
        let problem = HumanAccessPort::change_membership_role(
            &node.kernel,
            ChangeMembershipRoleCommand::new(
                mutation_proof(&actor_session, at(4)),
                target_membership_id,
                WorkspaceRole::Administrator,
            ),
        )
        .expect_err("recent authentication is required");
        assert_eq!(problem.code(), ProblemCode::Forbidden);

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let unchanged: (String, i64, i64) = connection
            .query_row(
                r#"
                SELECT membership.role, subject.authorization_epoch,
                       (SELECT COUNT(*) FROM access_audit_events
                        WHERE auth_subject_id = ?1
                          AND event_kind = 'membership_promoted')
                FROM workspace_memberships membership
                JOIN auth_subjects subject
                  ON subject.auth_subject_id = membership.auth_subject_id
                WHERE membership.membership_id = ?2
                "#,
                params![target_id.to_string(), target_membership_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("unchanged target");
        assert_eq!(unchanged, ("member".to_owned(), 0, 0));
    }

    #[test]
    fn maintenance_expires_pending_only_and_restart_recovers_each_live_state_once() {
        let (_root, kernel, installation) = active_kernel();
        let pending = ceremony(&installation, 1, 2);
        kernel
            .insert_auth_ceremony(&pending)
            .expect("insert pending");
        assert_eq!(
            kernel
                .maintain_auth_ceremonies(RequestCorrelationId::new_v7(), at(2))
                .expect("maintain"),
            vec![pending.id()]
        );
        assert!(kernel
            .maintain_auth_ceremonies(RequestCorrelationId::new_v7(), at(3))
            .expect("idempotent maintenance")
            .is_empty());
    }

    #[test]
    fn opening_the_kernel_recovers_live_ceremonies_once_before_exposure() {
        let root = tempfile::tempdir().expect("temporary root");
        let started = Utc::now();
        let kernel = SqliteKernel::open(root.path()).expect("kernel");
        let installation = kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                fixture_root_identity(),
                fixture_release_lock_identity(),
                false,
                RequestCorrelationId::new_v7(),
                started,
            )
            .expect("active installation");
        let make = || {
            AuthCeremony::try_new(
                OperationId::new_v7(),
                AuthCeremonyPurpose::SignIn,
                AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
                installation.id(),
                installation.activation_generation(),
                Sha256Digest::from_bytes(&sha256_bytes(
                    OperationId::new_v7().to_string().as_bytes(),
                )),
                None,
                false,
                AuthCallbackPath::parse("/auth/trailbase/callback").expect("callback"),
                AuthReturnTarget::ApplicationHome,
                RequestCorrelationId::new_v7(),
                started,
                started + Duration::minutes(5),
            )
            .expect("ceremony")
        };
        let pending = make();
        let claimed = make();
        kernel
            .insert_auth_ceremony(&pending)
            .expect("insert pending");
        kernel
            .insert_auth_ceremony(&claimed)
            .expect("insert claimed candidate");
        kernel
            .claim_auth_ceremony(
                claimed.browser_binding_digest(),
                installation.id(),
                installation.activation_generation(),
                claimed.callback_path(),
                RequestCorrelationId::new_v7(),
                started,
            )
            .expect("claim");
        drop(kernel);

        let reopened = SqliteKernel::open(root.path()).expect("reopened kernel");
        let connection = reopened.inner.connection.lock().expect("connection");
        let states: Vec<(String, String)> = {
            let mut statement = connection
                .prepare("SELECT state, failure FROM auth_ceremonies ORDER BY state, operation_id")
                .expect("prepare states");
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("query states")
                .collect::<Result<_, _>>()
                .expect("collect states")
        };
        assert_eq!(
            states,
            vec![
                (
                    "cleanup_uncertain".to_owned(),
                    "exchange_outcome_uncertain".to_owned()
                ),
                ("failed".to_owned(), "verifier_lost_on_restart".to_owned()),
            ]
        );
        let recovery_audits: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM access_audit_events WHERE event_kind IN ('ceremony_failed', 'ceremony_cleanup_uncertain')",
                [],
                |row| row.get(0),
            )
            .expect("recovery audit count");
        assert_eq!(recovery_audits, 2);
        drop(connection);
        drop(reopened);

        let reopened_again = SqliteKernel::open(root.path()).expect("second reopen");
        let connection = reopened_again.inner.connection.lock().expect("connection");
        let recovery_audits: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM access_audit_events WHERE event_kind IN ('ceremony_failed', 'ceremony_cleanup_uncertain')",
                [],
                |row| row.get(0),
            )
            .expect("recovery audit count");
        assert_eq!(recovery_audits, 2);
    }
}
