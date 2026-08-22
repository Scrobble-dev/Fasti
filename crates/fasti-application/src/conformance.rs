//! Ephemeral B1 conformance adapter.
//!
//! This module exists only when the `conformance-fixture` feature is enabled.
//! It is an in-memory executable example for application-contract tests, not a
//! production execution mode or a persistence adapter. Every direct result is
//! wrapped in [`FixtureOnly`], whose durability claim is explicitly
//! [`FixtureDurability::None`].

use crate::{
    authorize, AcceptObservationCommand, AcceptObservationOutcome, AcceptObservationReceipt,
    AccessSnapshot, ApplicationResult, AuthorizationRequirement, CapabilityKey, CredentialStatus,
    FastiProblem, GrantStatus, ObservationAcceptancePort, ReplayReceiptQuery, RequestAccessContext,
    ScopeKey,
};
use chrono::Utc;
use fasti_domain::{
    ClientId, CommittedAt, CredentialId, Observation, ObservationId, OperationId, ProfileGrantId,
    ProfileId, ReceiptId, RequestCorrelationId, Sha256Digest, WorkspaceId,
};
use std::{
    collections::HashMap,
    fmt,
    sync::{Mutex, MutexGuard},
};
use uuid::Uuid;

/// The only persistence statement made by the conformance fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureDurability {
    /// State exists only in this process and may disappear at any time.
    None,
}

/// Marks a value as produced only by the ephemeral B1 fixture.
///
/// This wrapper deliberately has no serialization implementation: it must not
/// be confused with a production transport or storage contract.
pub struct FixtureOnly<T> {
    value: T,
}

impl<T> FixtureOnly<T> {
    fn new(value: T) -> Self {
        Self { value }
    }

    pub const fn durability(&self) -> FixtureDurability {
        FixtureDurability::None
    }

    pub const fn is_fixture_only(&self) -> bool {
        true
    }

    pub const fn as_ref(&self) -> &T {
        &self.value
    }

    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T: fmt::Debug> fmt::Debug for FixtureOnly<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FixtureOnly")
            .field("durability", &FixtureDurability::None)
            .field("value", &self.value)
            .finish()
    }
}

/// Opaque, process-local credential material issued exactly once at enrollment.
///
/// The secret is neither `Debug`, `Clone`, nor serializable. Callers may inspect
/// it only through the explicitly fixture-scoped accessor needed by a transport
/// conformance test.
pub struct FixtureCredentialSecret {
    bytes: [u8; 32],
}

impl FixtureCredentialSecret {
    fn fresh() -> Self {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let mut bytes = [0_u8; 32];
        bytes[..16].copy_from_slice(first.as_bytes());
        bytes[16..].copy_from_slice(second.as_bytes());
        Self { bytes }
    }

    /// Inspect the secret only inside a non-production conformance harness.
    pub fn expose_for_fixture(&self) -> &[u8; 32] {
        &self.bytes
    }
}

impl Drop for FixtureCredentialSecret {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}

/// One-time initialization proof used to enroll the first fixture client.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct FixtureInitialization {
    access: RequestAccessContext,
}

impl FixtureInitialization {
    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }
}

impl fmt::Debug for FixtureInitialization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FixtureInitialization")
            .field("fixture_only", &true)
            .finish_non_exhaustive()
    }
}

/// First-client enrollment output. Credential plaintext is intentionally
/// excluded from `Debug` and serialization.
pub struct FixtureEnrollment {
    access: RequestAccessContext,
    credential_secret: FixtureCredentialSecret,
}

impl FixtureEnrollment {
    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn credential_secret(&self) -> &FixtureCredentialSecret {
        &self.credential_secret
    }
}

impl fmt::Debug for FixtureEnrollment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FixtureEnrollment")
            .field("fixture_only", &true)
            .field("access", &self.access)
            .field("credential_secret", &"[REDACTED]")
            .finish()
    }
}

/// Observable fixture phase without exposing stored observations or receipts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixturePhase {
    Empty,
    Initialized,
    Enrolled,
}

/// Counts useful to conformance assertions. They are not persistence receipts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixtureStateView {
    pub phase: FixturePhase,
    pub operation_count: usize,
    pub receipt_count: usize,
}

struct InitializedState {
    enrollment_access: RequestAccessContext,
    enrollment_snapshot: AccessSnapshot,
}

struct EnrolledState {
    access_snapshot: AccessSnapshot,
    credential_secret: [u8; 32],
    operations: HashMap<OperationId, StoredOperation>,
    receipts: HashMap<ReceiptId, AcceptObservationReceipt>,
}

struct StoredOperation {
    capability: CapabilityKey,
    digest: Sha256Digest,
    receipt: AcceptObservationReceipt,
}

enum FixtureState {
    Empty,
    Initialized(InitializedState),
    Enrolled(EnrolledState),
}

/// In-memory, feature-gated adapter for exercising the frozen B1 semantics.
///
/// There is intentionally no constructor that selects a production execution
/// mode. The type is available only through the compile-time fixture feature.
pub struct B1ConformanceFixture {
    state: Mutex<FixtureState>,
}

impl Default for B1ConformanceFixture {
    fn default() -> Self {
        Self::new()
    }
}

impl B1ConformanceFixture {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(FixtureState::Empty),
        }
    }

    /// Initialize a fresh fixture node. The state transition and bootstrap
    /// authorization happen under one mutex, so a race has exactly one winner.
    pub fn initialize_node(
        &self,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<FixtureOnly<FixtureInitialization>> {
        let mut state = self.lock_state();
        let snapshot = match &*state {
            FixtureState::Empty => AccessSnapshot::bootstrap_open(),
            FixtureState::Initialized(_) | FixtureState::Enrolled(_) => {
                AccessSnapshot::bootstrap_closed()
            }
        };
        authorize_capability(
            CapabilityKey::InitializeNode,
            None,
            &snapshot,
            correlation_id,
        )?;

        let workspace_id = WorkspaceId::new_v7();
        let profile_id = ProfileId::new_v7();
        let client_id = ClientId::new_v7();
        let credential_id = CredentialId::new_v7();
        let grant_id = ProfileGrantId::new_v7();
        let access = RequestAccessContext::new(
            workspace_id,
            profile_id,
            client_id,
            credential_id,
            grant_id,
            1,
        );
        let enrollment_snapshot = AccessSnapshot::established(
            workspace_id,
            profile_id,
            client_id,
            credential_id,
            grant_id,
            CredentialStatus::Active,
            GrantStatus::Active,
            1,
            [ScopeKey::ClientEnroll],
        );

        *state = FixtureState::Initialized(InitializedState {
            enrollment_access: access,
            enrollment_snapshot,
        });
        Ok(FixtureOnly::new(FixtureInitialization { access }))
    }

    /// Enroll the first fixture client and issue fresh process-local credential
    /// material. A concurrent second enrollment is denied without mutation.
    pub fn enroll_first_client(
        &self,
        correlation_id: RequestCorrelationId,
        initialization: &FixtureInitialization,
    ) -> ApplicationResult<FixtureOnly<FixtureEnrollment>> {
        let mut state = self.lock_state();
        let initialized = match &*state {
            FixtureState::Initialized(initialized) => initialized,
            FixtureState::Empty | FixtureState::Enrolled(_) => {
                return Err(Box::new(FastiProblem::forbidden(
                    CapabilityKey::EnrollFirstClient,
                    correlation_id,
                )));
            }
        };

        authorize_capability(
            CapabilityKey::EnrollFirstClient,
            Some(initialization.access()),
            &initialized.enrollment_snapshot,
            correlation_id,
        )?;
        if initialization.access != initialized.enrollment_access {
            return Err(Box::new(FastiProblem::forbidden(
                CapabilityKey::EnrollFirstClient,
                correlation_id,
            )));
        }

        let credential_secret = FixtureCredentialSecret::fresh();
        let access = RequestAccessContext::new(
            initialized.enrollment_access.workspace_id(),
            initialized.enrollment_access.profile_id(),
            initialized.enrollment_access.client_id(),
            CredentialId::new_v7(),
            ProfileGrantId::new_v7(),
            1,
        );
        let access_snapshot = AccessSnapshot::established(
            access.workspace_id(),
            access.profile_id(),
            access.client_id(),
            access.credential_id(),
            access.grant_id(),
            CredentialStatus::Active,
            GrantStatus::Active,
            access.presented_credential_epoch(),
            [ScopeKey::ObservationAccept, ScopeKey::ReceiptRead],
        );

        *state = FixtureState::Enrolled(EnrolledState {
            access_snapshot,
            credential_secret: credential_secret.bytes,
            operations: HashMap::new(),
            receipts: HashMap::new(),
        });
        Ok(FixtureOnly::new(FixtureEnrollment {
            access,
            credential_secret,
        }))
    }

    /// Exercise acceptance while preserving an explicit no-durability marker.
    pub fn accept_fixture(
        &self,
        credential_secret: &FixtureCredentialSecret,
        command: AcceptObservationCommand,
    ) -> ApplicationResult<FixtureOnly<AcceptObservationOutcome>> {
        self.accept(command, Some(credential_secret))
            .map(FixtureOnly::new)
    }

    /// Exercise receipt replay while preserving an explicit no-durability marker.
    pub fn replay_fixture(
        &self,
        credential_secret: &FixtureCredentialSecret,
        query: ReplayReceiptQuery,
    ) -> ApplicationResult<FixtureOnly<AcceptObservationReceipt>> {
        self.replay(query, Some(credential_secret))
            .map(FixtureOnly::new)
    }

    pub fn inspect_fixture(&self) -> FixtureOnly<FixtureStateView> {
        let state = self.lock_state();
        let view = match &*state {
            FixtureState::Empty => FixtureStateView {
                phase: FixturePhase::Empty,
                operation_count: 0,
                receipt_count: 0,
            },
            FixtureState::Initialized(_) => FixtureStateView {
                phase: FixturePhase::Initialized,
                operation_count: 0,
                receipt_count: 0,
            },
            FixtureState::Enrolled(enrolled) => FixtureStateView {
                phase: FixturePhase::Enrolled,
                operation_count: enrolled.operations.len(),
                receipt_count: enrolled.receipts.len(),
            },
        };
        FixtureOnly::new(view)
    }

    fn accept(
        &self,
        command: AcceptObservationCommand,
        credential_secret: Option<&FixtureCredentialSecret>,
    ) -> ApplicationResult<AcceptObservationOutcome> {
        let mut state = self.lock_state();
        let enrolled = enrolled_mut(
            &mut state,
            CapabilityKey::AcceptObservation,
            command.correlation_id(),
        )?;
        authenticate_fixture_credential(
            enrolled,
            credential_secret,
            CapabilityKey::AcceptObservation,
            command.correlation_id(),
        )?;
        authorize_capability(
            CapabilityKey::AcceptObservation,
            Some(command.access()),
            &enrolled.access_snapshot,
            command.correlation_id(),
        )?;

        if let Some(stored) = enrolled.operations.get(&command.operation_id()) {
            if stored.capability == CapabilityKey::AcceptObservation
                && stored.digest == *command.prepared_evidence().digest()
            {
                return Ok(AcceptObservationOutcome::Replayed(stored.receipt.clone()));
            }
            return Err(Box::new(FastiProblem::idempotency_conflict(
                CapabilityKey::AcceptObservation,
                command.correlation_id(),
            )));
        }

        let received_at = fasti_domain::ReceivedAt::from_application_clock(Utc::now());
        let (observation, _) = Observation::new_unresolved(
            ObservationId::new_v7(),
            command.access().workspace_id(),
            command.access().profile_id(),
            command.access().client_id(),
            command.prepared_evidence().clone(),
            command.occurred_at().cloned(),
            command.observed_at().clone(),
            received_at,
        );
        // This timestamp exercises the canonical receipt shape only. The
        // FixtureOnly wrapper above explicitly makes no durability claim.
        let receipt = AcceptObservationReceipt::try_from_observation(
            ReceiptId::new_v7(),
            command.operation_id(),
            &observation,
            CommittedAt::from_durability_boundary(received_at.value()),
        )
        .expect("a monotonic fixture transition cannot precede its receive instant");
        enrolled.operations.insert(
            command.operation_id(),
            StoredOperation {
                capability: CapabilityKey::AcceptObservation,
                digest: command.prepared_evidence().digest().clone(),
                receipt: receipt.clone(),
            },
        );
        enrolled
            .receipts
            .insert(receipt.receipt_id(), receipt.clone());
        Ok(AcceptObservationOutcome::Committed(receipt))
    }

    fn replay(
        &self,
        query: ReplayReceiptQuery,
        credential_secret: Option<&FixtureCredentialSecret>,
    ) -> ApplicationResult<AcceptObservationReceipt> {
        let state = self.lock_state();
        let enrolled = enrolled_ref(&state, CapabilityKey::ReplayReceipt, query.correlation_id())?;
        authenticate_fixture_credential(
            enrolled,
            credential_secret,
            CapabilityKey::ReplayReceipt,
            query.correlation_id(),
        )?;
        authorize_capability(
            CapabilityKey::ReplayReceipt,
            Some(query.access()),
            &enrolled.access_snapshot,
            query.correlation_id(),
        )?;
        enrolled
            .receipts
            .get(&query.receipt_id())
            .cloned()
            .ok_or_else(|| Box::new(FastiProblem::receipt_not_found(query.correlation_id())))
    }

    fn lock_state(&self) -> MutexGuard<'_, FixtureState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl ObservationAcceptancePort for B1ConformanceFixture {
    fn authorize_and_accept(
        &self,
        command: AcceptObservationCommand,
    ) -> ApplicationResult<AcceptObservationOutcome> {
        // Delivery adapters authenticate credential material before mapping a
        // transport request into this already-authenticated application port.
        self.accept(command, None)
    }

    fn authorize_and_replay(
        &self,
        query: ReplayReceiptQuery,
    ) -> ApplicationResult<AcceptObservationReceipt> {
        self.replay(query, None)
    }
}

fn authenticate_fixture_credential(
    enrolled: &EnrolledState,
    presented: Option<&FixtureCredentialSecret>,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    let Some(presented) = presented else {
        return Ok(());
    };
    let difference = enrolled
        .credential_secret
        .iter()
        .zip(presented.bytes.iter())
        .fold(0_u8, |difference, (expected, actual)| {
            difference | (expected ^ actual)
        });
    if difference == 0 {
        Ok(())
    } else {
        Err(Box::new(FastiProblem::forbidden(
            capability,
            correlation_id,
        )))
    }
}

fn authorize_capability(
    capability: CapabilityKey,
    request: Option<&RequestAccessContext>,
    snapshot: &AccessSnapshot,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    authorize(
        &AuthorizationRequirement::for_capability(capability),
        request,
        Some(snapshot),
    )
    .map(|_| ())
    .map_err(|_| Box::new(FastiProblem::forbidden(capability, correlation_id)))
}

fn enrolled_mut(
    state: &mut FixtureState,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<&mut EnrolledState> {
    match state {
        FixtureState::Enrolled(enrolled) => Ok(enrolled),
        FixtureState::Empty | FixtureState::Initialized(_) => Err(Box::new(
            FastiProblem::forbidden(capability, correlation_id),
        )),
    }
}

fn enrolled_ref(
    state: &FixtureState,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<&EnrolledState> {
    match state {
        FixtureState::Enrolled(enrolled) => Ok(enrolled),
        FixtureState::Empty | FixtureState::Initialized(_) => Err(Box::new(
            FastiProblem::forbidden(capability, correlation_id),
        )),
    }
}
