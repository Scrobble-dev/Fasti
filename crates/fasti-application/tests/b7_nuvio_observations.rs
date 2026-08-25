#![cfg(feature = "conformance-fixture")]

//! B7a Nuvio client observation, pairing, and durable outbox integration tests.
//!
//! # Product Boundary & Invariants
//!
//! 1. **Fasti records. Players play.** Fasti does not decode, transcode, or act as a player.
//! 2. **Playback Independence:** Playback in Nuvio never halts or errors on Fasti failure.
//! 3. **Idempotency & Zero False Rewatches:** Redelivery of buffered heartbeats and completion
//!    replays existing receipts without creating duplicate occurrences.
//! 4. **Deterministic Operation IDs:** `nuvio:session:<session_id>:beat:<seq>` and
//!    `nuvio:session:<session_id>:complete`.

use fasti_application::{
    conformance::{B1ConformanceFixture, FixtureEnrollment},
    derive_deterministic_evidence_digest, derive_deterministic_operation_id,
    nuvio_heartbeat_lexeme, AcceptObservationCommand, AcceptObservationOutcome,
    AcceptObservationReceipt, ApplicationResult, CapabilityKey, FastiProblem, NuvioDrainOutcome,
    NuvioOutbox, NuvioPlaybackSession, ObservationAcceptancePort, ProblemCode, ReplayReceiptQuery,
    RequestAccessContext,
};
use fasti_domain::{
    ClaimedTrust, EvidenceId, EvidenceReference, ExternalIdentifierClaim, Grain, ObservedAt,
    OccurredAt, RequestCorrelationId,
};

fn enroll(fixture: &B1ConformanceFixture) -> FixtureEnrollment {
    let init = fixture
        .initialize_node(RequestCorrelationId::new_v7())
        .expect("node initializes")
        .into_inner();
    fixture
        .enroll_first_client(RequestCorrelationId::new_v7(), &init)
        .expect("first client enrolls")
        .into_inner()
}

fn sample_observed_at(time_str: &str) -> ObservedAt {
    ObservedAt::parse(time_str, ClaimedTrust::DeviceObserved).expect("valid iso8601 timestamp")
}

fn sample_occurred_at(time_str: &str) -> OccurredAt {
    OccurredAt::parse(time_str, ClaimedTrust::DeviceObserved).expect("valid iso8601 timestamp")
}

fn sample_tmdb_claim(id: &str) -> ExternalIdentifierClaim {
    ExternalIdentifierClaim::try_new("tmdb.movie", Grain::Film, id)
        .expect("valid external identifier")
}

#[test]
fn nuvio_pairing_and_client_enrollment() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    assert_ne!(access.workspace_id().to_string(), "");
    assert_ne!(access.profile_id().to_string(), "");
    assert_ne!(access.client_id().to_string(), "");
    assert_ne!(access.credential_id().to_string(), "");
    assert_eq!(
        enrollment.credential_secret().expose_for_fixture().len(),
        32
    );
}

#[test]
fn nuvio_heartbeat_progression_over_playback_session() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    let mut session = NuvioPlaybackSession::new(
        "nuvio-session-film-42",
        Grain::Film,
        "Inception",
        vec![sample_tmdb_claim("27205")],
        8880, // 148 minutes
    );

    // Playback ticks every 10 minutes (600s)
    let timestamps = [
        ("2026-08-25T20:00:00Z", 0),
        ("2026-08-25T20:10:00Z", 600),
        ("2026-08-25T20:20:00Z", 1200),
        ("2026-08-25T20:30:00Z", 1800),
    ];

    for (ts, pos) in timestamps {
        let cmd = session.tick_heartbeat(access, pos, sample_observed_at(ts));
        let outcome = fixture
            .accept_fixture(enrollment.credential_secret(), cmd)
            .expect("heartbeat accepted")
            .into_inner();
        assert!(
            matches!(outcome, AcceptObservationOutcome::Committed(_)),
            "each heartbeat in a session commits as a distinct operation"
        );
    }

    assert_eq!(session.heartbeat_sequence(), 4);
    assert_eq!(session.current_position_seconds(), 1800);
    assert_eq!(fixture.inspect_fixture().as_ref().operation_count, 4);
}

#[test]
fn nuvio_completion_observation_emits_watched_event() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    let mut session = NuvioPlaybackSession::new(
        "nuvio-session-film-99",
        Grain::Film,
        "Blade Runner 2049",
        vec![sample_tmdb_claim("335984")],
        9840,
    );

    let cmd_complete = session.complete_session(
        access,
        sample_observed_at("2026-08-25T22:45:00Z"),
        Some(sample_occurred_at("2026-08-25T20:00:00Z")),
    );

    let outcome = fixture
        .accept_fixture(enrollment.credential_secret(), cmd_complete)
        .expect("completion accepted")
        .into_inner();

    assert!(
        matches!(outcome, AcceptObservationOutcome::Committed(_)),
        "completion event commits successfully"
    );
    assert!(session.is_completed());
    assert_eq!(session.current_position_seconds(), 9840);
}

#[test]
fn nuvio_offline_outbox_buffers_during_total_fasti_outage_and_drains() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    let mut outbox = NuvioOutbox::default();
    let mut session = NuvioPlaybackSession::new(
        "nuvio-session-offline-101",
        Grain::Film,
        "Interstellar",
        vec![sample_tmdb_claim("157336")],
        10140,
    );

    // Fasti is offline / unreachable: Nuvio continues playback and buffers all heartbeats + completion
    for i in 0..10 {
        let pos = (i + 1) * 900; // every 15 min
        let ts = format!("2026-08-25T20:{:02}:00Z", (i * 15) % 60);
        let cmd = session.tick_heartbeat(access, pos, sample_observed_at(&ts));
        outbox.enqueue(cmd);
    }
    let cmd_complete = session.complete_session(
        access,
        sample_observed_at("2026-08-25T22:49:00Z"),
        Some(sample_occurred_at("2026-08-25T20:00:00Z")),
    );
    outbox.enqueue(cmd_complete);

    assert_eq!(outbox.len(), 11);
    assert_eq!(fixture.inspect_fixture().as_ref().operation_count, 0);

    // Fasti daemon comes back online: drain outbox
    let results = outbox.drain(&fixture);
    assert_eq!(results.len(), 11);
    assert!(outbox.is_empty());

    for res in results {
        assert!(
            matches!(res, NuvioDrainOutcome::Committed(_)),
            "all buffered items commit upon reconnect"
        );
    }
    assert_eq!(fixture.inspect_fixture().as_ref().operation_count, 11);
}

#[test]
fn nuvio_outbox_reconnect_replays_without_duplicate_rewatches() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    let mut session = NuvioPlaybackSession::new(
        "nuvio-session-reconnect-202",
        Grain::Film,
        "Dune: Part Two",
        vec![sample_tmdb_claim("693134")],
        9960,
    );

    let cmd1 = session.tick_heartbeat(access, 3000, sample_observed_at("2026-08-25T21:00:00Z"));
    let cmd_comp = session.complete_session(
        access,
        sample_observed_at("2026-08-25T22:46:00Z"),
        Some(sample_occurred_at("2026-08-25T20:00:00Z")),
    );

    let mut outbox = NuvioOutbox::default();
    outbox.enqueue(cmd1.clone());
    outbox.enqueue(cmd_comp.clone());

    // First drain commits both
    let drain1 = outbox.drain(&fixture);
    assert_eq!(drain1.len(), 2);
    assert!(matches!(drain1[0], NuvioDrainOutcome::Committed(_)));
    assert!(matches!(drain1[1], NuvioDrainOutcome::Committed(_)));
    assert_eq!(fixture.inspect_fixture().as_ref().operation_count, 2);

    // Nuvio didn't receive receipts due to transient connection drop right after drain,
    // so it replays the exact same commands from a local copy.
    outbox.enqueue(cmd1);
    outbox.enqueue(cmd_comp);

    let drain2 = outbox.drain(&fixture);
    assert_eq!(drain2.len(), 2);
    assert!(
        matches!(drain2[0], NuvioDrainOutcome::Replayed(_)),
        "redelivery of heartbeat must replay"
    );
    assert!(
        matches!(drain2[1], NuvioDrainOutcome::Replayed(_)),
        "redelivery of completion must replay"
    );

    // Zero new operations or duplicate occurrences were created
    assert_eq!(
        fixture.inspect_fixture().as_ref().operation_count,
        2,
        "operation set must not grow on redelivery"
    );
}

#[test]
fn nuvio_distinct_rewatch_records_second_occurrence() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    // Day 1 watch session
    let mut day1_session = NuvioPlaybackSession::new(
        "nuvio-session-day1-555",
        Grain::Film,
        "Spirited Away",
        vec![sample_tmdb_claim("129")],
        7500,
    );
    let day1_cmd = day1_session.complete_session(
        access,
        sample_observed_at("2026-08-25T21:00:00Z"),
        Some(sample_occurred_at("2026-08-25T19:00:00Z")),
    );

    // Day 2 watch session (different session ID, different day)
    let mut day2_session = NuvioPlaybackSession::new(
        "nuvio-session-day2-777",
        Grain::Film,
        "Spirited Away",
        vec![sample_tmdb_claim("129")],
        7500,
    );
    let day2_cmd = day2_session.complete_session(
        access,
        sample_observed_at("2026-08-26T21:00:00Z"),
        Some(sample_occurred_at("2026-08-26T19:00:00Z")),
    );

    let mut outbox = NuvioOutbox::default();
    outbox.enqueue(day1_cmd);
    outbox.enqueue(day2_cmd);

    let drain = outbox.drain(&fixture);
    assert_eq!(drain.len(), 2);
    assert!(
        matches!(drain[0], NuvioDrainOutcome::Committed(_)),
        "Day 1 watch commits"
    );
    assert!(
        matches!(drain[1], NuvioDrainOutcome::Committed(_)),
        "Day 2 rewatch commits as a distinct valid occurrence"
    );
    assert_eq!(fixture.inspect_fixture().as_ref().operation_count, 2);
}

#[test]
fn nuvio_tampered_payload_fails_with_idempotency_conflict() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    let op_id = derive_deterministic_operation_id(&nuvio_heartbeat_lexeme("session-tamper-9", 0));
    let evidence1 = EvidenceReference::new(
        EvidenceId::new_v7(),
        derive_deterministic_evidence_digest("nuvio:progress:session-tamper-9:300:300s"),
        37,
    );
    let evidence2 = EvidenceReference::new(
        EvidenceId::new_v7(),
        derive_deterministic_evidence_digest("nuvio:progress:session-tamper-9:300:TAMPERED"),
        40,
    );

    let cmd1 = AcceptObservationCommand::new(
        RequestCorrelationId::new_v7(),
        access,
        op_id,
        None,
        sample_observed_at("2026-08-25T20:00:00Z"),
        evidence1,
    )
    .with_identity_clues(vec![sample_tmdb_claim("100")], Some(Grain::Film));

    let cmd2 = AcceptObservationCommand::new(
        RequestCorrelationId::new_v7(),
        access,
        op_id, // Same operation ID
        None,
        sample_observed_at("2026-08-25T20:00:00Z"),
        evidence2, // Mutated payload digest
    )
    .with_identity_clues(vec![sample_tmdb_claim("100")], Some(Grain::Film));

    let outcome1 = fixture
        .accept_fixture(enrollment.credential_secret(), cmd1)
        .expect("first submit succeeds")
        .into_inner();
    assert!(matches!(outcome1, AcceptObservationOutcome::Committed(_)));

    let problem = fixture
        .accept_fixture(enrollment.credential_secret(), cmd2)
        .expect_err("tampered resubmit must fail with conflict");
    assert_eq!(
        problem.code(),
        ProblemCode::IdempotencyConflict,
        "same operation id with altered payload must yield IdempotencyConflict"
    );
}

#[test]
fn nuvio_unauthorized_foreign_client_fails_closed() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access1 = *enrollment.access();

    // Create a second un-enrolled foreign access context
    let foreign_access = RequestAccessContext::new(
        access1.workspace_id(),
        access1.profile_id(),
        fasti_domain::ClientId::new_v7(),
        fasti_domain::CredentialId::new_v7(),
        fasti_domain::ProfileGrantId::new_v7(),
        1,
    );

    let mut session = NuvioPlaybackSession::new(
        "nuvio-foreign-sess",
        Grain::Film,
        "Unknown Film",
        vec![],
        3600,
    );
    let cmd = session.tick_heartbeat(
        foreign_access,
        300,
        sample_observed_at("2026-08-25T20:00:00Z"),
    );

    let mut outbox = NuvioOutbox::default();
    outbox.enqueue(cmd);

    let drain = outbox.drain(&fixture);
    assert_eq!(drain.len(), 1);
    match &drain[0] {
        NuvioDrainOutcome::Rejected(problem) => {
            assert_eq!(problem.code(), ProblemCode::Forbidden);
        }
        other => panic!("expected forbidden rejection, got {other:?}"),
    }
}

#[test]
fn nuvio_state_sync_engine_applies_remote_deltas_and_tracks_cursor() {
    let local_client_id = fasti_domain::ClientId::new_v7();
    let remote_client_id = fasti_domain::ClientId::new_v7();
    let mut sync_engine = fasti_application::NuvioStateSyncEngine::new(local_client_id);

    let watched_state = fasti_application::NuvioWatchedState::new(
        Grain::Film,
        "Princess Mononoke",
        vec![sample_tmdb_claim("128")],
        true,
        100,
        Some(sample_observed_at("2026-08-25T21:30:00Z")),
        1,
    );

    let delta = fasti_application::NuvioChangeDelta::new(
        42,
        remote_client_id,
        "tmdb.movie:128",
        watched_state.clone(),
    );

    let applied = sync_engine.apply_remote_delta(delta);
    assert!(applied, "valid remote delta must be applied");
    assert_eq!(sync_engine.last_synced_cursor(), 42);
    assert_eq!(sync_engine.item_count(), 1);

    let item = sync_engine
        .get_state("tmdb.movie:128")
        .expect("item present");
    assert!(item.is_watched);
    assert_eq!(item.progress_percent, 100);
}

#[test]
fn nuvio_state_sync_engine_suppresses_self_originated_echo_changes() {
    let local_client_id = fasti_domain::ClientId::new_v7();
    let mut sync_engine = fasti_application::NuvioStateSyncEngine::new(local_client_id);

    let local_state = fasti_application::NuvioWatchedState::new(
        Grain::Episode,
        "Cowboy Bebop - Asteroid Blues",
        vec![],
        true,
        100,
        Some(sample_observed_at("2026-08-25T22:00:00Z")),
        1,
    );
    sync_engine.record_local_state("kitsu.anime:1:ep:1", local_state);

    // Delta comes back from server echoing our own client ID
    let echo_delta = fasti_application::NuvioChangeDelta::new(
        100,
        local_client_id, // Same as local!
        "kitsu.anime:1:ep:1",
        fasti_application::NuvioWatchedState::new(
            Grain::Episode,
            "Cowboy Bebop - Asteroid Blues",
            vec![],
            true,
            100,
            Some(sample_observed_at("2026-08-25T22:00:00Z")),
            1,
        ),
    );

    let applied = sync_engine.apply_remote_delta(echo_delta);
    assert!(
        !applied,
        "echo delta from same client must be suppressed (loop prevention)"
    );
    assert_eq!(
        sync_engine.last_synced_cursor(),
        100,
        "cursor still advances"
    );
}

#[test]
fn nuvio_state_sync_engine_rejects_stale_versions() {
    let local_client_id = fasti_domain::ClientId::new_v7();
    let remote_client_id = fasti_domain::ClientId::new_v7();
    let mut sync_engine = fasti_application::NuvioStateSyncEngine::new(local_client_id);

    let current_state = fasti_application::NuvioWatchedState::new(
        Grain::Film,
        "Akira",
        vec![],
        true,
        100,
        Some(sample_observed_at("2026-08-25T23:00:00Z")),
        5, // Version 5
    );
    sync_engine.record_local_state("tmdb.movie:149", current_state);

    // Incoming delta with older version 4
    let stale_delta = fasti_application::NuvioChangeDelta::new(
        150,
        remote_client_id,
        "tmdb.movie:149",
        fasti_application::NuvioWatchedState::new(
            Grain::Film,
            "Akira",
            vec![],
            false,
            50,
            None,
            4, // Version 4 (stale)
        ),
    );

    let applied = sync_engine.apply_remote_delta(stale_delta);
    assert!(!applied, "stale version delta must be rejected");

    let current = sync_engine.get_state("tmdb.movie:149").unwrap();
    assert_eq!(current.version, 5, "current newer version preserved");
    assert!(current.is_watched);
}

#[test]
fn nuvio_catalog_descriptor_bounds_and_searchability() {
    let desc = fasti_application::NuvioCatalogDescriptor::new(
        "trending-anime",
        "Trending Anime",
        Grain::Episode,
        150, // Exceeds 100 max bound
        true,
    );

    assert_eq!(desc.catalog_id, "trending-anime");
    assert_eq!(desc.name, "Trending Anime");
    assert_eq!(desc.target_grain, Grain::Episode);
    assert_eq!(desc.default_page_size, 100, "page size clamped to 100");
    assert!(desc.is_searchable);
}

#[test]
fn nuvio_catalog_projection_store_filtering_and_pagination() {
    let mut store = fasti_application::NuvioCatalogProjectionStore::new();

    let movie1 = fasti_application::NuvioCatalogItem::new(
        "movie:1",
        Grain::Film,
        "The Matrix",
        vec![sample_tmdb_claim("603")],
        Some(1999),
        true,
        100,
    );

    let movie2 = fasti_application::NuvioCatalogItem::new(
        "movie:2",
        Grain::Film,
        "The Matrix Reloaded",
        vec![sample_tmdb_claim("604")],
        Some(2003),
        false,
        45,
    );

    let show1 = fasti_application::NuvioCatalogItem::new(
        "show:1",
        Grain::Episode,
        "Steins;Gate Episode 1",
        vec![],
        Some(2011),
        true,
        100,
    );

    store.insert(movie1);
    store.insert(movie2);
    store.insert(show1);

    // Filter by grain
    let film_filter = fasti_application::NuvioCollectionFilter {
        grain: Some(Grain::Film),
        ..Default::default()
    };
    let films = store.query(&film_filter, 0, 10);
    assert_eq!(films.len(), 2);

    // Filter in-progress only
    let in_progress_filter = fasti_application::NuvioCollectionFilter {
        in_progress_only: Some(true),
        ..Default::default()
    };
    let in_progress = store.query(&in_progress_filter, 0, 10);
    assert_eq!(in_progress.len(), 1);
    assert_eq!(in_progress[0].item_key, "movie:2");

    // Search query filter
    let search_filter = fasti_application::NuvioCollectionFilter {
        search_query: Some("Reloaded".to_owned()),
        ..Default::default()
    };
    let search_res = store.query(&search_filter, 0, 10);
    assert_eq!(search_res.len(), 1);
    assert_eq!(search_res[0].title, "The Matrix Reloaded");

    // Pagination (skip=1, take=1)
    let page = store.query(&film_filter, 1, 1);
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].item_key, "movie:2");
}

/// A port that fails its first call with a `RetrySafe` problem, proving
/// `NuvioOutbox::drain` requeues on transient failure instead of discarding
/// the observation. Any further call is a test bug, not real drain behavior.
struct FlakyStoragePort {
    remaining_failures: std::sync::atomic::AtomicU32,
}

impl ObservationAcceptancePort for FlakyStoragePort {
    fn authorize_and_accept(
        &self,
        _command: AcceptObservationCommand,
    ) -> ApplicationResult<AcceptObservationOutcome> {
        use std::sync::atomic::Ordering;
        if self.remaining_failures.load(Ordering::SeqCst) > 0 {
            self.remaining_failures.fetch_sub(1, Ordering::SeqCst);
            return Err(Box::new(FastiProblem::storage_unavailable(
                CapabilityKey::AcceptObservation,
                RequestCorrelationId::new_v7(),
            )));
        }
        panic!("drain() must stop after the first transient failure, not retry within one call");
    }

    fn authorize_and_replay(
        &self,
        _query: ReplayReceiptQuery,
    ) -> ApplicationResult<AcceptObservationReceipt> {
        unreachable!("not exercised by this test")
    }
}

#[test]
fn nuvio_outbox_requeues_a_transient_storage_failure_instead_of_discarding_it() {
    let mut session = NuvioPlaybackSession::new(
        "sess-flaky-storage",
        Grain::Film,
        "Flaky Storage Film",
        vec![],
        3600,
    );
    let access = RequestAccessContext::new(
        fasti_domain::WorkspaceId::new_v7(),
        fasti_domain::ProfileId::new_v7(),
        fasti_domain::ClientId::new_v7(),
        fasti_domain::CredentialId::new_v7(),
        fasti_domain::ProfileGrantId::new_v7(),
        1,
    );

    let mut outbox = NuvioOutbox::default();
    outbox.enqueue(session.tick_heartbeat(access, 600, sample_observed_at("2026-08-25T20:00:00Z")));
    assert_eq!(outbox.len(), 1);

    let port = FlakyStoragePort {
        remaining_failures: std::sync::atomic::AtomicU32::new(1),
    };
    let results = outbox.drain(&port);

    assert!(
        results.is_empty(),
        "a transient failure must not surface as a terminal drain outcome"
    );
    assert_eq!(
        outbox.len(),
        1,
        "the observation must be requeued for the next drain, not discarded"
    );
}
