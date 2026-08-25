//! B7a Nuvio client observation and pairing integration.
//!
//! # Product Boundary
//!
//! **Fasti records. Players play.**
//!
//! Fasti does not decode, stream, transcode, or act as a player. Nuvio is an
//! external player client that pairs with Fasti and pushes playback observations
//! through canonical application capabilities.
//!
//! # Invariants
//!
//! 1. **Playback Independence:** Playback in Nuvio must NEVER depend on Fasti's
//!    availability. If Fasti is offline or unreachable, playback proceeds
//!    normally and observations are buffered in Nuvio's durable outbox.
//! 2. **Idempotency & Replay:** Every observation uses a deterministically
//!    derived operation ID (`nuvio:session:<session_id>:beat:<seq>` or
//!    `nuvio:session:<session_id>:complete`). Network retries replay the
//!    existing receipt without creating false rewatches.
//! 3. **Canonical Contract:** Nuvio uses the standard
//!    [`AcceptObservationCommand`], [`CapabilityKey::AcceptObservation`], and
//!    [`ScopeKey::ObservationAccept`]. No privileged bypass exists.

use crate::{
    derive_deterministic_evidence_digest, derive_deterministic_operation_id,
    AcceptObservationCommand, AcceptObservationOutcome, AcceptObservationReceipt, FastiProblem,
    ObservationAcceptancePort, RequestAccessContext, Retryability,
};
use fasti_domain::{
    EvidenceId, EvidenceReference, ExternalIdentifierClaim, Grain, ObservedAt, OccurredAt,
    RequestCorrelationId,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Default maximum number of queued observations in a Nuvio client outbox.
pub const DEFAULT_MAX_OUTBOX_ENTRIES: usize = 1000;

/// Helper to format the lexeme for a periodic progress heartbeat.
pub fn nuvio_heartbeat_lexeme(session_id: &str, sequence: u64) -> String {
    format!("nuvio:session:{session_id}:beat:{sequence}")
}

/// Helper to format the lexeme for a session completion event.
pub fn nuvio_completion_lexeme(session_id: &str) -> String {
    format!("nuvio:session:{session_id}:complete")
}

/// The state of a playback session in the Nuvio player.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NuvioPlaybackSession {
    session_id: String,
    target_grain: Grain,
    raw_title: String,
    identifiers: Vec<ExternalIdentifierClaim>,
    total_duration_seconds: u64,
    current_position_seconds: u64,
    heartbeat_sequence: u64,
    is_completed: bool,
}

impl NuvioPlaybackSession {
    pub fn new(
        session_id: impl Into<String>,
        target_grain: Grain,
        raw_title: impl Into<String>,
        identifiers: Vec<ExternalIdentifierClaim>,
        total_duration_seconds: u64,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            target_grain,
            raw_title: raw_title.into(),
            identifiers,
            total_duration_seconds,
            current_position_seconds: 0,
            heartbeat_sequence: 0,
            is_completed: false,
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub const fn target_grain(&self) -> Grain {
        self.target_grain
    }

    pub fn raw_title(&self) -> &str {
        &self.raw_title
    }

    pub fn identifiers(&self) -> &[ExternalIdentifierClaim] {
        &self.identifiers
    }

    pub const fn total_duration_seconds(&self) -> u64 {
        self.total_duration_seconds
    }

    pub const fn current_position_seconds(&self) -> u64 {
        self.current_position_seconds
    }

    pub const fn heartbeat_sequence(&self) -> u64 {
        self.heartbeat_sequence
    }

    pub const fn is_completed(&self) -> bool {
        self.is_completed
    }

    /// Advance playback position and generate the next heartbeat command.
    pub fn tick_heartbeat(
        &mut self,
        access: RequestAccessContext,
        position_seconds: u64,
        observed_at: ObservedAt,
    ) -> AcceptObservationCommand {
        self.current_position_seconds = position_seconds.min(self.total_duration_seconds);
        let seq = self.heartbeat_sequence;
        self.heartbeat_sequence += 1;

        let lexeme = nuvio_heartbeat_lexeme(&self.session_id, seq);
        let operation_id = derive_deterministic_operation_id(&lexeme);

        let progress_str = format!("{}s", self.current_position_seconds);
        let evidence_bytes = format!(
            "nuvio:progress:{}:{}:{}",
            self.session_id, self.current_position_seconds, progress_str
        );
        let evidence_digest = derive_deterministic_evidence_digest(&evidence_bytes);
        let evidence = EvidenceReference::new(
            EvidenceId::new_v7(),
            evidence_digest,
            evidence_bytes.len() as u64,
        );

        AcceptObservationCommand::new(
            RequestCorrelationId::new_v7(),
            access,
            operation_id,
            None,
            observed_at,
            evidence,
        )
        .with_identity_clues(self.identifiers.clone(), Some(self.target_grain))
    }

    /// Mark playback completed and generate the completion command.
    pub fn complete_session(
        &mut self,
        access: RequestAccessContext,
        observed_at: ObservedAt,
        occurred_at: Option<OccurredAt>,
    ) -> AcceptObservationCommand {
        self.is_completed = true;
        self.current_position_seconds = self.total_duration_seconds;

        let lexeme = nuvio_completion_lexeme(&self.session_id);
        let operation_id = derive_deterministic_operation_id(&lexeme);

        let evidence_bytes = format!("nuvio:completion:{}", self.session_id);
        let evidence_digest = derive_deterministic_evidence_digest(&evidence_bytes);
        let evidence = EvidenceReference::new(
            EvidenceId::new_v7(),
            evidence_digest,
            evidence_bytes.len() as u64,
        );

        AcceptObservationCommand::new(
            RequestCorrelationId::new_v7(),
            access,
            operation_id,
            occurred_at,
            observed_at,
            evidence,
        )
        .with_identity_clues(self.identifiers.clone(), Some(self.target_grain))
    }
}

/// An entry buffered in Nuvio's client-side outbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NuvioOutboxEntry {
    command: AcceptObservationCommand,
    attempts: u32,
}

impl NuvioOutboxEntry {
    pub const fn new(command: AcceptObservationCommand) -> Self {
        Self {
            command,
            attempts: 0,
        }
    }

    pub const fn command(&self) -> &AcceptObservationCommand {
        &self.command
    }

    pub const fn attempts(&self) -> u32 {
        self.attempts
    }

    pub fn record_attempt(&mut self) {
        self.attempts = self.attempts.saturating_add(1);
    }
}

/// The result of attempting to drain one outbox entry.
#[derive(Debug)]
pub enum NuvioDrainOutcome {
    Committed(AcceptObservationReceipt),
    Replayed(AcceptObservationReceipt),
    Rejected(Box<FastiProblem>),
}

/// Client-side durable outbox for Nuvio.
///
/// Buffers observations when offline and drains them sequentially upon reconnect.
#[derive(Debug)]
pub struct NuvioOutbox {
    queue: VecDeque<NuvioOutboxEntry>,
    max_entries: usize,
}

impl Default for NuvioOutbox {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_OUTBOX_ENTRIES)
    }
}

/// The outcome of dispatching an observation through Nuvio's driver.
#[derive(Debug)]
pub enum NuvioDispatchResult {
    /// The command was sent directly to Fasti and accepted.
    Dispatched(Box<AcceptObservationOutcome>),
    /// Fasti was unreachable or rejected the request; the command was buffered in the outbox.
    Buffered,
}

impl NuvioOutbox {
    pub fn new(max_entries: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            max_entries,
        }
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Enqueue a command. If the outbox is full, discards the oldest queued
    /// entry (FIFO eviction) regardless of whether it is a heartbeat or a
    /// completion -- an outage long enough to fill `max_entries` has already
    /// lost history, and the alternative (unbounded growth) is worse.
    pub fn enqueue(&mut self, command: AcceptObservationCommand) {
        if self.queue.len() >= self.max_entries {
            self.queue.pop_front();
        }
        self.queue.push_back(NuvioOutboxEntry::new(command));
    }

    /// Attempt to send an observation directly. If acceptance fails, buffers it in the outbox.
    ///
    /// Never returns an error to the player so playback is never interrupted.
    pub fn dispatch_or_buffer(
        &mut self,
        port: &dyn ObservationAcceptancePort,
        command: AcceptObservationCommand,
    ) -> NuvioDispatchResult {
        match port.authorize_and_accept(command.clone()) {
            Ok(outcome) => NuvioDispatchResult::Dispatched(Box::new(outcome)),
            Err(_) => {
                self.enqueue(command);
                NuvioDispatchResult::Buffered
            }
        }
    }

    /// Drain buffered entries against the provided acceptance port, in order.
    ///
    /// A transient (`RetrySafe`) failure requeues the entry and stops the
    /// drain rather than discarding the observation: if the daemon is
    /// unreachable, every later entry would fail the same way, and popping
    /// them anyway would permanently lose observations that were never the
    /// player's fault. Only terminal rejections are removed from the queue.
    pub fn drain(&mut self, port: &dyn ObservationAcceptancePort) -> Vec<NuvioDrainOutcome> {
        let mut results = Vec::new();
        while let Some(mut entry) = self.queue.pop_front() {
            entry.record_attempt();
            match port.authorize_and_accept(entry.command().clone()) {
                Ok(AcceptObservationOutcome::Committed(receipt)) => {
                    results.push(NuvioDrainOutcome::Committed(receipt));
                }
                Ok(AcceptObservationOutcome::Replayed(receipt)) => {
                    results.push(NuvioDrainOutcome::Replayed(receipt));
                }
                Err(problem) => {
                    if problem.code().contract().retryability() == Retryability::RetrySafe {
                        self.queue.push_front(entry);
                        break;
                    }
                    results.push(NuvioDrainOutcome::Rejected(problem));
                }
            }
        }
        results
    }
}

// ---------------------------------------------------------------------------
// B7b — Nuvio State Synchronization & Loop Prevention
// ---------------------------------------------------------------------------

/// Exact current watched state for a media item tracked by Nuvio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NuvioWatchedState {
    pub grain: Grain,
    pub title: String,
    pub identifiers: Vec<ExternalIdentifierClaim>,
    pub is_watched: bool,
    pub progress_percent: u8,
    pub last_watched_at: Option<ObservedAt>,
    pub version: u64,
}

impl NuvioWatchedState {
    pub fn new(
        grain: Grain,
        title: impl Into<String>,
        identifiers: Vec<ExternalIdentifierClaim>,
        is_watched: bool,
        progress_percent: u8,
        last_watched_at: Option<ObservedAt>,
        version: u64,
    ) -> Self {
        Self {
            grain,
            title: title.into(),
            identifiers,
            is_watched,
            progress_percent: progress_percent.min(100),
            last_watched_at,
            version,
        }
    }
}

/// An atomic change item in the ordered change feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NuvioChangeDelta {
    pub cursor: u64,
    pub origin_client_id: fasti_domain::ClientId,
    pub item_key: String,
    pub state: NuvioWatchedState,
}

impl NuvioChangeDelta {
    pub fn new(
        cursor: u64,
        origin_client_id: fasti_domain::ClientId,
        item_key: impl Into<String>,
        state: NuvioWatchedState,
    ) -> Self {
        Self {
            cursor,
            origin_client_id,
            item_key: item_key.into(),
            state,
        }
    }
}

/// Client-side state synchronization engine for Nuvio.
///
/// Implements two-way watched state reconciliation, cursor tracking, and
/// loop prevention (suppressing echo changes from the same client).
#[derive(Debug)]
pub struct NuvioStateSyncEngine {
    local_client_id: fasti_domain::ClientId,
    last_synced_cursor: u64,
    tracked_items: std::collections::HashMap<String, NuvioWatchedState>,
}

impl NuvioStateSyncEngine {
    pub fn new(local_client_id: fasti_domain::ClientId) -> Self {
        Self {
            local_client_id,
            last_synced_cursor: 0,
            tracked_items: std::collections::HashMap::new(),
        }
    }

    pub const fn local_client_id(&self) -> fasti_domain::ClientId {
        self.local_client_id
    }

    pub const fn last_synced_cursor(&self) -> u64 {
        self.last_synced_cursor
    }

    pub fn item_count(&self) -> usize {
        self.tracked_items.len()
    }

    pub fn get_state(&self, item_key: &str) -> Option<&NuvioWatchedState> {
        self.tracked_items.get(item_key)
    }

    /// Record a local user action (e.g. playback progress or manual mark as watched).
    pub fn record_local_state(&mut self, item_key: impl Into<String>, state: NuvioWatchedState) {
        self.tracked_items.insert(item_key.into(), state);
    }

    /// Apply an incoming delta from Fasti's change feed.
    ///
    /// Returns `true` if the change was applied, or `false` if it was suppressed
    /// due to self-origin loop prevention or stale version.
    pub fn apply_remote_delta(&mut self, delta: NuvioChangeDelta) -> bool {
        // Advance the cursor watermark regardless of whether we apply or suppress.
        self.last_synced_cursor = self.last_synced_cursor.max(delta.cursor);

        // Loop prevention: if this delta originated from this exact client, ignore the echo.
        if delta.origin_client_id == self.local_client_id {
            return false;
        }

        // Stale update prevention: only apply if the incoming delta version is >= current version.
        if let Some(current) = self.tracked_items.get(&delta.item_key) {
            if delta.state.version < current.version {
                return false;
            }
        }

        self.tracked_items.insert(delta.item_key, delta.state);
        true
    }
}

// ---------------------------------------------------------------------------
// B7c — Shared Media Catalogs & Declarative Collection Projections
// ---------------------------------------------------------------------------

/// Declarative descriptor for a published Nuvio/Stremio catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NuvioCatalogDescriptor {
    pub catalog_id: String,
    pub name: String,
    pub target_grain: Grain,
    pub default_page_size: usize,
    pub is_searchable: bool,
}

impl NuvioCatalogDescriptor {
    pub fn new(
        catalog_id: impl Into<String>,
        name: impl Into<String>,
        target_grain: Grain,
        default_page_size: usize,
        is_searchable: bool,
    ) -> Self {
        Self {
            catalog_id: catalog_id.into(),
            name: name.into(),
            target_grain,
            default_page_size: default_page_size.clamp(1, 100),
            is_searchable,
        }
    }
}

/// A normalized media item projected into a published player catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NuvioCatalogItem {
    pub item_key: String,
    pub target_grain: Grain,
    pub title: String,
    pub identifiers: Vec<ExternalIdentifierClaim>,
    pub release_year: Option<u16>,
    pub is_watched: bool,
    pub progress_percent: u8,
}

impl NuvioCatalogItem {
    pub fn new(
        item_key: impl Into<String>,
        target_grain: Grain,
        title: impl Into<String>,
        identifiers: Vec<ExternalIdentifierClaim>,
        release_year: Option<u16>,
        is_watched: bool,
        progress_percent: u8,
    ) -> Self {
        Self {
            item_key: item_key.into(),
            target_grain,
            title: title.into(),
            identifiers,
            release_year,
            is_watched,
            progress_percent: progress_percent.min(100),
        }
    }
}

/// Filter criteria for querying collection sources and catalogs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NuvioCollectionFilter {
    pub grain: Option<Grain>,
    pub watched_only: Option<bool>,
    pub in_progress_only: Option<bool>,
    pub search_query: Option<String>,
}

impl NuvioCollectionFilter {
    pub fn matches(&self, item: &NuvioCatalogItem) -> bool {
        if let Some(grain) = self.grain {
            if item.target_grain != grain {
                return false;
            }
        }
        if let Some(watched_only) = self.watched_only {
            if watched_only && !item.is_watched {
                return false;
            }
            if !watched_only && item.is_watched {
                return false;
            }
        }
        if let Some(in_progress_only) = self.in_progress_only {
            if in_progress_only && (item.progress_percent == 0 || item.progress_percent >= 100) {
                return false;
            }
        }
        if let Some(query) = &self.search_query {
            if !item.title.to_lowercase().contains(&query.to_lowercase()) {
                return false;
            }
        }
        true
    }
}

/// A read-only collection projection store for player catalog consumption.
#[derive(Debug, Default)]
pub struct NuvioCatalogProjectionStore {
    items: Vec<NuvioCatalogItem>,
}

impl NuvioCatalogProjectionStore {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn insert(&mut self, item: NuvioCatalogItem) {
        if let Some(pos) = self.items.iter().position(|i| i.item_key == item.item_key) {
            self.items[pos] = item;
        } else {
            self.items.push(item);
        }
    }

    pub fn query(
        &self,
        filter: &NuvioCollectionFilter,
        skip: usize,
        take: usize,
    ) -> Vec<NuvioCatalogItem> {
        self.items
            .iter()
            .filter(|item| filter.matches(item))
            .skip(skip)
            .take(take)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "conformance-fixture")]
    use crate::conformance::B1ConformanceFixture;
    #[cfg(feature = "conformance-fixture")]
    use fasti_domain::RequestCorrelationId;
    use fasti_domain::{
        ClaimedTrust, ClientId, CredentialId, Grain, ProfileGrantId, ProfileId, WorkspaceId,
    };

    fn sample_access() -> RequestAccessContext {
        RequestAccessContext::new(
            WorkspaceId::new_v7(),
            ProfileId::new_v7(),
            ClientId::new_v7(),
            CredentialId::new_v7(),
            ProfileGrantId::new_v7(),
            1,
        )
    }

    fn sample_observed_at() -> ObservedAt {
        ObservedAt::parse("2026-08-25T08:00:00Z", ClaimedTrust::DeviceObserved)
            .expect("valid instant")
    }

    #[test]
    fn operation_id_is_deterministic_and_unique_per_sequence() {
        let op1 = derive_deterministic_operation_id(&nuvio_heartbeat_lexeme("sess-1", 0));
        let op2 = derive_deterministic_operation_id(&nuvio_heartbeat_lexeme("sess-1", 0));
        let op3 = derive_deterministic_operation_id(&nuvio_heartbeat_lexeme("sess-1", 1));
        let op_comp = derive_deterministic_operation_id(&nuvio_completion_lexeme("sess-1"));

        assert_eq!(op1, op2, "same lexeme must yield identical operation id");
        assert_ne!(
            op1, op3,
            "different sequence must yield different operation id"
        );
        assert_ne!(op1, op_comp, "completion must differ from heartbeat");
    }

    #[test]
    fn playback_session_advances_sequence_and_tracks_progress() {
        let mut session = NuvioPlaybackSession::new(
            "nuvio-session-123",
            Grain::Episode,
            "Example Episode",
            vec![],
            1800,
        );

        let access = sample_access();
        let cmd1 = session.tick_heartbeat(access, 300, sample_observed_at());
        assert_eq!(session.heartbeat_sequence(), 1);
        assert_eq!(session.current_position_seconds(), 300);
        assert!(!session.is_completed());

        let cmd2 = session.tick_heartbeat(access, 600, sample_observed_at());
        assert_eq!(session.heartbeat_sequence(), 2);
        assert_eq!(session.current_position_seconds(), 600);
        assert_ne!(cmd1.operation_id(), cmd2.operation_id());

        let cmd_complete = session.complete_session(access, sample_observed_at(), None);
        assert!(session.is_completed());
        assert_eq!(session.current_position_seconds(), 1800);
        assert_ne!(cmd2.operation_id(), cmd_complete.operation_id());
    }

    #[cfg(feature = "conformance-fixture")]
    #[test]
    fn outbox_buffers_when_offline_and_drains_cleanly() {
        let fixture = B1ConformanceFixture::new();
        let init = fixture
            .initialize_node(RequestCorrelationId::new_v7())
            .expect("init")
            .into_inner();
        let enrollment = fixture
            .enroll_first_client(RequestCorrelationId::new_v7(), &init)
            .expect("enroll")
            .into_inner();
        let access = *enrollment.access();

        let mut outbox = NuvioOutbox::default();
        let mut session = NuvioPlaybackSession::new(
            "sess-offline-test",
            Grain::Film,
            "Offline Movie",
            vec![],
            7200,
        );

        // Buffer 3 heartbeats and 1 completion
        outbox.enqueue(session.tick_heartbeat(access, 1800, sample_observed_at()));
        outbox.enqueue(session.tick_heartbeat(access, 3600, sample_observed_at()));
        outbox.enqueue(session.tick_heartbeat(access, 5400, sample_observed_at()));
        outbox.enqueue(session.complete_session(access, sample_observed_at(), None));

        assert_eq!(outbox.len(), 4);

        // Drain against fixture
        let outcomes = outbox.drain(&fixture);
        assert_eq!(outcomes.len(), 4);
        assert!(outbox.is_empty());

        for outcome in outcomes {
            assert!(
                matches!(outcome, NuvioDrainOutcome::Committed(_)),
                "first drain should commit all items"
            );
        }
    }
}
